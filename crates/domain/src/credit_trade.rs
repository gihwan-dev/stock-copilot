use crate::{CreditSide, SupplyContext, SupplyRate, SupplyShares};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreditDetail {
    new_quantity: SupplyShares,
    return_quantity: SupplyShares,
    balance_quantity: SupplyShares,
    balance_rate: SupplyRate,
    trading_rate: SupplyRate,
}
impl CreditDetail {
    #[must_use]
    pub const fn from_response(
        new_quantity: SupplyShares,
        return_quantity: SupplyShares,
        balance_quantity: SupplyShares,
        balance_rate: SupplyRate,
        trading_rate: SupplyRate,
    ) -> Self {
        Self {
            new_quantity,
            return_quantity,
            balance_quantity,
            balance_rate,
            trading_rate,
        }
    }
    #[must_use]
    pub const fn new_quantity(&self) -> &SupplyShares {
        &self.new_quantity
    }
    #[must_use]
    pub const fn return_quantity(&self) -> &SupplyShares {
        &self.return_quantity
    }
    #[must_use]
    pub const fn balance_quantity(&self) -> &SupplyShares {
        &self.balance_quantity
    }
    #[must_use]
    pub const fn balance_rate(&self) -> &SupplyRate {
        &self.balance_rate
    }
    #[must_use]
    pub const fn trading_rate(&self) -> &SupplyRate {
        &self.trading_rate
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreditTrade {
    context: SupplyContext,
    margin_loan: CreditSide,
    stock_loan: CreditSide,
}
impl CreditTrade {
    #[must_use]
    pub const fn from_response(
        context: SupplyContext,
        margin_loan: CreditSide,
        stock_loan: CreditSide,
    ) -> Self {
        Self {
            context,
            margin_loan,
            stock_loan,
        }
    }
    #[must_use]
    pub const fn context(&self) -> &SupplyContext {
        &self.context
    }
    #[must_use]
    pub const fn margin_loan(&self) -> &CreditSide {
        &self.margin_loan
    }
    #[must_use]
    pub const fn stock_loan(&self) -> &CreditSide {
        &self.stock_loan
    }
}
