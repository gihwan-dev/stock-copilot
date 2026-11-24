use crate::{
    AiJobQueue, AiQueueAdmission, AiQueueError, DisabledAiJobQueue, JobConfig, JobEnablement,
    JobError, JobFuture, JobKind, JobRegistry, JobRunDetail, JobRunOutcome, JobScheduleOverride,
    KST_OFFSET_SECONDS, KstTimeOfDay, KstWeekday, LoopControl, MissedPolicy, RegistryError,
    ScheduleRule, ScheduledJob, ScheduledJobId, Scheduler,
};
use application::{EventBusBuilder, EventSubscriber};
use domain::{
    DomainEvent, DomainEventKind, Nanosecond, OffsetSeconds, SpanSeconds, TimeSpan, Timestamp,
    UnixSeconds,
};
use std::collections::BTreeSet;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use std::time::Duration;
use testkit_ports::SteppedClock;
use tokio::sync::{RwLock, watch};

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .expect("테스트 런타임")
}

fn kst(year: i32, month: u8, day: u8, hour: u8, minute: u8) -> Timestamp {
    let date = domain::CalendarDate::from_parts(
        domain::CalendarYear::new(year),
        domain::MonthNumber::new(month),
        domain::DayNumber::new(day),
    )
    .expect("날짜");
    let epoch = domain::CalendarDate::from_parts(
        domain::CalendarYear::new(1970),
        domain::MonthNumber::new(1),
        domain::DayNumber::new(1),
    )
    .expect("에포크");
    let days = *epoch.days_until(date).expect("일수").as_i64();
    let total_kst = days
        .checked_mul(86_400)
        .and_then(|value| value.checked_add(i64::from(hour).checked_mul(3_600)?))
        .and_then(|value| value.checked_add(i64::from(minute).checked_mul(60)?))
        .expect("KST 초");
    let utc = total_kst
        .checked_sub(i64::from(KST_OFFSET_SECONDS))
        .expect("UTC 초");
    Timestamp::from_parts(
        UnixSeconds::new(utc),
        Nanosecond::new(0),
        OffsetSeconds::new(KST_OFFSET_SECONDS),
    )
    .expect("시각")
}

fn span_secs(seconds: u64) -> TimeSpan {
    TimeSpan::try_new(SpanSeconds::new(seconds), Nanosecond::new(0)).expect("기간")
}

struct CountingJob {
    id: ScheduledJobId,
    config: JobConfig,
    runs: Arc<AtomicUsize>,
    run_times: Arc<RwLock<Vec<Timestamp>>>,
}

impl CountingJob {
    fn new(
        id: ScheduledJobId,
        config: JobConfig,
    ) -> (Self, Arc<AtomicUsize>, Arc<RwLock<Vec<Timestamp>>>) {
        let runs = Arc::new(AtomicUsize::new(0));
        let run_times = Arc::new(RwLock::new(Vec::new()));
        (
            Self {
                id,
                config,
                runs: Arc::clone(&runs),
                run_times: Arc::clone(&run_times),
            },
            runs,
            run_times,
        )
    }
}

impl ScheduledJob for CountingJob {
    fn id(&self) -> ScheduledJobId {
        self.id
    }

    fn default_config(&self) -> JobConfig {
        self.config
    }

    fn run(&self, now: Timestamp) -> JobFuture<'_> {
        let runs = Arc::clone(&self.runs);
        let run_times = Arc::clone(&self.run_times);
        Box::pin(async move {
            runs.fetch_add(1, Ordering::SeqCst);
            run_times.write().await.push(now);
            Ok(JobRunDetail::new("ok", 1))
        })
    }
}

struct ScriptedAiQueue {
    admission: RwLock<Result<AiQueueAdmission, AiQueueError>>,
    requests: AtomicUsize,
    releases: AtomicUsize,
}

impl ScriptedAiQueue {
    fn new(admission: Result<AiQueueAdmission, AiQueueError>) -> Self {
        Self {
            admission: RwLock::new(admission),
            requests: AtomicUsize::new(0),
            releases: AtomicUsize::new(0),
        }
    }

