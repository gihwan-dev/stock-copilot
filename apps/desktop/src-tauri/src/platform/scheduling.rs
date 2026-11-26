use crate::platform::background::{BackgroundRuntime, CollectorWorker, IdleCollectorWorker};
use crate::platform::collector::{CollectorFeeds, app_collector};
use crate::platform::intraday::IntradayWatchSlot;
use crate::platform::job_settings::{
    AppJobSchedules, ScheduleIpcState, SettingsControl, job_record_queue,
};
use crate::platform::scheduled_ports::{AppSpecWatch, AppStoredJobs};
use crate::ports::{PortTypes, Ports};
use domain::Timestamp;
use scheduler::{
    BriefingJob, DailyBackupJob, DailyValuationJob, IntradayCompactionJob, RegistryError,
    TossSpecWatchJob, TransactionsSyncJob,
};
use scheduler::{
    DEFAULT_TICK_INTERVAL_SECONDS, JobError, JobRegistry, NoMarketCalendar, PortfolioSyncJob,
    PortfolioSyncPort, Scheduler, SchedulerEventBridge, SyncDemand,
};
use scheduler::{IntradayWatchInbox, IntradayWatchJob, IntradayWatchSubscriber};
use scheduler::{ScheduleSettings, SchedulerOptions};
use std::marker::PhantomData;
use std::sync::Arc;
use std::time::Duration;
use store_sqlite::SqliteRepositories;
use tauri::{App, AppHandle, Manager, Runtime};

pub struct AppPortfolioSync<R: Runtime, P: PortTypes> {
    app: AppHandle<R>,
    ports: PhantomData<fn() -> P>,
}

impl<R: Runtime, P: PortTypes> std::fmt::Debug for AppPortfolioSync<R, P> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppPortfolioSync").finish_non_exhaustive()
    }
}

impl<R: Runtime, P: PortTypes> AppPortfolioSync<R, P> {
    #[must_use]
    pub const fn new(app: AppHandle<R>) -> Self {
        Self {
            app,
            ports: PhantomData,
        }
    }
}

