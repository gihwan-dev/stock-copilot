use application::user_presence::UserPresence;
use application::{OrderConfirmation, RiskChangeConfirmation};
use domain::{OrderSummary, RiskChange};

#[derive(Debug)]
pub struct UnavailablePresence;

impl UserPresence for UnavailablePresence {
    fn confirm_order(
        &self,
        _summary: &OrderSummary,
    ) -> impl Future<Output = OrderConfirmation> + Send {
        std::future::ready(OrderConfirmation::Unavailable)
    }
    fn confirm_risk_change(
        &self,
        _change: RiskChange,
    ) -> impl Future<Output = RiskChangeConfirmation> + Send {
        std::future::ready(RiskChangeConfirmation::Unavailable)
    }
}
