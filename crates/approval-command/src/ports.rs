use application::{OrderGatePolicyContext, SystemPortError, WriteCompleted};
use domain::{
    AccountId, BrokerOrderId, ClientOrderId, Draft, LargeOrderConfirmation, OrderTerms, Proposal,
    ProposalId, ResolvedOrderInstrument, Timestamp,
};
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedSubmissionRecord {
    pub proposal_id: ProposalId,
    pub client_order_id: ClientOrderId,
    pub account_id: AccountId,
    pub instrument: ResolvedOrderInstrument,
    pub terms: OrderTerms,
    pub large_order: LargeOrderConfirmation,
    pub prepared_at: Timestamp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreSubmitCommitOutcome {
    Committed,
    AlreadyConsumed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubmissionResponseStatus {
    Accepted {
        broker_order_id: BrokerOrderId,
    },
    Rejected {
        reason: BrokerOrderRejectionReason,
    },
    Unknown {
        cause: UnknownOrderCause,
        retry: UnknownOrderRetryStatus,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrokerOrderRejectionReason {
    InvalidRequest,
    ConfirmHighValueRequired,
    AccountHeaderRequired,
    AuthenticationRequired,
    AccessForbidden,
    OppositePendingOrderExists,
    InsufficientBuyingPower,
    InsufficientSellableQuantity,
    OrderHoursClosed,
    StockRestricted,
    PriceOutOfRange,
    OrderTypeNotAllowed,
    PrerequisiteRequired,
    MarketNotSupportedForStock,
    InvestorExchangeNotIntegrated,
    AmountOrderOutsideRegularHours,
    FractionalQuantityOutsideRegularHours,
    AccountRestricted,
    MaxOrderAmountExceeded,
    IdempotencyKeyConflict,
    RateLimited,
    OtherRejected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnknownOrderCause {
    Maintenance,
    ServerError,
    RequestInProgress,
    Transport,
    InvalidResponse,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnknownOrderRetryStatus {
    Eligible,
    Forbidden,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubmissionResponseRecord {
    pub client_order_id: ClientOrderId,
    pub recorded_at: Timestamp,
    pub status: SubmissionResponseStatus,
}

pub trait PreSubmitTransactionPort: Send + Sync {
    fn commit_consumption_and_submission(
        &self,
        record: &PreparedSubmissionRecord,
    ) -> impl Future<Output = Result<PreSubmitCommitOutcome, SystemPortError>> + Send;

    fn record_submission_outcome(
        &self,
        record: &SubmissionResponseRecord,
    ) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send;
}
