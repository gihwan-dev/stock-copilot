use crate::registry::{JobFuture, ScheduledJob};
use crate::schedule_time::kst_time_of_day;
use crate::types::{
    JobConfig, JobEnablement, JobError, JobKind, JobRunDetail, KstTimeOfDay, MissedPolicy,
    ScheduleRule, ScheduledJobId, StartupPolicy,
};
use application::EventBus;
use application::resolve_estimates::{
    EstimateResolutionRepository, ResolutionMarketData, ResolutionStatus, ResolveEstimates,
};
use domain::{CalendarDate, KnownMarket, Nanosecond, SpanSeconds, TimeSpan, Timestamp};
use std::future::Future;
use std::sync::Arc;

pub const INTRADAY_RETENTION_DAYS: u64 = 30;
pub const BACKUP_RETENTION_DAYS: u64 = 7;
const SECONDS_PER_DAY: u64 = 86_400;
const SECONDS_PER_MINUTE: u64 = 60;

const fn code_job(schedule: ScheduleRule) -> JobConfig {
    JobConfig {
        kind: JobKind::CodeOnly,
        schedule,
        missed_policy: MissedPolicy::CatchUpOnce,
        enabled: JobEnablement::Enabled,
    }
}

fn kst_time(hour: u8, minute: u8) -> Result<KstTimeOfDay, JobError> {
    KstTimeOfDay::try_new(hour, minute)
        .map_err(|error| JobError::ExecutionFailed(format!("invalid-time: {error:?}")))
}

fn daily(hour: u8, minute: u8) -> ScheduleRule {
    match KstTimeOfDay::try_new(hour, minute) {
        Ok(time) => ScheduleRule::DailyKst(time),
        Err(_) => ScheduleRule::EventTriggered,
    }
}

fn minutes(count: u64) -> TimeSpan {
    span(count.saturating_mul(SECONDS_PER_MINUTE))
}

