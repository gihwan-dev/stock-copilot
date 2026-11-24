use crate::ai_queue::AiJobQueue;
use crate::engine::Scheduler;
use crate::types::{
    JobConfig, JobEnablement, JobError, JobKind, JobScheduleOverride, JobStatusSnapshot,
    KstTimeOfDay, ScheduleRule, ScheduledJobId,
};
use application::{Clock, EventBus};
use domain::DomainEvent;
use std::future::Future;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JobScheduleChange {
    pub id: ScheduledJobId,
    pub enabled: JobEnablement,
    pub time: Option<KstTimeOfDay>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScheduleSettingsError {
    UnknownJob(ScheduledJobId),
    NotConfigurable(ScheduledJobId),
    TimeNotAdjustable(ScheduledJobId),
    InvalidSchedule(ScheduledJobId),
    Storage(JobError),
}

pub trait JobScheduleSettingsRepository: Send + Sync {
    fn load(&self) -> impl Future<Output = Result<Vec<JobScheduleChange>, JobError>> + Send;
    fn save(&self, change: JobScheduleChange) -> impl Future<Output = Result<(), JobError>> + Send;
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ScheduleRestoreReport {
    pub applied: Vec<ScheduledJobId>,
    pub rejected: Vec<ScheduleSettingsError>,
}

#[derive(Debug)]
pub struct ScheduleSettings<C: Clock, Q: AiJobQueue, R> {
    scheduler: Arc<Scheduler<C, Q>>,
    repository: Arc<R>,
    events: EventBus,
}

const fn adjusted_rule(
    id: ScheduledJobId,
    config: JobConfig,
    time: Option<KstTimeOfDay>,
) -> Result<ScheduleRule, ScheduleSettingsError> {
    let Some(new_time) = time else {
        return Ok(config.schedule);
    };
    match config.schedule {
        ScheduleRule::DailyKst(_) => Ok(ScheduleRule::DailyKst(new_time)),
        ScheduleRule::WeeklyKst { weekday, .. } => Ok(ScheduleRule::WeeklyKst {
            weekday,
            time: new_time,
        }),
        ScheduleRule::Interval(_)
        | ScheduleRule::AfterMarketClose { .. }
        | ScheduleRule::BeforeMarketOpen { .. }
        | ScheduleRule::EventTriggered => Err(ScheduleSettingsError::TimeNotAdjustable(id)),
    }
}

impl<C: Clock, Q: AiJobQueue, R: JobScheduleSettingsRepository> ScheduleSettings<C, Q, R> {
    #[must_use]
    pub const fn new(
        scheduler: Arc<Scheduler<C, Q>>,
        repository: Arc<R>,
        events: EventBus,
    ) -> Self {
        Self {
            scheduler,
            repository,
            events,
        }
    }

    pub async fn job_status(&self) -> Vec<JobStatusSnapshot> {
        self.scheduler.status_snapshots().await
    }

    pub async fn configurable_jobs(&self) -> Vec<JobStatusSnapshot> {
        self.scheduler
            .status_snapshots()
            .await
            .into_iter()
            .filter(|snapshot| snapshot.config.kind == JobKind::AiBackground)
            .collect()
    }

    pub async fn change(
        &self,
        change: JobScheduleChange,
    ) -> Result<JobStatusSnapshot, ScheduleSettingsError> {
        let schedule = self.validate(change).await?;
        self.repository
            .save(change)
            .await
            .map_err(ScheduleSettingsError::Storage)?;
        let snapshot = self.apply(change.id, change.enabled, schedule).await?;
        let _delivery = self.events.publish(DomainEvent::SettingsChanged);
        Ok(snapshot)
    }

    pub async fn restore(&self) -> Result<ScheduleRestoreReport, ScheduleSettingsError> {
        let saved = self
            .repository
            .load()
            .await
            .map_err(ScheduleSettingsError::Storage)?;
        let mut report = ScheduleRestoreReport::default();
        for change in saved {
            let applied = match self.validate(change).await {
                Ok(schedule) => self.apply(change.id, change.enabled, schedule).await,
                Err(error) => Err(error),
            };
            match applied {
                Ok(_snapshot) => report.applied.push(change.id),
                Err(error) => report.rejected.push(error),
            }
        }
        Ok(report)
    }

    async fn validate(
        &self,
        change: JobScheduleChange,
    ) -> Result<ScheduleRule, ScheduleSettingsError> {
        let current = self
            .scheduler
            .status_snapshots()
            .await
            .into_iter()
            .find(|snapshot| snapshot.id == change.id)
            .ok_or(ScheduleSettingsError::UnknownJob(change.id))?;
        if current.config.kind != JobKind::AiBackground {
            return Err(ScheduleSettingsError::NotConfigurable(change.id));
        }
        adjusted_rule(change.id, current.config, change.time)
    }

    async fn apply(
        &self,
        id: ScheduledJobId,
        enabled: JobEnablement,
        schedule: ScheduleRule,
    ) -> Result<JobStatusSnapshot, ScheduleSettingsError> {
        self.scheduler
            .apply_override(JobScheduleOverride {
                id,
                enabled,
                schedule,
            })
            .await
            .map_err(|_error| ScheduleSettingsError::InvalidSchedule(id))?
            .ok_or(ScheduleSettingsError::UnknownJob(id))
    }
}
