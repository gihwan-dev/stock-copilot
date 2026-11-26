use std::collections::{BTreeSet, VecDeque};
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use application::{
    Clock, CollectedEntry, CollectedObservation, DataAvailability, DiagnosticsSink, FeedCursor,
    FeedSource, InformationError, NewsFeed, NewsMetadata, PublicationTime, SecretValue,
    TrackedInstrument,
};
use domain::{
    CalendarDate, DisclosureSource, InstrumentId, KnownMarket, Nanosecond, OffsetSeconds,
    SpanSeconds, TimeSpan, Timestamp,
};
use tokio::sync::Mutex;

use crate::{
    ContactUserAgent, DartDisclosureClient, NewsLinkHints, SecCik, SecEdgarClient,
    SecEdgarSubmissionsFeed, SecTickerCikMap, SourceEndpoint, SourceHttpClient, SourceWait,
    TrackedTargets,
};

const KST_OFFSET_SECONDS: i32 = 9 * 3_600;
const SEEN_LIMIT: usize = 4_000;
pub const MAX_DART_PAGES: u32 = 5;
const SEC_RECENT_SECONDS: u64 = 7_u64 * 86_400_u64;
const SEC_FILINGS_PER_COMPANY: usize = 5;
const SEC_MAP_REFRESH_SECONDS: u64 = 86_400;
const SEC_MAX_COMPANIES: usize = 20;

pub type SecretFuture<'a> = Pin<Box<dyn Future<Output = Option<SecretValue>> + Send + 'a>>;
pub type ContactFuture<'a> = Pin<Box<dyn Future<Output = Option<String>> + Send + 'a>>;

pub trait DartKeySource: Send + Sync {
    fn dart_key(&self) -> SecretFuture<'_>;
}

pub trait SecContactSource: Send + Sync {
    fn sec_contact(&self) -> ContactFuture<'_>;
}

#[derive(Debug, Default)]
struct SeenKeys {
    order: VecDeque<String>,
    known: BTreeSet<String>,
}

impl SeenKeys {
    fn insert(&mut self, key: &str) -> bool {
        if self.known.contains(key) {
            return false;
        }
        if self.order.len() >= SEEN_LIMIT
            && let Some(oldest) = self.order.pop_front()
        {
            let _removed = self.known.remove(&oldest);
        }
        self.order.push_back(key.to_owned());
        self.known.insert(key.to_owned())
    }
}

const fn disclosure_key(observation: &CollectedObservation) -> Option<&domain::NormalizedUrl> {
    match observation.entry() {
        CollectedEntry::Disclosure(item) => Some(item.collected().url()),
        CollectedEntry::News(_) => None,
    }
}

#[must_use]
pub fn kst_date(at: Timestamp) -> Option<CalendarDate> {
    at.date_at_offset(OffsetSeconds::new(KST_OFFSET_SECONDS))
        .ok()
}

pub struct DartLatestFeed {
    client: Arc<SourceHttpClient>,
    keys: Arc<dyn DartKeySource>,
    hints: NewsLinkHints,
    seen: Mutex<SeenKeys>,
}

impl fmt::Debug for DartLatestFeed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("DartLatestFeed")
    }
}

impl DartLatestFeed {
    #[must_use]
    pub fn new(
        client: Arc<SourceHttpClient>,
        keys: Arc<dyn DartKeySource>,
        hints: NewsLinkHints,
    ) -> Self {
        Self {
            client,
            keys,
            hints,
            seen: Mutex::new(SeenKeys::default()),
        }
    }

    async fn take_new(
        &self,
        page: &crate::DartDisclosurePage,
        observations: &mut Vec<CollectedObservation>,
    ) -> usize {
        let collected_at = self.client.now();
        let mut seen = self.seen.lock().await;
        let before = observations.len();
        for entry in page.entries() {
            let disclosure = entry.disclosure().clone();
            let url = disclosure.collected().url().clone();
            if !seen.insert(url.as_str()) {
                continue;
            }
            if let Some(instrument) = entry
                .stock_code()
                .and_then(|code| InstrumentId::try_new(code.as_str()).ok())
            {
                self.hints.remember(&url, instrument);
            }
            observations.push(CollectedObservation::new(
                CollectedEntry::Disclosure(disclosure),
                PublicationTime::NotProvided,
                collected_at,
            ));
        }
        observations.len().saturating_sub(before)
    }
}

