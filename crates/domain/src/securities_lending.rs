use crate::{SupplyContext, SupplyKrw, SupplyShares};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecuritiesLending {
    context: SupplyContext,
    execution_quantity: SupplyShares,
    repayment_quantity: SupplyShares,
    balance_quantity: SupplyShares,
    balance_amount: SupplyKrw,
}
impl SecuritiesLending {
    #[must_use]
    pub const fn from_response(
        context: SupplyContext,
        execution_quantity: SupplyShares,
        repayment_quantity: SupplyShares,
        balance_quantity: SupplyShares,
        balance_amount: SupplyKrw,
    ) -> Self {
        Self {
            context,
            execution_quantity,
            repayment_quantity,
            balance_quantity,
            balance_amount,
        }
    }
    #[must_use]
    pub const fn context(&self) -> &SupplyContext {
        &self.context
    }
    #[must_use]
    pub const fn execution_quantity(&self) -> &SupplyShares {
        &self.execution_quantity
    }
    #[must_use]
    pub const fn repayment_quantity(&self) -> &SupplyShares {
        &self.repayment_quantity
    }
    #[must_use]
    pub const fn balance_quantity(&self) -> &SupplyShares {
        &self.balance_quantity
    }
    #[must_use]
    pub const fn balance_amount(&self) -> &SupplyKrw {
        &self.balance_amount
    }
}
