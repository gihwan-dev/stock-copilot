use crate::ai_runtime::RunMetadataBook;
use application::{
    AiRunId, ArticleBody, ArticleFetchFailure, ArticleLocation, ArticleToolPort,
    BodyReadingSetting, Clock, CollectedEntry, CollectedItemHandle, CollectedItemQuery,
    CollectedItemReads, CollectedKind, DisclosureToolLine, NewsDataSettings,
    NewsDataSettingsRepository, NewsToolLine, NewsToolReads, ObservedCollectedItem,
    PublicationTime, RetryDelay, SystemFailure, SystemPort, SystemPortError, ThesisRunMetadata,
    ThesisToolPort, WatchlistRepository,
};
use domain::{
    AiBodyReadPolicy, EvidenceRef, EvidenceUrl, InformationSource, InstrumentId, Nanosecond,
    NewsSource, NormalizedUrl, SpanSeconds, Symbol, TimeSpan, Timestamp, UntrustedText,
};
use std::future::Future;
use std::sync::Arc;

#[cfg(test)]
mod tests;

const SECONDS_PER_DAY: u64 = 86_400;

const fn repository_unavailable() -> SystemPortError {
    SystemPortError::new(
        SystemPort::Repository,
        SystemFailure::Unavailable(RetryDelay::Unknown),
    )
}

fn since(now: Timestamp, query: &CollectedItemQuery) -> Result<Timestamp, SystemPortError> {
    lookback_start(now, u64::from(query.lookback.as_u16()))
}

pub(super) fn lookback_start(now: Timestamp, days: u64) -> Result<Timestamp, SystemPortError> {
    let seconds = days.saturating_mul(SECONDS_PER_DAY);
    TimeSpan::try_new(SpanSeconds::new(seconds), Nanosecond::new(0))
        .ok()
        .and_then(|span| now.checked_sub(span).ok())
        .ok_or_else(repository_unavailable)
}

fn symbols_of(item: &ObservedCollectedItem) -> Vec<Symbol> {
    item.instruments
        .iter()
        .filter_map(|instrument| Symbol::try_new(instrument.as_str()).ok())
        .collect()
}

fn matches(item: &ObservedCollectedItem, query: &CollectedItemQuery) -> bool {
    let symbol_ok = query.symbol.as_ref().is_none_or(|symbol| {
        item.instruments
            .iter()
            .any(|instrument| instrument.as_str() == symbol.as_str())
    });
    let keyword_ok = query.keyword.as_ref().is_none_or(|keyword| {
        let needle = keyword.as_str().to_lowercase();
        let content = collected(&item.entry);
        content
            .title()
            .as_untrusted_str()
            .to_lowercase()
            .contains(&needle)
            || content
                .body()
                .is_some_and(|body| body.as_untrusted_str().to_lowercase().contains(&needle))
    });
    symbol_ok && keyword_ok
}

const fn collected(entry: &CollectedEntry) -> &domain::CollectedItem {
    match entry {
        CollectedEntry::News(item) => item.collected(),
        CollectedEntry::Disclosure(item) => item.collected(),
    }
}

const fn published_or_collected(item: &ObservedCollectedItem) -> Timestamp {
    match item.published_at {
        PublicationTime::Known(at) => at,
        PublicationTime::NotProvided => item.collected_at,
    }
}

#[derive(Debug)]
pub struct StoredNews<S, C> {
    store: Arc<S>,
    clock: Arc<C>,
}

impl<S, C> StoredNews<S, C> {
    #[must_use]
    pub const fn new(store: Arc<S>, clock: Arc<C>) -> Self {
        Self { store, clock }
    }
}

impl<S: CollectedItemReads, C: Clock> StoredNews<S, C> {
    async fn recent(
        &self,
        kind: CollectedKind,
        query: &CollectedItemQuery,
    ) -> Result<Vec<ObservedCollectedItem>, SystemPortError> {
        let from = since(self.clock.now(), query)?;
        Ok(self
            .store
            .collected_since(kind, from)
            .await?
            .into_iter()
            .filter(|item| matches(item, query))
            .collect())
    }
}

