use std::future::Future;

use domain::{
    EvidenceRef, InstrumentId, Symbol, Thesis, ThesisError, ThesisPatch, ThesisVersion,
    UntrustedText,
};

use crate::{AiRunId, CollectedItemHandle, SystemPortError, ThesisRepository};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThesisRunMetadata {
    pub model: UntrustedText,
    pub prompt_version: UntrustedText,
}

pub trait ThesisToolPort: Send + Sync {
    fn resolve_target(
        &self,
        run: &AiRunId,
        symbol: &Symbol,
    ) -> impl Future<Output = Result<Option<InstrumentId>, SystemPortError>> + Send;

    fn collected_evidence(
        &self,
        handle: &CollectedItemHandle,
    ) -> impl Future<Output = Result<Option<EvidenceRef>, SystemPortError>> + Send;

    fn run_metadata(
        &self,
        run: &AiRunId,
    ) -> impl Future<Output = Result<ThesisRunMetadata, SystemPortError>> + Send;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ThesisUpdateFailure {
    Rejected(ThesisError),
    Storage(SystemPortError),
}

#[derive(Debug)]
pub struct UpdateThesis<'a, R> {
    repository: &'a R,
}

impl<'a, R: ThesisRepository> UpdateThesis<'a, R> {
    #[must_use]
    pub const fn new(repository: &'a R) -> Self {
        Self { repository }
    }

    pub async fn load(&self, instrument: &InstrumentId) -> Result<Thesis, SystemPortError> {
        match self.repository.load(instrument).await? {
            Some(thesis) => Ok(thesis),
            None => Ok(Thesis::new()),
        }
    }

    pub async fn execute(
        &self,
        instrument: &InstrumentId,
        base: ThesisVersion,
        patch: ThesisPatch,
    ) -> Result<Thesis, ThesisUpdateFailure> {
        let mut thesis = self
            .load(instrument)
            .await
            .map_err(ThesisUpdateFailure::Storage)?;
        thesis
            .apply_batch(base, patch)
            .map_err(ThesisUpdateFailure::Rejected)?;
        self.repository
            .store(instrument, &thesis)
            .await
            .map_err(ThesisUpdateFailure::Storage)?;
        Ok(thesis)
    }
}
