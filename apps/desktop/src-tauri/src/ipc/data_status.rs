use application::{JobRunEntry, JobRunResult};
use scheduler::{JobEnablement, JobStatusSnapshot, ScheduledJobId};
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Manager, Runtime};

use super::briefing::kst_iso;
use super::error::{IpcError, IpcErrorKind};
use super::schedule::{UiJobRun, UiJobRunOutcome};
use crate::platform::job_settings::{ScheduleControl, ScheduleIpcState};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Type)]
#[serde(deny_unknown_fields)]
#[specta(skip_attr = "serde")]
pub struct GetDataStatusRequest {
    pub scope: DataStatusScope,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Type)]
pub enum DataStatusScope {
    #[serde(rename = "current")]
    Current,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Type)]
pub enum UiDataJob {
    #[serde(rename = "holdings")]
    Holdings,
    #[serde(rename = "transactions")]
    Transactions,
    #[serde(rename = "daily-valuation")]
    DailyValuation,
    #[serde(rename = "estimates")]
    Estimates,
    #[serde(rename = "kr-scoring")]
    KrScoring,
    #[serde(rename = "us-scoring")]
    UsScoring,
    #[serde(rename = "morning-briefing")]
    MorningBriefing,
    #[serde(rename = "closing-review")]
    ClosingReview,
    #[serde(rename = "pre-us-market-check")]
    PreUsMarketCheck,
    #[serde(rename = "backup")]
    Backup,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct UiDataJobStatus {
    pub job: UiDataJob,
    pub last_success: Option<String>,
    pub last_run: Option<UiJobRun>,
    pub next_run: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct UiDataStatusView {
    pub jobs: Vec<UiDataJobStatus>,
}

#[must_use]
pub const fn data_job(id: ScheduledJobId) -> Option<UiDataJob> {
    match id {
        ScheduledJobId::PortfolioSync => Some(UiDataJob::Holdings),
        ScheduledJobId::TransactionsSync => Some(UiDataJob::Transactions),
        ScheduledJobId::DailyValuationSnapshot => Some(UiDataJob::DailyValuation),
        ScheduledJobId::EstimateCycle => Some(UiDataJob::Estimates),
        ScheduledJobId::EstimateResolution => Some(UiDataJob::KrScoring),
        ScheduledJobId::UsEstimateResolution => Some(UiDataJob::UsScoring),
        ScheduledJobId::MorningBriefing => Some(UiDataJob::MorningBriefing),
        ScheduledJobId::ClosingReview => Some(UiDataJob::ClosingReview),
        ScheduledJobId::PreUsMarketCheck => Some(UiDataJob::PreUsMarketCheck),
        ScheduledJobId::DailyBackup => Some(UiDataJob::Backup),
        ScheduledJobId::NewsCollection
        | ScheduledJobId::CandidateDiscovery
        | ScheduledJobId::WeeklyReview
        | ScheduledJobId::IntradayCompaction
        | ScheduledJobId::NewsRetention
        | ScheduledJobId::TossSpecWatch
        | ScheduledJobId::IntradayWatch => None,
    }
}

const fn outcome(result: &JobRunResult) -> UiJobRunOutcome {
    match result {
        JobRunResult::Executed { .. } => UiJobRunOutcome::Succeeded,
        JobRunResult::SkippedMissedWindow => UiJobRunOutcome::Skipped,
        JobRunResult::Deferred { .. } => UiJobRunOutcome::Waiting,
        JobRunResult::Failed(_) => UiJobRunOutcome::Failed,
    }
}

#[must_use]
pub fn data_job_status(
    snapshot: &JobStatusSnapshot,
    records: &[JobRunEntry],
) -> Option<UiDataJobStatus> {
    let job = data_job(snapshot.id)?;
    let last_success = records
        .iter()
        .filter(|entry| outcome(&entry.result) == UiJobRunOutcome::Succeeded)
        .map(|entry| entry.evaluated_at)
        .chain(snapshot.last_succeeded_at)
        .max()
        .and_then(kst_iso);
    let last_run = records
        .iter()
        .max_by_key(|entry| entry.evaluated_at)
        .and_then(|entry| {
            Some(UiJobRun {
                at: kst_iso(entry.evaluated_at)?,
                outcome: outcome(&entry.result),
            })
        });
    let next_run = match snapshot.config.enabled {
        JobEnablement::Enabled => snapshot.next_due.and_then(kst_iso),
        JobEnablement::Disabled => None,
    };
    Some(UiDataJobStatus {
        job,
        last_success,
        last_run,
        next_run,
    })
}

pub async fn load_data_status(control: &dyn ScheduleControl) -> Result<UiDataStatusView, IpcError> {
    let mut jobs = Vec::new();
    for snapshot in control.all_jobs().await {
        let records = control
            .recent_runs(snapshot.id)
            .await
            .map_err(|_error| IpcError {
                kind: IpcErrorKind::Retryable,
            })?;
        if let Some(view) = data_job_status(&snapshot, &records) {
            jobs.push(view);
        }
    }
    jobs.sort_by_key(|row| row.job);
    Ok(UiDataStatusView { jobs })
}

#[tauri::command]
#[specta::specta]
pub async fn get_data_status<R: Runtime>(
    app: AppHandle<R>,
    request: GetDataStatusRequest,
) -> Result<UiDataStatusView, IpcError> {
    let GetDataStatusRequest {
        scope: DataStatusScope::Current,
    } = request;
    let control = app
        .try_state::<ScheduleIpcState>()
        .map(|state| state.inner().clone())
        .ok_or(IpcError {
            kind: IpcErrorKind::NoData,
        })?;
    load_data_status(control.0.as_ref()).await
}

#[cfg(test)]
mod tests;
