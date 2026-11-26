use application::{
    CollectedCount, CollectedEntry, CollectedObservation, CollectedPresence,
    CollectedPresenceCheck, DataAvailability, DiagnosticEvent, FeedCursor, FeedDiagnosis,
    FeedFailure, FeedRound, FeedRoundOutcome, FeedRunResult, FeedSource, HeadlineAdmission,
    InformationError, LinkProbe, NewsFeed, NewsMetadata, RecentHeadlines, SystemPortError,
    feed_data_source,
};
use domain::{InformationSource, NewsSource};
use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use tokio::sync::Mutex;

#[derive(Debug, Clone, Default)]
pub struct SharedHeadlines(Arc<Mutex<RecentHeadlines>>);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeadlineRole {
    Original,
    Relay,
}

#[must_use]
pub const fn headline_role(entry: &CollectedEntry) -> HeadlineRole {
    match entry {
        CollectedEntry::Disclosure(_) => HeadlineRole::Original,
        CollectedEntry::News(item) => match item.collected().source() {
            InformationSource::News(NewsSource::GoogleNews | NewsSource::YahooFinance) => {
                HeadlineRole::Relay
            }
            InformationSource::News(
                NewsSource::Hankyung
                | NewsSource::Maekyung
                | NewsSource::Yonhap
                | NewsSource::Edaily
                | NewsSource::BankOfKorea
                | NewsSource::FederalReserve,
            )
            | InformationSource::Web
            | InformationSource::Disclosure(_) => HeadlineRole::Original,
        },
    }
}

const fn title_of(entry: &CollectedEntry) -> &domain::UntrustedText {
    match entry {
        CollectedEntry::News(item) => item.collected().title(),
        CollectedEntry::Disclosure(item) => item.collected().title(),
    }
}

impl SharedHeadlines {
    pub async fn admit(&self, observations: Vec<CollectedObservation>) -> AdmittedBatch {
        let mut recent = self.0.lock().await;
        let mut kept = Vec::with_capacity(observations.len());
        let mut duplicates = 0_u32;
        for observation in observations {
            if matches_disclosure(observation.entry()) {
                kept.push(observation);
                continue;
            }
            let admission = recent.admit(title_of(observation.entry()), observation.collected_at());
            match (admission, headline_role(observation.entry())) {
                (HeadlineAdmission::Duplicate, HeadlineRole::Relay) => {
                    duplicates = duplicates.saturating_add(1);
                }
                (HeadlineAdmission::New, _)
                | (HeadlineAdmission::Duplicate, HeadlineRole::Original) => {
                    kept.push(observation);
                }
            }
        }
        AdmittedBatch { kept, duplicates }
    }
}

const fn matches_disclosure(entry: &CollectedEntry) -> bool {
    match entry {
        CollectedEntry::Disclosure(_) => true,
        CollectedEntry::News(_) => false,
    }
}

#[derive(Debug)]
pub struct AdmittedBatch {
    pub kept: Vec<CollectedObservation>,
    pub duplicates: u32,
}

pub struct WatchedFeed<'a, F, K> {
    feed: &'a F,
    headlines: &'a SharedHeadlines,
    known: &'a K,
    probe: Option<&'a dyn crate::collected_links::TitleProbe>,
    fetched: AtomicU32,
    duplicates: AtomicU32,
    already: AtomicU32,
    failure: Mutex<Option<FeedFailure>>,
}

impl<F, K> fmt::Debug for WatchedFeed<'_, F, K> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("WatchedFeed")
    }
}

const fn failure_of(error: InformationError) -> FeedFailure {
    match error {
        InformationError::Unavailable(_) => FeedFailure::Unavailable,
        InformationError::InvalidResponse(_) => FeedFailure::InvalidResponse,
        InformationError::InvalidCredential(_) => FeedFailure::InvalidCredential,
    }
}

impl<'a, F: NewsFeed, K: CollectedPresenceCheck> WatchedFeed<'a, F, K> {
    #[must_use]
    pub const fn new(feed: &'a F, headlines: &'a SharedHeadlines, known: &'a K) -> Self {
        Self {
            feed,
            headlines,
            known,
            probe: None,
            fetched: AtomicU32::new(0),
            duplicates: AtomicU32::new(0),
            already: AtomicU32::new(0),
            failure: Mutex::const_new(None),
        }
    }

