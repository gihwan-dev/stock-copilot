use std::collections::BTreeSet;
use std::sync::Arc;

use application::{
    AiReadPolicy, CollectedEntry, CollectedObservation, ConditionalGet, DataAvailability,
    FeedCursor, FeedPosition, FeedSource, InformationError, NewsFeed, NewsMetadata,
    ResponseMismatch, Retention,
};
use domain::{
    Nanosecond, NewsItem, NewsSource, NormalizedUrl, SpanSeconds, TimeSpan, Timestamp,
    UntrustedText,
};

use crate::rss::{ParsedFeedItem, parse_feed};
use crate::{
    ConditionalValidators, HttpDataSource, SourceFetch, SourceFetchOutcome, SourceHttpClient,
    SourcePayload, SourceRegistry, ValidatorHeader, http_error_to_information,
    http_status_to_error,
};

const DEFAULT_NEWS_RETENTION_SECONDS: u64 = 90_u64 * 86_400_u64;
const MAX_CURSOR_SEEN_URLS: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PressMacroSource {
    Hankyung,
    Maekyung,
    Yonhap,
    Edaily,
    BankOfKorea,
    FederalReserve,
}

impl PressMacroSource {
    pub const ALL: [Self; 6] = [
        Self::Hankyung,
        Self::Maekyung,
        Self::Yonhap,
        Self::Edaily,
        Self::BankOfKorea,
        Self::FederalReserve,
    ];

    #[must_use]
    pub const fn news_source(self) -> NewsSource {
        match self {
            Self::Hankyung => NewsSource::Hankyung,
            Self::Maekyung => NewsSource::Maekyung,
            Self::Yonhap => NewsSource::Yonhap,
            Self::Edaily => NewsSource::Edaily,
            Self::BankOfKorea => NewsSource::BankOfKorea,
            Self::FederalReserve => NewsSource::FederalReserve,
        }
    }

    #[must_use]
    pub const fn http_source(self) -> HttpDataSource {
        match self {
            Self::Hankyung => HttpDataSource::Hankyung,
            Self::Maekyung => HttpDataSource::Maekyung,
            Self::Yonhap => HttpDataSource::Yonhap,
            Self::Edaily => HttpDataSource::Edaily,
            Self::BankOfKorea => HttpDataSource::BankOfKorea,
            Self::FederalReserve => HttpDataSource::FederalReserve,
        }
    }

    #[must_use]
    pub const fn default_feed_urls(self) -> &'static [&'static str] {
        match self {
            Self::Hankyung => &["https://www.hankyung.com/feed/finance"],
            Self::Maekyung => &["https://www.mk.co.kr/rss/50200011/"],
            Self::Yonhap => &[
                "https://www.yna.co.kr/rss/market.xml",
                "https://www.yna.co.kr/rss/economy.xml",
            ],
            Self::Edaily => &["https://rss.edaily.co.kr/stock_news.xml"],
            Self::BankOfKorea => {
                &["https://www.bok.or.kr/portal/bbs/B0000552/news.rss?menuNo=200690"]
            }
            Self::FederalReserve => &["https://www.federalreserve.gov/feeds/press_all.xml"],
        }
    }
}

#[derive(Default)]
struct CursorState {
    etag: Option<String>,
    last_modified: Option<String>,
    seen_urls: Vec<String>,
}

impl CursorState {
    fn decode(cursor: &FeedCursor) -> Self {
        let FeedCursor::Continue(position) = cursor else {
            return Self::default();
        };
        let mut state = Self::default();
        for line in position.as_untrusted_text().as_untrusted_str().lines() {
            state.apply_line(line);
        }
        state
    }

    fn apply_line(&mut self, line: &str) {
        if let Some(value) = line.strip_prefix("etag=").filter(|text| !text.is_empty()) {
            self.etag = Some(value.to_owned());
        } else if let Some(value) = line.strip_prefix("lm=").filter(|text| !text.is_empty()) {
            self.last_modified = Some(value.to_owned());
        } else if let Some(value) = line.strip_prefix("url=").filter(|text| !text.is_empty())
            && self.seen_urls.len() < MAX_CURSOR_SEEN_URLS
        {
            self.seen_urls.push(value.to_owned());
        }
    }

