use crate::collected_links::{CollectedLinker, LinkingTimes};
use crate::holdings_link::InstrumentDirectory;
use crate::news_tracking::{MatchName, NewsTracking, match_names};
use crate::platform::background::{CollectorFuture, CollectorWorker};
use crate::platform::feed_watch::{SharedHeadlines, WatchedFeed, record_feed};
use crate::ports::{PortTypes, Ports};
use application::{
    Clock, CollectFeed, CollectedObservation, CollectionConditions, CollectionDue,
    CollectionInputs, CollectionPhase, CollectionPlan, CollectionScheduler, CollectionTarget,
    DartCallCount, DartDailyUsage, DartKeyState, DataAvailability, FeedCursor, FeedRound,
    FeedSource, FomcDay, InformationError, NewsDataSettings, NewsDataSettingsRepository, NewsFeed,
    NewsMetadata, PlanCollectionSchedule, SourceSwitch, SystemPortError, WriteCompleted,
    collection_due,
};
use application::{SecretStore, SourceState, SourceStatusRepository};
use domain::{DataSource, Timestamp};
use scheduler::LoopControl;
use sources::{NewsLinkHints, TrackedTargets};
use std::collections::BTreeSet;
use std::fmt::{Debug, Formatter};
use std::marker::PhantomData;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;
use store_sqlite::SqliteRepositories;
use tauri::{AppHandle, Manager, Runtime};
use tokio::sync::{Mutex, watch};

#[cfg(test)]
mod tests;

pub const COLLECTOR_TICK_SECONDS: u64 = 30;

pub type FeedRoundFuture<'a> =
    Pin<Box<dyn Future<Output = Result<FeedRound, SystemPortError>> + Send + 'a>>;
pub type PlanFuture<'a> = Pin<Box<dyn Future<Output = CollectionPlan> + Send + 'a>>;
pub type ReadyFuture<'a> = Pin<Box<dyn Future<Output = RunnerReadiness> + Send + 'a>>;
pub type SwitchFuture<'a> = Pin<Box<dyn Future<Output = BTreeSet<DataSource>> + Send + 'a>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunnerReadiness {
    Ready,
    Waiting,
}

pub type FeedItemsFuture<'a> = Pin<
    Box<
        dyn Future<
                Output = Result<
                    DataAvailability<(Vec<CollectedObservation>, FeedCursor)>,
                    InformationError,
                >,
            > + Send
            + 'a,
    >,
>;

pub trait ErasedNewsFeed: Send + Sync {
    fn erased_source(&self) -> FeedSource;
    fn erased_metadata(&self) -> NewsMetadata;
    fn erased_new_items(&self, since: FeedCursor) -> FeedItemsFuture<'_>;
}

impl<F: NewsFeed> ErasedNewsFeed for F {
    fn erased_source(&self) -> FeedSource {
        self.source()
    }
    fn erased_metadata(&self) -> NewsMetadata {
        self.metadata()
    }
    fn erased_new_items(&self, since: FeedCursor) -> FeedItemsFuture<'_> {
        Box::pin(self.new_items(since))
    }
}

#[derive(Clone)]
pub struct SharedFeed(Arc<dyn ErasedNewsFeed>);

impl Debug for SharedFeed {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("SharedFeed")
            .field(&self.0.erased_source())
            .finish()
    }
}

impl SharedFeed {
    #[must_use]
    pub fn new<F: NewsFeed + 'static>(feed: F) -> Self {
        Self(Arc::new(feed))
    }
}

impl NewsFeed for SharedFeed {
    fn source(&self) -> FeedSource {
        self.0.erased_source()
    }
    fn metadata(&self) -> NewsMetadata {
        self.0.erased_metadata()
    }
    fn new_items(
        &self,
        since: FeedCursor,
    ) -> impl Future<
        Output = Result<
            DataAvailability<(Vec<CollectedObservation>, FeedCursor)>,
            InformationError,
        >,
    > + Send {
        self.0.erased_new_items(since)
    }
}

#[derive(Debug, Clone)]
pub struct CollectorFeed {
    pub target: CollectionTarget,
    pub startup: Option<SharedFeed>,
    pub regular: SharedFeed,
}

#[derive(Debug, Clone, Default)]
pub struct CollectorFeeds(pub Vec<CollectorFeed>);

pub trait FeedRunner: Send + Sync {
    fn collect(&self, cursor: FeedCursor) -> FeedRoundFuture<'_>;

    fn readiness(&self) -> ReadyFuture<'_> {
        Box::pin(std::future::ready(RunnerReadiness::Ready))
    }
}

