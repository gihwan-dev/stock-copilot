use crate::fixture_tests::{kst, runtime, span_secs};
use crate::{
    BriefingJob, BriefingPort, BriefingRunResult, DisabledAiJobQueue, JobError, JobKind,
    JobRegistry, JobRunOutcome, KstTimeOfDay, MissedPolicy, ScheduleRule, ScheduledJob,
    ScheduledJobId, Scheduler,
};
use domain::{BriefingKind, KnownMarket, Timestamp};
use std::sync::Arc;
use testkit_ports::SteppedClock;
use tokio::sync::RwLock;

struct ScriptedBriefing {
    result: BriefingRunResult,
    calls: RwLock<Vec<BriefingKind>>,
}

impl ScriptedBriefing {
    fn new(result: BriefingRunResult) -> Arc<Self> {
        Arc::new(Self {
            result,
            calls: RwLock::new(Vec::new()),
        })
    }
}

impl BriefingPort for ScriptedBriefing {
    async fn run(
        &self,
        kind: BriefingKind,
        _now: Timestamp,
    ) -> Result<BriefingRunResult, JobError> {
        self.calls.write().await.push(kind);
        Ok(self.result)
    }
}

fn time(hour: u8, minute: u8) -> KstTimeOfDay {
    KstTimeOfDay::try_new(hour, minute).expect("시각")
}

#[test]
fn briefing_jobs_use_design_schedule_and_ai_kind() {
    let port = ScriptedBriefing::new(BriefingRunResult::Completed);
    let morning = BriefingJob::morning(Arc::clone(&port));
    let closing = BriefingJob::closing_review(Arc::clone(&port));
    let pre_us = BriefingJob::pre_us_market(port);
    assert_eq!(morning.id(), ScheduledJobId::MorningBriefing);
    assert_eq!(closing.id(), ScheduledJobId::ClosingReview);
    assert_eq!(pre_us.id(), ScheduledJobId::PreUsMarketCheck);
    assert_eq!(
        morning.default_config().schedule,
        ScheduleRule::DailyKst(time(8, 20))
    );
    assert_eq!(
        closing.default_config().schedule,
        ScheduleRule::DailyKst(time(15, 50))
    );
    assert_eq!(
        pre_us.default_config().schedule,
        ScheduleRule::BeforeMarketOpen {
            market: KnownMarket::Us,
            lead: span_secs(2_400),
            fallback: time(22, 50),
        }
    );
    for job in [&morning, &closing, &pre_us] {
        assert_eq!(job.default_config().kind, JobKind::AiBackground);
        assert_eq!(job.default_config().missed_policy, MissedPolicy::Skip);
    }
}

#[test]
fn briefing_job_reports_completion_and_failure() {
    runtime().block_on(async {
        let done = ScriptedBriefing::new(BriefingRunResult::Completed);
        let detail = BriefingJob::morning(Arc::clone(&done))
            .run(kst(2026, 11, 25, 8, 20))
            .await
            .expect("완료");
        assert_eq!(detail.summary_code, "briefing_finished");
        assert_eq!(*done.calls.read().await, vec![BriefingKind::Morning]);
        let failed = ScriptedBriefing::new(BriefingRunResult::Failed);
        let outcome = BriefingJob::closing_review(failed)
            .run(kst(2026, 11, 25, 15, 50))
            .await;
        assert_eq!(
            outcome,
            Err(JobError::ExecutionFailed(String::from("briefing_failed")))
        );
    });
}

#[test]
fn briefing_waits_for_ai_queue_admission() {
    runtime().block_on(async {
        let port = ScriptedBriefing::new(BriefingRunResult::Completed);
        let mut registry = JobRegistry::new();
        registry
            .register(BriefingJob::morning(Arc::clone(&port)))
            .expect("등록");
        let clock = Arc::new(SteppedClock::new(kst(2026, 11, 25, 8, 0)));
        let scheduler = Scheduler::new(Arc::clone(&clock), Arc::new(DisabledAiJobQueue), registry)
            .expect("스케줄러");
        clock
            .advance_to(kst(2026, 11, 25, 8, 20))
            .expect("시각 이동");
        let report = scheduler.tick_once().await;
        assert_eq!(report.records.len(), 1);
        assert_eq!(
            report.records[0].outcome,
            JobRunOutcome::DeferredByAiQueue { resumes_at: None }
        );
        assert!(port.calls.read().await.is_empty());
    });
}
