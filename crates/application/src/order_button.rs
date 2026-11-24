use crate::{SettingsRepository, SystemPortError};
use domain::{
    MarketTradingStatus, OrderButtonState, ProposalAvailability, order_button_state,
    saved_order_permission,
};

#[derive(Debug)]
pub struct ReadOrderButton<'a, Settings> {
    settings: &'a Settings,
}

impl<'a, Settings: SettingsRepository> ReadOrderButton<'a, Settings> {
    #[must_use]
    pub const fn new(settings: &'a Settings) -> Self {
        Self { settings }
    }

    pub async fn state(
        &self,
        market: MarketTradingStatus,
        proposal: ProposalAvailability,
    ) -> Result<OrderButtonState, SystemPortError> {
        let saved = self.settings.risk_settings().await?;
        Ok(order_button_state(
            saved_order_permission(saved),
            market,
            proposal,
        ))
    }
}
