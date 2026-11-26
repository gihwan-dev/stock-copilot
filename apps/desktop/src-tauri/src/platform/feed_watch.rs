use application::{
    CollectedCount, CollectedEntry, CollectedObservation, DataAvailability, DiagnosticEvent,
    FeedCursor, FeedDiagnosis, FeedFailure, FeedRound, FeedRoundOutcome, FeedRunResult, FeedSource,
    HeadlineAdmission, InformationError, LinkProbe, NewsFeed, NewsMetadata, RecentHeadlines,
    SystemPortError, feed_data_source,
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

pub struct WatchedFeed<'a, F> {
    feed: &'a F,
    headlines: &'a SharedHeadlines,
    fetched: AtomicU32,
    duplicates: AtomicU32,
    failure: Mutex<Option<FeedFailure>>,
}

impl<F> fmt::Debug for WatchedFeed<'_, F> {
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

impl<'a, F: NewsFeed> WatchedFeed<'a, F> {
    #[must_use]
    pub const fn new(feed: &'a F, headlines: &'a SharedHeadlines) -> Self {
        Self {
            feed,
            headlines,
            fetched: AtomicU32::new(0),
            duplicates: AtomicU32::new(0),
            failure: Mutex::const_new(None),
        }
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

impl<F: NewsFeed> NewsFeed for WatchedFeed<'_, F> {
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
                let batch = self.headlines.admit(observations).await;
                self.duplicates.store(batch.duplicates, Ordering::Relaxed);
                Ok(DataAvailability::Available((batch.kept, next)))
            }
            Ok(other) => Ok(other),
            Err(error) => {
                *self.failure.lock().await = Some(failure_of(error));
                Err(error)
            }
        }
    }
}
