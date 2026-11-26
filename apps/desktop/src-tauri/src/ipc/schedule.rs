use application::{JobRunEntry, JobRunResult};
use scheduler::{
    JobEnablement, JobScheduleChange, JobStatusSnapshot, KstTimeOfDay, ScheduleRule,
    ScheduleSettingsError, ScheduledJobId,
};
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Manager, Runtime};

use super::briefing::kst_iso;
use super::error::{IpcError, IpcErrorKind};
use crate::platform::job_settings::{ScheduleControl, ScheduleIpcState};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Type)]
#[serde(deny_unknown_fields)]
#[specta(skip_attr = "serde")]
pub struct GetJobSchedulesRequest {
    pub scope: JobSchedulesScope,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Type)]
pub enum JobSchedulesScope {
    #[serde(rename = "ai")]
    Ai,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
pub enum UiScheduledJob {
    #[serde(rename = "morning-briefing")]
    MorningBriefing,
    #[serde(rename = "closing-review")]
    ClosingReview,
    #[serde(rename = "pre-us-market-check")]
    PreUsMarketCheck,
    #[serde(rename = "estimate-cycle")]
    EstimateCycle,
    #[serde(rename = "candidate-discovery")]
    CandidateDiscovery,
    #[serde(rename = "weekly-review")]
    WeeklyReview,
    #[serde(rename = "intraday-watch")]
    IntradayWatch,
    #[serde(rename = "news-collection")]
    NewsCollection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
pub enum UiJobSwitch {
    #[serde(rename = "on")]
    On,
    #[serde(rename = "off")]
    Off,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(deny_unknown_fields)]
#[specta(skip_attr = "serde")]
pub struct UiKstTime {
    pub hour: u8,
    pub minute: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
pub enum UiTimeAdjustment {
    #[serde(rename = "adjustable")]
    Adjustable,
    #[serde(rename = "not-adjustable")]
    NotAdjustable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
pub enum UiJobRunOutcome {
    #[serde(rename = "succeeded")]
    Succeeded,
    #[serde(rename = "skipped")]
    Skipped,
    #[serde(rename = "waiting")]
    Waiting,
    #[serde(rename = "failed")]
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct UiJobRun {
    pub at: String,
    pub outcome: UiJobRunOutcome,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct UiJobSchedule {
    pub job: UiScheduledJob,
    pub enabled: UiJobSwitch,
    pub time: Option<UiKstTime>,
    pub time_adjustment: UiTimeAdjustment,
    pub next_run: Option<String>,
    pub last_run: Option<UiJobRun>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct UiJobSchedulesView {
    pub jobs: Vec<UiJobSchedule>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Type)]
#[serde(deny_unknown_fields)]
#[specta(skip_attr = "serde")]
pub struct SetJobScheduleRequest {
    pub job: UiScheduledJob,
    pub enabled: UiJobSwitch,
    pub time: Option<UiKstTime>,
}

const fn job_id(job: UiScheduledJob) -> ScheduledJobId {
    match job {
        UiScheduledJob::MorningBriefing => ScheduledJobId::MorningBriefing,
        UiScheduledJob::ClosingReview => ScheduledJobId::ClosingReview,
        UiScheduledJob::PreUsMarketCheck => ScheduledJobId::PreUsMarketCheck,
        UiScheduledJob::EstimateCycle => ScheduledJobId::EstimateCycle,
        UiScheduledJob::CandidateDiscovery => ScheduledJobId::CandidateDiscovery,
        UiScheduledJob::WeeklyReview => ScheduledJobId::WeeklyReview,
        UiScheduledJob::IntradayWatch => ScheduledJobId::IntradayWatch,
        UiScheduledJob::NewsCollection => ScheduledJobId::NewsCollection,
    }
}

const fn ui_job(id: ScheduledJobId) -> Option<UiScheduledJob> {
    match id {
        ScheduledJobId::MorningBriefing => Some(UiScheduledJob::MorningBriefing),
        ScheduledJobId::ClosingReview => Some(UiScheduledJob::ClosingReview),
        ScheduledJobId::PreUsMarketCheck => Some(UiScheduledJob::PreUsMarketCheck),
        ScheduledJobId::EstimateCycle => Some(UiScheduledJob::EstimateCycle),
        ScheduledJobId::CandidateDiscovery => Some(UiScheduledJob::CandidateDiscovery),
        ScheduledJobId::WeeklyReview => Some(UiScheduledJob::WeeklyReview),
        ScheduledJobId::IntradayWatch => Some(UiScheduledJob::IntradayWatch),
        ScheduledJobId::NewsCollection => Some(UiScheduledJob::NewsCollection),
        ScheduledJobId::PortfolioSync
        | ScheduledJobId::TransactionsSync
        | ScheduledJobId::DailyValuationSnapshot
        | ScheduledJobId::EstimateResolution
        | ScheduledJobId::IntradayCompaction
        | ScheduledJobId::NewsRetention
        | ScheduledJobId::TossSpecWatch
        | ScheduledJobId::DailyBackup
        | ScheduledJobId::UsEstimateResolution => None,
    }
}

const fn time_of(rule: ScheduleRule) -> (Option<UiKstTime>, UiTimeAdjustment) {
    match rule {
        ScheduleRule::DailyKst(time) | ScheduleRule::WeeklyKst { time, .. } => (
            Some(UiKstTime {
                hour: time.hour(),
                minute: time.minute(),
            }),
            UiTimeAdjustment::Adjustable,
        ),
        ScheduleRule::Interval(_)
        | ScheduleRule::AfterMarketClose { .. }
        | ScheduleRule::BeforeMarketOpen { .. }
        | ScheduleRule::EventTriggered => (None, UiTimeAdjustment::NotAdjustable),
    }
}

fn last_run(records: &[JobRunEntry]) -> Option<UiJobRun> {
    let latest = records.first()?;
    Some(UiJobRun {
        at: kst_iso(latest.evaluated_at)?,
        outcome: match latest.result {
            JobRunResult::Executed { .. } => UiJobRunOutcome::Succeeded,
            JobRunResult::SkippedMissedWindow => UiJobRunOutcome::Skipped,
            JobRunResult::Deferred { .. } => UiJobRunOutcome::Waiting,
            JobRunResult::Failed(_) => UiJobRunOutcome::Failed,
        },
    })
}

fn to_view(snapshot: &JobStatusSnapshot, records: &[JobRunEntry]) -> Option<UiJobSchedule> {
    let (time, time_adjustment) = time_of(snapshot.config.schedule);
    Some(UiJobSchedule {
        job: ui_job(snapshot.id)?,
        enabled: match snapshot.config.enabled {
            JobEnablement::Enabled => UiJobSwitch::On,
            JobEnablement::Disabled => UiJobSwitch::Off,
        },
        time,
        time_adjustment,
        next_run: snapshot.next_due.and_then(kst_iso),
        last_run: last_run(records),
    })
}

const fn settings_error(error: &ScheduleSettingsError) -> IpcError {
    let kind = match error {
        ScheduleSettingsError::UnknownJob(_) => IpcErrorKind::NoData,
        ScheduleSettingsError::NotConfigurable(_)
        | ScheduleSettingsError::TimeNotAdjustable(_)
        | ScheduleSettingsError::InvalidSchedule(_) => IpcErrorKind::UserFixable,
        ScheduleSettingsError::Storage(_) => IpcErrorKind::Retryable,
    };
    IpcError { kind }
}

const fn retryable() -> IpcError {
    IpcError {
        kind: IpcErrorKind::Retryable,
    }
}

pub async fn load_job_schedules(
    control: &dyn ScheduleControl,
) -> Result<UiJobSchedulesView, IpcError> {
    let mut jobs = Vec::new();
    for snapshot in control.configurable_jobs().await {
        let records = control
            .recent_runs(snapshot.id)
            .await
            .map_err(|_error| retryable())?;
        if let Some(view) = to_view(&snapshot, &records) {
            jobs.push(view);
        }
    }
    Ok(UiJobSchedulesView { jobs })
}

pub async fn change_job_schedule(
    control: &dyn ScheduleControl,
    request: SetJobScheduleRequest,
) -> Result<UiJobSchedule, IpcError> {
    let time = match request.time {
        Some(value) => Some(
            KstTimeOfDay::try_new(value.hour, value.minute).map_err(|_error| IpcError {
                kind: IpcErrorKind::UserFixable,
            })?,
        ),
        None => None,
    };
    let snapshot = control
        .change(JobScheduleChange {
            id: job_id(request.job),
            enabled: match request.enabled {
                UiJobSwitch::On => JobEnablement::Enabled,
                UiJobSwitch::Off => JobEnablement::Disabled,
            },
            time,
        })
        .await
        .map_err(|error| settings_error(&error))?;
    let records = control
        .recent_runs(snapshot.id)
        .await
        .map_err(|_error| retryable())?;
    to_view(&snapshot, &records).ok_or(IpcError {
        kind: IpcErrorKind::Bug,
    })
}

fn managed<R: Runtime>(app: &AppHandle<R>) -> Result<ScheduleIpcState, IpcError> {
    app.try_state::<ScheduleIpcState>()
        .map(|state| state.inner().clone())
        .ok_or(IpcError {
            kind: IpcErrorKind::NoData,
        })
}

#[tauri::command]
#[specta::specta]
pub async fn get_job_schedules<R: Runtime>(
    app: AppHandle<R>,
    request: GetJobSchedulesRequest,
) -> Result<UiJobSchedulesView, IpcError> {
    let GetJobSchedulesRequest {
        scope: JobSchedulesScope::Ai,
    } = request;
    let state = managed(&app)?;
    load_job_schedules(state.0.as_ref()).await
}

#[tauri::command]
#[specta::specta]
pub async fn set_job_schedule<R: Runtime>(
    app: AppHandle<R>,
    request: SetJobScheduleRequest,
) -> Result<UiJobSchedule, IpcError> {
    let state = managed(&app)?;
    change_job_schedule(state.0.as_ref(), request).await
}

#[cfg(test)]
mod tests;
