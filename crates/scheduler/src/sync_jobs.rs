use crate::market_hours::{MarketCalendarSource, trading_status_now};
use crate::registry::{JobFuture, ScheduledJob};
use crate::types::{
    JobConfig, JobEnablement, JobError, JobKind, JobRunDetail, MissedPolicy, ScheduleRule,
    ScheduledJobId, StartupPolicy,
};
use application::{
    BrokerRead, Clock, EventBus, FullInspectionRepository, FullInspectionRun, PortfolioRepository,
    RunFullInspection, SyncPortfolio, TransactionSchedule, TransactionSyncRepository,
};
use domain::{
    AccountId, InspectionRangeDays, MarketTradingStatus, Nanosecond, SpanSeconds, TimeSpan,
    Timestamp,
};
use std::future::Future;
use std::sync::{Arc, RwLock};

pub const PORTFOLIO_SYNC_INTERVAL_MINUTES: u64 = 5;
pub const TRANSACTIONS_SYNC_INTERVAL_MINUTES: u64 = 30;
const SECONDS_PER_MINUTE: u64 = 60;

fn interval_minutes(count: u64) -> ScheduleRule {
    match TimeSpan::try_new(
        SpanSeconds::new(count.saturating_mul(SECONDS_PER_MINUTE)),
        Nanosecond::new(0),
    ) {
        Ok(span) => ScheduleRule::Interval(span),
        Err(_) => ScheduleRule::EventTriggered,
    }
}

