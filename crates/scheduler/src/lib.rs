mod ai_queue;
mod briefing_jobs;
mod code_jobs;
mod engine;
mod event_bridge;
mod intraday_watch;
mod market_hours;
mod registry;
mod schedule_settings;
mod schedule_time;
mod sync_jobs;
mod types;

pub use ai_queue::{AiJobQueue, AiQueueAdmission, AiQueueError, DisabledAiJobQueue};
pub use briefing_jobs::{
    BriefingJob, BriefingPort, BriefingRunResult, PRE_US_MARKET_LEAD_MINUTES, RunBriefingRunner,
};
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
pub use event_bridge::{BridgeBinding, BridgedSchedulerEvents, SchedulerEventBridge};
pub use intraday_watch::{
    IntradayReviewPort, IntradayWatchInbox, IntradayWatchJob, IntradayWatchSubscriber, WatchCause,
};
pub use market_hours::{MarketCalendarSource, NoMarketCalendar, trading_status_now};
pub use registry::{JobFuture, JobRegistry, RegisteredJobMeta, RegistryError, ScheduledJob};
pub use schedule_settings::{
    JobScheduleChange, JobScheduleSettingsRepository, ScheduleRestoreReport, ScheduleSettings,
    ScheduleSettingsError,
};
pub use schedule_time::{
    KST_OFFSET_SECONDS, ScheduleTimeError, is_within_skip_grace, kst_time_of_day, next_due_after,
};
pub use sync_jobs::{
    PORTFOLIO_SYNC_INTERVAL_MINUTES, PortfolioSyncJob, PortfolioSyncPort, SyncDemand,
    SyncPortfolioRunner, SyncTransactionsParts, SyncTransactionsRunner,
    TRANSACTIONS_SYNC_INTERVAL_MINUTES, TransactionSyncPort, TransactionSyncTally,
    TransactionsSyncJob,
};
pub use types::{
    JobConfig, JobEnablement, JobError, JobExecutionRecord, JobKind, JobRunDetail, JobRunOutcome,
    JobScheduleOverride, JobStatusSnapshot, KstTimeError, KstTimeOfDay, KstWeekday, LoopControl,
    MissedPolicy, ScheduleRule, ScheduledJobId, StartupPolicy, TickReport,
};

#[cfg(test)]
mod briefing_jobs_tests;
#[cfg(test)]
mod code_jobs_tests;
#[cfg(test)]
mod engine_tests;
#[cfg(test)]
mod event_bridge_tests;
#[cfg(test)]
mod fixture_tests;
#[cfg(test)]
mod intraday_watch_tests;
#[cfg(test)]
mod schedule_settings_tests;
#[cfg(test)]
mod sync_jobs_tests;
#[cfg(test)]
mod tests;
