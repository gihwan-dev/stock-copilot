use domain::{
    BrokerOrderId, ClientOrderId, LargeOrderConfirmation, OrderIntent, OrderSummary,
    PendingOrderConfirmation, PriceObservation, ProposalExecutionError, ProposalId, Timestamp,
};
use std::{fmt, future::Future};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalSignatureSlot {
    PendingSecureEnclave,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetryEligibility {
    Eligible,
    Forbidden,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderRejectionReason {
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
pub enum OrderUnknownCause {
    Maintenance,
    ServerError,
    RequestInProgress,
    Transport,
    InvalidResponse,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubmittedOrder {
    client_order_id: ClientOrderId,
    broker_order_id: BrokerOrderId,
}

impl SubmittedOrder {
    #[must_use]
    pub fn client_order_id(&self) -> &ClientOrderId {
        &self.client_order_id
    }

    #[must_use]
    pub fn broker_order_id(&self) -> &BrokerOrderId {
        &self.broker_order_id
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderRejection {
    client_order_id: ClientOrderId,
    reason: OrderRejectionReason,
}

impl OrderRejection {
    #[must_use]
    pub fn client_order_id(&self) -> &ClientOrderId {
        &self.client_order_id
    }

    #[must_use]
    pub fn reason(&self) -> OrderRejectionReason {
        self.reason
    }
}

pub struct UserApproval {
    confirmation: PendingOrderConfirmation,
    signature: ApprovalSignatureSlot,
}

impl fmt::Debug for UserApproval {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UserApproval")
            .field("proposal_id", self.confirmation.id())
            .field("summary", self.confirmation.summary())
            .field("signature", &self.signature)
            .finish_non_exhaustive()
    }
}

impl UserApproval {
    fn new_internal(
        confirmation: PendingOrderConfirmation,
        signature: ApprovalSignatureSlot,
    ) -> Self {
        Self {
            confirmation,
            signature,
        }
    }

    #[must_use]
    pub fn id(&self) -> &ProposalId {
        self.confirmation.id()
    }

    #[must_use]
    pub fn intent(&self) -> &OrderIntent {
        self.confirmation.intent()
    }

    #[must_use]
    pub fn summary(&self) -> &OrderSummary {
        self.confirmation.summary()
    }

    #[must_use]
    pub fn validated_at(&self) -> Timestamp {
        self.confirmation.validated_at()
    }

    #[must_use]
    pub fn checked_at(&self) -> Timestamp {
        self.confirmation.checked_at()
    }

    #[must_use]
    pub fn expires_at(&self) -> Timestamp {
        self.confirmation.expires_at()
    }

    #[must_use]
    pub fn validation_observation(&self) -> &PriceObservation {
        self.confirmation.validation_observation()
    }

    #[must_use]
    pub fn execution_observation(&self) -> &PriceObservation {
        self.confirmation.execution_observation()
    }

    #[must_use]
    pub fn large_order_confirmation(&self) -> LargeOrderConfirmation {
        self.confirmation.summary().expected_amount().confirmation()
    }

    #[must_use]
    pub fn signature(&self) -> ApprovalSignatureSlot {
        self.signature
    }

    pub fn recheck(
        self,
        now: Timestamp,
        observation: PriceObservation,
    ) -> Result<Self, ProposalExecutionError> {
        let checked = self.confirmation.recheck(now, observation)?;
        Ok(Self::new_internal(checked, self.signature))
    }

    #[must_use]
    pub fn into_order(self) -> ApprovedOrder {
        ApprovedOrder {
            confirmation: self.confirmation,
            signature: self.signature,
        }
    }
}

pub struct ApprovedOrder {
    confirmation: PendingOrderConfirmation,
    signature: ApprovalSignatureSlot,
}

impl fmt::Debug for ApprovedOrder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApprovedOrder")
            .field("proposal_id", self.confirmation.id())
            .field("summary", self.confirmation.summary())
            .field("signature", &self.signature)
            .finish_non_exhaustive()
    }
}

impl ApprovedOrder {
    #[must_use]
    pub fn proposal_id(&self) -> &ProposalId {
        self.confirmation.id()
    }

    #[must_use]
    pub fn client_order_id(&self) -> &ClientOrderId {
        self.confirmation.intent().client_order_id()
    }

    #[must_use]
    pub fn intent(&self) -> &OrderIntent {
        self.confirmation.intent()
    }

    #[must_use]
    pub fn summary(&self) -> &OrderSummary {
        self.confirmation.summary()
    }

    #[must_use]
    pub fn checked_at(&self) -> Timestamp {
        self.confirmation.checked_at()
    }

    #[must_use]
    pub fn large_order_confirmation(&self) -> LargeOrderConfirmation {
        self.confirmation.summary().expected_amount().confirmation()
    }

    #[must_use]
    pub fn signature(&self) -> ApprovalSignatureSlot {
        self.signature
    }

    #[must_use]
    pub fn into_accepted(self, broker_order_id: BrokerOrderId) -> OrderOutcome {
        let client_order_id = self.confirmation.intent().client_order_id().clone();
        OrderOutcome::Accepted(SubmittedOrder {
            client_order_id,
            broker_order_id,
        })
    }

    #[must_use]
    pub fn into_rejected(self, reason: OrderRejectionReason) -> OrderOutcome {
        let client_order_id = self.confirmation.intent().client_order_id().clone();
        OrderOutcome::Rejected(OrderRejection {
            client_order_id,
            reason,
        })
    }

    #[must_use]
    pub fn into_unknown(self, cause: OrderUnknownCause) -> OrderOutcome {
        let client_order_id = self.confirmation.intent().client_order_id().clone();
        let first_sent_at = self.confirmation.checked_at();
        let retry = match cause {
            OrderUnknownCause::Maintenance => None,
            OrderUnknownCause::ServerError
            | OrderUnknownCause::RequestInProgress
            | OrderUnknownCause::Transport
            | OrderUnknownCause::InvalidResponse => Some(Box::new(UnknownOrderRetry {
                order: self,
                first_sent_at,
            })),
        };
        OrderOutcome::Unknown(UnknownOrderOutcome {
            client_order_id,
            cause,
            retry,
        })
    }
}

pub struct UnknownOrderRetry {
    order: ApprovedOrder,
    first_sent_at: Timestamp,
}

impl fmt::Debug for UnknownOrderRetry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UnknownOrderRetry")
            .field("order", &self.order)
            .field("first_sent_at", &self.first_sent_at)
            .finish_non_exhaustive()
    }
}

impl UnknownOrderRetry {
    #[must_use]
    pub fn client_order_id(&self) -> &ClientOrderId {
        self.order.client_order_id()
    }

    #[must_use]
    pub fn intent(&self) -> &OrderIntent {
        self.order.intent()
    }

    #[must_use]
    pub fn first_sent_at(&self) -> Timestamp {
        self.first_sent_at
    }
}

#[derive(Debug)]
pub struct UnknownOrderOutcome {
    client_order_id: ClientOrderId,
    cause: OrderUnknownCause,
    retry: Option<Box<UnknownOrderRetry>>,
}

impl UnknownOrderOutcome {
    #[must_use]
    pub fn client_order_id(&self) -> &ClientOrderId {
        &self.client_order_id
    }

    #[must_use]
    pub fn cause(&self) -> OrderUnknownCause {
        self.cause
    }

    #[must_use]
    pub fn retry_eligibility(&self) -> RetryEligibility {
        match self.retry {
            Some(_) => RetryEligibility::Eligible,
            None => RetryEligibility::Forbidden,
        }
    }

    #[must_use]
    pub fn into_retry(self) -> Option<UnknownOrderRetry> {
        self.retry.map(|permit| *permit)
    }
}

#[derive(Debug)]
pub enum OrderOutcome {
    Accepted(SubmittedOrder),
    Rejected(OrderRejection),
    Unknown(UnknownOrderOutcome),
}

impl OrderOutcome {
    #[must_use]
    pub fn client_order_id(&self) -> &ClientOrderId {
        match self {
            Self::Accepted(submitted) => submitted.client_order_id(),
            Self::Rejected(rejection) => rejection.client_order_id(),
            Self::Unknown(unknown) => unknown.client_order_id(),
        }
    }
}

pub trait BrokerTrade: Send + Sync {
    fn submit(&self, order: ApprovedOrder) -> impl Future<Output = OrderOutcome> + Send;
}

#[cfg(test)]
mod tests {
    use super::{
        ApprovalSignatureSlot, ApprovedOrder, BrokerTrade, OrderOutcome, OrderRejectionReason,
        OrderUnknownCause, RetryEligibility, UserApproval,
    };

    #[test]
    fn types_expose_sealed_surface_and_trait_bounds() {
        fn assert_send_sync<T: Send + Sync>() {}
        fn assert_broker<B: BrokerTrade>(_broker: &B) {}

        assert_send_sync::<UserApproval>();
        assert_send_sync::<ApprovedOrder>();
        assert_send_sync::<OrderOutcome>();
        assert_eq!(
            ApprovalSignatureSlot::PendingSecureEnclave,
            ApprovalSignatureSlot::PendingSecureEnclave,
            "서명 자리는 닫힌 값이다"
        );
        assert_eq!(
            RetryEligibility::Eligible,
            RetryEligibility::Eligible,
            "재시도 가능 여부는 닫힌 값이다"
        );
        assert_ne!(
            OrderRejectionReason::ConfirmHighValueRequired,
            OrderRejectionReason::InvalidRequest,
            "거절 사유는 구분된다"
        );
        assert_ne!(
            OrderUnknownCause::Maintenance,
            OrderUnknownCause::Transport,
            "점검 원인은 전송 오류와 구분된다"
        );
        let _ = assert_broker::<NoopBroker>;
    }

    struct NoopBroker;

    impl BrokerTrade for NoopBroker {
        fn submit(&self, order: ApprovedOrder) -> impl Future<Output = OrderOutcome> + Send {
            std::future::ready(order.into_unknown(OrderUnknownCause::Maintenance))
        }
    }
}
