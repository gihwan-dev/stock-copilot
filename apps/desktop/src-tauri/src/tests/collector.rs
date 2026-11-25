use crate::platform::background::CollectorWorker;
use crate::platform::collector::{
    CollectionPlanner, FeedCollectorWorker, FeedRoundFuture, FeedRunner, OffHoursPlanner,
    PlanFuture, ScheduledFeed, SharedFeed,
};
use application::{
    AiReadPolicy, CollectedCount, CollectionConditions, CollectionPhase, CollectionPlan,
    CollectionTarget, ConditionalGet, DartCallCount, DartDailyUsage, DartKeyState,
    DataAvailability, FeedCursor, FeedPosition, FeedRound, FeedRoundOutcome, FeedSource, FomcDay,
    NewsFeed, NewsMetadata, Retention, SystemFailure, SystemPort, SystemPortError,
    UserAgentRequirement,
};
use domain::{
    DataSource, Nanosecond, NewsSource, OffsetSeconds, SpanSeconds, TimeSpan, Timestamp,
    UnixSeconds, UntrustedText,
};
use scheduler::LoopControl;
use std::sync::{Arc, RwLock};
use std::time::Duration;
use testkit_ports::SteppedClock;
use testkit_ports::scripted::ScriptedNewsFeed;
use tokio::sync::watch;

fn at(seconds: i64) -> Timestamp {
    Timestamp::from_parts(
        UnixSeconds::new(seconds),
        Nanosecond::new(0),
        OffsetSeconds::new(0),
    )
    .expect("시각")
}

fn minutes(count: u64) -> TimeSpan {
    TimeSpan::try_new(
        SpanSeconds::new(count.saturating_mul(60)),
        Nanosecond::new(0),
    )
    .expect("간격")
}

fn position(value: &str) -> FeedCursor {
    FeedCursor::Continue(FeedPosition::new(
        UntrustedText::try_new(value).expect("텍스트"),
    ))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Reply {
    Advance,
    Fail,
}

struct FakeRunner {
    replies: RwLock<std::collections::VecDeque<Reply>>,
    seen: RwLock<Vec<FeedCursor>>,
}

impl FakeRunner {
    fn new(replies: Vec<Reply>) -> Arc<Self> {
        Arc::new(Self {
            replies: RwLock::new(replies.into()),
            seen: RwLock::new(Vec::new()),
        })
    }

    fn seen(&self) -> Vec<FeedCursor> {
        self.seen.write().expect("잠금").clone()
    }
}

impl FeedRunner for FakeRunner {
    fn collect(&self, cursor: FeedCursor) -> FeedRoundFuture<'_> {
        let count = {
            let mut seen = self.seen.write().expect("잠금");
            seen.push(cursor);
            seen.len()
        };
        let reply = {
            let mut replies = self.replies.write().expect("잠금");
            replies.pop_front().unwrap_or(Reply::Advance)
        };
        Box::pin(std::future::ready(match reply {
            Reply::Advance => Ok(FeedRound {
                source: DataSource::Yonhap,
                outcome: FeedRoundOutcome::Collected,
                cursor: position(&format!("c{count}")),
                news: CollectedCount::default(),
                disclosures: CollectedCount::default(),
            }),
            Reply::Fail => Err(SystemPortError::new(
                SystemPort::Repository,
                SystemFailure::InvalidData,
            )),
        }))
    }
}

fn shared(runner: &Arc<FakeRunner>) -> Arc<dyn FeedRunner> {
    Arc::<FakeRunner>::clone(runner)
}

fn block_on<F: Future>(future: F) -> F::Output {
    tauri::async_runtime::block_on(future)
}

fn worker(feeds: Vec<ScheduledFeed>, clock: &Arc<SteppedClock>) -> FeedCollectorWorker {
    worker_with(feeds, clock, Arc::new(OffHoursPlanner))
}

fn worker_with(
    feeds: Vec<ScheduledFeed>,
    clock: &Arc<SteppedClock>,
    planner: Arc<dyn CollectionPlanner>,
) -> FeedCollectorWorker {
    let shared: Arc<dyn application::Clock> = Arc::<SteppedClock>::clone(clock);
    FeedCollectorWorker::new(feeds, planner, shared, Duration::from_millis(10))
}

#[test]
fn each_feed_runs_at_its_off_hours_cadence_and_passes_the_returned_cursor_forward() {
    let clock = Arc::new(SteppedClock::new(at(1_000_000)));
    let press = FakeRunner::new(vec![]);
    let google = FakeRunner::new(vec![]);
    let collector = worker(
        vec![
            ScheduledFeed::new(CollectionTarget::PressRss, shared(&press)),
            ScheduledFeed::new(CollectionTarget::GoogleNews, shared(&google)),
        ],
        &clock,
    );
    assert_eq!(block_on(collector.collect_due()).attempted, 2);
    assert_eq!(block_on(collector.collect_due()).attempted, 0);
    clock.advance_by(minutes(30)).expect("30분");
    assert_eq!(block_on(collector.collect_due()).attempted, 1);
    clock.advance_by(minutes(30)).expect("60분");
    assert_eq!(block_on(collector.collect_due()).attempted, 2);
    assert_eq!(
        press.seen(),
        vec![FeedCursor::Start, position("c1"), position("c2")]
    );
    assert_eq!(google.seen(), vec![FeedCursor::Start, position("c1")]);
}