    async fn set(&self, admission: Result<AiQueueAdmission, AiQueueError>) {
        *self.admission.write().await = admission;
    }
}

impl AiJobQueue for ScriptedAiQueue {
    async fn request_background_slot(
        &self,
        _job: ScheduledJobId,
        _now: Timestamp,
    ) -> Result<AiQueueAdmission, AiQueueError> {
        self.requests.fetch_add(1, Ordering::SeqCst);
        *self.admission.read().await
    }

    fn release_background_slot(
        &self,
        _job: ScheduledJobId,
        _now: Timestamp,
    ) -> impl Future<Output = Result<(), AiQueueError>> + Send {
        self.releases.fetch_add(1, Ordering::SeqCst);
        std::future::ready(Ok(()))
    }
}

#[test]
fn registry_rejects_duplicates_and_lists_deterministically() {
    let mut registry = JobRegistry::new();
    let config = JobConfig {
        kind: JobKind::CodeOnly,
        schedule: ScheduleRule::DailyKst(KstTimeOfDay::try_new(16, 0).expect("16:00")),
        missed_policy: MissedPolicy::CatchUpOnce,
        enabled: JobEnablement::Enabled,
    };
    let (job_a, _, _) = CountingJob::new(ScheduledJobId::DailyBackup, config);
    let (job_b, _, _) = CountingJob::new(ScheduledJobId::DailyValuationSnapshot, config);
    let (dup, _, _) = CountingJob::new(ScheduledJobId::DailyBackup, config);
    registry.register(job_a).expect("첫 등록");
    registry.register(job_b).expect("둘째 등록");
    assert_eq!(
        registry.register(dup),
        Err(RegistryError::DuplicateJob(ScheduledJobId::DailyBackup))
    );
    let ids: Vec<ScheduledJobId> = registry.list().into_iter().map(|item| item.id).collect();
    assert_eq!(
        ids,
        vec![
            ScheduledJobId::DailyValuationSnapshot,
            ScheduledJobId::DailyBackup,
        ]
    );
}

#[test]
fn jumping_multiple_days_catches_up_only_once_for_catch_up_policy() {
    runtime().block_on(async {
        let clock = Arc::new(SteppedClock::new(kst(2026, 11, 10, 8, 0)));
        let (job, runs, run_times) = CountingJob::new(
            ScheduledJobId::DailyValuationSnapshot,
            JobConfig {
                kind: JobKind::CodeOnly,
                schedule: ScheduleRule::DailyKst(KstTimeOfDay::try_new(16, 0).expect("16:00")),
                missed_policy: MissedPolicy::CatchUpOnce,
                enabled: JobEnablement::Enabled,
            },
        );
        let mut registry = JobRegistry::new();
        registry.register(job).expect("등록");
        let scheduler = Scheduler::new(Arc::clone(&clock), Arc::new(DisabledAiJobQueue), registry)
            .expect("스케줄러");

        let before = scheduler.tick_once().await;
        assert!(before.records.is_empty(), "예정 시각 전에는 실행하지 않음");
        assert_eq!(runs.load(Ordering::SeqCst), 0);

        clock
            .advance_to(kst(2026, 11, 15, 17, 30))
            .expect("5일 건너뛰기");
        let after_jump = scheduler.tick_once().await;
        assert_eq!(after_jump.records.len(), 1);
        assert_eq!(runs.load(Ordering::SeqCst), 1);
        assert_eq!(*run_times.read().await, vec![kst(2026, 11, 15, 17, 30)]);

        let repeat = scheduler.tick_once().await;
        assert!(repeat.records.is_empty(), "따라잡은 직후 중복 실행 없음");
        assert_eq!(runs.load(Ordering::SeqCst), 1);

        let snapshots = scheduler.status_snapshots().await;
        assert_eq!(
            snapshots.first().expect("상태").next_due,
            Some(kst(2026, 11, 16, 16, 0))
        );
    });
}

