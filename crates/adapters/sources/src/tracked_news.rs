use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, RwLock};

use application::{
    AiReadPolicy, CollectedEntry, CollectedObservation, DataAvailability, FeedCursor, FeedSource,
    InformationError, NewsFeed, NewsMetadata, PublicationTime, Retention, TrackedInstrument,
};
use domain::{
    ApprovedWebSearchQuery, InstrumentId, KnownMarket, Nanosecond, NewsSource, NormalizedUrl,
    SpanSeconds, TimeSpan, Timestamp, WebSearchCandidateQuery, WebSearchInspection,
    WebSearchSensitiveContext, inspect_web_search_query,
};
use tokio::sync::Mutex;

use crate::{
    GOOGLE_NEWS_RETENTION_SECONDS, GoogleNewsFeed, HttpDataSource, SourceHttpClient,
    SourceRegistry, YahooFinanceFeed,
};

pub const MAX_TRACKED_TARGETS: usize = 20;
pub const MAX_ITEMS_PER_TARGET: usize = 10;
pub const TRACKED_RECENCY_SECONDS: u64 = 3_u64 * 86_400_u64;
const SEEN_URL_LIMIT: usize = 2_000;
const HINT_LIMIT: usize = 2_000;

pub trait TrackedTargets: Send + Sync {
    fn tracked(&self) -> Pin<Box<dyn Future<Output = Vec<TrackedInstrument>> + Send + '_>>;
}

#[derive(Debug, Clone, Default)]
pub struct NewsLinkHints {
    by_url: Arc<RwLock<BTreeMap<NormalizedUrl, BTreeSet<InstrumentId>>>>,
}

impl NewsLinkHints {
    pub fn remember(&self, url: &NormalizedUrl, instrument: InstrumentId) {
        let mut guard = match self.by_url.write() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        if guard.len() >= HINT_LIMIT && !guard.contains_key(url) {
            guard.clear();
        }
        guard.entry(url.clone()).or_default().insert(instrument);
    }

    #[must_use]
    pub fn take(&self, url: &NormalizedUrl) -> BTreeSet<InstrumentId> {
        let mut guard = match self.by_url.write() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        guard.remove(url).into_iter().flatten().collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrackedSearch {
    GoogleNews,
    YahooFinance,
}

impl TrackedSearch {
    const fn news_source(self) -> NewsSource {
        match self {
            Self::GoogleNews => NewsSource::GoogleNews,
            Self::YahooFinance => NewsSource::YahooFinance,
        }
    }

    const fn wants(self, market: KnownMarket) -> bool {
        match (self, market) {
            (Self::GoogleNews, KnownMarket::Kr | KnownMarket::Us)
            | (Self::YahooFinance, KnownMarket::Us) => true,
            (Self::YahooFinance, KnownMarket::Kr) => false,
        }
    }
}

pub const MAX_GOOGLE_SEARCHES: usize = 30;

#[must_use]
pub fn with_alias_searches(tracked: Vec<TrackedInstrument>) -> Vec<TrackedInstrument> {
    let mut searches = Vec::with_capacity(tracked.len());
    let mut asked: BTreeSet<String> = BTreeSet::new();
    let mut extra = Vec::new();
    for target in tracked {
        asked.insert(target.name.as_str().trim().to_owned());
        for alias in &target.aliases {
            if let Ok(name) = domain::DisplayInstrumentName::try_new(alias.as_str()) {
                extra.push(TrackedInstrument {
                    symbol: target.symbol.clone(),
                    market: target.market,
                    name,
                    aliases: Vec::new(),
                });
            }
        }
        searches.push(target);
    }
    for search in extra {
        if searches.len() >= MAX_GOOGLE_SEARCHES {
            break;
        }
        if asked.insert(search.name.as_str().trim().to_owned()) {
            searches.push(search);
        }
    }
    searches
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExactPhrase {
    Quoted,
    Loose,
}

const MIN_EXACT_PHRASE_CHARS: usize = 3;

fn exact_phrase(target: &TrackedInstrument) -> ExactPhrase {
    if target.name.as_str().trim().chars().count() >= MIN_EXACT_PHRASE_CHARS {
        ExactPhrase::Quoted
    } else {
        ExactPhrase::Loose
    }
}

#[must_use]
pub fn google_query(target: &TrackedInstrument) -> String {
    let name = target.name.as_str().trim();
    match exact_phrase(target) {
        ExactPhrase::Quoted => format!("\"{name}\""),
        ExactPhrase::Loose => name.to_owned(),
    }
}

fn approved_query(name: &str) -> Option<ApprovedWebSearchQuery> {
    match inspect_web_search_query(
        &WebSearchCandidateQuery::new(name),
        &WebSearchSensitiveContext::default(),
    ) {
        WebSearchInspection::Approved(query) => Some(query),
        WebSearchInspection::Rejected(_rejected) => None,
    }
}

#[must_use]
pub fn is_recent(published: PublicationTime, now: Timestamp) -> bool {
    let PublicationTime::Known(at) = published else {
        return true;
    };
    let limit = TimeSpan::try_new(
        SpanSeconds::new(TRACKED_RECENCY_SECONDS),
        Nanosecond::new(0_u32),
    )
    .ok();
    match (at.elapsed_until(now), limit) {
        (Ok(elapsed), Some(window)) => elapsed <= window,
        (Err(_future), _) => true,
        (Ok(_), None) => false,
    }
}

const fn sort_key(observation: &CollectedObservation) -> Option<Timestamp> {
    match observation.published_at() {
        PublicationTime::Known(at) => Some(at),
        PublicationTime::NotProvided => None,
    }
}

const fn url_of(observation: &CollectedObservation) -> &NormalizedUrl {
    match observation.entry() {
        CollectedEntry::News(item) => item.collected().url(),
        CollectedEntry::Disclosure(item) => item.collected().url(),
    }
}

#[derive(Debug, Default)]
struct SeenUrls {
    order: VecDeque<NormalizedUrl>,
    known: BTreeSet<NormalizedUrl>,
}

impl SeenUrls {
    fn insert(&mut self, url: &NormalizedUrl) -> bool {
        if self.known.contains(url) {
            return false;
        }
        if self.order.len() >= SEEN_URL_LIMIT
            && let Some(oldest) = self.order.pop_front()
        {
            let _removed = self.known.remove(&oldest);
        }
        self.order.push_back(url.clone());
        self.known.insert(url.clone())
    }
}

type TargetItems =
    Result<DataAvailability<(Vec<CollectedObservation>, FeedCursor)>, InformationError>;

pub struct TrackedNewsFeed {
    client: Arc<SourceHttpClient>,
    search: TrackedSearch,
    targets: Arc<dyn TrackedTargets>,
    hints: NewsLinkHints,
    seen: Mutex<SeenUrls>,
}

impl fmt::Debug for TrackedNewsFeed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TrackedNewsFeed")
            .field("search", &self.search)
            .finish_non_exhaustive()
    }
}

impl TrackedNewsFeed {
    #[must_use]
    pub fn new(
        client: Arc<SourceHttpClient>,
        search: TrackedSearch,
        targets: Arc<dyn TrackedTargets>,
        hints: NewsLinkHints,
    ) -> Self {
        Self {
            client,
            search,
            targets,
            hints,
            seen: Mutex::new(SeenUrls::default()),
        }
    }