pub trait CollectionPlanner: Send + Sync {
    fn plan(&self) -> PlanFuture<'_>;

    fn disabled(&self) -> SwitchFuture<'_> {
        Box::pin(std::future::ready(BTreeSet::new()))
    }
}

#[must_use]
pub fn off_hours_plan() -> CollectionPlan {
    CollectionPlan::build(&CollectionConditions {
        phase: CollectionPhase::OffHours,
        fomc: FomcDay::Regular,
        dart_key: DartKeyState::Missing,
        dart_usage: DartDailyUsage::new(DartCallCount::new(0), DartCallCount::new(1)),
    })
}

#[derive(Debug, Default, Clone, Copy)]
pub struct OffHoursPlanner;

impl CollectionPlanner for OffHoursPlanner {
    fn plan(&self) -> PlanFuture<'_> {
        Box::pin(std::future::ready(off_hours_plan()))
    }
}

pub struct ScheduledFeed {
    target: CollectionTarget,
    source: Option<DataSource>,
    startup: Option<Arc<dyn FeedRunner>>,
    regular: Arc<dyn FeedRunner>,
}

impl Debug for ScheduledFeed {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ScheduledFeed")
            .field("target", &self.target)
            .finish_non_exhaustive()
    }
}

impl ScheduledFeed {
    #[must_use]
    pub fn new(target: CollectionTarget, regular: Arc<dyn FeedRunner>) -> Self {
        Self {
            target,
            source: None,
            startup: None,
            regular,
        }
    }

    #[must_use]
    pub const fn with_source(mut self, source: DataSource) -> Self {
        self.source = Some(source);
        self
    }

    #[must_use]
    pub fn with_startup(mut self, startup: Arc<dyn FeedRunner>) -> Self {
        self.startup = Some(startup);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FeedState {
    cursor: FeedCursor,
    last_attempt: Option<Timestamp>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CollectorRoundReport {
    pub attempted: u32,
    pub failed: u32,
}

struct CollectorInner {
    feeds: Vec<ScheduledFeed>,
    planner: Arc<dyn CollectionPlanner>,
    clock: Arc<dyn Clock>,
    tick: Duration,
    states: Mutex<Vec<FeedState>>,
}

#[derive(Clone)]
pub struct FeedCollectorWorker {
    inner: Arc<CollectorInner>,
}

impl Debug for FeedCollectorWorker {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FeedCollectorWorker")
            .field("feeds", &self.inner.feeds)
            .finish_non_exhaustive()
    }
}

impl FeedCollectorWorker {
    #[must_use]
    pub fn new(
        feeds: Vec<ScheduledFeed>,
        planner: Arc<dyn CollectionPlanner>,
        clock: Arc<dyn Clock>,
        tick: Duration,
    ) -> Self {
        let states = feeds
            .iter()
            .map(|_feed| FeedState {
                cursor: FeedCursor::Start,
                last_attempt: None,
            })
            .collect();
        Self {
            inner: Arc::new(CollectorInner {
                feeds,
                planner,
                clock,
                tick,
                states: Mutex::new(states),
            }),
        }
    }

    pub async fn collect_due(&self) -> CollectorRoundReport {
        let plan = self.inner.planner.plan().await;
        let disabled = self.inner.planner.disabled().await;
        let mut states = self.inner.states.lock().await;
        let mut report = CollectorRoundReport::default();
        for (feed, state) in self.inner.feeds.iter().zip(states.iter_mut()) {
            if feed.source.is_some_and(|source| disabled.contains(&source)) {
                continue;
            }
            let now = self.inner.clock.now();
            match collection_due(plan.cadence(feed.target), state.last_attempt, now) {
                CollectionDue::Now => {}
                CollectionDue::At(_) | CollectionDue::Off => continue,
            }
            let runner = match (&feed.startup, state.last_attempt) {
                (Some(startup), None) => startup,
                (Some(_) | None, Some(_)) | (None, None) => &feed.regular,
            };
            if runner.readiness().await == RunnerReadiness::Waiting {
                continue;
            }
            report.attempted = report.attempted.saturating_add(1);
            state.last_attempt = Some(now);
            match runner.collect(state.cursor.clone()).await {
                Ok(round) => state.cursor = round.cursor,
                Err(_error) => report.failed = report.failed.saturating_add(1),
            }
        }
        report
    }
}

async fn wait_for_stop(shutdown: &mut watch::Receiver<LoopControl>) {
    while *shutdown.borrow() != LoopControl::Stop && shutdown.changed().await.is_ok() {}
}

async fn run_collector(worker: FeedCollectorWorker, mut shutdown: watch::Receiver<LoopControl>) {
    while *shutdown.borrow() != LoopControl::Stop {
        let _report = worker.collect_due().await;
        let tick = worker.inner.tick;
        if tokio::time::timeout(tick, wait_for_stop(&mut shutdown))
            .await
            .is_ok()
        {
            return;
        }
    }
}

impl CollectorWorker for FeedCollectorWorker {
    fn run(&self, shutdown: watch::Receiver<LoopControl>) -> CollectorFuture {
        Box::pin(run_collector(self.clone(), shutdown))
    }
}

pub struct AppFeedRunner<R: Runtime, P: PortTypes, F> {
    app: AppHandle<R>,
    repositories: SqliteRepositories,
    feed: F,
    headlines: SharedHeadlines,
    ports: PhantomData<fn() -> P>,
}

impl<R: Runtime, P: PortTypes, F> Debug for AppFeedRunner<R, P, F> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppFeedRunner").finish_non_exhaustive()
    }
}

