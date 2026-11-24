mod ai_queue;
mod engine;
mod registry;
mod schedule_time;
mod types;

pub use ai_queue::{AiJobQueue, AiQueueAdmission, AiQueueError, DisabledAiJobQueue};
pub use engine::{
    DEFAULT_TICK_INTERVAL_SECONDS, Scheduler, SchedulerEventSubscriber, SchedulerWakeSignal,
};
pub use registry::{JobFuture, JobRegistry, RegisteredJobMeta, RegistryError, ScheduledJob};
pub use schedule_time::{
    KST_OFFSET_SECONDS, ScheduleTimeError, is_within_skip_grace, next_due_after,
};
pub use types::{
    JobConfig, JobEnablement, JobError, JobExecutionRecord, JobKind, JobRunDetail, JobRunOutcome,
    JobScheduleOverride, JobStatusSnapshot, KstTimeError, KstTimeOfDay, KstWeekday, LoopControl,
    MissedPolicy, ScheduleRule, ScheduledJobId, TickReport,
};

#[cfg(test)]
mod tests;
