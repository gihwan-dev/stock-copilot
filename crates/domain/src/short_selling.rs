use crate::{SupplyContext, SupplyKrw, SupplyRatio, SupplyShares};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShortSelling {
    context: SupplyContext,
    volume: SupplyShares,
    amount: SupplyKrw,
    volume_rate: SupplyRatio,
    amount_rate: SupplyRatio,
}
impl ShortSelling {
    #[must_use]
    pub const fn from_response(
        context: SupplyContext,
        volume: SupplyShares,
        amount: SupplyKrw,
        volume_rate: SupplyRatio,
        amount_rate: SupplyRatio,
    ) -> Self {
        Self {
            context,
            volume,
            amount,
            volume_rate,
            amount_rate,
        }
    }
    #[must_use]
    pub const fn context(&self) -> &SupplyContext {
        &self.context
    }
    #[must_use]
    pub const fn volume(&self) -> &SupplyShares {
        &self.volume
    }
    #[must_use]
    pub const fn amount(&self) -> &SupplyKrw {
        &self.amount
    }
    #[must_use]
    pub const fn volume_rate(&self) -> &SupplyRatio {
        &self.volume_rate
    }
    #[must_use]
    pub const fn amount_rate(&self) -> &SupplyRatio {
        &self.amount_rate
    }
}
