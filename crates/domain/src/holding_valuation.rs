use crate::{Holding, Money, MoneyError, RatioQuotient};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HoldingReturn {
    Defined(RatioQuotient),
    ZeroCost,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvaluationError {
    MissingPrice,
    MissingFx,
    Arithmetic(MoneyError),
    Conversion(crate::FxConversionError),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Comparison {
    NotProvided,
    Matches,
    Different,
    Unavailable,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValuationSource {
    TossOpenApi,
    Calculated,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ComputedHolding {
    valuation: Money,
    purchase: Money,
    profit: Money,
    return_rate: Result<HoldingReturn, MoneyError>,
}
impl ComputedHolding {
    #[must_use]
    pub const fn valuation(self) -> Money {
        self.valuation
    }
    #[must_use]
    pub const fn purchase(self) -> Money {
        self.purchase
    }
    #[must_use]
    pub const fn profit(self) -> Money {
        self.profit
    }
    pub const fn return_rate(self) -> Result<HoldingReturn, MoneyError> {
        self.return_rate
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HoldingValuation {
    computed: Result<ComputedHolding, EvaluationError>,
    selected: Result<Money, EvaluationError>,
    comparison: Comparison,
    source: ValuationSource,
}
impl HoldingValuation {
    pub const fn computed(self) -> Result<ComputedHolding, EvaluationError> {
        self.computed
    }
    pub const fn selected(self) -> Result<Money, EvaluationError> {
        self.selected
    }
    #[must_use]
    pub const fn comparison(self) -> Comparison {
        self.comparison
    }
    #[must_use]
    pub const fn source(self) -> ValuationSource {
        self.source
    }

    #[must_use]
    pub(crate) fn evaluate(holding: &Holding) -> Self {
        let computed = Self::compute(holding);
        let source = if holding.reported().is_some() {
            ValuationSource::TossOpenApi
        } else {
            ValuationSource::Calculated
        };
        let comparison = match (computed, holding.reported()) {
            (_, None) => Comparison::NotProvided,
            (Err(_), Some(_)) => Comparison::Unavailable,
            (Ok(value), Some(reported)) => {
                let rate_matches = value
                    .return_rate
                    .is_ok_and(|return_rate| match return_rate {
                        HoldingReturn::ZeroCost => false,
                        HoldingReturn::Defined(RatioQuotient::Exact(rate)) => {
                            rate == reported.return_rate()
                        }
                        HoldingReturn::Defined(RatioQuotient::Approximate(rate)) => {
                            rate.cmp_ratio(&reported.return_rate()).is_eq()
                        }
                    });
                if value.valuation == reported.valuation()
                    && value.profit == reported.profit()
                    && rate_matches
                {
                    Comparison::Matches
                } else {
                    Comparison::Different
                }
            }
        };
        let selected = computed.map(|value| {
            holding
                .reported()
                .map_or(value.valuation, crate::ReportedHolding::valuation)
        });
        Self {
            computed,
            selected,
            comparison,
            source,
        }
    }
    fn compute(holding: &Holding) -> Result<ComputedHolding, EvaluationError> {
        let purchase = holding
            .acquisition_price()
            .checked_mul(holding.quantity())
            .map_err(EvaluationError::Arithmetic)?;
        let valuation = if holding.quantity().as_decimal().is_zero() {
            crate::holding::zero(holding.currency())
        } else {
            holding
                .current_price()
                .ok_or(EvaluationError::MissingPrice)?
                .checked_mul(&holding.quantity())
                .map_err(EvaluationError::Arithmetic)?
        };
        let profit = valuation
            .checked_sub(&purchase)
            .map_err(EvaluationError::Arithmetic)?;
        let return_rate = if purchase.amount().as_decimal().is_zero() {
            Ok(HoldingReturn::ZeroCost)
        } else {
            profit.checked_ratio(&purchase).map(HoldingReturn::Defined)
        };
        Ok(ComputedHolding {
            valuation,
            purchase,
            profit,
            return_rate,
        })
    }
}
