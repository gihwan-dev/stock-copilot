use crate::{CollectedEntry, SystemPortError, WriteCompleted};
use domain::{Disclosure, DisclosureKey, InstrumentId, NewsSource, NormalizedUrl, UntrustedText};
use std::future::Future;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InformationKey {
    News(NewsSource, NormalizedUrl),
    Disclosure(DisclosureKey),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SearchLimit(usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchLimitError {
    OutOfRange,
}
impl SearchLimit {
    pub fn try_new(value: usize) -> Result<Self, SearchLimitError> {
        if !(1..=100).contains(&value) {
            return Err(SearchLimitError::OutOfRange);
        }
        Ok(Self(value))
    }
    #[must_use]
    pub const fn as_usize(&self) -> &usize {
        &self.0
    }
}

pub trait DisclosureRepository: Send + Sync {
    fn store_disclosure(
        &self,
        item: &Disclosure,
    ) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send;
    fn disclosure(
        &self,
        key: &DisclosureKey,
    ) -> impl Future<Output = Result<Option<Disclosure>, SystemPortError>> + Send;
    fn disclosures_for_instrument(
        &self,
        instrument: &InstrumentId,
    ) -> impl Future<Output = Result<Vec<Disclosure>, SystemPortError>> + Send;
}
pub trait CandidateNewsRepository: Send + Sync {
    fn retain_candidate_news(
        &self,
        item: &domain::NewsItem,
        instrument: &InstrumentId,
        published_at: domain::Timestamp,
        collected_at: domain::Timestamp,
    ) -> impl Future<Output = Result<crate::ObservedCollectedItem, SystemPortError>> + Send;
}
pub trait InformationLinks: Send + Sync {
    fn set_instruments(
        &self,
        item: &InformationKey,
        instruments: &[InstrumentId],
    ) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send;
}
pub trait InformationSearch: Send + Sync {
    fn search(
        &self,
        query: &UntrustedText,
        limit: SearchLimit,
    ) -> impl Future<Output = Result<Vec<CollectedEntry>, SystemPortError>> + Send;
    fn rebuild_index(&self)
    -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send;
}
