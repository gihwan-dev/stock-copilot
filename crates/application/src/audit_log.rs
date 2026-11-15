use crate::{OrderConfirmation, RecordOutcome, RiskChangeConfirmation, SystemPortError};
use domain::{ClientOrderId, InstrumentId, ProposalId, RiskChange, Timestamp};
use std::future::Future;

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
