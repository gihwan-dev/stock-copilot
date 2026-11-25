use crate::collected_item_reads::{CollectedTimes, NoCollectedTimes};
use crate::{
    Clock, CollectedEntry, CollectedObservation, CollectionCadence, DataAvailability,
    DisclosureRepository, EventBus, FeedCursor, FeedSource, NewsFeed, NewsRepository,
    RecordSourceStatus, SourceAttemptOutcome, SourceStatusRepository, SystemPortError,
};
use domain::{
    DataSource, DisclosureSource, DomainEvent, InformationSource, Nanosecond, NewsSource,
    SpanSeconds, TimeSpan, Timestamp,
};
use std::fmt::{self, Debug, Formatter};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CollectionDue {
    Now,
    At(Timestamp),
    Off,
}

#[must_use]
pub fn collection_due(
    cadence: CollectionCadence,
    last_attempt: Option<Timestamp>,
    now: Timestamp,
) -> CollectionDue {
    let CollectionCadence::Every(minutes) = cadence else {
        return CollectionDue::Off;
    };
    let Some(last) = last_attempt else {
        return CollectionDue::Now;
    };
    let seconds = u64::from(*minutes.as_u16()).saturating_mul(60);
    let next = TimeSpan::try_new(SpanSeconds::new(seconds), Nanosecond::new(0))
        .ok()
        .and_then(|span| last.checked_add(span).ok());
    match next {
        Some(at) if at > now => CollectionDue::At(at),
        Some(_) | None => CollectionDue::Now,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct CollectedCount(u32);

impl CollectedCount {
    #[must_use]
    pub const fn as_u32(&self) -> &u32 {
        &self.0
    }

    const fn incremented(self) -> Self {
        Self(self.0.saturating_add(1))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeedRoundOutcome {
    Collected,
    NoData,
    NotConfigured,
    SourceFailed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeedRound {
    pub source: DataSource,
    pub outcome: FeedRoundOutcome,
    pub cursor: FeedCursor,
    pub news: CollectedCount,
    pub disclosures: CollectedCount,
}

#[must_use]
pub const fn feed_data_source(source: &FeedSource) -> DataSource {
    match source {
        FeedSource::News(news) => match news {
            NewsSource::Hankyung => DataSource::Hankyung,
            NewsSource::Maekyung => DataSource::Maekyung,
            NewsSource::Yonhap => DataSource::Yonhap,
            NewsSource::Edaily => DataSource::Edaily,
            NewsSource::BankOfKorea => DataSource::BankOfKorea,
            NewsSource::FederalReserve => DataSource::FederalReserve,
            NewsSource::GoogleNews => DataSource::GoogleNews,
            NewsSource::YahooFinance => DataSource::YahooFinance,
        },
        FeedSource::Disclosure(disclosure) => match disclosure {
            DisclosureSource::Kind => DataSource::KindDisclosure,
            DisclosureSource::Dart => DataSource::DartDisclosureApi,
            DisclosureSource::SecEdgar => DataSource::SecEdgar,
        },
    }
}

fn information_source(source: FeedSource) -> InformationSource {
    match source {
        FeedSource::News(news) => InformationSource::News(news),
        FeedSource::Disclosure(disclosure) => InformationSource::Disclosure(disclosure),
    }
}

const NO_TIMES: NoCollectedTimes = NoCollectedTimes;

pub struct CollectFeed<'a, Store, Statuses, Times = NoCollectedTimes> {
    store: &'a Store,
    statuses: &'a Statuses,
    clock: &'a dyn Clock,
    events: &'a EventBus,
    times: &'a Times,
}

impl<Store, Statuses, Times> Debug for CollectFeed<'_, Store, Statuses, Times> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("CollectFeed").finish_non_exhaustive()
    }
}

impl<'a, Store, Statuses> CollectFeed<'a, Store, Statuses>
where
    Store: NewsRepository + DisclosureRepository,
    Statuses: SourceStatusRepository,
{
    #[must_use]
    pub const fn new(
        store: &'a Store,
        statuses: &'a Statuses,
        clock: &'a dyn Clock,
        events: &'a EventBus,
    ) -> Self {
        Self {
            store,
            statuses,
            clock,
            events,
            times: &NO_TIMES,
        }
    }

    #[must_use]
    pub const fn with_times<Times: CollectedTimes>(
        self,
        times: &'a Times,
    ) -> CollectFeed<'a, Store, Statuses, Times> {
        CollectFeed {
            store: self.store,
            statuses: self.statuses,
            clock: self.clock,
            events: self.events,
            times,
        }
    }
}

impl<Store, Statuses, Times> CollectFeed<'_, Store, Statuses, Times>
where
    Store: NewsRepository + DisclosureRepository,
    Statuses: SourceStatusRepository,
    Times: CollectedTimes,
{
    pub async fn collect<Feed: NewsFeed>(
        &self,
        feed: &Feed,
        cursor: FeedCursor,
    ) -> Result<FeedRound, SystemPortError> {
        let feed_source = feed.source();
        let source = feed_data_source(&feed_source);
        let idle = |outcome, kept| FeedRound {
            source,
            outcome,
            cursor: kept,
            news: CollectedCount::default(),
            disclosures: CollectedCount::default(),
        };
        let (attempt, round) = match feed.new_items(cursor.clone()).await {
            Err(error) => (
                SourceAttemptOutcome::Failed(error),
                idle(FeedRoundOutcome::SourceFailed, cursor),
            ),
            Ok(DataAvailability::NotConfigured) => (
                SourceAttemptOutcome::NotConfigured,
                idle(FeedRoundOutcome::NotConfigured, cursor),
            ),
            Ok(DataAvailability::NoData) => (
                SourceAttemptOutcome::NoData,
                idle(FeedRoundOutcome::NoData, cursor),
            ),
            Ok(DataAvailability::Available((observations, next))) => {
                let (news, disclosures) = self.store_all(&observations).await?;
                let _delivery =
                    self.events
                        .publish(DomainEvent::CollectionFinished(information_source(
                            feed_source,
                        )));
                (
                    SourceAttemptOutcome::Succeeded,
                    FeedRound {
                        source,
                        outcome: FeedRoundOutcome::Collected,
                        cursor: next,
                        news,
                        disclosures,
                    },
                )
            }
        };
        let _report = RecordSourceStatus::new(self.statuses, self.clock, self.events)
            .record(source, attempt)
            .await?;
        Ok(round)
    }

    async fn store_all(
        &self,
        observations: &[CollectedObservation],
    ) -> Result<(CollectedCount, CollectedCount), SystemPortError> {
        let mut news = CollectedCount::default();
        let mut disclosures = CollectedCount::default();
        let mut arrived = Vec::new();
        for observation in observations {
            match observation.entry() {
                CollectedEntry::News(item) => {
                    let crate::WriteCompleted = self.store.store(item).await?;
                    news = news.incremented();
                }
                CollectedEntry::Disclosure(item) => {
                    let crate::WriteCompleted = self.store.store_disclosure(item).await?;
                    disclosures = disclosures.incremented();
                    arrived.push(item.key().clone());
                }
            }
            let crate::WriteCompleted = self
                .times
                .record_times(
                    observation.entry(),
                    observation.published_at(),
                    observation.collected_at(),
                )
                .await?;
        }
        for key in arrived {
            let _delivery = self.events.publish(DomainEvent::DisclosureArrived(key));
        }
        Ok((news, disclosures))
    }
}
