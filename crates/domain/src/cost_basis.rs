use crate::{Amount, Currency, Money, MoneyError, Quantity, RatioQuotient};

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
    pub(crate) fn calculate(cost: Money, quantity: Quantity) -> Result<Self, MoneyError> {
        Ok(Self {
            quotient: crate::exact_arithmetic::divide(
                cost.amount(),
                Amount::new(quantity.as_decimal()),
            )?,
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

pub(crate) fn portion(cost: Money, take: Quantity, total: Quantity) -> Result<Money, MoneyError> {
    if take == total {
        return Ok(cost);
    }
    let product = crate::exact_arithmetic::multiply(cost.amount(), Amount::new(take.as_decimal()))?;
    match crate::exact_arithmetic::divide(product, Amount::new(total.as_decimal()))? {
        RatioQuotient::Exact(value) => {
            Ok(Money::new(Amount::new(value.as_decimal()), cost.currency()))
        }
        RatioQuotient::Approximate(_) => Err(MoneyError::PrecisionLoss),
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
