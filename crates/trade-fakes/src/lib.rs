use domain::{
    BrokerOrderId, ClientOrderId, ClientOrderPrefix, DisplayInstrumentName, EstimatedOrderFee,
    LargeOrderConfirmation, Market, MaskedAccountNumber, OrderExpectedAmount, OrderIntent,
    OrderSide, OrderSummary, ProposalId, SummaryLimitPrice, SummaryQuantity, Symbol,
};
use order_execution::{
    ApprovedOrder, BrokerTrade, OrderOutcome, OrderRejectionReason, OrderUnknownCause,
};
use std::collections::VecDeque;
use std::future::Future;
use std::sync::RwLock;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SentOrderSummary {
    pub instrument_name: DisplayInstrumentName,
    pub symbol: Symbol,
    pub market: Market,
    pub side: OrderSide,
    pub quantity: SummaryQuantity,
    pub limit_price: SummaryLimitPrice,
    pub expected_amount: OrderExpectedAmount,
    pub expected_fee: EstimatedOrderFee,
    pub account_suffix: MaskedAccountNumber,
    pub client_order_prefix: ClientOrderPrefix,
}

impl SentOrderSummary {
    #[must_use]
    pub fn from_summary(summary: &OrderSummary) -> Self {
        Self {
            instrument_name: summary.instrument_name().clone(),
            symbol: summary.symbol().clone(),
            market: summary.market(),
            side: summary.side(),
            quantity: summary.quantity(),
            limit_price: summary.limit_price(),
            expected_amount: *summary.expected_amount(),
            expected_fee: summary.expected_fee().clone(),
            account_suffix: summary.account_suffix().clone(),
            client_order_prefix: summary.client_order_prefix().clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SentOrder {
    pub proposal_id: ProposalId,
    pub client_order_id: ClientOrderId,
    pub intent: OrderIntent,
    pub summary: SentOrderSummary,
    pub large_order: LargeOrderConfirmation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BrokerReply {
    Accept(BrokerOrderId),
    Reject(OrderRejectionReason),
    Unknown(OrderUnknownCause),
}

#[derive(Debug, Default)]
pub struct RecordingBroker {
    replies: RwLock<VecDeque<BrokerReply>>,
    sent: RwLock<Vec<SentOrder>>,
}

impl RecordingBroker {
    #[must_use]
    pub fn with_replies<Replies: IntoIterator<Item = BrokerReply>>(replies: Replies) -> Self {
        Self {
            replies: RwLock::new(replies.into_iter().collect()),
            sent: RwLock::new(Vec::new()),
        }
    }

    #[must_use]
    pub fn sent(&self) -> Vec<SentOrder> {
        match self.sent.read() {
            Ok(guard) => guard.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        }
    }

    fn next_reply(&self) -> BrokerReply {
        let reply = match self.replies.write() {
            Ok(mut guard) => guard.pop_front(),
            Err(poisoned) => poisoned.into_inner().pop_front(),
        };
        reply.unwrap_or(BrokerReply::Reject(OrderRejectionReason::OtherRejected))
    }
}

impl BrokerTrade for RecordingBroker {
    fn submit(&self, order: ApprovedOrder) -> impl Future<Output = OrderOutcome> + Send {
        let record = SentOrder {
            proposal_id: order.proposal_id().clone(),
            client_order_id: order.client_order_id().clone(),
            intent: order.intent().clone(),
            summary: SentOrderSummary::from_summary(order.summary()),
            large_order: order.large_order_confirmation(),
        };
        match self.sent.write() {
            Ok(mut guard) => guard.push(record),
            Err(poisoned) => poisoned.into_inner().push(record),
        }
        let outcome = match self.next_reply() {
            BrokerReply::Accept(id) => order.into_accepted(id),
            BrokerReply::Reject(reason) => order.into_rejected(reason),
            BrokerReply::Unknown(cause) => order.into_unknown(cause),
        };
        std::future::ready(outcome)
    }
}
