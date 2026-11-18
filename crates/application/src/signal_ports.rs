use crate::SystemPortError;
use domain::{
    BuySignalRecord, CalibrationLedgerHead, CollectedItem, Disclosure, DomainEvent, InstrumentId,
    IssuedEstimateTaskId, NoSignalReason, RaisedSignal, ResolvedOrderInstrument, SignalDecision,
    SignalSourceLedger, SignalTransition, ToolReference,
};
use std::future::Future;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SignalRevision(u64);
impl SignalRevision {
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }
    #[must_use]
    pub const fn as_u64(&self) -> &u64 {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredSignal {
    pub signal: RaisedSignal,
    pub submission: IssuedEstimateTaskId,
    pub occurrence: Option<BuySignalRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignalSnapshot {
    pub revision: SignalRevision,
    pub submission: Option<IssuedEstimateTaskId>,
    pub current: Option<StoredSignal>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignalEvaluation {
    pub submission: IssuedEstimateTaskId,
    pub decision: SignalDecision,
    pub transition: SignalTransition,
    pub calibration_head: Option<CalibrationLedgerHead>,
    pub occurrence: Option<BuySignalRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SignalMutation {
    Evaluate(Box<SignalEvaluation>),
    Discard(Box<SignalTransition>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignalCommit {
    pub events: Vec<DomainEvent>,
    pub no_signal_reason: Option<NoSignalReason>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SignalCommitOutcome {
    Applied(SignalCommit),
    Conflict,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SignalSourceFacts {
    pub items: Vec<CollectedItem>,
    pub disclosures: Vec<Disclosure>,
    pub tools: Vec<ToolReference>,
}
impl SignalSourceFacts {
    #[must_use]
    pub fn ledger(&self) -> SignalSourceLedger<'_> {
        SignalSourceLedger::new(&self.items, &self.disclosures, &self.tools)
    }
}

pub trait SignalRepository: Send + Sync {
    fn snapshot(
        &self,
        instrument: &InstrumentId,
    ) -> impl Future<Output = Result<SignalSnapshot, SystemPortError>> + Send;
    fn commit(
        &self,
        instrument: &InstrumentId,
        expected: &SignalSnapshot,
        mutation: &SignalMutation,
    ) -> impl Future<Output = Result<SignalCommitOutcome, SystemPortError>> + Send;
    fn occurrences(
        &self,
        instrument: &InstrumentId,
        identity: &ResolvedOrderInstrument,
    ) -> impl Future<Output = Result<Vec<BuySignalRecord>, SystemPortError>> + Send;
    fn sources(
        &self,
        task: &IssuedEstimateTaskId,
    ) -> impl Future<Output = Result<SignalSourceFacts, SystemPortError>> + Send;
    fn current(
        &self,
        instrument: &InstrumentId,
    ) -> impl Future<Output = Result<Vec<RaisedSignal>, SystemPortError>> + Send;
}
