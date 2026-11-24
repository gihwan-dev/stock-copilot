use crate::{
    AiRunId, DeliveryReport, EstimateSubmissionInput, EstimateSubmissionRepository, SubmitEstimate,
    SubmitEstimateError, SystemPortError, ThesisRepository,
};
use domain::{
    EvidenceBundle, InstrumentId, PolicyViolation, ResearchCount, ResearchSourceWarning,
    ResearchTally, Thesis, ThesisError, ThesisPatch, ThesisStatus, VerifiedClaim,
    research_source_warnings, tally_research_claims,
};
use std::future::Future;

use super::AnalysisFailure;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResearchConclusion {
    pub thesis: Option<ThesisPatch>,
    pub estimates: Vec<EstimateSubmissionInput>,
}

pub trait ResearchConcluder: Send + Sync {
    fn conclude(
        &self,
        ledger: &EvidenceBundle,
        verified: &[VerifiedClaim],
    ) -> impl Future<Output = Result<ResearchConclusion, AnalysisFailure>> + Send;
}

pub trait ResearchPrincipleCheck: Send + Sync {
    fn violations(
        &self,
        instrument: &InstrumentId,
    ) -> impl Future<Output = Result<Vec<PolicyViolation>, SystemPortError>> + Send;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResearchOutcome {
    pub tally: ResearchTally,
    pub source_warnings: Vec<ResearchSourceWarning>,
    pub principle_violations: Vec<PolicyViolation>,
    pub thesis_status: ThesisStatus,
    pub pending_changes: ResearchCount,
    pub deliveries: Vec<DeliveryReport>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FinishResearchError {
    TooManyEstimates,
    Principles(SystemPortError),
    ThesisStorage(SystemPortError),
    Thesis(ThesisError),
    Estimate(SubmitEstimateError),
}

#[derive(Debug)]
pub struct FinishResearch<'a, Theses, Estimates, Principles> {
    pub theses: &'a Theses,
    pub estimates: &'a SubmitEstimate<'a, Estimates>,
    pub principles: &'a Principles,
}

impl<Theses, Estimates, Principles> FinishResearch<'_, Theses, Estimates, Principles>
where
    Theses: ThesisRepository,
    Estimates: EstimateSubmissionRepository,
    Principles: ResearchPrincipleCheck,
{
    pub async fn execute(
        &self,
        run: &AiRunId,
        ledger: &EvidenceBundle,
        verified: &[VerifiedClaim],
        conclusion: &ResearchConclusion,
    ) -> Result<ResearchOutcome, FinishResearchError> {
        if conclusion.estimates.len() > 3 {
            return Err(FinishResearchError::TooManyEstimates);
        }
        let instrument = ledger.instrument();
        let principle_violations = self
            .principles
            .violations(instrument)
            .await
            .map_err(FinishResearchError::Principles)?;
        let mut thesis = match self
            .theses
            .load(instrument)
            .await
            .map_err(FinishResearchError::ThesisStorage)?
        {
            Some(saved) => saved,
            None => Thesis::new(),
        };
        if let Some(patch) = &conclusion.thesis {
            thesis
                .apply_batch(thesis.version(), patch.clone())
                .map_err(FinishResearchError::Thesis)?;
            self.theses
                .store(instrument, &thesis)
                .await
                .map_err(FinishResearchError::ThesisStorage)?;
        }
        let deliveries = self
            .estimates
            .execute(run, conclusion.estimates.clone())
            .await
            .map_err(FinishResearchError::Estimate)?;
        Ok(ResearchOutcome {
            tally: tally_research_claims(verified),
            source_warnings: research_source_warnings(ledger, verified),
            principle_violations,
            thesis_status: thesis.status(),
            pending_changes: ResearchCount::of_pending(&thesis),
            deliveries,
        })
    }
}