const fn sync_config(schedule: ScheduleRule) -> JobConfig {
    JobConfig {
        kind: JobKind::CodeOnly,
        schedule,
        missed_policy: MissedPolicy::Skip,
        enabled: JobEnablement::Enabled,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SyncDemandState {
    Idle,
    Requested,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SyncRunGate {
    Startup,
    MarketOpen,
    Demanded,
    MarketClosed,
}

#[derive(Debug)]
pub struct SyncDemand {
    state: RwLock<SyncDemandState>,
}

impl Default for SyncDemand {
    fn default() -> Self {
        Self {
            state: RwLock::new(SyncDemandState::Idle),
        }
    }
}

fn take_state(lock: &RwLock<SyncDemandState>) -> SyncDemandState {
    let mut guard = match lock.write() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    let previous = *guard;
    *guard = SyncDemandState::Idle;
    previous
}

impl SyncDemand {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn request(&self) {
        match self.state.write() {
            Ok(mut guard) => *guard = SyncDemandState::Requested,
            Err(poisoned) => *poisoned.into_inner() = SyncDemandState::Requested,
        }
    }

    fn take(&self) -> SyncDemandState {
        take_state(&self.state)
    }
}

struct MarketGate {
    calendars: Arc<dyn MarketCalendarSource>,
    started: RwLock<SyncDemandState>,
}

impl std::fmt::Debug for MarketGate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MarketGate").finish_non_exhaustive()
    }
}

impl MarketGate {
    fn new(calendars: Arc<dyn MarketCalendarSource>) -> Self {
        Self {
            calendars,
            started: RwLock::new(SyncDemandState::Requested),
        }
    }

    fn decide(&self, now: Timestamp, demand: Option<&SyncDemand>) -> SyncRunGate {
        let first = take_state(&self.started);
        let demanded = demand.map_or(SyncDemandState::Idle, SyncDemand::take);
        if first == SyncDemandState::Requested {
            return SyncRunGate::Startup;
        }
        if demanded == SyncDemandState::Requested {
            return SyncRunGate::Demanded;
        }
        match trading_status_now(self.calendars.as_ref(), now) {
            MarketTradingStatus::Open | MarketTradingStatus::Unknown => SyncRunGate::MarketOpen,
            MarketTradingStatus::Closed => SyncRunGate::MarketClosed,
        }
    }
}

pub trait PortfolioSyncPort: Send + Sync {
    fn sync_all(&self, now: Timestamp) -> impl Future<Output = Result<u32, JobError>> + Send;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TransactionSyncTally {
    pub recent_synced: u32,
    pub full_inspections_completed: u32,
}

pub trait TransactionSyncPort: Send + Sync {
    fn sync_all(
        &self,
        now: Timestamp,
    ) -> impl Future<Output = Result<TransactionSyncTally, JobError>> + Send;
}

#[derive(Debug)]
pub struct PortfolioSyncJob<P> {
    port: Arc<P>,
    gate: MarketGate,
    demand: Arc<SyncDemand>,
}

impl<P: PortfolioSyncPort> PortfolioSyncJob<P> {
    #[must_use]
    pub fn new(
        port: Arc<P>,
        calendars: Arc<dyn MarketCalendarSource>,
        demand: Arc<SyncDemand>,
    ) -> Self {
        Self {
            port,
            gate: MarketGate::new(calendars),
            demand,
        }
    }
}

impl<P: PortfolioSyncPort> ScheduledJob for PortfolioSyncJob<P> {
    fn id(&self) -> ScheduledJobId {
        ScheduledJobId::PortfolioSync
    }

    fn default_config(&self) -> JobConfig {
        sync_config(interval_minutes(PORTFOLIO_SYNC_INTERVAL_MINUTES))
    }

    fn startup_policy(&self) -> StartupPolicy {
        StartupPolicy::RunAtStartup
    }

    fn run(&self, now: Timestamp) -> JobFuture<'_> {
        Box::pin(async move {
            match self.gate.decide(now, Some(self.demand.as_ref())) {
                SyncRunGate::MarketClosed => Ok(JobRunDetail::new("market_closed", 0)),
                SyncRunGate::Startup | SyncRunGate::MarketOpen | SyncRunGate::Demanded => {
                    let synced = self.port.sync_all(now).await?;
                    Ok(JobRunDetail::new("portfolio_synced", synced))
                }
            }
        })
    }
}

#[derive(Debug)]
pub struct TransactionsSyncJob<P> {
    port: Arc<P>,
    gate: MarketGate,
}

impl<P: TransactionSyncPort> TransactionsSyncJob<P> {
    #[must_use]
    pub fn new(port: Arc<P>, calendars: Arc<dyn MarketCalendarSource>) -> Self {
        Self {
            port,
            gate: MarketGate::new(calendars),
        }
    }
}

impl<P: TransactionSyncPort> ScheduledJob for TransactionsSyncJob<P> {
    fn id(&self) -> ScheduledJobId {
        ScheduledJobId::TransactionsSync
    }

    fn default_config(&self) -> JobConfig {
        sync_config(interval_minutes(TRANSACTIONS_SYNC_INTERVAL_MINUTES))
    }

    fn startup_policy(&self) -> StartupPolicy {
        StartupPolicy::RunAtStartup
    }

    fn run(&self, now: Timestamp) -> JobFuture<'_> {
        Box::pin(async move {
            match self.gate.decide(now, None) {
                SyncRunGate::MarketClosed => Ok(JobRunDetail::new("market_closed", 0)),
                SyncRunGate::Startup | SyncRunGate::MarketOpen | SyncRunGate::Demanded => {
                    let tally = self.port.sync_all(now).await?;
                    Ok(JobRunDetail::new(
                        "transactions_synced",
                        tally.recent_synced,
                    ))
                }
            }
        })
    }
}

#[derive(Debug)]
pub struct SyncPortfolioRunner<B, R, C> {
    broker: Arc<B>,
    repository: Arc<R>,
    clock: Arc<C>,
    events: EventBus,
    accounts: Vec<AccountId>,
}

impl<B: BrokerRead, R: PortfolioRepository, C: Clock> SyncPortfolioRunner<B, R, C> {
    #[must_use]
    pub const fn new(
        broker: Arc<B>,
        repository: Arc<R>,
        clock: Arc<C>,
        events: EventBus,
        accounts: Vec<AccountId>,
    ) -> Self {
        Self {
            broker,
            repository,
            clock,
            events,
            accounts,
        }
    }
}

