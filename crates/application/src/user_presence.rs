use domain::{OrderSummary, RiskChange};
use std::future::Future;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderConfirmation {
    Confirmed,
    Denied,
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RiskChangeConfirmation {
    Confirmed,
    Denied,
    Unavailable,
}

pub trait UserPresence: Send + Sync {
    fn confirm_order(
        &self,
        summary: OrderSummary,
    ) -> impl Future<Output = OrderConfirmation> + Send;
    fn confirm_risk_change(
        &self,
        change: RiskChange,
    ) -> impl Future<Output = RiskChangeConfirmation> + Send;
}
