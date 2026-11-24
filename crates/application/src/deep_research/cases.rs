use crate::ModelSelection;
use domain::{
    CitedEvidence, ClaimSupport, EvidenceBundle, EvidenceItem, ResearchClaim, ResearchDraft,
    ResearchStance, VerifiedClaim,
};
use std::future::Future;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnalysisFailure {
    Interrupted,
    Unavailable,
    Rejected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResearchCaseError {
    Analysis(AnalysisFailure),
    StanceMismatch,
}

pub trait ResearchAnalyst: Send + Sync {
    fn draft(
        &self,
        stance: ResearchStance,
        model: &ModelSelection,
        ledger: &EvidenceBundle,
    ) -> impl Future<Output = Result<ResearchDraft, AnalysisFailure>> + Send;
}

pub trait ClaimVerifier: Send + Sync {
    fn verify(
        &self,
        claim: &ResearchClaim,
        evidence: &[EvidenceItem],
    ) -> impl Future<Output = Result<ClaimSupport, AnalysisFailure>> + Send;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResearchModels {
    pub bull: ModelSelection,
    pub bear: ModelSelection,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResearchCases {
    pub bull: ResearchDraft,
    pub bear: ResearchDraft,
}

impl ResearchCases {
    #[must_use]
    pub fn claims(&self) -> Vec<(ResearchStance, &ResearchClaim)> {
        self.bull
            .claims()
            .iter()
            .map(|claim| (ResearchStance::Bull, claim))
            .chain(
                self.bear
                    .claims()
                    .iter()
                    .map(|claim| (ResearchStance::Bear, claim)),
            )
            .collect()
    }
}

#[derive(Debug)]
pub struct DraftResearchCases<'a, Analyst> {
    pub analyst: &'a Analyst,
    pub models: &'a ResearchModels,
}

impl<Analyst: ResearchAnalyst> DraftResearchCases<'_, Analyst> {
    async fn one(
        &self,
        stance: ResearchStance,
        model: &ModelSelection,
        ledger: &EvidenceBundle,
    ) -> Result<ResearchDraft, ResearchCaseError> {
        let draft = self
            .analyst
            .draft(stance, model, ledger)
            .await
            .map_err(ResearchCaseError::Analysis)?;
        if draft.stance() == stance {
            Ok(draft)
        } else {
            Err(ResearchCaseError::StanceMismatch)
        }
    }

    pub async fn execute(
        &self,
        ledger: &EvidenceBundle,
    ) -> Result<ResearchCases, ResearchCaseError> {
        let bull = self
            .one(ResearchStance::Bull, &self.models.bull, ledger)
            .await?;
        let bear = self
            .one(ResearchStance::Bear, &self.models.bear, ledger)
            .await?;
        Ok(ResearchCases { bull, bear })
    }
}

#[derive(Debug)]
pub struct VerifyResearchClaims<'a, Verifier> {
    pub verifier: &'a Verifier,
}

impl<Verifier: ClaimVerifier> VerifyResearchClaims<'_, Verifier> {
    pub async fn execute(
        &self,
        ledger: &EvidenceBundle,
        cases: &ResearchCases,
    ) -> Result<Vec<VerifiedClaim>, AnalysisFailure> {
        let mut verified = Vec::new();
        for (stance, claim) in cases.claims() {
            let result = match claim.cited_in(ledger) {
                CitedEvidence::Missing => VerifiedClaim::missing_evidence(stance, claim.clone()),
                CitedEvidence::Found(evidence) => {
                    let support = self.verifier.verify(claim, &evidence).await?;
                    VerifiedClaim::checked(stance, claim.clone(), support)
                }
            };
            verified.push(result);
        }
        Ok(verified)
    }
}