impl<B: BrokerRead, R: PortfolioRepository, C: Clock> PortfolioSyncPort
    for SyncPortfolioRunner<B, R, C>
{
    async fn sync_all(&self, _now: Timestamp) -> Result<u32, JobError> {
        let use_case = SyncPortfolio::new(
            self.broker.as_ref(),
            self.repository.as_ref(),
            self.clock.as_ref(),
            &self.events,
        );
        let mut synced = 0_u32;
        let mut first_failure = None;
        for account in &self.accounts {
            match use_case.execute(account).await {
                Ok(_report) => synced = synced.saturating_add(1),
                Err(error) => {
                    first_failure = first_failure
                        .or_else(|| Some(JobError::StorageFailure(format!("{error:?}"))));
                }
            }
        }
        match first_failure {
            Some(error) => Err(error),
            None => Ok(synced),
        }
    }
}

#[derive(Debug)]
pub struct SyncTransactionsRunner<B, R, I, C> {
    broker: Arc<B>,
    repository: Arc<R>,
    inspections: Arc<I>,
    clock: Arc<C>,
    events: EventBus,
    accounts: Vec<AccountId>,
    range_limit: InspectionRangeDays,
}

#[derive(Debug)]
pub struct SyncTransactionsParts<B, R, I, C> {
    pub broker: Arc<B>,
    pub repository: Arc<R>,
    pub inspections: Arc<I>,
    pub clock: Arc<C>,
    pub events: EventBus,
    pub accounts: Vec<AccountId>,
    pub range_limit: InspectionRangeDays,
}

impl<B, R, I, C> SyncTransactionsRunner<B, R, I, C>
where
    B: BrokerRead,
    R: TransactionSyncRepository,
    I: FullInspectionRepository,
    C: Clock,
{
    #[must_use]
    pub fn new(parts: SyncTransactionsParts<B, R, I, C>) -> Self {
        Self {
            broker: parts.broker,
            repository: parts.repository,
            inspections: parts.inspections,
            clock: parts.clock,
            events: parts.events,
            accounts: parts.accounts,
            range_limit: parts.range_limit,
        }
    }
}

impl<B, R, I, C> TransactionSyncPort for SyncTransactionsRunner<B, R, I, C>
where
    B: BrokerRead,
    R: TransactionSyncRepository,
    I: FullInspectionRepository,
    C: Clock,
{
    async fn sync_all(&self, _now: Timestamp) -> Result<TransactionSyncTally, JobError> {
        let recent = TransactionSchedule {
            broker: self.broker.as_ref(),
            repository: self.repository.as_ref(),
            inspections: self.inspections.as_ref(),
            clock: self.clock.as_ref(),
            events: &self.events,
        };
        let full = RunFullInspection {
            broker: self.broker.as_ref(),
            repository: self.repository.as_ref(),
            inspections: self.inspections.as_ref(),
            clock: self.clock.as_ref(),
            events: &self.events,
        };
        let mut tally = TransactionSyncTally::default();
        let mut first_failure = None;
        for account in &self.accounts {
            match recent.sync_recent(account).await {
                Ok(_delivery) => tally.recent_synced = tally.recent_synced.saturating_add(1),
                Err(error) => {
                    first_failure = first_failure
                        .or_else(|| Some(JobError::StorageFailure(format!("{error:?}"))));
                }
            }
            match full
                .run_full_inspection(account, self.range_limit)
                .await
                .outcome
            {
                Ok(FullInspectionRun::Completed { .. }) => {
                    tally.full_inspections_completed =
                        tally.full_inspections_completed.saturating_add(1);
                }
                Ok(FullInspectionRun::AlreadyCompletedToday) => {}
                Err(error) => {
                    first_failure = first_failure
                        .or_else(|| Some(JobError::ExecutionFailed(format!("{error:?}"))));
                }
            }
        }
        match first_failure {
            Some(error) => Err(error),
            None => Ok(tally),
        }
    }
}