impl<R: Runtime, P: PortTypes, F: NewsFeed> AppFeedRunner<R, P, F> {
    #[must_use]
    pub fn new(app: AppHandle<R>, repositories: SqliteRepositories, feed: F) -> Self {
        Self {
            app,
            repositories,
            feed,
            headlines: SharedHeadlines::default(),
            ports: PhantomData,
        }
    }

    #[must_use]
    pub fn sharing(mut self, headlines: &SharedHeadlines) -> Self {
        self.headlines = headlines.clone();
        self
    }

    async fn settings(&self) -> NewsDataSettings {
        match NewsDataSettingsRepository::load(&self.repositories).await {
            Ok(Some(settings)) => settings,
            Ok(None) | Err(_) => NewsDataSettings::initial(),
        }
    }

    async fn tracked_names(&self) -> Option<Vec<MatchName>> {
        let tracking = self.app.try_state::<NewsTracking>()?;
        let directory = self
            .app
            .try_state::<InstrumentDirectory>()
            .map_or_else(InstrumentDirectory::default, |names| names.inner().clone());
        let tracked = tracking.tracked().await;
        let settings = self.settings().await;
        Some(match_names(&tracked, settings.aliases(), &directory))
    }
}

impl<R: Runtime, P: PortTypes, F: NewsFeed + 'static> FeedRunner for AppFeedRunner<R, P, F> {
    fn collect(&self, cursor: FeedCursor) -> FeedRoundFuture<'_> {
        Box::pin(async move {
            let unopened = || {
                SystemPortError::new(
                    application::SystemPort::Repository,
                    application::SystemFailure::Unavailable(application::RetryDelay::Unknown),
                )
            };
            if self.repositories.shared().read().await.is_none() {
                return Err(unopened());
            }
            let ports = self.app.try_state::<Ports<P>>().ok_or_else(unopened)?;
            let names = self.tracked_names().await;
            let hints = self.app.try_state::<NewsLinkHints>();
            let linker = self
                .app
                .try_state::<CollectedLinker>()
                .map(|state| Arc::clone(&state.0));
            let plain = LinkingTimes::new(&self.repositories, linker.as_deref());
            let times = match names.as_deref() {
                Some(found) => plain.with_names(found, hints.as_ref().map(tauri::State::inner)),
                None => plain,
            };
            let watched = WatchedFeed::new(&self.feed, &self.headlines);
            let round = CollectFeed::new(
                &self.repositories,
                &self.repositories,
                ports.clock.as_ref(),
                &ports.events,
            )
            .with_times(&times)
            .collect(&watched, cursor)
            .await;
            record_feed(watched.diagnosis(&round, times.linked()).await);
            round
        })
    }

    fn readiness(&self) -> ReadyFuture<'_> {
        Box::pin(async move {
            let opened = self.repositories.shared().read().await.is_some();
            match (opened, self.app.try_state::<Ports<P>>()) {
                (true, Some(_)) => RunnerReadiness::Ready,
                (true, None) | (false, _) => RunnerReadiness::Waiting,
            }
        })
    }
}

#[derive(Debug, Default, Clone, Copy)]
struct PlanOnly;

impl CollectionScheduler for PlanOnly {
    fn apply(
        &self,
        _plan: &CollectionPlan,
    ) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send {
        std::future::ready(Ok(WriteCompleted))
    }
}

