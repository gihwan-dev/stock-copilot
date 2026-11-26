use super::{
    AfterClose, CollectionPlanner, FeedCollectorWorker, FeedRoundFuture, FeedRunner,
    OffHoursPlanner, PlanFuture, ReadyFuture, RunnerReadiness, ScheduledFeed, SwitchFuture,
    after_close_boost, after_close_window, off_hours_plan,
};
use application::{CollectedCount, CollectionTarget, FeedCursor, FeedRound, FeedRoundOutcome};
use domain::{
    DataSource, Nanosecond, OffsetSeconds, SpanSeconds, TimeSpan, Timestamp, UnixSeconds,
};
use std::collections::BTreeSet;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;
use testkit_ports::SteppedClock;
use tokio::sync::RwLock;

fn at(seconds: i64) -> Timestamp {
    Timestamp::from_parts(
        UnixSeconds::new(seconds),
        Nanosecond::new(0),
        OffsetSeconds::new(0),
    )
    .expect("시각")
}

fn seconds(count: u64) -> TimeSpan {
    TimeSpan::try_new(SpanSeconds::new(count), Nanosecond::new(0)).expect("간격")
}

struct CountingRunner {
    calls: AtomicU32,
    ready: RwLock<RunnerReadiness>,
}

impl CountingRunner {
    fn new(ready: RunnerReadiness) -> Arc<Self> {
        Arc::new(Self {
            calls: AtomicU32::new(0),
            ready: RwLock::new(ready),
        })
    }

    fn calls(&self) -> u32 {
        self.calls.load(Ordering::SeqCst)
    }
}

impl FeedRunner for CountingRunner {
    fn collect(&self, cursor: FeedCursor) -> FeedRoundFuture<'_> {
        let _previous = self.calls.fetch_add(1, Ordering::SeqCst);
        Box::pin(std::future::ready(Ok(FeedRound {
            source: DataSource::Yonhap,
            outcome: FeedRoundOutcome::NoData,
            cursor,
            news: CollectedCount::default(),
            disclosures: CollectedCount::default(),
        })))
    }

    fn readiness(&self) -> ReadyFuture<'_> {
        Box::pin(async move { *self.ready.read().await })
    }
}

struct SwitchedOff(BTreeSet<DataSource>);

impl CollectionPlanner for SwitchedOff {
    fn plan(&self) -> PlanFuture<'_> {
        Box::pin(std::future::ready(off_hours_plan()))
    }

    fn disabled(&self) -> SwitchFuture<'_> {
        Box::pin(std::future::ready(self.0.clone()))
    }
}

fn worker(
    feeds: Vec<ScheduledFeed>,
    clock: &Arc<SteppedClock>,
    planner: Arc<dyn CollectionPlanner>,
) -> FeedCollectorWorker {
    let shared: Arc<dyn application::Clock> = Arc::<SteppedClock>::clone(clock);
    FeedCollectorWorker::new(feeds, planner, shared, Duration::from_millis(10))
}

fn runner(counting: &Arc<CountingRunner>) -> Arc<dyn FeedRunner> {
    Arc::<CountingRunner>::clone(counting)
}

#[test]
fn a_source_switched_off_in_settings_is_never_fetched() {
    let clock = Arc::new(SteppedClock::new(at(1_000_000)));
    let yonhap = CountingRunner::new(RunnerReadiness::Ready);
    let google = CountingRunner::new(RunnerReadiness::Ready);
    let collector = worker(
        vec![
            ScheduledFeed::new(CollectionTarget::PressRss, runner(&yonhap))
                .with_source(DataSource::Yonhap),
            ScheduledFeed::new(CollectionTarget::GoogleNews, runner(&google))
                .with_source(DataSource::GoogleNews),
        ],
        &clock,
        Arc::new(SwitchedOff(BTreeSet::from([DataSource::GoogleNews]))),
    );
    let report = tauri::async_runtime::block_on(collector.collect_due());
    assert_eq!(report.attempted, 1);
    assert_eq!((yonhap.calls(), google.calls()), (1, 0));
}

