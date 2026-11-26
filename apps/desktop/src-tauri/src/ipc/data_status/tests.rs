use super::{UiDataJob, data_job_status, load_data_status};
use crate::ipc::schedule::UiJobRunOutcome;
use crate::ipc::state::BoxFuture;
use crate::platform::job_settings::ScheduleControl;
use application::{
    JobCode, JobFailureKind, JobItemCount, JobRunEntry, JobRunResult, JobSummary, SystemPortError,
};
use domain::{Nanosecond, OffsetSeconds, SpanSeconds, TimeSpan, Timestamp, UnixSeconds};
use scheduler::{
    JobConfig, JobEnablement, JobKind, JobScheduleChange, JobStatusSnapshot, MissedPolicy,
    ScheduleRule, ScheduleSettingsError, ScheduledJobId,
};

fn at(seconds: i64) -> Timestamp {
    Timestamp::from_parts(
        UnixSeconds::new(seconds),
        Nanosecond::new(0),
        OffsetSeconds::new(0),
    )
    .expect("시각")
}

fn snapshot(id: ScheduledJobId, enabled: JobEnablement) -> JobStatusSnapshot {
    JobStatusSnapshot {
        id,
        config: JobConfig {
            kind: JobKind::CodeOnly,
            schedule: ScheduleRule::Interval(
                TimeSpan::try_new(SpanSeconds::new(300), Nanosecond::new(0)).expect("간격"),
            ),
            missed_policy: MissedPolicy::Skip,
            enabled,
        },
        next_due: Some(at(1_795_562_400)),
        last_evaluated_at: None,
        last_succeeded_at: None,
        last_outcome: None,
        recent_history: Vec::new(),
    }
}

fn entry(code: &str, seconds: i64, result: JobRunResult) -> JobRunEntry {
    JobRunEntry {
        job: JobCode::try_new(code).expect("작업"),
        scheduled_for: at(seconds),
        evaluated_at: at(seconds),
        result,
    }
}

fn executed() -> JobRunResult {
    JobRunResult::Executed {
        summary: JobSummary::new("portfolio_synced"),
        processed: JobItemCount::new(1),
    }
}

struct AllJobs(Vec<JobStatusSnapshot>);

impl ScheduleControl for AllJobs {
    fn configurable_jobs(&self) -> BoxFuture<'_, Vec<JobStatusSnapshot>> {
        Box::pin(std::future::ready(Vec::new()))
    }

    fn all_jobs(&self) -> BoxFuture<'_, Vec<JobStatusSnapshot>> {
        Box::pin(std::future::ready(self.0.clone()))
    }

    fn change(
        &self,
        change: JobScheduleChange,
    ) -> BoxFuture<'_, Result<JobStatusSnapshot, ScheduleSettingsError>> {
        Box::pin(std::future::ready(Err(
            ScheduleSettingsError::NotConfigurable(change.id),
        )))
    }

    fn recent_runs(
        &self,
        job: ScheduledJobId,
    ) -> BoxFuture<'_, Result<Vec<JobRunEntry>, SystemPortError>> {
        let runs = if job == ScheduledJobId::PortfolioSync {
            vec![
                entry(
                    "portfolio_sync",
                    1_795_476_600,
                    JobRunResult::Failed(JobFailureKind::StorageFailure),
                ),
                entry("portfolio_sync", 1_795_476_300, executed()),
            ]
        } else {
            Vec::new()
        };
        Box::pin(std::future::ready(Ok(runs)))
    }
}

#[test]
fn data_status_shows_last_success_latest_run_and_next_run_per_data_job() {
    let control = AllJobs(vec![
        snapshot(ScheduledJobId::TransactionsSync, JobEnablement::Enabled),
        snapshot(ScheduledJobId::IntradayCompaction, JobEnablement::Enabled),
        snapshot(ScheduledJobId::PortfolioSync, JobEnablement::Enabled),
        snapshot(ScheduledJobId::DailyBackup, JobEnablement::Disabled),
    ]);
    let view = tauri::async_runtime::block_on(load_data_status(&control)).expect("상태");
    crate::ipc::tests::assert_response_snapshot("get_data_status", &view);
    let jobs: Vec<UiDataJob> = view.jobs.iter().map(|row| row.job).collect();
    assert_eq!(
        jobs,
        vec![
            UiDataJob::Holdings,
            UiDataJob::Transactions,
            UiDataJob::Backup
        ]
    );
    let holdings = view.jobs.first().expect("보유 종목");
    assert_eq!(
        holdings.last_success.as_deref(),
        Some("2026-11-24T08:25:00+09:00")
    );
    let last = holdings.last_run.as_ref().expect("마지막 실행");
    assert_eq!(last.at, "2026-11-24T08:30:00+09:00");
    assert_eq!(last.outcome, UiJobRunOutcome::Failed);
    assert_eq!(
        holdings.next_run.as_deref(),
        Some("2026-11-25T08:20:00+09:00")
    );
    let transactions = view.jobs.get(1).expect("거래내역");
    assert_eq!(transactions.last_success, None);
    assert_eq!(transactions.last_run, None);
    let backup = view.jobs.get(2).expect("백업");
    assert_eq!(backup.next_run, None);
}

#[test]
fn a_success_seen_only_in_this_session_still_counts() {
    let mut current = snapshot(
        ScheduledJobId::DailyValuationSnapshot,
        JobEnablement::Enabled,
    );
    current.last_succeeded_at = Some(at(1_795_562_000));
    let row = data_job_status(&current, &[]).expect("일별 평가");
    assert_eq!(row.job, UiDataJob::DailyValuation);
    assert_eq!(
        row.last_success.as_deref(),
        Some("2026-11-25T08:13:20+09:00")
    );
    assert_eq!(
        data_job_status(
            &snapshot(ScheduledJobId::TossSpecWatch, JobEnablement::Enabled),
            &[]
        ),
        None
    );
}
