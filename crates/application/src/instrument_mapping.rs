use crate::SystemPortError;
use domain::{
    ConfirmedIdentifierChange, ConfirmedLinkCorrection, ConfirmedRetirementCorrection,
    InstrumentId, InstrumentMappingError, InstrumentMappingRecord, InstrumentSource,
    IssuedInstrumentId, MappingAssignment, MappingHistoryEvent, MappingTransitionError,
};
use std::future::Future;

pub trait InstrumentIdIssuer: Send + Sync {
    fn issue(&self) -> impl Future<Output = Result<IssuedInstrumentId, SystemPortError>> + Send;
}

pub trait InstrumentMasterAuthority: Send + Sync {
    type Session<'a>: InstrumentMasterSession + Send + Sync + 'a
    where
        Self: 'a;

    fn begin(
        &self,
        market: domain::KnownMarket,
    ) -> impl Future<Output = Result<Self::Session<'_>, crate::MasterNotReady>> + Send;
}

pub trait InstrumentMasterSession: InstrumentMappingRepository {
    fn verify_observation(
        &self,
        observations: &[InstrumentMasterObservation],
    ) -> impl Future<Output = Result<(), crate::MasterNotReady>> + Send;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstrumentMasterObservation {
    pub source: InstrumentSource,
    pub listing_market: domain::MasterCode,
    pub listing_status: domain::ListingStatus,
    pub observed_at: domain::Timestamp,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MappingStoreError {
    Mapping(InstrumentMappingError),
    Storage(SystemPortError),
}

pub trait InstrumentMappingRepository: Send + Sync {
    fn resolve(
        &self,
        source: &InstrumentSource,
    ) -> impl Future<Output = Result<InstrumentId, MappingStoreError>> + Send;

    fn lookup(
        &self,
        source: &InstrumentSource,
    ) -> impl Future<Output = Result<InstrumentMappingRecord, MappingStoreError>> + Send;

    fn for_id(
        &self,
        id: &InstrumentId,
    ) -> impl Future<Output = Result<Vec<InstrumentMappingRecord>, MappingStoreError>> + Send;

    fn ensure(
        &self,
        source: &InstrumentSource,
        candidate: IssuedInstrumentId,
    ) -> impl Future<Output = Result<MappingAssignment, MappingStoreError>> + Send;

    fn retire(
        &self,
        source: &InstrumentSource,
    ) -> impl Future<Output = Result<InstrumentId, MappingStoreError>> + Send;
}

pub trait InstrumentMappingHistoryRepository: Send + Sync {
    fn history(
        &self,
    ) -> impl Future<Output = Result<Vec<MappingHistoryEvent>, SystemPortError>> + Send;

    fn history_for_id(
        &self,
        id: &InstrumentId,
    ) -> impl Future<Output = Result<Vec<MappingHistoryEvent>, SystemPortError>> + Send;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MappingTransitionStoreError {
    Transition(MappingTransitionError),
    Storage(SystemPortError),
}

pub trait InstrumentMappingCorrectionRepository: Send + Sync {
    fn link_confirmed_change(
        &self,
        change: ConfirmedIdentifierChange,
    ) -> impl Future<Output = Result<InstrumentId, MappingTransitionStoreError>> + Send;

    fn correct_retirement(
        &self,
        correction: ConfirmedRetirementCorrection,
    ) -> impl Future<Output = Result<InstrumentId, MappingTransitionStoreError>> + Send;

    fn correct_link(
        &self,
        correction: ConfirmedLinkCorrection,
    ) -> impl Future<Output = Result<InstrumentId, MappingTransitionStoreError>> + Send;
}