#[test]
fn skip_policy_skips_missed_window_and_executes_when_on_time() {
    runtime().block_on(async {
        let clock = Arc::new(SteppedClock::new(kst(2026, 11, 10, 8, 0)));
        let (job, runs, _) = CountingJob::new(
            ScheduledJobId::MorningBriefing,
            JobConfig {
                kind: JobKind::CodeOnly,
                schedule: ScheduleRule::DailyKst(KstTimeOfDay::try_new(8, 20).expect("08:20")),
                missed_policy: MissedPolicy::Skip,
                enabled: JobEnablement::Enabled,
            },
        );
        let mut registry = JobRegistry::new();
        registry.register(job).expect("등록");
        let scheduler = Scheduler::new(Arc::clone(&clock), Arc::new(DisabledAiJobQueue), registry)
            .expect("스케줄러");

        clock.advance_to(kst(2026, 11, 10, 10, 0)).expect("창 초과");
        let skipped = scheduler.tick_once().await;
        assert_eq!(skipped.records.len(), 1);
        assert_eq!(
            skipped.records.first().expect("기록").outcome,
            JobRunOutcome::SkippedMissedWindow
        );
        assert_eq!(runs.load(Ordering::SeqCst), 0);

        clock
            .advance_to(kst(2026, 11, 11, 8, 20))
            .expect("다음 날 정시");
        let on_time = scheduler.tick_once().await;
        assert_eq!(on_time.records.len(), 1);
        assert_eq!(
            on_time.records.first().expect("기록").outcome,
            JobRunOutcome::Executed(JobRunDetail::new("ok", 1))
        );
        assert_eq!(runs.load(Ordering::SeqCst), 1);
    });
}

#[test]
fn ai_background_job_defers_until_ai_queue_admits_and_releases_after_run() {
    runtime().block_on(async {
        let clock = Arc::new(SteppedClock::new(kst(2026, 11, 10, 8, 20)));
        let resume_at = kst(2026, 11, 10, 9, 0);
        let ai_queue = Arc::new(ScriptedAiQueue::new(Ok(AiQueueAdmission::RateLimited {
            resumes_at: Some(resume_at),
        })));
        let (job, runs, _) = CountingJob::new(
            ScheduledJobId::MorningBriefing,
            JobConfig {
                kind: JobKind::AiBackground,
                schedule: ScheduleRule::DailyKst(KstTimeOfDay::try_new(8, 20).expect("08:20")),
                missed_policy: MissedPolicy::CatchUpOnce,
                enabled: JobEnablement::Enabled,
            },
        );
        let mut registry = JobRegistry::new();
        registry.register(job).expect("등록");
        let scheduler =
            Scheduler::new(Arc::clone(&clock), Arc::clone(&ai_queue), registry).expect("스케줄러");

        let deferred = scheduler.tick_once().await;
        assert_eq!(
            deferred.records.first().expect("기록").outcome,
            JobRunOutcome::DeferredByAiQueue {
                resumes_at: Some(resume_at),
            }
        );
        assert_eq!(runs.load(Ordering::SeqCst), 0);
        assert_eq!(ai_queue.releases.load(Ordering::SeqCst), 0);

        clock
            .advance_to(kst(2026, 11, 10, 8, 45))
            .expect("재개 시각 전");
        assert!(
            scheduler.tick_once().await.records.is_empty(),
            "재개 시각 전 대기"
        );

        ai_queue.set(Ok(AiQueueAdmission::Admitted)).await;
        clock.advance_to(resume_at).expect("재개 시각 도달");
        let executed = scheduler.tick_once().await;
        assert_eq!(
            executed.records.first().expect("기록").outcome,
            JobRunOutcome::Executed(JobRunDetail::new("ok", 1))
        );
        assert_eq!(runs.load(Ordering::SeqCst), 1);
        assert_eq!(ai_queue.releases.load(Ordering::SeqCst), 1);
    });
}

