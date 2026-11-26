use crate::ipc::state::BoxFuture;
use application::{
    Clock, JobClockTime, JobCode, JobFailureKind, JobHour, JobItemCount, JobMinute, JobRunEntry,
    JobRunResult, JobScheduleStore, JobSummary, JobSwitch, SavedJobSchedule, SystemPortError,
};
use scheduler::{
    AiJobQueue, JobEnablement, JobError, JobExecutionRecord, JobRecordSink, JobRunOutcome,
    JobScheduleChange, JobScheduleSettingsRepository, JobStatusSnapshot, KstTimeOfDay,
    ScheduleRestoreReport, ScheduleSettings, ScheduleSettingsError, ScheduledJobId,
};
use std::fmt::{Debug, Formatter};
use std::sync::Arc;
use tokio::sync::mpsc;

fn storage(error: impl Debug) -> JobError {
    JobError::StorageFailure(format!("{error:?}"))
}

pub fn job_code(id: ScheduledJobId) -> Result<JobCode, SystemPortError> {
    JobCode::try_new(id.as_code()).map_err(|_error| {
        SystemPortError::new(
            application::SystemPort::Repository,
            application::SystemFailure::InvalidData,
        )
    })
}

pub fn to_saved_schedule(change: JobScheduleChange) -> Result<SavedJobSchedule, JobError> {
    let time = match change.time {
        Some(time) => Some(JobClockTime {
            hour: JobHour::try_new(time.hour()).map_err(storage)?,
            minute: JobMinute::try_new(time.minute()).map_err(storage)?,
        }),
        None => None,
    };
    Ok(SavedJobSchedule {
        job: job_code(change.id).map_err(storage)?,
        switch: match change.enabled {
            JobEnablement::Enabled => JobSwitch::Enabled,
            JobEnablement::Disabled => JobSwitch::Disabled,
        },
        time,
    })
}

pub fn from_saved_schedule(saved: &SavedJobSchedule) -> Result<JobScheduleChange, JobError> {
    let id = ScheduledJobId::from_code(saved.job.as_str())
        .ok_or_else(|| JobError::StorageFailure(String::from("unknown-job")))?;
    let time = match saved.time {
        Some(time) => {
            Some(KstTimeOfDay::try_new(*time.hour.as_u8(), *time.minute.as_u8()).map_err(storage)?)
        }
        None => None,
    };
    Ok(JobScheduleChange {
        id,
        enabled: match saved.switch {
            JobSwitch::Enabled => JobEnablement::Enabled,
            JobSwitch::Disabled => JobEnablement::Disabled,
        },
        time,
    })
}

pub fn to_run_entry(record: &JobExecutionRecord) -> Result<JobRunEntry, SystemPortError> {
    Ok(JobRunEntry {
        job: job_code(record.id)?,
        scheduled_for: record.scheduled_for,
        evaluated_at: record.evaluated_at,
        result: match &record.outcome {
            JobRunOutcome::Executed(detail) => JobRunResult::Executed {
                summary: JobSummary::new(&detail.summary_code),
                processed: JobItemCount::new(detail.processed_items),
            },
            JobRunOutcome::SkippedMissedWindow => JobRunResult::SkippedMissedWindow,
            JobRunOutcome::DeferredByAiQueue { resumes_at } => JobRunResult::Deferred {
                resumes_at: *resumes_at,
            },
            JobRunOutcome::Failed(error) => JobRunResult::Failed(match error {
                JobError::PortUnavailable(_) => JobFailureKind::PortUnavailable,
                JobError::StorageFailure(_) => JobFailureKind::StorageFailure,
                JobError::CalendarUnavailable => JobFailureKind::CalendarUnavailable,
                JobError::ExecutionFailed(_) => JobFailureKind::ExecutionFailed,
            }),
        },
    })
}

pub struct AppJobSchedules<S> {
    store: Arc<S>,
}

impl<S> Debug for AppJobSchedules<S> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppJobSchedules").finish_non_exhaustive()
    }
}

impl<S: JobScheduleStore> AppJobSchedules<S> {
    #[must_use]
    pub const fn new(store: Arc<S>) -> Self {
        Self { store }
    }
}

impl<S: JobScheduleStore> JobScheduleSettingsRepository for AppJobSchedules<S> {
    async fn load(&self) -> Result<Vec<JobScheduleChange>, JobError> {
        let saved = self.store.schedules().await.map_err(storage)?;
        let mut changes = Vec::with_capacity(saved.len());
        for item in &saved {
            if let Ok(change) = from_saved_schedule(item) {
                changes.push(change);
            }
        }
        Ok(changes)
    }

