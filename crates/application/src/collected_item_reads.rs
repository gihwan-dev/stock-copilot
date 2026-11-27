use crate::{
    CollectedEntry, CollectedItemHandle, PublicationTime, SystemPortError, WriteCompleted,
};
use domain::{InstrumentId, Timestamp};
use std::future::Future;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CollectedKind {
    News,
    Disclosure,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservedCollectedItem {
    pub handle: CollectedItemHandle,
    pub entry: CollectedEntry,
    pub published_at: PublicationTime,
    pub collected_at: Timestamp,
    pub instruments: Vec<InstrumentId>,
}

pub trait CollectedTimes: Send + Sync {
    fn record_times(
        &self,
        entry: &CollectedEntry,
        published_at: PublicationTime,
        collected_at: Timestamp,
    ) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send;
}

pub trait CollectedItemReads: Send + Sync {
    fn citation_digest(&self, _entry: &CollectedEntry) -> Option<crate::CitationDigest> {
        None
    }

    fn collected_since(
        &self,
        kind: CollectedKind,
        since: Timestamp,
    ) -> impl Future<Output = Result<Vec<ObservedCollectedItem>, SystemPortError>> + Send;

    fn collected_item(
        &self,
        handle: &CollectedItemHandle,
    ) -> impl Future<Output = Result<Option<ObservedCollectedItem>, SystemPortError>> + Send;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct NoCollectedTimes;

impl CollectedTimes for NoCollectedTimes {
    fn record_times(
        &self,
        _entry: &CollectedEntry,
        _published_at: PublicationTime,
        _collected_at: Timestamp,
    ) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send {
        std::future::ready(Ok(WriteCompleted))
    }
}
