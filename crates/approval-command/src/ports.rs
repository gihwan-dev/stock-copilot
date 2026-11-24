use application::{OrderGatePolicyContext, SystemPortError};
use domain::{AccountId, Draft, Proposal, ProposalId, ResolvedOrderInstrument, Timestamp};
use std::future::Future;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolFreezeStatus {
    Unfrozen,
    Frozen,
}

pub trait SymbolFreezePort: Send + Sync {
    fn freeze_status(
        &self,
        account: &AccountId,
        instrument: &ResolvedOrderInstrument,
    ) -> impl Future<Output = Result<SymbolFreezeStatus, SystemPortError>> + Send;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct NeverFrozen;

impl SymbolFreezePort for NeverFrozen {
    fn freeze_status(
        &self,
        _account: &AccountId,
        _instrument: &ResolvedOrderInstrument,
    ) -> impl Future<Output = Result<SymbolFreezeStatus, SystemPortError>> + Send {
        std::future::ready(Ok(SymbolFreezeStatus::Unfrozen))
    }
}

pub trait ProposalContextPort: Send + Sync {
    fn policy_context(
        &self,
        proposal_id: &ProposalId,
    ) -> impl Future<Output = Result<Option<OrderGatePolicyContext>, SystemPortError>> + Send;
}

#[derive(Debug)]
pub enum RedraftOutcome {
    Redrafted(Box<Proposal<Draft>>),
    OutOfEstimateRange,
    MarketClosed,
    NoSignal,
}

pub trait ExpiredProposalRedrafter: Send + Sync {
    fn redraft(
        &self,
        expired: &Proposal<Draft>,
        now: Timestamp,
    ) -> impl Future<Output = Result<RedraftOutcome, SystemPortError>> + Send;
}
