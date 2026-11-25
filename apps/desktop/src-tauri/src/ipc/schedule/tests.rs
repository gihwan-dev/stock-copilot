use super::{
    SetJobScheduleRequest, UiJobRunOutcome, UiJobSwitch, UiKstTime, UiScheduledJob,
    UiTimeAdjustment, change_job_schedule, load_job_schedules,
};
use crate::ipc::error::IpcErrorKind;
use crate::ipc::state::BoxFuture;
use crate::platform::job_settings::ScheduleControl;
use application::{
    JobCode, JobItemCount, JobRunEntry, JobRunResult, JobSummary, SystemFailure, SystemPort,
    SystemPortError,
};
use domain::{
    KnownMarket, Nanosecond, OffsetSeconds, SpanSeconds, TimeSpan, Timestamp, UnixSeconds,
};
use scheduler::{
    JobConfig, JobEnablement, JobError, JobKind, JobScheduleChange, JobStatusSnapshot,
    KstTimeOfDay, MissedPolicy, ScheduleRule, ScheduleSettingsError, ScheduledJobId,
};
use std::sync::RwLock;

fn at(seconds: i64) -> Timestamp {
    Timestamp::from_parts(
        UnixSeconds::new(seconds),
        Nanosecond::new(0),
        OffsetSeconds::new(0),
    )
    .expect("시각")
}

fn kst(hour: u8, minute: u8) -> KstTimeOfDay {
    KstTimeOfDay::try_new(hour, minute).expect("시각")
}