    fn validators(&self) -> Option<ConditionalValidators> {
        let etag = self
            .etag
            .as_deref()
            .and_then(|value| ValidatorHeader::try_new(value).ok());
        let last_modified = self
            .last_modified
            .as_deref()
            .and_then(|value| ValidatorHeader::try_new(value).ok());
        if etag.is_none() && last_modified.is_none() {
            None
        } else {
            Some(ConditionalValidators::new(etag, last_modified))
        }
    }

    fn encode(&self) -> Option<FeedCursor> {
        let mut lines = Vec::new();
        if let Some(etag) = &self.etag {
            lines.push(format!("etag={etag}"));
        }
        if let Some(lm) = &self.last_modified {
            lines.push(format!("lm={lm}"));
        }
        for url in self.seen_urls.iter().take(MAX_CURSOR_SEEN_URLS) {
            lines.push(format!("url={url}"));
        }
        let joined = lines.join("\n");
        if joined.is_empty() {
            return None;
        }
        UntrustedText::try_new(&joined)
            .ok()
            .map(|text| FeedCursor::Continue(FeedPosition::new(text)))
    }
}

struct BatchCollector<'seen> {
    source: NewsSource,
    seen_before: BTreeSet<&'seen str>,
    observations: Vec<CollectedObservation>,
    batch_urls: BTreeSet<String>,
    latest_urls: Vec<String>,
    parsed_total: usize,
    valid_total: usize,
}

impl<'seen> BatchCollector<'seen> {
    fn new(source: NewsSource, seen_before: BTreeSet<&'seen str>) -> Self {
        Self {
            source,
            seen_before,
            observations: Vec::new(),
            batch_urls: BTreeSet::new(),
            latest_urls: Vec::new(),
            parsed_total: 0_usize,
            valid_total: 0_usize,
        }
    }

    fn ingest(&mut self, parsed: &[ParsedFeedItem], collected_at: Timestamp) {
        self.parsed_total = self.parsed_total.saturating_add(parsed.len());
        for item in parsed {
            self.ingest_one(item, collected_at);
        }
    }

    fn ingest_one(&mut self, item: &ParsedFeedItem, collected_at: Timestamp) {
        if item.title.is_empty() || item.link.is_empty() {
            return;
        }
        let Ok(normalized_url) = NormalizedUrl::try_new(&item.link) else {
            return;
        };
        let Ok(title) = UntrustedText::try_new(&item.title) else {
            return;
        };
        let summary = item
            .summary
            .as_deref()
            .and_then(|text| UntrustedText::try_new(text).ok());
        self.valid_total = self.valid_total.saturating_add(1_usize);
        let url_key = normalized_url.as_str().to_owned();
        if !self.batch_urls.insert(url_key.clone()) {
            return;
        }
        if self.latest_urls.len() < MAX_CURSOR_SEEN_URLS {
            self.latest_urls.push(url_key.clone());
        }
        if self.seen_before.contains(url_key.as_str()) {
            return;
        }
        let news_item = NewsItem::new(self.source.clone(), normalized_url, title, summary);
        self.observations.push(CollectedObservation::new(
            CollectedEntry::News(news_item),
            item.published_at,
            collected_at,
        ));
    }
}

fn update_validators(
    mode: ConditionalGet,
    payload: &SourcePayload,
    next_etag: &mut Option<String>,
    next_last_modified: &mut Option<String>,
) {
    if mode == ConditionalGet::Unsupported {
        return;
    }
    if let Some(etag) = payload.etag() {
        *next_etag = Some(etag.to_owned());
    }
    if let Some(lm) = payload.last_modified() {
        *next_last_modified = Some(lm.to_owned());
    }
}

fn merge_seen_urls(mut latest: Vec<String>, previous: Vec<String>) -> Vec<String> {
    for old in previous {
        if latest.len() >= MAX_CURSOR_SEEN_URLS {
            break;
        }
        if !latest.contains(&old) {
            latest.push(old);
        }
    }
    latest
}

#[derive(Debug)]
pub struct RssNewsFeed {
    client: Arc<SourceHttpClient>,
    source: PressMacroSource,
    feed_urls: Vec<String>,
}

