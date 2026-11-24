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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuditHeadError {
    ZeroSequence,
    InvalidHash,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct AuditSequence(u64);

impl AuditSequence {
    pub const fn try_new(value: u64) -> Result<Self, AuditHeadError> {
        if value == 0 {
            return Err(AuditHeadError::ZeroSequence);
        }
        Ok(Self(value))
    }

    #[must_use]
    pub const fn as_u64(&self) -> &u64 {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditChainHash(String);

impl AuditChainHash {
    pub fn try_new(value: &str) -> Result<Self, AuditHeadError> {
        if value.len() != 64
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(AuditHeadError::InvalidHash);
        }
        Ok(Self(value.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditHead {
    sequence: AuditSequence,
    hash: AuditChainHash,
}

impl AuditHead {
    #[must_use]
    pub const fn new(sequence: AuditSequence, hash: AuditChainHash) -> Self {
        Self { sequence, hash }
    }

    #[must_use]
    pub const fn sequence(&self) -> AuditSequence {
        self.sequence
    }

    #[must_use]
    pub const fn hash(&self) -> &AuditChainHash {
        &self.hash
    }
}

pub trait AuditHeadAnchor: Send + Sync {
    fn load_head(&self) -> impl Future<Output = Result<Option<AuditHead>, SystemPortError>> + Send;
    fn store_head(
        &self,
        head: &AuditHead,
    ) -> impl Future<Output = Result<crate::WriteCompleted, SystemPortError>> + Send;
}
