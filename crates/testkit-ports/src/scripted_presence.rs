use crate::{Script, script::State};
use application::user_presence::UserPresence;
use application::{OrderConfirmation, RiskChangeConfirmation};
use domain::{
    ClientOrderPrefix, DisplayInstrumentName, EstimatedOrderFee, Market, MaskedAccountNumber,
    OrderExpectedAmount, OrderSide, SummaryLimitPrice, SummaryQuantity, Symbol,
};
use domain::{OrderSummary, RiskChange};
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordedOrderSummary {
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
impl RecordedOrderSummary {
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
pub enum PresenceCall {
    Order(Box<RecordedOrderSummary>),
    RiskChange(RiskChange),
}
#[derive(Debug)]
pub struct ScriptedPresence {
    order: Script<RecordedOrderSummary, OrderConfirmation>,
    risk: Script<RiskChange, RiskChangeConfirmation>,
    calls: State<Vec<PresenceCall>>,
}
impl ScriptedPresence {
    pub fn new<
        Orders: IntoIterator<Item = OrderConfirmation>,
        Risks: IntoIterator<Item = RiskChangeConfirmation>,
    >(
        order: Orders,
        risk: Risks,
    ) -> Self {
        Self {
            order: Script::new(order),
            risk: Script::new(risk),
            calls: State::default(),
        }
    }
    #[must_use]
    pub fn calls(&self) -> Vec<PresenceCall> {
        self.calls.write().clone()
    }
    #[must_use]
    pub fn order_calls(&self) -> Vec<RecordedOrderSummary> {
        self.order.calls()
    }
    #[must_use]
    pub fn risk_change_calls(&self) -> Vec<RiskChange> {
        self.risk.calls()
    }
}
impl UserPresence for ScriptedPresence {
    async fn confirm_order(&self, summary: &OrderSummary) -> OrderConfirmation {
        let recorded = RecordedOrderSummary::from_summary(summary);
        self.calls
            .write()
            .push(PresenceCall::Order(Box::new(recorded.clone())));
        self.order.run(recorded).await
    }
    async fn confirm_risk_change(&self, change: RiskChange) -> RiskChangeConfirmation {
        self.calls
            .write()
            .push(PresenceCall::RiskChange(change.clone()));
        self.risk.run(change).await
    }
}