    #[must_use]
    pub const fn probing(mut self, probe: &'a dyn crate::collected_links::TitleProbe) -> Self {
        self.probe = Some(probe);
        self
    }

    pub async fn diagnosis(
        &self,
        round: &Result<FeedRound, SystemPortError>,
        linked: u32,
        probe: LinkProbe,
    ) -> FeedDiagnosis {
        let source = feed_data_source(&self.feed.source());
        let failure = *self.failure.lock().await;
        let (result, stored) = match (round, failure) {
            (Err(_storage), _) => (FeedRunResult::Failed(FeedFailure::Storage), 0_u32),
            (Ok(found), Some(kind)) => (
                FeedRunResult::Failed(kind),
                found
                    .news
                    .as_u32()
                    .saturating_add(*found.disclosures.as_u32()),
            ),
            (Ok(found), None) => (
                match found.outcome {
                    FeedRoundOutcome::Collected => FeedRunResult::Collected,
                    FeedRoundOutcome::NoData => FeedRunResult::NoData,
                    FeedRoundOutcome::NotConfigured => FeedRunResult::NotConfigured,
                    FeedRoundOutcome::SourceFailed => {
                        FeedRunResult::Failed(FeedFailure::Unavailable)
                    }
                },
                found
                    .news
                    .as_u32()
                    .saturating_add(*found.disclosures.as_u32()),
            ),
        };
        FeedDiagnosis {
            source,
            result,
            fetched: CollectedCount::new(self.fetched.load(Ordering::Relaxed)),
            duplicates: CollectedCount::new(self.duplicates.load(Ordering::Relaxed)),
            known: CollectedCount::new(self.already.load(Ordering::Relaxed)),
            stored: CollectedCount::new(stored),
            linked: CollectedCount::new(linked),
            probe,
        }
    }
}

pub fn record_feed(diagnosis: FeedDiagnosis) {
    crate::diagnostics_log::record(&DiagnosticEvent::Feed(diagnosis));
}

fn count(items: &[CollectedObservation]) -> u32 {
    items
        .iter()
        .fold(0_u32, |total, _item| total.saturating_add(1))
}

impl<F: NewsFeed, K: CollectedPresenceCheck> WatchedFeed<'_, F, K> {
    async fn unseen(&self, observations: Vec<CollectedObservation>) -> Vec<CollectedObservation> {
        let mut fresh = Vec::with_capacity(observations.len());
        let mut already = 0_u32;
        for observation in observations {
            let url = match observation.entry() {
                CollectedEntry::News(item) => item.collected().url(),
                CollectedEntry::Disclosure(item) => item.collected().url(),
            };
            match self.known.presence(url).await {
                Ok(CollectedPresence::Stored) => already = already.saturating_add(1),
                Ok(CollectedPresence::New) | Err(_) => fresh.push(observation),
            }
        }
        self.already.store(already, Ordering::Relaxed);
        fresh
    }
}

impl<F: NewsFeed, K: CollectedPresenceCheck> NewsFeed for WatchedFeed<'_, F, K> {
    fn source(&self) -> FeedSource {
        self.feed.source()
    }

    fn metadata(&self) -> NewsMetadata {
        self.feed.metadata()
    }

    async fn new_items(
        &self,
        since: FeedCursor,
    ) -> Result<DataAvailability<(Vec<CollectedObservation>, FeedCursor)>, InformationError> {
        match self.feed.new_items(since).await {
            Ok(DataAvailability::Available((observations, next))) => {
                self.fetched.store(count(&observations), Ordering::Relaxed);
                if let Some(probe) = self.probe {
                    for observation in &observations {
                        probe.inspect(observation.entry());
                    }
                }
                let batch = self.headlines.admit(observations).await;
                self.duplicates.store(batch.duplicates, Ordering::Relaxed);
                Ok(DataAvailability::Available((
                    self.unseen(batch.kept).await,
                    next,
                )))
            }
            Ok(other) => Ok(other),
            Err(error) => {
                *self.failure.lock().await = Some(failure_of(error));
                Err(error)
            }
        }
    }
}