    fn links_directly(&self, target: &TrackedInstrument) -> bool {
        match self.search {
            TrackedSearch::YahooFinance => true,
            TrackedSearch::GoogleNews => exact_phrase(target) == ExactPhrase::Quoted,
        }
    }

    async fn fetch_target(&self, target: &TrackedInstrument) -> Option<TargetItems> {
        match self.search {
            TrackedSearch::GoogleNews => {
                let query = approved_query(&google_query(target))?;
                let feed = GoogleNewsFeed::new(Arc::clone(&self.client), query);
                Some(feed.new_items(FeedCursor::Start).await)
            }
            TrackedSearch::YahooFinance => {
                let feed =
                    YahooFinanceFeed::new(Arc::clone(&self.client), vec![target.symbol.clone()]);
                Some(feed.new_items(FeedCursor::Start).await)
            }
        }
    }

    async fn keep_new(
        &self,
        target: &TrackedInstrument,
        mut found: Vec<CollectedObservation>,
    ) -> Vec<CollectedObservation> {
        let now = self.client.now();
        found.retain(|observation| is_recent(observation.published_at(), now));
        found.sort_by_key(|observation| std::cmp::Reverse(sort_key(observation)));
        let mut seen = self.seen.lock().await;
        let mut kept = Vec::new();
        for observation in found {
            if kept.len() >= MAX_ITEMS_PER_TARGET {
                break;
            }
            if !seen.insert(url_of(&observation)) {
                continue;
            }
            if self.links_directly(target)
                && let Some(instrument) = target.link_id()
            {
                self.hints.remember(url_of(&observation), instrument);
            }
            kept.push(observation);
        }
        kept
    }
}

impl NewsFeed for TrackedNewsFeed {
    fn source(&self) -> FeedSource {
        FeedSource::News(self.search.news_source())
    }

    fn metadata(&self) -> NewsMetadata {
        match self.search {
            TrackedSearch::GoogleNews => {
                let spec = SourceRegistry::spec(HttpDataSource::GoogleNews);
                let retention = TimeSpan::try_new(
                    SpanSeconds::new(GOOGLE_NEWS_RETENTION_SECONDS),
                    Nanosecond::new(0_u32),
                )
                .map_or(Retention::NotStored, Retention::For);
                NewsMetadata::new(
                    retention,
                    AiReadPolicy::TitleOnly,
                    spec.conditional_get(),
                    spec.user_agent(),
                )
            }
            TrackedSearch::YahooFinance => {
                YahooFinanceFeed::new(Arc::clone(&self.client), Vec::new()).metadata()
            }
        }
    }

    async fn new_items(
        &self,
        since: FeedCursor,
    ) -> Result<DataAvailability<(Vec<CollectedObservation>, FeedCursor)>, InformationError> {
        let tracked: Vec<TrackedInstrument> = self
            .targets
            .tracked()
            .await
            .into_iter()
            .filter(|target| self.search.wants(target.market))
            .take(MAX_TRACKED_TARGETS)
            .collect();
        let targets = match self.search {
            TrackedSearch::GoogleNews => with_alias_searches(tracked),
            TrackedSearch::YahooFinance => tracked,
        };
        if targets.is_empty() {
            return Ok(DataAvailability::NoData);
        }
        let mut observations = Vec::new();
        let mut failure = None;
        let mut answered = 0_usize;
        for target in &targets {
            match self.fetch_target(target).await {
                None => {}
                Some(Ok(DataAvailability::Available((found, _cursor)))) => {
                    answered = answered.saturating_add(1);
                    observations.extend(self.keep_new(target, found).await);
                }
                Some(Ok(DataAvailability::NoData | DataAvailability::NotConfigured)) => {
                    answered = answered.saturating_add(1);
                }
                Some(Err(error)) => failure = failure.or(Some(error)),
            }
        }
        match (observations.is_empty(), failure, answered) {
            (true, Some(error), 0) => Err(error),
            (true, Some(_) | None, _) => Ok(DataAvailability::NoData),
            (false, Some(_) | None, _) => Ok(DataAvailability::Available((observations, since))),
        }
    }
}