pub struct AppCollectionPlanner<R: Runtime, P: PortTypes> {
    app: AppHandle<R>,
    ports: PhantomData<fn() -> P>,
}

impl<R: Runtime, P: PortTypes> Debug for AppCollectionPlanner<R, P> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppCollectionPlanner")
            .finish_non_exhaustive()
    }
}

impl<R: Runtime, P: PortTypes> AppCollectionPlanner<R, P> {
    #[must_use]
    pub const fn new(app: AppHandle<R>) -> Self {
        Self {
            app,
            ports: PhantomData,
        }
    }

    async fn dart_working(&self) -> bool {
        let Some(repositories) = self.app.try_state::<Arc<SqliteRepositories>>() else {
            return false;
        };
        match SourceStatusRepository::find(repositories.as_ref(), DataSource::DartDisclosureApi)
            .await
        {
            Ok(Some(status)) => match status.state() {
                SourceState::Healthy | SourceState::NoData => true,
                SourceState::NeverRun | SourceState::NotConfigured | SourceState::Failed(_) => {
                    false
                }
            },
            Ok(None) | Err(_) => false,
        }
    }

    async fn dart_key(&self) -> DartKeyState {
        let Some(ports) = self.app.try_state::<Ports<P>>() else {
            return DartKeyState::Missing;
        };
        match ports.secrets.load(application::SecretKey::DartApiKey).await {
            Ok(Some(_present)) => DartKeyState::Configured,
            Ok(None) | Err(_) => DartKeyState::Missing,
        }
    }

    async fn settings(&self) -> Option<NewsDataSettings> {
        let repositories = self.app.try_state::<Arc<SqliteRepositories>>()?;
        NewsDataSettingsRepository::load(repositories.as_ref())
            .await
            .ok()
            .flatten()
    }
}

impl<R: Runtime, P: PortTypes> CollectionPlanner for AppCollectionPlanner<R, P> {
    fn plan(&self) -> PlanFuture<'_> {
        Box::pin(async move {
            let Some(ports) = self.app.try_state::<Ports<P>>() else {
                return off_hours_plan();
            };
            let planned = PlanCollectionSchedule {
                market_info: ports.market_info.as_ref(),
                scheduler: &PlanOnly,
                clock: ports.clock.as_ref(),
            }
            .run(CollectionInputs {
                fomc: FomcDay::Regular,
                dart_key: self.dart_key().await,
                dart_usage: DartDailyUsage::new(DartCallCount::new(0), DartCallCount::new(1)),
            })
            .await;
            let plan = match planned {
                Ok(plan) => plan,
                Err(_error) => off_hours_plan(),
            };
            match self.settings().await {
                Some(settings) => settings.adjusted_plan(&plan),
                None => plan,
            }
        })
    }

    fn disabled(&self) -> SwitchFuture<'_> {
        Box::pin(async move {
            let mut off: BTreeSet<DataSource> = match self.settings().await {
                Some(settings) => DataSource::all()
                    .into_iter()
                    .filter(|source| settings.source_switch(*source) == SourceSwitch::Off)
                    .collect(),
                None => BTreeSet::new(),
            };
            let dart_primary = self.dart_key().await == DartKeyState::Configured
                && !off.contains(&DataSource::DartDisclosureApi)
                && self.dart_working().await;
            if dart_primary {
                off.insert(DataSource::KindDisclosure);
            }
            off
        })
    }
}

#[must_use]
pub fn app_collector<R: Runtime, P: PortTypes>(
    app: &AppHandle<R>,
    repositories: &SqliteRepositories,
    feeds: &CollectorFeeds,
    clock: Arc<dyn Clock>,
) -> FeedCollectorWorker {
    let headlines = SharedHeadlines::default();
    let runner = |feed: &SharedFeed| -> Arc<dyn FeedRunner> {
        Arc::new(
            AppFeedRunner::<R, P, SharedFeed>::new(app.clone(), repositories.clone(), feed.clone())
                .sharing(&headlines),
        )
    };
    let scheduled = feeds
        .0
        .iter()
        .map(|feed| {
            let regular = ScheduledFeed::new(feed.target, runner(&feed.regular))
                .with_source(application::feed_data_source(&feed.regular.source()));
            match &feed.startup {
                Some(startup) => regular.with_startup(runner(startup)),
                None => regular,
            }
        })
        .collect();
    FeedCollectorWorker::new(
        scheduled,
        Arc::new(AppCollectionPlanner::<R, P>::new(app.clone())),
        clock,
        Duration::from_secs(COLLECTOR_TICK_SECONDS),
    )
}