impl NewsFeed for DartLatestFeed {
    fn source(&self) -> FeedSource {
        FeedSource::Disclosure(DisclosureSource::Dart)
    }

    fn metadata(&self) -> NewsMetadata {
        crate::dart_disclosures::dart_feed_metadata()
    }

    async fn new_items(
        &self,
        since: FeedCursor,
    ) -> Result<DataAvailability<(Vec<CollectedObservation>, FeedCursor)>, InformationError> {
        let Some(key) = self.keys.dart_key().await else {
            return Ok(DataAvailability::NotConfigured);
        };
        let Some(today) = kst_date(self.client.now()) else {
            return Ok(DataAvailability::NoData);
        };
        let dart = DartDisclosureClient::new(Arc::clone(&self.client), Some(Arc::new(key)));
        let mut observations = Vec::new();
        let mut page_no = 1_u32;
        loop {
            let page = match dart.fetch_page(today, today, page_no).await {
                Ok(DataAvailability::Available(page)) => page,
                Ok(DataAvailability::NotConfigured) if page_no == 1 => {
                    return Ok(DataAvailability::NotConfigured);
                }
                Ok(DataAvailability::NoData | DataAvailability::NotConfigured) => break,
                Err(error) if page_no == 1 => return Err(error),
                Err(_later_page) => break,
            };
            let fresh = self.take_new(&page, &mut observations).await;
            if fresh == 0 || page_no >= page.total_page() || page_no >= MAX_DART_PAGES {
                break;
            }
            page_no = page_no.saturating_add(1);
        }
        if observations.is_empty() {
            return Ok(DataAvailability::NoData);
        }
        Ok(DataAvailability::Available((observations, since)))
    }
}

#[must_use]
pub fn sec_contact_header(contact: &str) -> Option<ContactUserAgent> {
    let trimmed = contact.trim();
    ContactUserAgent::try_new(trimmed)
        .or_else(|_single| ContactUserAgent::try_new(&format!("StockCopilot {trimmed}")))
        .ok()
}

pub struct SecClientParts {
    pub endpoint: SourceEndpoint,
    pub clock: Arc<dyn Clock>,
    pub wait: Arc<dyn SourceWait>,
    pub diagnostics: Option<Arc<dyn DiagnosticsSink>>,
}

impl fmt::Debug for SecClientParts {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SecClientParts")
            .field("endpoint", &self.endpoint)
            .finish_non_exhaustive()
    }
}

#[derive(Default)]
struct SecState {
    client: Option<(String, SecEdgarClient)>,
    ciks: Option<(Timestamp, SecTickerCikMap)>,
    seen: SeenKeys,
}

pub struct SecTrackedFeed {
    parts: SecClientParts,
    targets: Arc<dyn TrackedTargets>,
    contacts: Arc<dyn SecContactSource>,
    hints: NewsLinkHints,
    state: Mutex<SecState>,
}

impl fmt::Debug for SecTrackedFeed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SecTrackedFeed")
    }
}

fn span(seconds: u64) -> Option<TimeSpan> {
    TimeSpan::try_new(SpanSeconds::new(seconds), Nanosecond::new(0_u32)).ok()
}

fn within(earlier: Timestamp, later: Timestamp, seconds: u64) -> bool {
    match (earlier.elapsed_until(later), span(seconds)) {
        (Ok(elapsed), Some(limit)) => elapsed <= limit,
        (Err(_future), _) => true,
        (Ok(_), None) => false,
    }
}

impl SecTrackedFeed {
    #[must_use]
    pub fn new(
        parts: SecClientParts,
        targets: Arc<dyn TrackedTargets>,
        contacts: Arc<dyn SecContactSource>,
        hints: NewsLinkHints,
    ) -> Self {
        Self {
            parts,
            targets,
            contacts,
            hints,
            state: Mutex::new(SecState::default()),
        }
    }

    fn client_for(&self, contact: &str) -> Option<SecEdgarClient> {
        let header = sec_contact_header(contact)?;
        let http = SourceHttpClient::new(
            self.parts.endpoint,
            Arc::clone(&self.parts.clock),
            Arc::clone(&self.parts.wait),
            Some(header),
        )
        .ok()?;
        let observed = match &self.parts.diagnostics {
            Some(sink) => http.with_diagnostics(Arc::clone(sink)),
            None => http,
        };
        Some(SecEdgarClient::new(Arc::new(observed)))
    }

