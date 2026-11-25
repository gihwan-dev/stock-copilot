use crate::platform::background::{BackgroundRuntime, IdleCollectorWorker};
use crate::platform::briefing_context::HoldingsBriefingContext;
use crate::platform::scheduled_ports::{AppSpecWatch, AppStoredJobs};
use crate::ports::{PortTypes, Ports};
use application::SyncPortfolio;
use domain::Timestamp;
use scheduler::{BriefingJob, RunBriefingRunner};
use scheduler::{
    DEFAULT_TICK_INTERVAL_SECONDS, JobError, JobRegistry, NoMarketCalendar, PortfolioSyncJob,
    PortfolioSyncPort, Scheduler, SchedulerEventBridge, SyncDemand,
};
use scheduler::{
    DailyBackupJob, IntradayCompactionJob, RegistryError, TossSpecWatchJob, TransactionsSyncJob,
};
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
        let use_case = SyncPortfolio::new(
            ports.broker_read.as_ref(),
            ports.portfolios.as_ref(),
            ports.clock.as_ref(),
            &ports.events,
        );
        let mut synced = 0_u32;
        let mut first_failure = None;
        for account in &ports.saved_accounts {
            match use_case.execute(account).await {
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
        && register_briefing_jobs(&mut registry, &ports, repositories.as_ref()).is_err()
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
        )
        .is_err()
    {
        return SchedulingStart::InvalidSchedule;
    }
    let Ok(scheduler) = Scheduler::new(
        Arc::clone(&ports.clock),
        Arc::clone(&ports.ai_jobs),
        registry,
    ) else {
        return SchedulingStart::InvalidSchedule;
    };
    bridge.bind(scheduler.event_subscriber().with_order_sync(demand));
    let runtime = BackgroundRuntime::spawn(
        Arc::new(scheduler),
        Duration::from_secs(DEFAULT_TICK_INTERVAL_SECONDS),
        Arc::new(IdleCollectorWorker),
    );
    app.manage(Arc::new(runtime));
    SchedulingStart::Started
}

pub fn register_briefing_jobs<P: PortTypes>(
    registry: &mut JobRegistry,
    ports: &Ports<P>,
    repositories: &SqliteRepositories,
) -> Result<(), RegistryError> {
    let runner = Arc::new(RunBriefingRunner::with_shared_events(
        Arc::clone(&ports.ai_provider),
        Arc::new(repositories.clone()),
        Arc::new(HoldingsBriefingContext::new(
            Arc::clone(&ports.portfolios),
            ports.saved_accounts.clone(),
        )),
        Arc::clone(&ports.clock),
        Arc::clone(&ports.events),
    ));
    registry.register(BriefingJob::morning(Arc::clone(&runner)))?;
    registry.register(BriefingJob::closing_review(Arc::clone(&runner)))?;
    registry.register(BriefingJob::pre_us_market(runner))?;
    Ok(())
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
) -> Result<(), RegistryError> {
    let stored = || {
        Arc::new(AppStoredJobs::<R, P>::new(
            app.clone(),
            repositories.clone(),
        ))
    };
    registry.register(IntradayCompactionJob::new(stored()))?;
    registry.register(DailyBackupJob::new(stored()))?;
    registry.register(TransactionsSyncJob::new(
        stored(),
        Arc::new(NoMarketCalendar),
    ))?;
    if let Some(inputs) = spec_watch {
        registry.register(TossSpecWatchJob::new(Arc::new(AppSpecWatch::new(
            AppStoredJobs::<R, P>::new(app.clone(), repositories.clone()),
            inputs.source,
            inputs.baseline,
        ))))?;
    }
    Ok(())
}
