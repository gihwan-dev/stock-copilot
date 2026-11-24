use crate::{
    AiRunId, AuditLog, Clock, EstimateSubmissionRepository, SystemPortError, ThesisRepository,
    WriteCompleted,
};
use domain::{
    DisplayInstrumentName, EvidenceBundle, InstrumentId, ResearchTopic, VerifiedClaim,
    WebSearchSensitiveContext,
};
use std::future::Future;

use super::{
    AnalysisFailure, ClaimVerifier, DraftResearchCases, FinishResearch, FinishResearchError,
    GatherEvidenceError, GatherResearchEvidence, GatheredEvidence, ResearchAnalyst,
    ResearchCaseError, ResearchCases, ResearchConcluder, ResearchConclusion,
    ResearchEvidenceSource, ResearchOutcome, ResearchPrincipleCheck, ResearchWebCollector,
    ResearchWebStore, VerifyResearchClaims,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResearchCheckpoint {
    Gathered(GatheredEvidence),
    Cased(GatheredEvidence, ResearchCases),
    Verified(GatheredEvidence, Vec<VerifiedClaim>),
    Concluded(GatheredEvidence, Vec<VerifiedClaim>, ResearchConclusion),
    Finished(ResearchOutcome),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResearchStage {
    Draft,
    Verify,
    Conclude,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeepResearchRun {
    Finished(ResearchOutcome),
    Paused(ResearchStage),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeepResearchError {
    Gather(GatherEvidenceError),
    Cases(ResearchCaseError),
    Verify(AnalysisFailure),
    Conclude(AnalysisFailure),
    Finish(FinishResearchError),
    Checkpoint(SystemPortError),
}

pub trait ResearchCheckpointStore: Send + Sync {
    fn load(
        &self,
        run: &AiRunId,
    ) -> impl Future<Output = Result<Option<ResearchCheckpoint>, SystemPortError>> + Send;
    fn save(
        &self,
        run: &AiRunId,
        checkpoint: &ResearchCheckpoint,
    ) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send;
}

pub trait DeepResearchStages: Send + Sync {
    fn gather(&self) -> impl Future<Output = Result<GatheredEvidence, DeepResearchError>> + Send;
    fn cases(
        &self,
        ledger: &EvidenceBundle,
    ) -> impl Future<Output = Result<ResearchCases, DeepResearchError>> + Send;
    fn verify(
        &self,
        ledger: &EvidenceBundle,
        cases: &ResearchCases,
    ) -> impl Future<Output = Result<Vec<VerifiedClaim>, DeepResearchError>> + Send;
    fn conclude(
        &self,
        ledger: &EvidenceBundle,
        verified: &[VerifiedClaim],
    ) -> impl Future<Output = Result<ResearchConclusion, DeepResearchError>> + Send;
    fn finish(
        &self,
        run: &AiRunId,
        ledger: &EvidenceBundle,
        verified: &[VerifiedClaim],
        conclusion: &ResearchConclusion,
    ) -> impl Future<Output = Result<ResearchOutcome, DeepResearchError>> + Send;
}

#[derive(Debug)]
pub struct RunDeepResearch<'a, Stages, Checkpoints> {
    pub stages: &'a Stages,
    pub checkpoints: &'a Checkpoints,
}

const fn interrupted(error: &DeepResearchError) -> Option<ResearchStage> {
    match error {
        DeepResearchError::Cases(ResearchCaseError::Analysis(AnalysisFailure::Interrupted)) => {
            Some(ResearchStage::Draft)
        }
        DeepResearchError::Verify(AnalysisFailure::Interrupted) => Some(ResearchStage::Verify),
        DeepResearchError::Conclude(AnalysisFailure::Interrupted) => Some(ResearchStage::Conclude),
        DeepResearchError::Gather(_)
        | DeepResearchError::Cases(_)
        | DeepResearchError::Verify(_)
        | DeepResearchError::Conclude(_)
        | DeepResearchError::Finish(_)
        | DeepResearchError::Checkpoint(_) => None,
    }
}

impl<Stages: DeepResearchStages, Checkpoints: ResearchCheckpointStore>
    RunDeepResearch<'_, Stages, Checkpoints>
{
    async fn advance(
        &self,
        run: &AiRunId,
        checkpoint: Option<ResearchCheckpoint>,
    ) -> Result<ResearchCheckpoint, DeepResearchError> {
        let next = match checkpoint {
            None => ResearchCheckpoint::Gathered(self.stages.gather().await?),
            Some(ResearchCheckpoint::Gathered(gathered)) => {
                let cases = self.stages.cases(&gathered.bundle).await?;
                ResearchCheckpoint::Cased(gathered, cases)
            }
            Some(ResearchCheckpoint::Cased(gathered, cases)) => {
                let verified = self.stages.verify(&gathered.bundle, &cases).await?;
                ResearchCheckpoint::Verified(gathered, verified)
            }
            Some(ResearchCheckpoint::Verified(gathered, verified)) => {
                let conclusion = self.stages.conclude(&gathered.bundle, &verified).await?;
                ResearchCheckpoint::Concluded(gathered, verified, conclusion)
            }
            Some(ResearchCheckpoint::Concluded(gathered, verified, conclusion)) => {
                ResearchCheckpoint::Finished(
                    self.stages
                        .finish(run, &gathered.bundle, &verified, &conclusion)
                        .await?,
                )
            }
            Some(finished @ ResearchCheckpoint::Finished(_)) => return Ok(finished),
        };
        self.checkpoints
            .save(run, &next)
            .await
            .map_err(DeepResearchError::Checkpoint)?;
        Ok(next)
    }

    pub async fn execute(&self, run: &AiRunId) -> Result<DeepResearchRun, DeepResearchError> {
        let mut checkpoint = self
            .checkpoints
            .load(run)
            .await
            .map_err(DeepResearchError::Checkpoint)?;
        loop {
            if let Some(ResearchCheckpoint::Finished(outcome)) = checkpoint {
                return Ok(DeepResearchRun::Finished(outcome));
            }
            match self.advance(run, checkpoint).await {
                Ok(next) => checkpoint = Some(next),
                Err(error) => {
                    return match interrupted(&error) {
                        Some(stage) => Ok(DeepResearchRun::Paused(stage)),
                        None => Err(error),
                    };
                }
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeepResearchRequest {
    pub instrument: InstrumentId,
    pub name: DisplayInstrumentName,
    pub topics: Vec<ResearchTopic>,
    pub context: WebSearchSensitiveContext,
}

#[derive(Debug)]
pub struct DeepResearchPipeline<
    'a,
    Source,
    Audit: AuditLog,
    Collector,
    Store,
    Time,
    Analyst,
    Verifier,
    Concluder,
    Theses,
    Estimates,
    Principles,
> {
    pub request: &'a DeepResearchRequest,
    pub gather: GatherResearchEvidence<'a, Source, Audit, Collector, Store, Time>,
    pub cases: DraftResearchCases<'a, Analyst>,
    pub verify: VerifyResearchClaims<'a, Verifier>,
    pub concluder: &'a Concluder,
    pub finish: FinishResearch<'a, Theses, Estimates, Principles>,
}

impl<
    Source,
    Audit,
    Collector,
    Store,
    Time,
    Analyst,
    Verifier,
    Concluder,
    Theses,
    Estimates,
    Principles,
> DeepResearchStages
    for DeepResearchPipeline<
        '_,
        Source,
        Audit,
        Collector,
        Store,
        Time,
        Analyst,
        Verifier,
        Concluder,
        Theses,
        Estimates,
        Principles,
    >
where
    Source: ResearchEvidenceSource,
    Audit: AuditLog,
    Collector: ResearchWebCollector,
    Store: ResearchWebStore,
    Time: Clock,
    Analyst: ResearchAnalyst,
    Verifier: ClaimVerifier,
    Concluder: ResearchConcluder,
    Theses: ThesisRepository,
    Estimates: EstimateSubmissionRepository,
    Principles: ResearchPrincipleCheck,
{
    async fn gather(&self) -> Result<GatheredEvidence, DeepResearchError> {
        self.gather
            .execute(
                &self.request.instrument,
                &self.request.name,
                &self.request.topics,
                &self.request.context,
            )
            .await
            .map_err(DeepResearchError::Gather)
    }
    async fn cases(&self, ledger: &EvidenceBundle) -> Result<ResearchCases, DeepResearchError> {
        self.cases
            .execute(ledger)
            .await
            .map_err(DeepResearchError::Cases)
    }
    async fn verify(
        &self,
        ledger: &EvidenceBundle,
        cases: &ResearchCases,
    ) -> Result<Vec<VerifiedClaim>, DeepResearchError> {
        self.verify
            .execute(ledger, cases)
            .await
            .map_err(DeepResearchError::Verify)
    }
    async fn conclude(
        &self,
        ledger: &EvidenceBundle,
        verified: &[VerifiedClaim],
    ) -> Result<ResearchConclusion, DeepResearchError> {
        self.concluder
            .conclude(ledger, verified)
            .await
            .map_err(DeepResearchError::Conclude)
    }
    async fn finish(
        &self,
        run: &AiRunId,
        ledger: &EvidenceBundle,
        verified: &[VerifiedClaim],
        conclusion: &ResearchConclusion,
    ) -> Result<ResearchOutcome, DeepResearchError> {
        self.finish
            .execute(run, ledger, verified, conclusion)
            .await
            .map_err(DeepResearchError::Finish)
    }
}
