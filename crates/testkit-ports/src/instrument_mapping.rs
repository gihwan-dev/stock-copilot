use crate::script::State;
use application::{InstrumentMappingRepository, MappingStoreError};
use domain::{InstrumentId, InstrumentMappings, InstrumentSource, MappingAssignment};
use std::{
    future::{Future, poll_fn},
    task::Poll,
};

#[derive(Debug, Default)]
pub struct MemoryInstrumentMappingRepository {
    mappings: State<InstrumentMappings>,
}

impl InstrumentMappingRepository for MemoryInstrumentMappingRepository {
    fn resolve(
        &self,
        source: &InstrumentSource,
    ) -> impl Future<Output = Result<InstrumentId, MappingStoreError>> + Send {
        poll_fn(move |_| {
            Poll::Ready(
                self.mappings
                    .write()
                    .resolve(source)
                    .map_err(MappingStoreError::Mapping),
            )
        })
    }

    fn ensure(
        &self,
        source: &InstrumentSource,
        candidate: &InstrumentId,
    ) -> impl Future<Output = Result<MappingAssignment, MappingStoreError>> + Send {
        poll_fn(move |_| {
            Poll::Ready(
                self.mappings
                    .write()
                    .ensure(source, candidate)
                    .map_err(MappingStoreError::Mapping),
            )
        })
    }

    fn retire(
        &self,
        source: &InstrumentSource,
    ) -> impl Future<Output = Result<InstrumentId, MappingStoreError>> + Send {
        poll_fn(move |_| {
            Poll::Ready(
                self.mappings
                    .write()
                    .retire(source)
                    .map_err(MappingStoreError::Mapping),
            )
        })
    }
}