impl<R: Runtime, P: PortTypes> PortfolioSyncPort for AppPortfolioSync<R, P> {
    async fn sync_all(&self, _now: Timestamp) -> Result<u32, JobError> {
        let ports = self
            .app
            .try_state::<Ports<P>>()
            .ok_or_else(|| JobError::PortUnavailable(String::from("ports")))?;
        let inputs = crate::holdings_link::SyncParts {
            broker: ports.broker_read.as_ref(),
            portfolios: ports.portfolios.as_ref(),
            clock: ports.clock.as_ref(),
            events: &ports.events,
            market: ports.market_info.as_ref(),
            market_data: ports.market_data.as_ref(),
        };
        let mut synced = 0_u32;
        let mut first_failure = None;
        for account in &ports.saved_accounts.current() {
            match crate::holdings_link::sync_with_rate(&inputs, account).await {
                Ok(_report) => synced = synced.saturating_add(1),
                Err(error) => {
                    first_failure = first_failure
                        .or_else(|| Some(JobError::StorageFailure(format!("{error:?}"))));
                }
            }
        }
        first_failure.map_or(Ok(synced), Err)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchedulingStart {
    Started,
    AlreadyRunning,
    NotConfigured,
    InvalidSchedule,
}

pub fn start_background<R: Runtime, P: PortTypes>(app: &App<R>) -> SchedulingStart {
    if app.try_state::<Arc<BackgroundRuntime>>().is_some() {
        return SchedulingStart::AlreadyRunning;
    }
    let Some(bridge) = app
        .try_state::<Arc<SchedulerEventBridge>>()
        .map(|state| Arc::clone(&state))
    else {
        return SchedulingStart::NotConfigured;
    };
    let Some(ports) = app.try_state::<Ports<P>>() else {
        return SchedulingStart::NotConfigured;
    };
    let demand = Arc::new(SyncDemand::new());
    let intraday = Arc::new(IntradayWatchInbox::new());
    let mut registry = JobRegistry::new();
    if registry
        .register(PortfolioSyncJob::new(
            Arc::new(AppPortfolioSync::<R, P>::new(app.handle().clone())),
            Arc::new(NoMarketCalendar),
            Arc::clone(&demand),
        ))
        .is_err()
    {
        return SchedulingStart::InvalidSchedule;
    }
    if let Some(repositories) = app.try_state::<Arc<SqliteRepositories>>()
        && register_stored_jobs::<R, P>(
            &mut registry,
            app.handle(),
            repositories.as_ref(),
            app.try_state::<SpecWatchInputs>()
                .map(|inputs| inputs.inner().clone()),
            Arc::clone(&intraday),
        )
        .is_err()
    {
        return SchedulingStart::InvalidSchedule;
    }
    let stored = app
        .try_state::<Arc<SqliteRepositories>>()
        .map(|state| state.as_ref().clone());
    let Ok(built) = build_scheduler(&ports, registry, stored.as_ref()) else {
        return SchedulingStart::InvalidSchedule;
    };
    let scheduler = Arc::new(built);
    bridge.bind(scheduler.event_subscriber().with_order_sync(demand));
    bind_listeners(app, &scheduler, &ports, intraday, stored);
    let clock: Arc<dyn application::Clock> = Arc::<P::Clock>::clone(&ports.clock);
    let collector = collector_worker::<R, P>(app, clock);
    let runtime = BackgroundRuntime::spawn(
        scheduler,
        Duration::from_secs(DEFAULT_TICK_INTERVAL_SECONDS),
        collector,
    );
    app.manage(Arc::new(runtime));
    SchedulingStart::Started
}

type PortScheduler<P> = Scheduler<<P as PortTypes>::Clock, <P as PortTypes>::AiJobQueue>;

fn build_scheduler<P: PortTypes>(
    ports: &Ports<P>,
    registry: JobRegistry,
    stored: Option<&SqliteRepositories>,
) -> Result<PortScheduler<P>, scheduler::ScheduleTimeError> {
    match stored {
        Some(repositories) => {
            let (records, _writer) = job_record_queue(Arc::new(repositories.clone()));
            Scheduler::with_options(
                Arc::clone(&ports.clock),
                Arc::clone(&ports.ai_jobs),
                registry,
                SchedulerOptions {
                    calendars: Arc::new(NoMarketCalendar),
                    records: Arc::new(records),
                },
            )
        }
        None => Scheduler::new(
            Arc::clone(&ports.clock),
            Arc::clone(&ports.ai_jobs),
            registry,
        ),
    }
}

fn bind_listeners<R: Runtime, P: PortTypes>(
    app: &App<R>,
    scheduler: &Arc<PortScheduler<P>>,
    ports: &Ports<P>,
    intraday: Arc<IntradayWatchInbox>,
    stored: Option<SqliteRepositories>,
) {
    if let Some(slot) = app.try_state::<Arc<IntradayWatchSlot>>() {
        slot.bind(IntradayWatchSubscriber::new(intraday, scheduler.triggers()));
    }
    if let Some(repositories) = stored {
        manage_schedule_settings(app, scheduler, ports, repositories);
    }
}

fn manage_schedule_settings<R: Runtime, P: PortTypes>(
    app: &App<R>,
    scheduler: &Arc<PortScheduler<P>>,
    ports: &Ports<P>,
    repositories: SqliteRepositories,
) {
    let store = Arc::new(repositories);
    let control = SettingsControl::new(
        ScheduleSettings::with_shared_events(
            Arc::clone(scheduler),
            Arc::new(AppJobSchedules::new(Arc::clone(&store))),
            Arc::clone(&ports.events),
        ),
        store,
    );
    let _restored = tauri::async_runtime::block_on(control.restore());
    app.manage(ScheduleIpcState(Arc::new(control)));
}

fn collector_worker<R: Runtime, P: PortTypes>(
    app: &App<R>,
    clock: Arc<dyn application::Clock>,
) -> Arc<dyn CollectorWorker> {
    if let Some(tracking) = app.try_state::<crate::news_tracking::NewsTracking>() {
        tracking.bind(Arc::new(crate::news_tracking::AppNewsInputs::<R, P>::new(
            app.handle().clone(),
        )));
    }
    match (
        app.try_state::<Arc<SqliteRepositories>>(),
        app.try_state::<CollectorFeeds>(),
    ) {
        (Some(stored), Some(listed)) if !listed.0.is_empty() => Arc::new(app_collector::<R, P>(
            app.handle(),
            stored.as_ref(),
            listed.inner(),
            clock,
        )),
        (Some(_) | None, Some(_) | None) => Arc::new(IdleCollectorWorker),
    }
}

#[derive(Debug, Clone)]
pub struct SpecWatchInputs {
    pub source: Arc<broker_toss::TossSpecClient>,
    pub baseline: application::TossSpecBaseline,
}

pub fn register_stored_jobs<R: Runtime, P: PortTypes>(
    registry: &mut JobRegistry,
    app: &AppHandle<R>,
    repositories: &SqliteRepositories,
    spec_watch: Option<SpecWatchInputs>,
    intraday: Arc<IntradayWatchInbox>,
) -> Result<(), RegistryError> {
    let stored = || {
        Arc::new(AppStoredJobs::<R, P>::new(
            app.clone(),
            repositories.clone(),
        ))
    };
    registry.register(IntradayCompactionJob::new(stored()))?;
    registry.register(DailyBackupJob::new(stored()))?;
    registry.register(BriefingJob::morning(stored()))?;
    registry.register(BriefingJob::closing_review(stored()))?;
    registry.register(BriefingJob::pre_us_market(stored()))?;
    registry.register(DailyValuationJob::new(stored()))?;
    registry.register(IntradayWatchJob::new(stored(), intraday))?;
    registry.register(TransactionsSyncJob::new(
        stored(),
        Arc::new(NoMarketCalendar),
    ))?;
    if app
        .try_state::<crate::platform::resolution::ResolutionDataSlot>()
        .is_some()
    {
        registry.register(scheduler::EstimateResolutionJob::domestic(stored()))?;
        registry.register(scheduler::EstimateResolutionJob::us(stored()))?;
    }
    if app
        .try_state::<crate::estimate_flow::EstimateFlowSlot>()
        .is_some()
    {
        registry.register(scheduler::EstimateCycleJob::new(stored()))?;
    }
    if let Some(inputs) = spec_watch {
        registry.register(TossSpecWatchJob::new(Arc::new(AppSpecWatch::new(
            AppStoredJobs::<R, P>::new(app.clone(), repositories.clone()),
            inputs.source,
            inputs.baseline,
        ))))?;
    }
    Ok(())
}