#[test]
fn a_failed_round_keeps_the_previous_cursor_and_waits_for_the_next_cadence() {
    let clock = Arc::new(SteppedClock::new(at(1_000_000)));
    let press = FakeRunner::new(vec![Reply::Advance, Reply::Fail, Reply::Advance]);
    let collector = worker(
        vec![ScheduledFeed::new(
            CollectionTarget::PressRss,
            shared(&press),
        )],
        &clock,
    );
    assert_eq!(block_on(collector.collect_due()).failed, 0);
    clock.advance_by(minutes(30)).expect("30분");
    let failed = block_on(collector.collect_due());
    assert_eq!((failed.attempted, failed.failed), (1, 1));
    assert_eq!(block_on(collector.collect_due()).attempted, 0);
    clock.advance_by(minutes(30)).expect("60분");
    let _collected = block_on(collector.collect_due());
    assert_eq!(
        press.seen(),
        vec![FeedCursor::Start, position("c1"), position("c1")]
    );
}

#[test]
fn startup_catch_up_runner_is_used_only_for_the_first_round() {
    let clock = Arc::new(SteppedClock::new(at(1_000_000)));
    let catch_up = FakeRunner::new(vec![]);
    let regular = FakeRunner::new(vec![]);
    let collector = worker(
        vec![
            ScheduledFeed::new(CollectionTarget::KindDisclosure, shared(&regular))
                .with_startup(shared(&catch_up)),
        ],
        &clock,
    );
    let _first = block_on(collector.collect_due());
    clock.advance_by(minutes(30)).expect("30분");
    let _second = block_on(collector.collect_due());
    assert_eq!(catch_up.seen(), vec![FeedCursor::Start]);
    assert_eq!(regular.seen(), vec![position("c1")]);
}

struct DomesticPlanner;

impl CollectionPlanner for DomesticPlanner {
    fn plan(&self) -> PlanFuture<'_> {
        Box::pin(std::future::ready(CollectionPlan::build(
            &CollectionConditions {
                phase: CollectionPhase::DomesticSession,
                fomc: FomcDay::Regular,
                dart_key: DartKeyState::Missing,
                dart_usage: DartDailyUsage::new(DartCallCount::new(0), DartCallCount::new(1)),
            },
        )))
    }
}

#[test]
fn domestic_session_plan_shortens_kind_disclosure_polling_and_turns_dart_off_without_key() {
    let clock = Arc::new(SteppedClock::new(at(1_000_000)));
    let kind = FakeRunner::new(vec![]);
    let dart = FakeRunner::new(vec![]);
    let collector = worker_with(
        vec![
            ScheduledFeed::new(CollectionTarget::KindDisclosure, shared(&kind)),
            ScheduledFeed::new(CollectionTarget::DartList, shared(&dart)),
        ],
        &clock,
        Arc::new(DomesticPlanner),
    );
    let _first = block_on(collector.collect_due());
    clock.advance_by(minutes(2)).expect("2분");
    let _second = block_on(collector.collect_due());
    assert_eq!(kind.seen().len(), 2);
    assert_eq!(dart.seen(), Vec::<FeedCursor>::new());
}

#[test]
fn run_loop_collects_then_stops_when_shutdown_is_signalled() {
    let clock = Arc::new(SteppedClock::new(at(1_000_000)));
    let press = FakeRunner::new(vec![]);
    let collector = worker(
        vec![ScheduledFeed::new(
            CollectionTarget::PressRss,
            shared(&press),
        )],
        &clock,
    );
    let (control, receiver) = watch::channel(LoopControl::Run);
    let task = tauri::async_runtime::spawn(collector.run(receiver));
    block_on(async {
        for _attempt in 0_u32..200_u32 {
            if !press.seen().is_empty() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        control.send(LoopControl::Stop).expect("중지");
        tokio::time::timeout(Duration::from_secs(2), task)
            .await
            .expect("제때 멈춤")
            .expect("작업");
    });
    assert_eq!(press.seen(), vec![FeedCursor::Start]);
}

#[test]
fn shared_feed_forwards_source_metadata_and_items_of_the_wrapped_feed() {
    let metadata = NewsMetadata::new(
        Retention::NotStored,
        AiReadPolicy::TitleOnly,
        ConditionalGet::Unsupported,
        UserAgentRequirement::Standard,
    );
    let scripted = ScriptedNewsFeed::new(FeedSource::News(NewsSource::Edaily), metadata.clone());
    scripted
        .script_new_items()
        .push(Ok(DataAvailability::NoData));
    let shared = SharedFeed::new(scripted);
    assert_eq!(shared.source(), FeedSource::News(NewsSource::Edaily));
    assert_eq!(shared.metadata(), metadata);
    assert_eq!(
        block_on(shared.new_items(position("x"))),
        Ok(DataAvailability::NoData)
    );
}