impl RssNewsFeed {
    #[must_use]
    pub fn new(client: Arc<SourceHttpClient>, source: PressMacroSource) -> Self {
        Self {
            client,
            source,
            feed_urls: source
                .default_feed_urls()
                .iter()
                .map(|url| (*url).to_owned())
                .collect(),
        }
    }

    #[must_use]
    pub fn with_feed_urls(
        client: Arc<SourceHttpClient>,
        source: PressMacroSource,
        feed_urls: Vec<String>,
    ) -> Self {
        Self {
            client,
            source,
            feed_urls,
        }
    }

    #[must_use]
    pub fn feed_urls(&self) -> &[String] {
        &self.feed_urls
    }
    async fn fetch_one(
        &self,
        url: &str,
        conditional_mode: ConditionalGet,
        prev: &CursorState,
    ) -> Result<Option<SourcePayload>, InformationError> {
        let http_source = self.source.http_source();
        let mut fetch = SourceFetch::new(http_source, url);
        if conditional_mode == ConditionalGet::Supported
            && let Some(validators) = prev.validators()
        {
            fetch = fetch.with_validators(validators);
        }
        let outcome = self
            .client
            .fetch(&fetch)
            .await
            .map_err(|error| http_error_to_information(http_source, error))?;
        match outcome {
            SourceFetchOutcome::NotModified => Ok(None),
            SourceFetchOutcome::HttpFailure { status, .. } => {
                Err(http_status_to_error(http_source, status))
            }
            SourceFetchOutcome::Modified(payload) => Ok(Some(payload)),
        }
    }
}

impl NewsFeed for RssNewsFeed {
    fn source(&self) -> FeedSource {
        FeedSource::News(self.source.news_source())
    }

    fn metadata(&self) -> NewsMetadata {
        let spec = SourceRegistry::spec(self.source.http_source());
        let retention = TimeSpan::try_new(
            SpanSeconds::new(DEFAULT_NEWS_RETENTION_SECONDS),
            Nanosecond::new(0_u32),
        )
        .map_or(Retention::NotStored, Retention::For);
        NewsMetadata::new(
            retention,
            AiReadPolicy::TitleAndSummary,
            spec.conditional_get(),
            spec.user_agent(),
        )
    }

    async fn new_items(
        &self,
        since: FeedCursor,
    ) -> Result<DataAvailability<(Vec<CollectedObservation>, FeedCursor)>, InformationError> {
        let spec = SourceRegistry::spec(self.source.http_source());
        let prev = CursorState::decode(&since);
        let conditional_mode = if spec.conditional_get() == ConditionalGet::Supported
            && self.feed_urls.len() == 1_usize
        {
            ConditionalGet::Supported
        } else {
            ConditionalGet::Unsupported
        };
        let mut next_etag = prev.etag.clone();
        let mut next_last_modified = prev.last_modified.clone();
        let mut collector = BatchCollector::new(
            self.source.news_source(),
            prev.seen_urls.iter().map(String::as_str).collect(),
        );
        let mut any_modified = false;
        for url in &self.feed_urls {
            if let Some(payload) = self.fetch_one(url, conditional_mode, &prev).await? {
                any_modified = true;
                update_validators(
                    conditional_mode,
                    &payload,
                    &mut next_etag,
                    &mut next_last_modified,
                );
                collector.ingest(&parse_feed(payload.body())?, self.client.now());
            }
        }
        let BatchCollector {
            observations,
            latest_urls,
            parsed_total,
            valid_total,
            ..
        } = collector;
        if !any_modified {
            return Ok(DataAvailability::NoData);
        }
        if parsed_total > 0_usize && valid_total == 0_usize {
            return Err(InformationError::InvalidResponse(ResponseMismatch::Schema));
        }
        if observations.is_empty() {
            return Ok(DataAvailability::NoData);
        }
        let next_state = CursorState {
            etag: next_etag,
            last_modified: next_last_modified,
            seen_urls: merge_seen_urls(latest_urls, prev.seen_urls),
        };
        Ok(DataAvailability::Available((
            observations,
            next_state.encode().unwrap_or(since),
        )))
    }
}
