use crate::{
    EvidenceRef, IssuedEstimateTask, IssuedEstimateTaskId, ModelIdentityPart, ScorecardPromptId,
    SignalSubmission, UntrustedText,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EstimateSourceStatus {
    Missing,
    External,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EstimateRecordError {
    TaskMismatch,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EstimatePrompt {
    pub id: ScorecardPromptId,
    pub version: ModelIdentityPart,
    pub hash: PromptHash,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromptHash(String);
impl PromptHash {
    pub fn try_new(value: &str) -> Result<Self, EstimateMetadataError> {
        if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(EstimateMetadataError::InvalidHash);
        }
        Ok(Self(value.to_ascii_lowercase()))
    }
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EstimateMetadataError {
    InvalidHash,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceBundleId(crate::CalibrationEstimateId);
impl EvidenceBundleId {
    #[must_use]
    pub const fn new(value: crate::CalibrationEstimateId) -> Self {
        Self(value)
    }
    #[must_use]
    pub const fn as_estimate_id(&self) -> &crate::CalibrationEstimateId {
        &self.0
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EstimateMetadata<Effort> {
    pub effort: Effort,
    pub cli_version: ModelIdentityPart,
    pub prompt: EstimatePrompt,
    pub evidence_bundle: EvidenceBundleId,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EstimateNarrative {
    pub rationale: UntrustedText,
    pub counterargument: UntrustedText,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EstimateSubmissionRecord<Run, Effort> {
    task_id: IssuedEstimateTaskId,
    run: Run,
    submission: SignalSubmission,
    narrative: EstimateNarrative,
    metadata: EstimateMetadata<Effort>,
}
impl<Run: Clone, Effort> EstimateSubmissionRecord<Run, Effort> {
    pub fn create(
        issued: &IssuedEstimateTask<Run>,
        submission: SignalSubmission,
        narrative: EstimateNarrative,
        metadata: EstimateMetadata<Effort>,
    ) -> Result<Self, EstimateRecordError> {
        if submission.task_key().task() != issued.task().task() {
            return Err(EstimateRecordError::TaskMismatch);
        }
        Ok(Self {
            task_id: issued.id().clone(),
            run: issued.run().clone(),
            submission,
            narrative,
            metadata,
        })
    }
    #[must_use]
    pub const fn task_id(&self) -> &IssuedEstimateTaskId {
        &self.task_id
    }
    #[must_use]
    pub const fn run(&self) -> &Run {
        &self.run
    }
    #[must_use]
    pub const fn submission(&self) -> &SignalSubmission {
        &self.submission
    }
    #[must_use]
    pub const fn narrative(&self) -> &EstimateNarrative {
        &self.narrative
    }
    #[must_use]
    pub const fn metadata(&self) -> &EstimateMetadata<Effort> {
        &self.metadata
    }
    #[must_use]
    pub fn source_status(&self) -> EstimateSourceStatus {
        match self.submission.evidence() {
            Some(evidence)
                if evidence.as_items().iter().any(|item| match item {
                    EvidenceRef::Web(_)
                    | EvidenceRef::News(_, _)
                    | EvidenceRef::Disclosure(_, _) => true,
                    EvidenceRef::Tool(_) => false,
                }) =>
            {
                EstimateSourceStatus::External
            }
            _ => EstimateSourceStatus::Missing,
        }
    }
}