impl<S: CollectedItemReads, C: Clock> NewsToolReads for StoredNews<S, C> {
    async fn search_news(
        &self,
        query: &CollectedItemQuery,
    ) -> Result<Vec<NewsToolLine>, SystemPortError> {
        Ok(self
            .cited_news(query)
            .await?
            .into_iter()
            .map(|item| item.line)
            .collect())
    }
    async fn disclosures(
        &self,
        query: &CollectedItemQuery,
    ) -> Result<Vec<DisclosureToolLine>, SystemPortError> {
        Ok(self
            .cited_disclosures(query)
            .await?
            .into_iter()
            .map(|item| item.line)
            .collect())
    }
    async fn cited_news(
        &self,
        query: &CollectedItemQuery,
    ) -> Result<Vec<application::CitedLine<NewsToolLine>>, SystemPortError> {
        Ok(self
            .recent(CollectedKind::News, query)
            .await?
            .into_iter()
            .filter_map(|item| match &item.entry {
                CollectedEntry::News(news) => Some(application::CitedLine {
                    digest: self.store.citation_digest(&item.entry),
                    line: NewsToolLine {
                        handle: item.handle.clone(),
                        source: news.collected().source().clone(),
                        published_at: published_or_collected(&item),
                        collected_at: item.collected_at,
                        title: news.collected().title().clone(),
                        summary: news.collected().body().cloned(),
                        symbols: symbols_of(&item),
                    },
                }),
                CollectedEntry::Disclosure(_) => None,
            })
            .collect())
    }

    async fn cited_disclosures(
        &self,
        query: &CollectedItemQuery,
    ) -> Result<Vec<application::CitedLine<DisclosureToolLine>>, SystemPortError> {
        Ok(self
            .recent(CollectedKind::Disclosure, query)
            .await?
            .into_iter()
            .filter_map(|item| match &item.entry {
                CollectedEntry::Disclosure(filing) => Some(application::CitedLine {
                    digest: self.store.citation_digest(&item.entry),
                    line: DisclosureToolLine {
                        handle: item.handle.clone(),
                        source: filing.key().source(),
                        filed_at: published_or_collected(&item),
                        collected_at: item.collected_at,
                        title: filing.collected().title().clone(),
                        symbols: symbols_of(&item),
                    },
                }),
                CollectedEntry::News(_) => None,
            })
            .collect())
    }
}

pub trait ArticleBodies: Send + Sync {
    fn fetch(
        &self,
        source: &NewsSource,
        url: &NormalizedUrl,
        settings: &NewsDataSettings,
    ) -> impl Future<Output = Result<UntrustedText, ArticleFetchFailure>> + Send;
}

#[derive(Debug)]
pub struct StoredArticles<S, B, C> {
    store: Arc<S>,
    bodies: Arc<B>,
    clock: Arc<C>,
}

impl<S, B, C> StoredArticles<S, B, C> {
    #[must_use]
    pub const fn new(store: Arc<S>, bodies: Arc<B>, clock: Arc<C>) -> Self {
        Self {
            store,
            bodies,
            clock,
        }
    }
}

impl<S: NewsDataSettingsRepository, B: Send + Sync, C: Send + Sync> StoredArticles<S, B, C> {
    async fn settings(&self) -> Result<NewsDataSettings, SystemPortError> {
        Ok(self
            .store
            .load()
            .await?
            .unwrap_or_else(NewsDataSettings::initial))
    }
}

