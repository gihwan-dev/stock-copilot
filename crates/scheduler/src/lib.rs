mod ai_queue;
mod code_jobs;
mod engine;
mod market_hours;
mod registry;
mod schedule_time;
mod types;

pub use ai_queue::{AiJobQueue, AiQueueAdmission, AiQueueError, DisabledAiJobQueue};
pub use code_jobs::{
    BACKUP_RETENTION_DAYS, BackupResult, BackupRetention, CompactionResult, DailyBackupJob,
    DailyValuationJob, DailyValuationPort, DatabaseBackupPort, EstimateResolutionJob,
    EstimateResolutionPort, INTRADAY_RETENTION_DAYS, IntradayCompactionJob, IntradayCompactionPort,
    ResolutionTally, ResolveEstimatesRunner, SpecWatchPort, SpecWatchResult, TossSpecWatchJob,
};
pub use engine::{
    DEFAULT_TICK_INTERVAL_SECONDS, DiscardJobRecords, JobRecordSink, JobTriggers, Scheduler,
    SchedulerEventSubscriber, SchedulerOptions, SchedulerWakeSignal,
};
pub use market_hours::{MarketCalendarSource, NoMarketCalendar, trading_status_now};
pub use registry::{JobFuture, JobRegistry, RegisteredJobMeta, RegistryError, ScheduledJob};
pub use schedule_time::{
    KST_OFFSET_SECONDS, ScheduleTimeError, is_within_skip_grace, kst_time_of_day, next_due_after,
};
pub use types::{
    JobConfig, JobEnablement, JobError, JobExecutionRecord, JobKind, JobRunDetail, JobRunOutcome,
    JobScheduleOverride, JobStatusSnapshot, KstTimeError, KstTimeOfDay, KstWeekday, LoopControl,
    MissedPolicy, ScheduleRule, ScheduledJobId, StartupPolicy, TickReport,
};

#[cfg(test)]
mod code_jobs_tests;
#[cfg(test)]
mod engine_tests;
#[cfg(test)]
mod fixture_tests;
#[cfg(test)]
mod tests;