    async fn ciks(
        &self,
        state: &mut SecState,
        client: &SecEdgarClient,
    ) -> Result<Option<SecTickerCikMap>, InformationError> {
        let now = client.now();
        if let Some((fetched, map)) = &state.ciks
            && within(*fetched, now, SEC_MAP_REFRESH_SECONDS)
        {
            return Ok(Some(map.clone()));
        }
        match client.fetch_ticker_cik_map().await? {
            DataAvailability::Available(map) => {
                state.ciks = Some((now, map.clone()));
                Ok(Some(map))
            }
            DataAvailability::NoData | DataAvailability::NotConfigured => Ok(None),
        }
    }

    fn keep_recent(
        &self,
        state: &mut SecState,
        instrument: Option<&InstrumentId>,
        found: Vec<CollectedObservation>,
        now: Timestamp,
    ) -> Vec<CollectedObservation> {
        let mut kept = Vec::new();
        for observation in found {
            if kept.len() >= SEC_FILINGS_PER_COMPANY {
                break;
            }
            let recent = match observation.published_at() {
                PublicationTime::Known(at) => within(at, now, SEC_RECENT_SECONDS),
                PublicationTime::NotProvided => true,
            };
            let Some(url) = disclosure_key(&observation).cloned() else {
                continue;
            };
            if !recent || !state.seen.insert(url.as_str()) {
                continue;
            }
            if let Some(id) = instrument {
                self.hints.remember(&url, id.clone());
            }
            kept.push(observation);
        }
        kept
    }
}

fn sec_cik(map: &SecTickerCikMap, target: &TrackedInstrument) -> Option<SecCik> {
    map.cik_for_symbol(&target.symbol).cloned()
}

impl NewsFeed for SecTrackedFeed {
    fn source(&self) -> FeedSource {
        FeedSource::Disclosure(DisclosureSource::SecEdgar)
    }

    fn metadata(&self) -> NewsMetadata {
        crate::us_sources::sec_feed_metadata()
    }

    async fn new_items(
        &self,
        since: FeedCursor,
    ) -> Result<DataAvailability<(Vec<CollectedObservation>, FeedCursor)>, InformationError> {
        let Some(contact) = self.contacts.sec_contact().await else {
            return Ok(DataAvailability::NotConfigured);
        };
        let targets: Vec<TrackedInstrument> = self
            .targets
            .tracked()
            .await
            .into_iter()
            .filter(|target| target.market == KnownMarket::Us)
            .take(SEC_MAX_COMPANIES)
            .collect();
        if targets.is_empty() {
            return Ok(DataAvailability::NoData);
        }
        let mut state = self.state.lock().await;
        let client = match &state.client {
            Some((known, client)) if *known == contact => client.clone(),
            Some(_) | None => {
                let Some(fresh) = self.client_for(&contact) else {
                    return Ok(DataAvailability::NotConfigured);
                };
                state.client = Some((contact.clone(), fresh.clone()));
                fresh
            }
        };
        let Some(map) = self.ciks(&mut state, &client).await? else {
            return Ok(DataAvailability::NoData);
        };
        let mut observations = Vec::new();
        let mut failure = None;
        let mut answered = 0_usize;
        for target in &targets {
            let Some(cik) = sec_cik(&map, target) else {
                continue;
            };
            let feed = SecEdgarSubmissionsFeed::new(client.clone(), cik);
            match feed.new_items(FeedCursor::Start).await {
                Ok(DataAvailability::Available((found, _cursor))) => {
                    answered = answered.saturating_add(1);
                    let now = client.now();
                    let kept = self.keep_recent(&mut state, target.link_id().as_ref(), found, now);
                    observations.extend(kept);
                }
                Ok(DataAvailability::NoData | DataAvailability::NotConfigured) => {
                    answered = answered.saturating_add(1);
                }
                Err(error) => failure = failure.or(Some(error)),
            }
        }
        match (observations.is_empty(), failure, answered) {
            (true, Some(error), 0) => Err(error),
            (true, Some(_) | None, _) => Ok(DataAvailability::NoData),
            (false, Some(_) | None, _) => Ok(DataAvailability::Available((observations, since))),
        }
    }
}