#[test]
fn weekly_interval_overrides_and_run_loop_stop_cleanly() {
    runtime().block_on(async {
        let clock = Arc::new(SteppedClock::new(kst(2026, 11, 10, 10, 0)));
        let (job, runs, _) = CountingJob::new(
            ScheduledJobId::WeeklyReview,
            JobConfig {
                kind: JobKind::CodeOnly,
                schedule: ScheduleRule::WeeklyKst {
                    weekday: KstWeekday::Saturday,
                    time: KstTimeOfDay::try_new(9, 0).expect("09:00"),
                },
                missed_policy: MissedPolicy::CatchUpOnce,
                enabled: JobEnablement::Enabled,
            },
        );
        let mut registry = JobRegistry::new();
        registry.register(job).expect("등록");
        let scheduler = Arc::new(
            Scheduler::new(Arc::clone(&clock), Arc::new(DisabledAiJobQueue), registry)
                .expect("스케줄러"),
        );

        let initial = scheduler.status_snapshots().await;
        assert_eq!(
            initial.first().expect("상태").next_due,
            Some(kst(2026, 11, 14, 9, 0))
        );

        let updated = scheduler
            .apply_override(JobScheduleOverride {
                id: ScheduledJobId::WeeklyReview,
                enabled: JobEnablement::Enabled,
                schedule: ScheduleRule::Interval(span_secs(60)),
            })
            .await
            .expect("오버라이드")
            .expect("존재함");
        assert_eq!(updated.next_due, Some(kst(2026, 11, 10, 10, 1)));

        let mut bus_builder = EventBusBuilder::default();
        bus_builder.subscribe(
            BTreeSet::from([DomainEventKind::SettingsChanged]),
            scheduler.event_subscriber(),
        );
        let bus = bus_builder.build();
        let delivery = bus.publish(DomainEvent::SettingsChanged);
        assert!(delivery.failures.is_empty(), "이벤트 구독 성공");
        scheduler
            .event_subscriber()
            .on_event(&DomainEvent::AiBudgetChanged)
            .expect("예산 변경 이벤트");

        clock.advance_to(kst(2026, 11, 10, 10, 2)).expect("2분 후");
        let (tx, rx) = watch::channel(LoopControl::Run);
        let runner = Arc::clone(&scheduler);
        let handle = tokio::spawn(async move {
            runner.run_loop(Duration::from_millis(5), rx).await;
        });
        tokio::time::sleep(Duration::from_millis(25)).await;
        tx.send(LoopControl::Stop).expect("종료 신호");
        handle.await.expect("루프 종료");
        assert_eq!(runs.load(Ordering::SeqCst), 1);
    });
}

#[test]
fn failing_job_records_failure_and_advances_to_next_slot() {
    struct FailingJob;
    impl ScheduledJob for FailingJob {
        fn id(&self) -> ScheduledJobId {
            ScheduledJobId::TossSpecWatch
        }
        fn default_config(&self) -> JobConfig {
            JobConfig {
                kind: JobKind::CodeOnly,
                schedule: ScheduleRule::Interval(span_secs(300)),
                missed_policy: MissedPolicy::CatchUpOnce,
                enabled: JobEnablement::Enabled,
            }
        }
        fn run(&self, _now: Timestamp) -> JobFuture<'_> {
            Box::pin(async { Err(JobError::PortUnavailable(String::from("network"))) })
        }
    }

    runtime().block_on(async {
        let clock = Arc::new(SteppedClock::new(kst(2026, 11, 10, 10, 0)));
        let mut registry = JobRegistry::new();
        registry.register(FailingJob).expect("등록");
        let scheduler = Scheduler::new(Arc::clone(&clock), Arc::new(DisabledAiJobQueue), registry)
            .expect("스케줄러");
        clock.advance_to(kst(2026, 11, 10, 10, 5)).expect("5분 후");
        let report = scheduler.tick_once().await;
        assert_eq!(
            report.records.first().expect("실패 기록").outcome,
            JobRunOutcome::Failed(JobError::PortUnavailable(String::from("network")))
        );
        let snapshot = scheduler.status_snapshots().await;
        assert!(snapshot.first().expect("상태").last_succeeded_at.is_none());
        assert_eq!(
            snapshot.first().expect("상태").next_due,
            Some(kst(2026, 11, 10, 10, 10))
        );
    });
}