    async fn save(&self, change: JobScheduleChange) -> Result<(), JobError> {
        self.store
            .save_schedule(&to_saved_schedule(change)?)
            .await
            .map(|_written| ())
            .map_err(storage)
    }
}

#[derive(Debug, Clone)]
pub struct QueuedJobRecords {
    sender: mpsc::UnboundedSender<JobExecutionRecord>,
}

impl JobRecordSink for QueuedJobRecords {
    fn record(&self, record: &JobExecutionRecord) {
        crate::diagnostics_log::record(&application::DiagnosticEvent::Job(job_diagnosis(record)));
        let _queued = self.sender.send(record.clone());
    }
}

#[must_use]
pub fn job_diagnosis(record: &JobExecutionRecord) -> application::JobDiagnosis {
    let note = match &record.outcome {
        JobRunOutcome::Executed(detail) => application::JobRunNote::Executed {
            summary: application::DiagnosticText::new(&detail.summary_code),
            processed: JobItemCount::new(detail.processed_items),
        },
        JobRunOutcome::SkippedMissedWindow => application::JobRunNote::SkippedMissedWindow,
        JobRunOutcome::DeferredByAiQueue { .. } => application::JobRunNote::Deferred,
        JobRunOutcome::Failed(error) => {
            application::JobRunNote::Failed(application::DiagnosticText::new(&match error {
                JobError::PortUnavailable(port) => format!("port-unavailable:{port}"),
                JobError::StorageFailure(detail) => format!("storage-failure:{detail}"),
                JobError::CalendarUnavailable => String::from("calendar-unavailable"),
                JobError::ExecutionFailed(detail) => format!("execution-failed:{detail}"),
            }))
        }
    };
    application::JobDiagnosis {
        job: application::DiagnosticText::new(record.id.as_code()),
        note,
    }
}

#[must_use]
pub fn job_record_queue<S: JobScheduleStore + 'static>(
    store: Arc<S>,
) -> (QueuedJobRecords, tauri::async_runtime::JoinHandle<()>) {
    let (sender, mut receiver) = mpsc::unbounded_channel::<JobExecutionRecord>();
    let writer = tauri::async_runtime::spawn(async move {
        while let Some(record) = receiver.recv().await {
            if let Ok(entry) = to_run_entry(&record) {
                let _written = store.append_run(&entry).await;
            }
        }
    });
    (QueuedJobRecords { sender }, writer)
}

pub trait ScheduleControl: Send + Sync {
    fn configurable_jobs(&self) -> BoxFuture<'_, Vec<JobStatusSnapshot>>;
    fn change(
        &self,
        change: JobScheduleChange,
    ) -> BoxFuture<'_, Result<JobStatusSnapshot, ScheduleSettingsError>>;
    fn recent_runs(
        &self,
        job: ScheduledJobId,
    ) -> BoxFuture<'_, Result<Vec<JobRunEntry>, SystemPortError>>;
}

pub struct SettingsControl<C: Clock, Q: AiJobQueue, S> {
    settings: ScheduleSettings<C, Q, AppJobSchedules<S>>,
    store: Arc<S>,
}

impl<C: Clock, Q: AiJobQueue, S> Debug for SettingsControl<C, Q, S> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SettingsControl").finish_non_exhaustive()
    }
}

impl<C: Clock, Q: AiJobQueue, S: JobScheduleStore> SettingsControl<C, Q, S> {
    #[must_use]
    pub const fn new(settings: ScheduleSettings<C, Q, AppJobSchedules<S>>, store: Arc<S>) -> Self {
        Self { settings, store }
    }

    pub async fn restore(&self) -> Result<ScheduleRestoreReport, ScheduleSettingsError> {
        self.settings.restore().await
    }
}

impl<C, Q, S> ScheduleControl for SettingsControl<C, Q, S>
where
    C: Clock + 'static,
    Q: AiJobQueue + 'static,
    S: JobScheduleStore + 'static,
{
    fn configurable_jobs(&self) -> BoxFuture<'_, Vec<JobStatusSnapshot>> {
        Box::pin(self.settings.configurable_jobs())
    }

    fn change(
        &self,
        change: JobScheduleChange,
    ) -> BoxFuture<'_, Result<JobStatusSnapshot, ScheduleSettingsError>> {
        Box::pin(self.settings.change(change))
    }

    fn recent_runs(
        &self,
        job: ScheduledJobId,
    ) -> BoxFuture<'_, Result<Vec<JobRunEntry>, SystemPortError>> {
        Box::pin(async move { self.store.recent_runs(&job_code(job)?).await })
    }
}

#[derive(Clone)]
pub struct ScheduleIpcState(pub Arc<dyn ScheduleControl>);

impl Debug for ScheduleIpcState {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str("ScheduleIpcState")
    }
}
