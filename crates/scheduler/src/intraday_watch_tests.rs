use crate::fixture_tests::{kst, runtime};
use crate::{
    AiJobQueue, AiQueueAdmission, AiQueueError, IntradayReviewPort, IntradayWatchInbox,
    IntradayWatchJob, IntradayWatchSubscriber, JobError, JobRegistry, JobRunOutcome, ScheduleRule,
    ScheduledJob, ScheduledJobId, Scheduler, WatchCause,
};
use application::EventSubscriber;
use domain::{
    DartReceiptNo, Disclosure, DisclosureKey, DomainEvent, InstrumentId, NormalizedUrl, ReceiptNo,
    Timestamp, UntrustedText,
};
use std::sync::Arc;
use testkit_ports::SteppedClock;
use tokio::sync::RwLock;

#[derive(Default)]
struct RecordingReview {
    calls: RwLock<Vec<WatchCause>>,
    failing: RwLock<Vec<WatchCause>>,
}

impl IntradayReviewPort for RecordingReview {
    async fn review(&self, cause: WatchCause, _now: Timestamp) -> Result<(), JobError> {
        self.calls.write().await.push(cause.clone());
        if self.failing.read().await.contains(&cause) {
            return Err(JobError::ExecutionFailed(String::from("ai")));
        }
        Ok(())
    }
}

struct AdmitAll;

impl AiJobQueue for AdmitAll {
    fn request_background_slot(
        &self,
        _job: ScheduledJobId,
        _now: Timestamp,
    ) -> impl Future<Output = Result<AiQueueAdmission, AiQueueError>> + Send {
        std::future::ready(Ok(AiQueueAdmission::Admitted))
    }

    fn release_background_slot(
        &self,
        _job: ScheduledJobId,
        _now: Timestamp,
    ) -> impl Future<Output = Result<(), AiQueueError>> + Send {
        std::future::ready(Ok(()))
    }
}

fn samsung() -> InstrumentId {
    InstrumentId::try_new("005930").expect("종목")
}

fn disclosure_key() -> DisclosureKey {
    Disclosure::new(
        ReceiptNo::Dart(DartReceiptNo::try_new("20261125000001").expect("접수")),
        NormalizedUrl::try_new("https://example.com/d").expect("주소"),
        UntrustedText::try_new("유상증자 결정").expect("제목"),
        None,
    )
    .key()
    .clone()
}

struct Fixture {
    scheduler: Scheduler<SteppedClock, AdmitAll>,
    clock: Arc<SteppedClock>,
    subscriber: IntradayWatchSubscriber,
    review: Arc<RecordingReview>,
}

fn fixture() -> Fixture {
    let review = Arc::new(RecordingReview::default());
    let inbox = Arc::new(IntradayWatchInbox::new());
    let mut registry = JobRegistry::new();
    registry
        .register(IntradayWatchJob::new(
            Arc::clone(&review),
            Arc::clone(&inbox),
        ))
        .expect("등록");
    let clock = Arc::new(SteppedClock::new(kst(2026, 11, 25, 10, 0)));
    let scheduler =
        Scheduler::new(Arc::clone(&clock), Arc::new(AdmitAll), registry).expect("스케줄러");
    let subscriber = IntradayWatchSubscriber::new(inbox, scheduler.triggers());
    Fixture {
        scheduler,
        clock,
        subscriber,
        review,
    }
}

#[test]
fn intraday_watch_is_event_triggered_ai_job() {
    let job = IntradayWatchJob::new(
        Arc::new(RecordingReview::default()),
        Arc::new(IntradayWatchInbox::new()),
    );
    assert_eq!(job.id(), ScheduledJobId::IntradayWatch);
    assert_eq!(job.default_config().schedule, ScheduleRule::EventTriggered);
}

#[test]
fn triggers_run_review_once_per_event_and_ignore_other_events() {
    runtime().block_on(async {
        let fx = fixture();
        assert_eq!(fx.scheduler.tick_once().await.records.len(), 0);
        for event in [
            DomainEvent::PriceMoved(samsung()),
            DomainEvent::PriceMoved(samsung()),
            DomainEvent::DisclosureTriggerRecorded(disclosure_key()),
            DomainEvent::NewsActivityIncreased(samsung()),
            DomainEvent::SettingsChanged,
        ] {
            fx.subscriber.on_event(&event).expect("구독");
        }
        let report = fx.scheduler.tick_once().await;
        assert_eq!(report.records.len(), 1);
        match &report.records[0].outcome {
            JobRunOutcome::Executed(detail) => assert_eq!(detail.processed_items, 3),
            JobRunOutcome::SkippedMissedWindow
            | JobRunOutcome::DeferredByAiQueue { .. }
            | JobRunOutcome::Failed(_) => panic!("실행되지 않음"),
        }
        assert_eq!(
            *fx.review.calls.read().await,
            vec![
                WatchCause::PriceMove(samsung()),
                WatchCause::UrgentDisclosure(disclosure_key()),
                WatchCause::NewsSurge(samsung()),
            ]
        );
        fx.subscriber
            .on_event(&DomainEvent::PriceMoved(samsung()))
            .expect("같은 날 재발생");
        fx.subscriber
            .on_event(&DomainEvent::DisclosureTriggerRecorded(disclosure_key()))
            .expect("같은 공시 재발생");
        fx.clock.advance_to(kst(2026, 11, 25, 10, 5)).expect("시각");
        fx.scheduler.tick_once().await;
        assert_eq!(fx.review.calls.read().await.len(), 3);
        fx.subscriber
            .on_event(&DomainEvent::PriceMoved(samsung()))
            .expect("다음 날");
        fx.subscriber
            .on_event(&DomainEvent::DisclosureTriggerRecorded(disclosure_key()))
            .expect("다음 날 같은 공시");
        fx.clock
            .advance_to(kst(2026, 11, 26, 10, 0))
            .expect("다음 날");
        fx.scheduler.tick_once().await;
        assert_eq!(fx.review.calls.read().await.len(), 4);
    });
}

#[test]
fn failed_review_is_retried_on_the_next_trigger() {
    runtime().block_on(async {
        let fx = fixture();
        fx.review
            .failing
            .write()
            .await
            .push(WatchCause::NewsSurge(samsung()));
        fx.subscriber
            .on_event(&DomainEvent::NewsActivityIncreased(samsung()))
            .expect("구독");
        let report = fx.scheduler.tick_once().await;
        assert_eq!(
            report.records[0].outcome,
            JobRunOutcome::Failed(JobError::ExecutionFailed(String::from("ai")))
        );
        fx.review.failing.write().await.clear();
        fx.subscriber
            .on_event(&DomainEvent::NewsActivityIncreased(samsung()))
            .expect("재발생");
        fx.clock.advance_to(kst(2026, 11, 25, 10, 1)).expect("시각");
        fx.scheduler.tick_once().await;
        assert_eq!(fx.review.calls.read().await.len(), 2);
    });
}
