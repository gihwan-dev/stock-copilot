use crate::{Policy, SellOrderCalculationError};
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SavedMarketOrderSetting {
    #[default]
    Disabled,
    Enabled,
}
#[derive(Clone, PartialEq, Eq)]
pub struct SavedMarketOrderPolicy {
    pub policy: Policy,
    pub market_orders: SavedMarketOrderSetting,
}
#[derive(Clone, PartialEq, Eq)]
pub struct MarketOrderPolicy {
    policy: Policy,
    setting: SavedMarketOrderSetting,
}
impl std::fmt::Debug for MarketOrderPolicy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("MarketOrderPolicy(..)")
    }
}
impl MarketOrderPolicy {
    #[must_use]
    pub const fn policy(&self) -> &Policy {
        &self.policy
    }
    #[must_use]
    pub const fn setting(&self) -> SavedMarketOrderSetting {
        self.setting
    }
}
#[must_use]
pub fn default_market_order_policy(policy: Policy) -> MarketOrderPolicy {
    MarketOrderPolicy {
        policy,
        setting: SavedMarketOrderSetting::Disabled,
    }
}
#[must_use]
pub fn restore_market_order_policy(saved: SavedMarketOrderPolicy) -> MarketOrderPolicy {
    MarketOrderPolicy {
        policy: saved.policy,
        setting: saved.market_orders,
    }
}
pub(crate) fn allow_market(
    snapshot: MarketOrderPolicy,
) -> Result<Policy, SellOrderCalculationError> {
    match snapshot.setting {
        SavedMarketOrderSetting::Disabled => Err(SellOrderCalculationError::MarketDisabled),
        SavedMarketOrderSetting::Enabled => Ok(snapshot.policy),
    }
}

impl std::fmt::Debug for SavedMarketOrderPolicy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SavedMarketOrderPolicy(..)")
    }
}
