use crate::{Currency, Money, MoneyError, RatioQuotient};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CostBasisMethod {
    Fifo,
    MovingAverage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LotCost {
    Known(Money),
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LotUnitPrice {
    quotient: RatioQuotient,
    currency: Currency,
}
impl LotUnitPrice {
    pub(crate) fn calculate(
        cost: Money,
        quantity: &crate::ShareQuantity,
    ) -> Result<Self, crate::LotError> {
        Ok(Self {
            quotient: crate::cost_basis_arithmetic::unit_ratio(cost, quantity)?,
            currency: cost.currency(),
        })
    }
    #[must_use]
    pub const fn quotient(self) -> RatioQuotient {
        self.quotient
    }
    #[must_use]
    pub const fn currency(self) -> Currency {
        self.currency
    }
}

pub(crate) fn add_cost(left: LotCost, right: LotCost) -> Result<LotCost, MoneyError> {
    match (left, right) {
        (LotCost::Known(first), LotCost::Known(second)) => {
            first.checked_add(&second).map(LotCost::Known)
        }
        _ => Ok(LotCost::Unknown),
    }
}
