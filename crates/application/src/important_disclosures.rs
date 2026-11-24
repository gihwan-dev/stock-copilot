use crate::{Clock, SystemPortError};
use domain::{
    DisclosureGrade, DisclosureKey, InstrumentId, Nanosecond, NormalizedUrl, SpanSeconds, TimeSpan,
    Timestamp, UntrustedText,
};
use std::cmp::Reverse;
use std::collections::BTreeSet;
use std::future::Future;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisclosureCardFact {
    pub key: DisclosureKey,
    pub instrument: InstrumentId,
    pub title: UntrustedText,
    pub url: NormalizedUrl,
    pub grade: DisclosureGrade,
    pub recorded_at: Timestamp,
}

pub trait DisclosureTriggerLog: Send + Sync {
    fn recorded_since(
        &self,
        since: Timestamp,
    ) -> impl Future<Output = Result<Vec<DisclosureCardFact>, SystemPortError>> + Send;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DisclosureCardLevel {
    High,
    Urgent,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportantDisclosureCard {
    key: DisclosureKey,
    instrument: InstrumentId,
    title: UntrustedText,
    url: NormalizedUrl,
    level: DisclosureCardLevel,
    recorded_at: Timestamp,
}

impl ImportantDisclosureCard {
    #[must_use]
    pub const fn key(&self) -> &DisclosureKey {
        &self.key
    }

    #[must_use]
    pub const fn instrument(&self) -> &InstrumentId {
        &self.instrument
    }

    #[must_use]
    pub const fn title(&self) -> &UntrustedText {
        &self.title
    }

    #[must_use]
    pub const fn url(&self) -> &NormalizedUrl {
        &self.url
    }

    #[must_use]
    pub const fn level(&self) -> DisclosureCardLevel {
        self.level
    }

    #[must_use]
    pub const fn recorded_at(&self) -> Timestamp {
        self.recorded_at
    }
}

const fn card_level(grade: DisclosureGrade) -> Option<DisclosureCardLevel> {
    match grade {
        DisclosureGrade::Urgent => Some(DisclosureCardLevel::Urgent),
        DisclosureGrade::High => Some(DisclosureCardLevel::High),
        DisclosureGrade::Normal | DisclosureGrade::Low => None,
    }
}

#[derive(Debug)]
pub struct GetImportantDisclosureCards<'a, Log, Time> {
    pub log: &'a Log,
    pub clock: &'a Time,
}

impl<Log: DisclosureTriggerLog, Time: Clock> GetImportantDisclosureCards<'_, Log, Time> {
    pub async fn execute(
        &self,
        held: &[InstrumentId],
    ) -> Result<Vec<ImportantDisclosureCard>, SystemPortError> {
        let now = self.clock.now();
        let window = TimeSpan::try_new(SpanSeconds::new(86_400), Nanosecond::new(0));
        let since = match window.and_then(|span| now.checked_sub(span)) {
            Ok(start) => start,
            Err(_error) => now,
        };
        let facts = self.log.recorded_since(since).await?;
        let holdings: BTreeSet<&InstrumentId> = held.iter().collect();
        let mut cards: Vec<ImportantDisclosureCard> = facts
            .into_iter()
            .filter(|fact| holdings.contains(&fact.instrument))
            .filter(|fact| since <= fact.recorded_at && fact.recorded_at <= now)
            .filter_map(|fact| {
                card_level(fact.grade).map(|level| ImportantDisclosureCard {
                    key: fact.key,
                    instrument: fact.instrument,
                    title: fact.title,
                    url: fact.url,
                    level,
                    recorded_at: fact.recorded_at,
                })
            })
            .collect();
        cards.sort_by_key(|card| (Reverse(card.level), Reverse(card.recorded_at)));
        let mut seen = BTreeSet::new();
        cards.retain(|card| seen.insert(card.key.clone()));
        cards.truncate(5);
        Ok(cards)
    }
}
