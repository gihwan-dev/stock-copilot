use crate::{HoursPermissionState, RiskAppetite, SavedMarketOrderSetting, WeightCaps};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderPermission {
    Disabled,
    Enabled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RiskSettings {
    orders: OrderPermission,
    market_orders: SavedMarketOrderSetting,
    hours: HoursPermissionState,
    weight_caps: WeightCaps,
    risk_appetite: RiskAppetite,
}

impl RiskSettings {
    #[must_use]
    pub const fn new(
        orders: OrderPermission,
        market_orders: SavedMarketOrderSetting,
        hours: HoursPermissionState,
        weight_caps: WeightCaps,
        risk_appetite: RiskAppetite,
    ) -> Self {
        Self {
            orders,
            market_orders,
            hours,
            weight_caps,
            risk_appetite,
        }
    }
    #[must_use]
    pub const fn orders(self) -> OrderPermission {
        self.orders
    }
    #[must_use]
    pub const fn market_orders(self) -> SavedMarketOrderSetting {
        self.market_orders
    }
    #[must_use]
    pub const fn hours(self) -> HoursPermissionState {
        self.hours
    }
    #[must_use]
    pub const fn weight_caps(self) -> WeightCaps {
        self.weight_caps
    }
    #[must_use]
    pub const fn risk_appetite(self) -> RiskAppetite {
        self.risk_appetite
    }
}
