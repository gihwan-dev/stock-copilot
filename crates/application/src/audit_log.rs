use crate::{OrderConfirmation, RecordOutcome, RiskChangeConfirmation, SystemPortError};
use domain::{
    ClientOrderId, InstrumentId, ProposalId, RiskChange, Timestamp, WebSearchInspection,
    WebSearchQueryDigest, WebSearchRejectionReason,
};
use std::collections::BTreeSet;
use std::future::Future;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebSearchAuditError {
    EmptyRejectionReasons,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebSearchQueryMode {
    CodeGenerated,
    UserDirectMessage,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WebSearchAuditOutcome {
    Approved,
    Rejected(Vec<WebSearchRejectionReason>),
    BypassedForUserMessage,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebSearchAuditRecord {
    mode: WebSearchQueryMode,
    digest: WebSearchQueryDigest,
    outcome: WebSearchAuditOutcome,
}

impl WebSearchAuditRecord {
    #[must_use]
    pub fn from_inspection(inspection: &WebSearchInspection) -> Self {
        match inspection {
            WebSearchInspection::Approved(approved) => {
                Self::code_approved(approved.digest().clone())
            }
            WebSearchInspection::Rejected(rejected) => Self {
                mode: WebSearchQueryMode::CodeGenerated,
                digest: rejected.digest().clone(),
                outcome: WebSearchAuditOutcome::Rejected(rejected.reasons().to_vec()),
            },
        }
    }

    #[must_use]
    pub const fn code_approved(digest: WebSearchQueryDigest) -> Self {
        Self {
            mode: WebSearchQueryMode::CodeGenerated,
            digest,
            outcome: WebSearchAuditOutcome::Approved,
        }
    }

    pub fn try_code_rejected(
        digest: WebSearchQueryDigest,
        reasons: Vec<WebSearchRejectionReason>,
    ) -> Result<Self, WebSearchAuditError> {
        if reasons.is_empty() {
            return Err(WebSearchAuditError::EmptyRejectionReasons);
        }
        let canonical: BTreeSet<WebSearchRejectionReason> = reasons.into_iter().collect();
        Ok(Self {
            mode: WebSearchQueryMode::CodeGenerated,
            digest,
            outcome: WebSearchAuditOutcome::Rejected(canonical.into_iter().collect()),
        })
    }

    #[must_use]
    pub const fn user_direct(digest: WebSearchQueryDigest) -> Self {
        Self {
            mode: WebSearchQueryMode::UserDirectMessage,
            digest,
            outcome: WebSearchAuditOutcome::BypassedForUserMessage,
        }
    }

    #[must_use]
    pub const fn mode(&self) -> WebSearchQueryMode {
        self.mode
    }

    #[must_use]
    pub const fn digest(&self) -> &WebSearchQueryDigest {
        &self.digest
    }

    #[must_use]
    pub const fn outcome(&self) -> &WebSearchAuditOutcome {
        &self.outcome
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuditKind {
    EstimateSubmitted(InstrumentId),
    SignalRaised(InstrumentId),
    ValidationRecorded(ProposalId),
    ApprovalRequested(ProposalId),
    PresenceRecorded {
        proposal: ProposalId,
        outcome: OrderConfirmation,
    },
    ProposalSkipped(ProposalId),
    SubmissionPrepared(ClientOrderId),
    SubmissionResponseRecorded(ClientOrderId),
    RiskSettingsChanged(RiskChange),
    RiskChangeConfirmationRecorded {
        change: RiskChange,
        outcome: RiskChangeConfirmation,
    },
    WebSearchChecked(WebSearchAuditRecord),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditEntry {
    at: Timestamp,
    kind: AuditKind,
}

impl AuditEntry {
    #[must_use]
    pub const fn new(at: Timestamp, kind: AuditKind) -> Self {
        Self { at, kind }
    }
    #[must_use]
    pub const fn at(&self) -> Timestamp {
        self.at
    }
    #[must_use]
    pub const fn kind(&self) -> &AuditKind {
        &self.kind
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuditIntegrity {
    Verified,
    Tampered,
}

pub trait AuditLog: Send + Sync {
    fn append(
        &self,
        entry: &AuditEntry,
    ) -> impl Future<Output = Result<RecordOutcome, SystemPortError>> + Send;
    fn verify(&self) -> impl Future<Output = Result<AuditIntegrity, SystemPortError>> + Send;
}