fn span(seconds: u64) -> TimeSpan {
    match TimeSpan::try_new(SpanSeconds::new(seconds), Nanosecond::new(0)) {
        Ok(value) => value,
        Err(_) => TimeSpan::NANOSECOND,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpecWatchResult {
    Unchanged,
    Changed,
}

pub trait SpecWatchPort: Send + Sync {
    fn check(
        &self,
        now: Timestamp,
    ) -> impl Future<Output = Result<SpecWatchResult, JobError>> + Send;
}

#[derive(Debug)]
pub struct TossSpecWatchJob<P> {
    port: Arc<P>,
}

impl<P: SpecWatchPort> TossSpecWatchJob<P> {
    #[must_use]
    pub const fn new(port: Arc<P>) -> Self {
        Self { port }
    }
}

impl<P: SpecWatchPort> ScheduledJob for TossSpecWatchJob<P> {
    fn id(&self) -> ScheduledJobId {
        ScheduledJobId::TossSpecWatch
    }

    fn default_config(&self) -> JobConfig {
        code_job(daily(7, 0))
    }

    fn run(&self, now: Timestamp) -> JobFuture<'_> {
        Box::pin(async move {
            match self.port.check(now).await? {
                SpecWatchResult::Unchanged => Ok(JobRunDetail::new("spec_unchanged", 0)),
                SpecWatchResult::Changed => Ok(JobRunDetail::new("spec_changed", 1)),
            }
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BackupRetention {
    days: u64,
}

impl BackupRetention {
    #[must_use]
    pub const fn days(self) -> u64 {
        self.days
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackupResult {
    Confirmed { retained_copies: u32 },
    VerificationFailed,
}

pub trait DatabaseBackupPort: Send + Sync {
    fn back_up(
        &self,
        now: Timestamp,
        retention: BackupRetention,
    ) -> impl Future<Output = Result<BackupResult, JobError>> + Send;
}

#[derive(Debug)]
pub struct DailyBackupJob<P> {
    port: Arc<P>,
}

impl<P: DatabaseBackupPort> DailyBackupJob<P> {
    #[must_use]
    pub const fn new(port: Arc<P>) -> Self {
        Self { port }
    }
}

impl<P: DatabaseBackupPort> ScheduledJob for DailyBackupJob<P> {
    fn id(&self) -> ScheduledJobId {
        ScheduledJobId::DailyBackup
    }

    fn default_config(&self) -> JobConfig {
        code_job(daily(4, 0))
    }

    fn run(&self, now: Timestamp) -> JobFuture<'_> {
        Box::pin(async move {
            let retention = BackupRetention {
                days: BACKUP_RETENTION_DAYS,
            };
            match self.port.back_up(now, retention).await? {
                BackupResult::Confirmed { retained_copies } => {
                    Ok(JobRunDetail::new("backup_confirmed", retained_copies))
                }
                BackupResult::VerificationFailed => Err(JobError::ExecutionFailed(String::from(
                    "backup_verification_failed",
                ))),
            }
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompactionResult {
    pub compacted_days: u32,
}

pub trait IntradayCompactionPort: Send + Sync {
    fn compact_before(
        &self,
        cutoff: Timestamp,
    ) -> impl Future<Output = Result<CompactionResult, JobError>> + Send;
    fn incremental_vacuum(&self) -> impl Future<Output = Result<(), JobError>> + Send;
}

#[derive(Debug)]
pub struct IntradayCompactionJob<P> {
    port: Arc<P>,
}

impl<P: IntradayCompactionPort> IntradayCompactionJob<P> {
    #[must_use]
    pub const fn new(port: Arc<P>) -> Self {
        Self { port }
    }
}

impl<P: IntradayCompactionPort> ScheduledJob for IntradayCompactionJob<P> {
    fn id(&self) -> ScheduledJobId {
        ScheduledJobId::IntradayCompaction
    }

    fn default_config(&self) -> JobConfig {
        code_job(daily(3, 30))
    }

    fn run(&self, now: Timestamp) -> JobFuture<'_> {
        Box::pin(async move {
            let retention = span(INTRADAY_RETENTION_DAYS.saturating_mul(SECONDS_PER_DAY));
            let cutoff = now.checked_sub(retention).map_err(|error| {
                JobError::ExecutionFailed(format!("cutoff-out-of-range: {error:?}"))
            })?;
            let result = self.port.compact_before(cutoff).await?;
            self.port.incremental_vacuum().await?;
            Ok(JobRunDetail::new(
                "intraday_compacted",
                result.compacted_days,
            ))
        })
    }
}

pub trait DailyValuationPort: Send + Sync {
    fn record_today(&self, now: Timestamp) -> impl Future<Output = Result<(), JobError>> + Send;
    fn missing_days(
        &self,
        now: Timestamp,
    ) -> impl Future<Output = Result<Vec<CalendarDate>, JobError>> + Send;
    fn recompute_day(&self, day: CalendarDate)
    -> impl Future<Output = Result<(), JobError>> + Send;
}

#[derive(Debug)]
pub struct DailyValuationJob<P> {
    port: Arc<P>,
}

impl<P: DailyValuationPort> DailyValuationJob<P> {
    #[must_use]
    pub const fn new(port: Arc<P>) -> Self {
        Self { port }
    }
}

const VALUATION_HOUR: u8 = 16;

impl<P: DailyValuationPort> ScheduledJob for DailyValuationJob<P> {
    fn id(&self) -> ScheduledJobId {
        ScheduledJobId::DailyValuationSnapshot
    }

    fn default_config(&self) -> JobConfig {
        code_job(daily(VALUATION_HOUR, 0))
    }

    fn startup_policy(&self) -> StartupPolicy {
        StartupPolicy::RunAtStartup
    }

    fn run(&self, now: Timestamp) -> JobFuture<'_> {
        Box::pin(async move {
            let record_time = kst_time(VALUATION_HOUR, 0)?;
            let time_of_day = kst_time_of_day(now).map_err(|error| {
                JobError::ExecutionFailed(format!("time-out-of-range: {error:?}"))
            })?;
            let mut processed = 0_u32;
            let mut first_failure = None;
            if time_of_day >= record_time {
                match self.port.record_today(now).await {
                    Ok(()) => processed = processed.saturating_add(1),
                    Err(error) => first_failure = Some(error),
                }
            }
            for day in self.port.missing_days(now).await? {
                match self.port.recompute_day(day).await {
                    Ok(()) => processed = processed.saturating_add(1),
                    Err(error) => first_failure = first_failure.or(Some(error)),
                }
            }
            match first_failure {
                Some(error) => Err(error),
                None => Ok(JobRunDetail::new("valuation_recorded", processed)),
            }
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ResolutionTally {
    pub resolved: u32,
    pub ongoing: u32,
    pub unavailable: u32,
    pub brier: domain::BrierTally,
}

pub trait EstimateResolutionPort: Send + Sync {
    fn resolve_due(
        &self,
        now: Timestamp,
    ) -> impl Future<Output = Result<ResolutionTally, JobError>> + Send;
}

#[derive(Debug)]
pub struct EstimateResolutionJob<P> {
    market: KnownMarket,
    port: Arc<P>,
}

impl<P: EstimateResolutionPort> EstimateResolutionJob<P> {
    #[must_use]
    pub const fn domestic(port: Arc<P>) -> Self {
        Self {
            market: KnownMarket::Kr,
            port,
        }
    }

    #[must_use]
    pub const fn us(port: Arc<P>) -> Self {
        Self {
            market: KnownMarket::Us,
            port,
        }
    }
}

impl<P: EstimateResolutionPort> ScheduledJob for EstimateResolutionJob<P> {
    fn id(&self) -> ScheduledJobId {
        match self.market {
            KnownMarket::Kr => ScheduledJobId::EstimateResolution,
            KnownMarket::Us => ScheduledJobId::UsEstimateResolution,
        }
    }

    fn default_config(&self) -> JobConfig {
        let (delay, hour, minute) = match self.market {
            KnownMarket::Kr => (40, 16, 10),
            KnownMarket::Us => (30, 6, 30),
        };
        let schedule = match KstTimeOfDay::try_new(hour, minute) {
            Ok(fallback) => ScheduleRule::AfterMarketClose {
                market: self.market,
                delay: minutes(delay),
                fallback,
            },
            Err(_) => ScheduleRule::EventTriggered,
        };
        code_job(schedule)
    }

    fn startup_policy(&self) -> StartupPolicy {
        match self.market {
            KnownMarket::Kr => StartupPolicy::RunAtStartup,
            KnownMarket::Us => StartupPolicy::WaitForSchedule,
        }
    }

    fn run(&self, now: Timestamp) -> JobFuture<'_> {
        Box::pin(async move {
            let tally = self.port.resolve_due(now).await?;
            Ok(JobRunDetail::new("estimates_resolved", tally.resolved))
        })
    }
}

#[derive(Debug)]
pub struct ResolveEstimatesRunner<R, D> {
    repository: Arc<R>,
    data: Arc<D>,
    events: EventBus,
}

impl<R: EstimateResolutionRepository, D: ResolutionMarketData> ResolveEstimatesRunner<R, D> {
    #[must_use]
    pub const fn new(repository: Arc<R>, data: Arc<D>, events: EventBus) -> Self {
        Self {
            repository,
            data,
            events,
        }
    }
}

impl<R: EstimateResolutionRepository, D: ResolutionMarketData> EstimateResolutionPort
    for ResolveEstimatesRunner<R, D>
{
    async fn resolve_due(&self, now: Timestamp) -> Result<ResolutionTally, JobError> {
        let reports =
            ResolveEstimates::new(self.repository.as_ref(), self.data.as_ref(), &self.events)
                .execute(now)
                .await
                .map_err(|error| JobError::StorageFailure(format!("{error:?}")))?;
        let mut tally = ResolutionTally::default();
        for report in reports {
            match report.status {
                ResolutionStatus::Resolved { brier, .. } => {
                    tally.resolved = tally.resolved.saturating_add(1);
                    tally.brier = brier
                        .and_then(|comparison| tally.brier.clone().with(comparison))
                        .unwrap_or_else(|| tally.brier.clone());
                }
                ResolutionStatus::AlreadyResolved => {
                    tally.resolved = tally.resolved.saturating_add(1);
                }
                ResolutionStatus::Ongoing => tally.ongoing = tally.ongoing.saturating_add(1),
                ResolutionStatus::Unavailable(_)
                | ResolutionStatus::Storage(_)
                | ResolutionStatus::Calibration(_)
                | ResolutionStatus::MissingFact
                | ResolutionStatus::Conflict => {
                    tally.unavailable = tally.unavailable.saturating_add(1);
                }
            }
        }
        Ok(tally)
    }
}
