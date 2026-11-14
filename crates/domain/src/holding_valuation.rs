use crate::{Amount, DailyProfitLoss, Holding, Money, MoneyError, RatioQuotient, ReturnBasis};

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
    BuyingPower(crate::PortfolioError),
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
pub struct SelectedValue<T> {
    value: T,
    source: ValuationSource,
}
impl<T: Copy> SelectedValue<T> {
    pub(crate) fn new(value: T, source: ValuationSource) -> Self {
        Self { value, source }
    }
    #[must_use]
    pub const fn value(self) -> T {
        self.value
    }
    #[must_use]
    pub const fn source(self) -> ValuationSource {
        self.source
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReturnObservation {
    rate: HoldingReturn,
    basis: ReturnBasis,
}
impl ReturnObservation {
    pub(crate) fn reported(rate: crate::Ratio, basis: ReturnBasis) -> Self {
        Self {
            rate: HoldingReturn::Defined(RatioQuotient::Exact(rate)),
            basis,
        }
    }
    #[must_use]
    pub const fn rate(self) -> HoldingReturn {
        self.rate
    }
    #[must_use]
    pub const fn basis(self) -> ReturnBasis {
        self.basis
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HoldingComparisons {
    valuation: Comparison,
    profit: Comparison,
    return_rate: Comparison,
}
impl HoldingComparisons {
    #[must_use]
    pub const fn valuation(self) -> Comparison {
        self.valuation
    }
    #[must_use]
    pub const fn profit(self) -> Comparison {
        self.profit
    }
    #[must_use]
    pub const fn return_rate(self) -> Comparison {
        self.return_rate
    }
    #[must_use]
    pub fn overall(self) -> Comparison {
        let values = [self.valuation, self.profit, self.return_rate];
        if values.contains(&Comparison::Different) {
            Comparison::Different
        } else if values.contains(&Comparison::Unavailable) {
            Comparison::Unavailable
        } else if values.contains(&Comparison::NotProvided) {
            Comparison::NotProvided
        } else {
            Comparison::Matches
        }
    }
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
    valuation: Result<SelectedValue<Money>, EvaluationError>,
    profit: Result<SelectedValue<Money>, EvaluationError>,
    return_rate: Result<SelectedValue<ReturnObservation>, EvaluationError>,
    comparisons: HoldingComparisons,
    daily_profit_loss: Option<DailyProfitLoss>,
}
impl HoldingValuation {
    pub const fn computed(self) -> Result<ComputedHolding, EvaluationError> {
        self.computed
    }
    pub fn selected(self) -> Result<Money, EvaluationError> {
        self.valuation.map(SelectedValue::value)
    }
    pub const fn selected_valuation(self) -> Result<SelectedValue<Money>, EvaluationError> {
        self.valuation
    }
    pub const fn selected_profit(self) -> Result<SelectedValue<Money>, EvaluationError> {
        self.profit
    }
    pub const fn selected_return(
        self,
    ) -> Result<SelectedValue<ReturnObservation>, EvaluationError> {
        self.return_rate
    }
    #[must_use]
    pub fn source(self) -> Option<ValuationSource> {
        self.valuation.map(SelectedValue::source).ok()
    }
    #[must_use]
    pub fn comparison(self) -> Comparison {
        self.comparisons.overall()
    }
    #[must_use]
    pub const fn comparisons(self) -> HoldingComparisons {
        self.comparisons
    }
    #[must_use]
    pub const fn daily_profit_loss(self) -> Option<DailyProfitLoss> {
        self.daily_profit_loss
    }
    pub(crate) fn evaluate(holding: &Holding) -> Self {
        let computed = Self::compute(holding);
        let basis = ReturnBasis::TradingCurrency(holding.currency());
        let comparisons = compare_holding(computed, holding.reported());
        let (valuation, profit, return_rate) = match holding.reported() {
            Some(report) => (
                Ok(SelectedValue::new(
                    report.valuation(),
                    ValuationSource::TossOpenApi,
                )),
                Ok(SelectedValue::new(
                    report.profit(),
                    ValuationSource::TossOpenApi,
                )),
                Ok(SelectedValue::new(
                    ReturnObservation {
                        rate: HoldingReturn::Defined(RatioQuotient::Exact(report.return_rate())),
                        basis,
                    },
                    ValuationSource::TossOpenApi,
                )),
            ),
            None => (
                computed.map(|value| {
                    SelectedValue::new(value.valuation(), ValuationSource::Calculated)
                }),
                computed
                    .map(|value| SelectedValue::new(value.profit(), ValuationSource::Calculated)),
                computed
                    .and_then(|value| value.return_rate().map_err(EvaluationError::Arithmetic))
                    .map(|rate| {
                        SelectedValue::new(
                            ReturnObservation { rate, basis },
                            ValuationSource::Calculated,
                        )
                    }),
            ),
        };
        Self {
            computed,
            valuation,
            profit,
            return_rate,
            comparisons,
            daily_profit_loss: holding
                .reported()
                .and_then(crate::ReportedHolding::daily_profit_loss),
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
pub(crate) fn compare_money(computed: Money, reported: Money) -> Comparison {
    if computed.currency() != reported.currency() {
        return Comparison::Different;
    }
    crate::exact_arithmetic::compare_reported(computed.amount(), reported.amount())
}
fn compare_holding(
    computed: Result<ComputedHolding, EvaluationError>,
    reported: Option<crate::ReportedHolding>,
) -> HoldingComparisons {
    match (computed, reported) {
        (_, None) => HoldingComparisons {
            valuation: Comparison::NotProvided,
            profit: Comparison::NotProvided,
            return_rate: Comparison::NotProvided,
        },
        (Err(_), Some(_)) => HoldingComparisons {
            valuation: Comparison::Unavailable,
            profit: Comparison::Unavailable,
            return_rate: Comparison::Unavailable,
        },
        (Ok(value), Some(report)) => HoldingComparisons {
            valuation: compare_money(value.valuation(), report.valuation()),
            profit: compare_money(value.profit(), report.profit()),
            return_rate: compare_return(value.return_rate(), report.return_rate()),
        },
    }
}
pub(crate) fn compare_return(
    value: Result<HoldingReturn, MoneyError>,
    reported: crate::Ratio,
) -> Comparison {
    match value {
        Ok(HoldingReturn::Defined(quotient)) => {
            let decimal = match quotient {
                RatioQuotient::Exact(rate) => rate.as_decimal(),
                RatioQuotient::Approximate(rate) => rate.as_decimal(),
            };
            crate::exact_arithmetic::compare_reported(
                Amount::new(decimal),
                Amount::new(reported.as_decimal()),
            )
        }
        Ok(HoldingReturn::ZeroCost) | Err(_) => Comparison::Unavailable,
    }
}
