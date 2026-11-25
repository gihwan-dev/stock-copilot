use crate::{InstrumentIdIssuerStore, InstrumentMappingStore, SqliteRepositories, not_opened};
use application::{
    InstrumentIdIssuer, InstrumentMappingRepository, InstrumentMasterAuthority,
    InstrumentMasterObservation, InstrumentMasterSession, MappingStoreError, MasterNotReady,
    SystemPort, SystemPortError,
};
use domain::{
    InstrumentId, InstrumentMappingRecord, InstrumentSource, IssuedInstrumentId, KnownMarket,
    MappingAssignment,
};

const REPOSITORY: SystemPort = SystemPort::Repository;

#[derive(Debug, Clone, Copy)]
pub struct SqliteMasterSession<'a>(&'a SqliteRepositories);

impl InstrumentMappingRepository for SqliteMasterSession<'_> {
    async fn resolve(&self, source: &InstrumentSource) -> Result<InstrumentId, MappingStoreError> {
        let guard = self.0.shared().read().await;
        let database = guard
            .as_ref()
            .ok_or(MappingStoreError::Storage(not_opened(REPOSITORY)))?;
        InstrumentMappingStore::new(database).resolve(source).await
    }

    async fn lookup(
        &self,
        source: &InstrumentSource,
    ) -> Result<InstrumentMappingRecord, MappingStoreError> {
        let guard = self.0.shared().read().await;
        let database = guard
            .as_ref()
            .ok_or(MappingStoreError::Storage(not_opened(REPOSITORY)))?;
        InstrumentMappingStore::new(database).lookup(source).await
    }

    async fn for_id(
        &self,
        id: &InstrumentId,
    ) -> Result<Vec<InstrumentMappingRecord>, MappingStoreError> {
        let guard = self.0.shared().read().await;
        let database = guard
            .as_ref()
            .ok_or(MappingStoreError::Storage(not_opened(REPOSITORY)))?;
        InstrumentMappingStore::new(database).for_id(id).await
    }

    async fn ensure(
        &self,
        source: &InstrumentSource,
        candidate: IssuedInstrumentId,
    ) -> Result<MappingAssignment, MappingStoreError> {
        let guard = self.0.shared().read().await;
        let database = guard
            .as_ref()
            .ok_or(MappingStoreError::Storage(not_opened(REPOSITORY)))?;
        InstrumentMappingStore::new(database)
            .ensure(source, candidate)
            .await
    }

    async fn retire(&self, source: &InstrumentSource) -> Result<InstrumentId, MappingStoreError> {
        let guard = self.0.shared().read().await;
        let database = guard
            .as_ref()
            .ok_or(MappingStoreError::Storage(not_opened(REPOSITORY)))?;
        InstrumentMappingStore::new(database).retire(source).await
    }
}

impl InstrumentMasterSession for SqliteMasterSession<'_> {
    async fn verify_observation(
        &self,
        _observations: &[InstrumentMasterObservation],
    ) -> Result<(), MasterNotReady> {
        let guard = self.0.shared().read().await;
        guard
            .as_ref()
            .map(|_database| ())
            .ok_or(MasterNotReady::Storage(not_opened(REPOSITORY)))
    }
}

impl InstrumentMasterAuthority for SqliteRepositories {
    type Session<'a> = SqliteMasterSession<'a>;

    async fn begin(&self, _market: KnownMarket) -> Result<Self::Session<'_>, MasterNotReady> {
        let guard = self.shared().read().await;
        guard
            .as_ref()
            .map(|_database| SqliteMasterSession(self))
            .ok_or(MasterNotReady::Storage(not_opened(REPOSITORY)))
    }
}

impl InstrumentIdIssuer for SqliteRepositories {
    async fn issue(&self) -> Result<IssuedInstrumentId, SystemPortError> {
        let guard = self.shared().read().await;
        let database = guard.as_ref().ok_or(not_opened(REPOSITORY))?;
        InstrumentIdIssuerStore::new(database).issue().await
    }
}
