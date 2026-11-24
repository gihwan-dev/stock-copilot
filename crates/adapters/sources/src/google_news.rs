use std::collections::BTreeSet;
use std::sync::Arc;

use application::{
    AiReadPolicy, CollectedEntry, CollectedObservation, DataAvailability, FeedCursor, FeedPosition,
    FeedSource, InformationError, NewsFeed, NewsMetadata, PublicationTime, ResponseMismatch,
    Retention,
};
use domain::{
    ApprovedWebSearchQuery, InformationSource, Nanosecond, NewsItem, NewsSource, NormalizedUrl,
    SpanSeconds, TimeSpan, Timestamp, UntrustedText,
};
use form_urlencoded::Serializer;

use crate::{
    HttpDataSource, ParsedFeedItem, SourceFetch, SourceFetchOutcome, SourceHttpClient,
    SourceRegistry, http_error_to_information, http_status_to_error, parse_feed,
};

pub const GOOGLE_NEWS_RETENTION_SECONDS: u64 = 7_u64 * 86_400_u64;
const MAX_CURSOR_SEEN_URLS: usize = 128;

#[must_use]
pub fn build_google_news_search_url(query: &ApprovedWebSearchQuery) -> String {
    let encoded = Serializer::new(String::new())
        .append_pair("q", query.text().as_str())
        .append_pair("hl", "ko")
        .append_pair("gl", "KR")
        .append_pair("ceid", "KR:ko")
        .finish();
    format!("https://news.google.com/rss/search?{encoded}")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoogleNewsDisplayItem {
    title: UntrustedText,
    link: NormalizedUrl,
    publisher: Option<UntrustedText>,
    published_at: PublicationTime,
    collected_at: Timestamp,
}

impl GoogleNewsDisplayItem {
    #[must_use]
    pub const fn title(&self) -> &UntrustedText {
        &self.title
    }

    #[must_use]
    pub const fn link(&self) -> &NormalizedUrl {
        &self.link
    }

    #[must_use]
    pub const fn publisher(&self) -> Option<&UntrustedText> {
        self.publisher.as_ref()
    }

    #[must_use]
    pub const fn published_at(&self) -> PublicationTime {
        self.published_at
    }

    #[must_use]
    pub const fn collected_at(&self) -> Timestamp {
        self.collected_at
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GoogleNewsRetentionStatus {
    Keep,
    Expired,
}

#[must_use]
pub fn google_news_retention_status(
    collected_at: Timestamp,
    now: Timestamp,
) -> GoogleNewsRetentionStatus {
    let Ok(limit) = TimeSpan::try_new(
        SpanSeconds::new(GOOGLE_NEWS_RETENTION_SECONDS),
        Nanosecond::new(0_u32),
    ) else {
        return GoogleNewsRetentionStatus::Expired;
    };
    match collected_at.elapsed_until(now) {
        Ok(elapsed) if elapsed >= limit => GoogleNewsRetentionStatus::Expired,
        Ok(_) | Err(_) => GoogleNewsRetentionStatus::Keep,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoogleNewsPruneResult {
    pub retained: Vec<CollectedObservation>,
    pub deleted_count: usize,
}

#[must_use]
pub fn prune_expired_google_news(
    items: Vec<CollectedObservation>,
    now: Timestamp,
) -> GoogleNewsPruneResult {
    let mut retained = Vec::with_capacity(items.len());
    let mut deleted_count = 0_usize;
    for item in items {
        let is_google_news = match item.entry() {
            CollectedEntry::News(news) => {
                news.collected().source() == &InformationSource::News(NewsSource::GoogleNews)
            }
            CollectedEntry::Disclosure(_) => false,
        };
        if is_google_news
            && google_news_retention_status(item.collected_at(), now)
                == GoogleNewsRetentionStatus::Expired
        {
            deleted_count = deleted_count.saturating_add(1_usize);
        } else {
            retained.push(item);
        }
    }
    GoogleNewsPruneResult {
        retained,
        deleted_count,
    }
}

fn parse_google_item(
    item: &ParsedFeedItem,
    collected_at: Timestamp,
) -> Option<(GoogleNewsDisplayItem, CollectedObservation)> {
    if item.title.is_empty() || item.link.is_empty() {
        return None;
    }
    let link = NormalizedUrl::try_new(&item.link).ok()?;
    let title = UntrustedText::try_new(&item.title).ok()?;
    let publisher = item
        .source_name
        .as_deref()
        .and_then(|name| UntrustedText::try_new(name).ok());
    let news_item = NewsItem::new(NewsSource::GoogleNews, link.clone(), title.clone(), None);
    let observation = CollectedObservation::new(
        CollectedEntry::News(news_item),
        item.published_at,
        collected_at,
    );
    let display = GoogleNewsDisplayItem {
        title,
        link,
        publisher,
        published_at: item.published_at,
        collected_at,
    };
    Some((display, observation))
}

fn decode_seen_urls(cursor: &FeedCursor) -> Vec<String> {
    let FeedCursor::Continue(position) = cursor else {
        return Vec::new();
    };
    position
        .as_untrusted_text()
        .as_untrusted_str()
        .lines()
        .filter_map(|line| line.strip_prefix("url="))
        .filter(|value| !value.is_empty())
        .take(MAX_CURSOR_SEEN_URLS)
        .map(str::to_owned)
        .collect()
}

fn encode_seen_urls(latest: Vec<String>, previous: Vec<String>) -> Option<FeedCursor> {
    let mut merged = Vec::with_capacity(MAX_CURSOR_SEEN_URLS);
    for url in latest.into_iter().chain(previous) {
        if merged.len() >= MAX_CURSOR_SEEN_URLS {
            break;
        }
        if !merged.contains(&url) {
            merged.push(url);
        }
    }
    let text = merged
        .into_iter()
        .map(|url| format!("url={url}"))
        .collect::<Vec<_>>()
        .join("\n");
    if text.is_empty() {
        return None;
    }
    UntrustedText::try_new(&text)
        .ok()
        .map(|value| FeedCursor::Continue(FeedPosition::new(value)))
}

#[derive(Debug)]
pub struct GoogleNewsFeed {
    client: Arc<SourceHttpClient>,
    query: ApprovedWebSearchQuery,
}

impl GoogleNewsFeed {
    #[must_use]
    pub const fn new(client: Arc<SourceHttpClient>, query: ApprovedWebSearchQuery) -> Self {
        Self { client, query }
    }

    #[must_use]
    pub fn search_url(&self) -> String {
        build_google_news_search_url(&self.query)
    }

    async fn fetch_parsed(
        &self,
    ) -> Result<Vec<(GoogleNewsDisplayItem, CollectedObservation)>, InformationError> {
        let url = self.search_url();
        let fetch = SourceFetch::new(HttpDataSource::GoogleNews, &url);
        let outcome = self
            .client
            .fetch(&fetch)
            .await
            .map_err(|error| http_error_to_information(HttpDataSource::GoogleNews, error))?;
        let payload = match outcome {
            SourceFetchOutcome::Modified(data) => data,
            SourceFetchOutcome::NotModified => return Ok(Vec::new()),
            SourceFetchOutcome::HttpFailure { status, .. } => {
                return Err(http_status_to_error(HttpDataSource::GoogleNews, status));
            }
        };
        let parsed_items = parse_feed(payload.body())?;
        let collected_at = self.client.now();
        let mut results = Vec::with_capacity(parsed_items.len());
        for item in &parsed_items {
            if let Some(pair) = parse_google_item(item, collected_at) {
                results.push(pair);
            }
        }
        if !parsed_items.is_empty() && results.is_empty() {
            return Err(InformationError::InvalidResponse(ResponseMismatch::Schema));
        }
        Ok(results)
    }

    pub async fn fetch_display_items(
        &self,
    ) -> Result<Vec<GoogleNewsDisplayItem>, InformationError> {
        let pairs = self.fetch_parsed().await?;
        Ok(pairs.into_iter().map(|(display, _obs)| display).collect())
    }
}

impl NewsFeed for GoogleNewsFeed {
    fn source(&self) -> FeedSource {
        FeedSource::News(NewsSource::GoogleNews)
    }

    fn metadata(&self) -> NewsMetadata {
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

    async fn new_items(
        &self,
        since: FeedCursor,
    ) -> Result<DataAvailability<(Vec<CollectedObservation>, FeedCursor)>, InformationError> {
        let previous = decode_seen_urls(&since);
        let seen: BTreeSet<&str> = previous.iter().map(String::as_str).collect();
        let pairs = self.fetch_parsed().await?;
        let mut latest_urls = Vec::new();
        let mut observations = Vec::new();
        for (display, obs) in pairs {
            let key = display.link().as_str().to_owned();
            if !latest_urls.contains(&key) {
                latest_urls.push(key.clone());
            }
            if seen.contains(key.as_str()) {
                continue;
            }
            observations.push(obs);
        }
        if observations.is_empty() {
            return Ok(DataAvailability::NoData);
        }
        let next_cursor = encode_seen_urls(latest_urls, previous).unwrap_or(since);
        Ok(DataAvailability::Available((observations, next_cursor)))
    }
}
