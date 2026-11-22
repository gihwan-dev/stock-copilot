use crate::SystemPortError;
use domain::{InstrumentId, InstrumentMappingError, InstrumentSource, MappingAssignment};
use std::future::Future;

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

    fn ensure(
        &self,
        source: &InstrumentSource,
        candidate: &InstrumentId,
    ) -> impl Future<Output = Result<MappingAssignment, MappingStoreError>> + Send;

    fn retire(
        &self,
        source: &InstrumentSource,
    ) -> impl Future<Output = Result<InstrumentId, MappingStoreError>> + Send;
}