#[test]
fn a_round_before_the_database_opens_is_retried_on_the_next_tick_not_a_cadence_later() {
    let clock = Arc::new(SteppedClock::new(at(1_000_000)));
    let press = CountingRunner::new(RunnerReadiness::Waiting);
    let collector = worker(
        vec![ScheduledFeed::new(
            CollectionTarget::PressRss,
            runner(&press),
        )],
        &clock,
        Arc::new(OffHoursPlanner),
    );
    assert_eq!(
        tauri::async_runtime::block_on(collector.collect_due()).attempted,
        0
    );
    tauri::async_runtime::block_on(async {
        *press.ready.write().await = RunnerReadiness::Ready;
    });
    clock.advance_by(seconds(30)).expect("30초");
    assert_eq!(
        tauri::async_runtime::block_on(collector.collect_due()).attempted,
        1
    );
    assert_eq!(press.calls(), 1);
}

struct FailingRunner {
    calls: AtomicU32,
}

impl FeedRunner for FailingRunner {
    fn collect(&self, cursor: FeedCursor) -> FeedRoundFuture<'_> {
        let _previous = self.calls.fetch_add(1, Ordering::SeqCst);
        Box::pin(std::future::ready(Ok(FeedRound {
            source: DataSource::Edaily,
            outcome: FeedRoundOutcome::SourceFailed,
            cursor,
            news: CollectedCount::default(),
            disclosures: CollectedCount::default(),
        })))
    }
}

#[test]
fn a_source_failing_three_times_in_a_row_backs_off_to_six_hours() {
    let clock = Arc::new(SteppedClock::new(at(1_000_000)));
    let edaily = Arc::new(FailingRunner {
        calls: AtomicU32::new(0),
    });
    let shared_runner: Arc<dyn FeedRunner> = Arc::<FailingRunner>::clone(&edaily);
    let collector = worker(
        vec![ScheduledFeed::new(
            CollectionTarget::PressRss,
            shared_runner,
        )],
        &clock,
        Arc::new(OffHoursPlanner),
    );
    for _round in 0_u8..3 {
        tauri::async_runtime::block_on(collector.collect_due());
        clock.advance_by(seconds(30 * 60)).expect("30분");
    }
    assert_eq!(edaily.calls.load(Ordering::SeqCst), 3);
    for _wait in 0_u8..11 {
        tauri::async_runtime::block_on(collector.collect_due());
        clock.advance_by(seconds(30 * 60)).expect("30분");
    }
    assert_eq!(edaily.calls.load(Ordering::SeqCst), 3, "6시간 동안 쉰다");
    tauri::async_runtime::block_on(collector.collect_due());
    assert_eq!(edaily.calls.load(Ordering::SeqCst), 4);
}

fn kst(seconds_from_midnight: i64, day_offset: i64) -> Timestamp {
    at(1_795_618_800_i64
        .saturating_add(day_offset.saturating_mul(86_400))
        .saturating_add(seconds_from_midnight))
}

#[test]
fn after_close_window_is_weekday_kst_15_30_to_19_00() {
    assert_eq!(
        after_close_window(kst(15 * 3_600 + 30 * 60, 0)),
        AfterClose::Inside
    );
    assert_eq!(
        after_close_window(kst(18 * 3_600 + 59 * 60, 0)),
        AfterClose::Inside
    );
    assert_eq!(after_close_window(kst(19 * 3_600, 0)), AfterClose::Outside);
    assert_eq!(after_close_window(kst(15 * 3_600, 0)), AfterClose::Outside);
    assert_eq!(
        after_close_window(kst(16 * 3_600, 2)),
        AfterClose::Outside,
        "토요일"
    );
}

#[test]
fn kind_polls_every_ten_minutes_only_after_close_in_off_hours() {
    let off = off_hours_plan();
    let boosted = after_close_boost(&off, kst(16 * 3_600, 0));
    assert_eq!(
        boosted.cadence(CollectionTarget::KindDisclosure),
        application::CollectionCadence::Every(
            application::CollectionMinutes::try_new(10).expect("주기")
        )
    );
    assert_eq!(
        boosted.cadence(CollectionTarget::PressRss),
        off.cadence(CollectionTarget::PressRss)
    );
    assert_eq!(after_close_boost(&off, kst(20 * 3_600, 0)), off);
}
