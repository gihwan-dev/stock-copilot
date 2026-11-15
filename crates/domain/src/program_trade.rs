use crate::{MarketDataError, ProgramContext, SupplyShares, SupplySignedShares};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramVolume {
    buy: SupplyShares,
    sell: SupplyShares,
    net: SupplySignedShares,
}
impl ProgramVolume {
    pub fn try_new(
        buy: SupplyShares,
        sell: SupplyShares,
        net: SupplySignedShares,
    ) -> Result<Self, MarketDataError> {
        if buy.to_amount().checked_sub(&sell.to_amount()) != Ok(net.to_amount()) {
            return Err(MarketDataError::InvalidFlow);
        }
        Ok(Self { buy, sell, net })
    }
    #[must_use]
    pub const fn buy(&self) -> &SupplyShares {
        &self.buy
    }
    #[must_use]
    pub const fn sell(&self) -> &SupplyShares {
        &self.sell
    }
    #[must_use]
    pub const fn net(&self) -> &SupplySignedShares {
        &self.net
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramTrade {
    context: ProgramContext,
    arbitrage: ProgramVolume,
    non_arbitrage: ProgramVolume,
}
impl ProgramTrade {
    #[must_use]
    pub const fn from_response(
        context: ProgramContext,
        arbitrage: ProgramVolume,
        non_arbitrage: ProgramVolume,
    ) -> Self {
        Self {
            context,
            arbitrage,
            non_arbitrage,
        }
    }
    #[must_use]
    pub const fn context(&self) -> &ProgramContext {
        &self.context
    }
    #[must_use]
    pub const fn arbitrage(&self) -> &ProgramVolume {
        &self.arbitrage
    }
    #[must_use]
    pub const fn non_arbitrage(&self) -> &ProgramVolume {
        &self.non_arbitrage
    }
}