fn snapshot(
    id: ScheduledJobId,
    schedule: ScheduleRule,
    enabled: JobEnablement,
) -> JobStatusSnapshot {
    JobStatusSnapshot {
        id,
        config: JobConfig {
            kind: JobKind::AiBackground,
            schedule,
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

fn morning(time: KstTimeOfDay, enabled: JobEnablement) -> JobStatusSnapshot {
    snapshot(
        ScheduledJobId::MorningBriefing,
        ScheduleRule::DailyKst(time),
        enabled,
    )
}

fn pre_us() -> JobStatusSnapshot {
    snapshot(
        ScheduledJobId::PreUsMarketCheck,
        ScheduleRule::BeforeMarketOpen {
            market: KnownMarket::Us,
            lead: TimeSpan::try_new(SpanSeconds::new(2_400), Nanosecond::new(0)).expect("간격"),
            fallback: kst(22, 50),
        },
        JobEnablement::Disabled,
    )
}

struct FakeControl {
    jobs: Vec<JobStatusSnapshot>,
    reply: RwLock<Option<Result<JobStatusSnapshot, ScheduleSettingsError>>>,
    changes: RwLock<Vec<JobScheduleChange>>,
    runs_fail: RunReads,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum RunReads {
    Available,
    Failing,
}

impl FakeControl {
    fn new(jobs: Vec<JobStatusSnapshot>) -> Self {
        Self {
            jobs,
            reply: RwLock::new(None),
            changes: RwLock::new(Vec::new()),
            runs_fail: RunReads::Available,
        }
    }
}

impl ScheduleControl for FakeControl {
    fn configurable_jobs(&self) -> BoxFuture<'_, Vec<JobStatusSnapshot>> {
        Box::pin(std::future::ready(self.jobs.clone()))
    }

    fn change(
        &self,
        change: JobScheduleChange,
    ) -> BoxFuture<'_, Result<JobStatusSnapshot, ScheduleSettingsError>> {
        self.changes.write().expect("잠금").push(change);
        let reply = self
            .reply
            .write()
            .expect("잠금")
            .take()
            .expect("준비된 응답");
        Box::pin(std::future::ready(reply))
    }

    fn recent_runs(
        &self,
        job: ScheduledJobId,
    ) -> BoxFuture<'_, Result<Vec<JobRunEntry>, SystemPortError>> {
        let runs = match (self.runs_fail, job) {
            (RunReads::Failing, _) => Err(SystemPortError::new(
                SystemPort::Repository,
                SystemFailure::InvalidData,
            )),
            (RunReads::Available, ScheduledJobId::MorningBriefing) => Ok(vec![JobRunEntry {
                job: JobCode::try_new("morning_briefing").expect("작업"),
                scheduled_for: at(1_795_476_000),
                evaluated_at: at(1_795_476_005),
                result: JobRunResult::Executed {
                    summary: JobSummary::new("completed"),
                    processed: JobItemCount::new(1),
                },
            }]),
            (RunReads::Available, _) => Ok(Vec::new()),
        };
        Box::pin(std::future::ready(runs))
    }
}

fn block_on<F: Future>(future: F) -> F::Output {
    tauri::async_runtime::block_on(future)
}

#[test]
fn schedules_view_shows_adjustable_daily_time_market_bound_jobs_and_the_last_run() {
    let control = FakeControl::new(vec![morning(kst(8, 20), JobEnablement::Enabled), pre_us()]);
    let view = block_on(load_job_schedules(&control)).expect("조회");
    let [first, second] = view.jobs.as_slice() else {
        panic!("두 작업: {view:?}");
    };
    assert_eq!(first.job, UiScheduledJob::MorningBriefing);
    assert_eq!(first.enabled, UiJobSwitch::On);
    assert_eq!(
        first.time,
        Some(UiKstTime {
            hour: 8,
            minute: 20
        })
    );
    assert_eq!(first.time_adjustment, UiTimeAdjustment::Adjustable);
    assert_eq!(first.next_run.as_deref(), Some("2026-11-25T08:20:00+09:00"));
    let last = first.last_run.as_ref().expect("마지막 실행");
    assert_eq!(last.outcome, UiJobRunOutcome::Succeeded);
    assert_eq!(last.at, "2026-11-24T08:20:05+09:00");
    assert_eq!(second.job, UiScheduledJob::PreUsMarketCheck);
    assert_eq!(second.enabled, UiJobSwitch::Off);
    assert_eq!(second.time, None);
    assert_eq!(second.time_adjustment, UiTimeAdjustment::NotAdjustable);
    assert_eq!(second.last_run, None);
}

#[test]
fn unreadable_run_history_is_retryable() {
    let mut control = FakeControl::new(vec![morning(kst(8, 20), JobEnablement::Enabled)]);
    control.runs_fail = RunReads::Failing;
    let error = block_on(load_job_schedules(&control)).expect_err("실패");
    assert_eq!(error.kind, IpcErrorKind::Retryable);
}

#[test]
fn change_maps_the_request_and_returns_the_applied_schedule() {
    let control = FakeControl::new(Vec::new());
    *control.reply.write().expect("잠금") = Some(Ok(morning(kst(7, 45), JobEnablement::Enabled)));
    let view = block_on(change_job_schedule(
        &control,
        SetJobScheduleRequest {
            job: UiScheduledJob::MorningBriefing,
            enabled: UiJobSwitch::On,
            time: Some(UiKstTime {
                hour: 7,
                minute: 45,
            }),
        },
    ))
    .expect("변경");
    assert_eq!(
        view.time,
        Some(UiKstTime {
            hour: 7,
            minute: 45
        })
    );
    assert_eq!(
        *control.changes.read().expect("잠금"),
        vec![JobScheduleChange {
            id: ScheduledJobId::MorningBriefing,
            enabled: JobEnablement::Enabled,
            time: Some(kst(7, 45)),
        }]
    );
}

#[test]
fn out_of_range_time_is_rejected_before_any_change() {
    let control = FakeControl::new(Vec::new());
    let error = block_on(change_job_schedule(
        &control,
        SetJobScheduleRequest {
            job: UiScheduledJob::MorningBriefing,
            enabled: UiJobSwitch::On,
            time: Some(UiKstTime {
                hour: 24,
                minute: 0,
            }),
        },
    ))
    .expect_err("거부");
    assert_eq!(error.kind, IpcErrorKind::UserFixable);
    assert_eq!(*control.changes.read().expect("잠금"), Vec::new());
}

#[test]
fn settings_errors_map_to_user_fixable_or_retryable() {
    for (reply, expected) in [
        (
            ScheduleSettingsError::TimeNotAdjustable(ScheduledJobId::PreUsMarketCheck),
            IpcErrorKind::UserFixable,
        ),
        (
            ScheduleSettingsError::NotConfigurable(ScheduledJobId::PreUsMarketCheck),
            IpcErrorKind::UserFixable,
        ),
        (
            ScheduleSettingsError::Storage(JobError::StorageFailure(String::from("closed"))),
            IpcErrorKind::Retryable,
        ),
        (
            ScheduleSettingsError::UnknownJob(ScheduledJobId::PreUsMarketCheck),
            IpcErrorKind::NoData,
        ),
    ] {
        let control = FakeControl::new(Vec::new());
        *control.reply.write().expect("잠금") = Some(Err(reply));
        let error = block_on(change_job_schedule(
            &control,
            SetJobScheduleRequest {
                job: UiScheduledJob::PreUsMarketCheck,
                enabled: UiJobSwitch::Off,
                time: Some(UiKstTime {
                    hour: 22,
                    minute: 0,
                }),
            },
        ))
        .expect_err("실패");
        assert_eq!(error.kind, expected);
    }
}

#[test]
fn set_request_rejects_unknown_fields() {
    let parsed: Result<SetJobScheduleRequest, _> =
        serde_json::from_str(r#"{"job":"morning-briefing","enabled":"on","time":null,"extra":1}"#);
    let _rejected = parsed.expect_err("모르는 필드 거부");
    let accepted: SetJobScheduleRequest = serde_json::from_str(
        r#"{"job":"morning-briefing","enabled":"off","time":{"hour":7,"minute":0}}"#,
    )
    .expect("요청");
    assert_eq!(accepted.enabled, UiJobSwitch::Off);
}