impl<S, B, C> ArticleToolPort for StoredArticles<S, B, C>
where
    S: CollectedItemReads + NewsDataSettingsRepository,
    B: ArticleBodies,
    C: Clock,
{
    async fn locate(
        &self,
        handle: &CollectedItemHandle,
    ) -> Result<Option<ArticleLocation>, SystemPortError> {
        Ok(self
            .store
            .collected_item(handle)
            .await?
            .and_then(|item| match item.entry {
                CollectedEntry::News(news) => Some(ArticleLocation {
                    source: news.collected().source().clone(),
                    url: news.collected().url().clone(),
                }),
                CollectedEntry::Disclosure(_) => None,
            }))
    }

    async fn body_reading(
        &self,
        source: &NewsSource,
    ) -> Result<BodyReadingSetting, SystemPortError> {
        Ok(
            match self.settings().await?.body_read().policy_for(source) {
                AiBodyReadPolicy::BodyAllowed => BodyReadingSetting::Enabled,
                AiBodyReadPolicy::TitleOnly | AiBodyReadPolicy::TitleAndSummary => {
                    BodyReadingSetting::Disabled
                }
            },
        )
    }

    async fn fetch_body(&self, url: &NormalizedUrl) -> Result<ArticleBody, ArticleFetchFailure> {
        let settings = self
            .settings()
            .await
            .map_err(ArticleFetchFailure::Unavailable)?;
        let since = Timestamp::from_parts(
            domain::UnixSeconds::new(0_i64),
            Nanosecond::new(0_u32),
            domain::OffsetSeconds::new(0_i32),
        )
        .map_err(|_error| ArticleFetchFailure::Unavailable(repository_unavailable()))?;
        let items = self
            .store
            .collected_since(CollectedKind::News, since)
            .await
            .map_err(ArticleFetchFailure::Unavailable)?;
        let source = items
            .iter()
            .find_map(|item| match &item.entry {
                CollectedEntry::News(news) if news.collected().url() == url => {
                    match news.collected().source() {
                        InformationSource::News(source) => Some(source.clone()),
                        InformationSource::Web | InformationSource::Disclosure(_) => None,
                    }
                }
                CollectedEntry::News(_) | CollectedEntry::Disclosure(_) => None,
            })
            .ok_or(ArticleFetchFailure::NotFound)?;
        let text = self.bodies.fetch(&source, url, &settings).await?;
        Ok(ArticleBody {
            text,
            fetched_at: self.clock.now(),
        })
    }
}

#[derive(Debug)]
pub struct StoredThesis<S> {
    store: Arc<S>,
    runs: RunMetadataBook,
}

impl<S> StoredThesis<S> {
    #[must_use]
    pub const fn new(store: Arc<S>, runs: RunMetadataBook) -> Self {
        Self { store, runs }
    }
}

impl<S> ThesisToolPort for StoredThesis<S>
where
    S: CollectedItemReads + WatchlistRepository,
{
    async fn resolve_target(
        &self,
        _run: &AiRunId,
        symbol: &Symbol,
    ) -> Result<Option<InstrumentId>, SystemPortError> {
        Ok(WatchlistRepository::load(self.store.as_ref())
            .await?
            .instruments()
            .into_iter()
            .find(|instrument| instrument.as_str() == symbol.as_str()))
    }

    async fn collected_evidence(
        &self,
        _run: &AiRunId,
        _instrument: &InstrumentId,
        handle: &CollectedItemHandle,
    ) -> Result<Option<EvidenceRef>, SystemPortError> {
        Ok(self
            .store
            .collected_item(handle)
            .await?
            .and_then(|item| match &item.entry {
                CollectedEntry::News(news) => match news.collected().source() {
                    InformationSource::News(source) => {
                        EvidenceUrl::try_new(news.collected().url().clone())
                            .ok()
                            .map(|url| EvidenceRef::News(source.clone(), url))
                    }
                    InformationSource::Web | InformationSource::Disclosure(_) => None,
                },
                CollectedEntry::Disclosure(filing) => EvidenceRef::from_disclosure(filing).ok(),
            }))
    }

    fn run_metadata(
        &self,
        run: &AiRunId,
    ) -> impl Future<Output = Result<ThesisRunMetadata, SystemPortError>> + Send {
        std::future::ready(
            self.runs
                .thesis_metadata(run)
                .ok_or_else(repository_unavailable),
        )
    }
}
