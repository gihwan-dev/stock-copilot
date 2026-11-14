use crate::{
    Comparison, Currency, CurrencyAmounts, KrwValuation, Money, PortfolioDailyProfitLoss,
    SelectedValue, SnapshotCalendar, SnapshotDates, SnapshotError, SnapshotSource,
    TotalCompleteness, ValuationSnapshot, ValuationSource,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapshotPrevious {
    Missing,
    Observed,
    Recalculated,
    Incomplete,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SnapshotExternalFlows {
    dates: SnapshotDates,
    amounts: Option<CurrencyAmounts>,
}
impl SnapshotExternalFlows {
    #[must_use]
    pub const fn classified(calendar: &SnapshotCalendar, amounts: CurrencyAmounts) -> Self {
        Self {
            dates: calendar.dates(),
            amounts: Some(amounts),
        }
    }
    #[must_use]
    pub const fn unavailable(calendar: &SnapshotCalendar) -> Self {
        Self {
            dates: calendar.dates(),
            amounts: None,
        }
    }
    pub fn amount(self, currency: Currency) -> Result<Money, SnapshotError> {
        self.amounts
            .map(|value| value.resolved(currency))
            .ok_or(SnapshotError::MissingFlows)
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DailyCurrencyProfit {
    computed: Result<Money, SnapshotError>,
    selected: Result<SelectedValue<Money>, SnapshotError>,
    comparison: Comparison,
}
impl DailyCurrencyProfit {
    fn new(computed: Result<Money, SnapshotError>, reported: Option<Money>) -> Self {
        let (selected, comparison) = match reported {
            Some(value) => (
                Ok(SelectedValue::new(value, ValuationSource::TossOpenApi)),
                computed.map_or(Comparison::Unavailable, |calculated| {
                    crate::holding_valuation::compare_money(calculated, value)
                }),
            ),
            None => (
                computed.map(|value| SelectedValue::new(value, ValuationSource::Calculated)),
                Comparison::NotProvided,
            ),
        };
        Self {
            computed,
            selected,
            comparison,
        }
    }
    pub const fn computed(self) -> Result<Money, SnapshotError> {
        self.computed
    }
    pub const fn selected(self) -> Result<SelectedValue<Money>, SnapshotError> {
        self.selected
    }
    #[must_use]
    pub fn source(self) -> Option<ValuationSource> {
        self.selected.ok().map(SelectedValue::source)
    }
    #[must_use]
    pub const fn comparison(self) -> Comparison {
        self.comparison
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DailyPortfolioProfit {
    krw: DailyCurrencyProfit,
    usd: DailyCurrencyProfit,
    converted: Result<KrwValuation, SnapshotError>,
    selected_converted: Result<KrwValuation, SnapshotError>,
    previous: SnapshotPrevious,
    current_source: SnapshotSource,
    reported: Option<PortfolioDailyProfitLoss>,
}
impl DailyPortfolioProfit {
    #[must_use]
    pub fn calculate(
        calendar: &SnapshotCalendar,
        today: &ValuationSnapshot,
        previous: Option<&ValuationSnapshot>,
        flows: &SnapshotExternalFlows,
        reported: Option<PortfolioDailyProfitLoss>,
    ) -> Self {
        let previous_state = match previous {
            None => SnapshotPrevious::Missing,
            Some(value) if value.completeness() == TotalCompleteness::Incomplete => {
                SnapshotPrevious::Incomplete
            }
            Some(value) => match value.source() {
                SnapshotSource::Observed => SnapshotPrevious::Observed,
                SnapshotSource::Recalculated => SnapshotPrevious::Recalculated,
            },
        };
        let validated = previous
            .ok_or(SnapshotError::MissingPrevious)
            .and_then(|value| {
                let prior_dates = value.dates();
                if today.dates() != calendar.dates()
                    || prior_dates != calendar.previous_dates()
                    || flows.dates != calendar.dates()
                {
                    return Err(SnapshotError::WrongPeriod);
                }
                Ok(value)
            });
        let difference = |currency| {
            let old = validated?;
            currency_difference(
                today.currency_value(currency),
                old.currency_value(currency),
                flows.amount(currency),
            )
        };
        let krw = DailyCurrencyProfit::new(
            difference(Currency::Krw),
            reported.and_then(|value| value.amounts().amount(Currency::Krw)),
        );
        let usd = DailyCurrencyProfit::new(
            difference(Currency::Usd),
            reported.and_then(|value| value.amounts().amount(Currency::Usd)),
        );
        let converted = validated.and_then(|old| converted_difference(today, old, flows));
        let selected_converted = match reported {
            Some(_value) => krw.selected().and_then(|native| {
                crate::valuation_snapshot::aggregate(
                    native.value(),
                    usd.selected()?.value(),
                    today.fx(),
                    today.dates().date(Currency::Usd),
                )
            }),
            None => converted.clone(),
        };
        Self {
            krw,
            usd,
            converted,
            selected_converted,
            previous: previous_state,
            current_source: today.source(),
            reported,
        }
    }
    #[must_use]
    pub const fn currency(&self, currency: Currency) -> DailyCurrencyProfit {
        match currency {
            Currency::Krw => self.krw,
            Currency::Usd => self.usd,
        }
    }
    pub fn krw_computed(&self) -> Result<&KrwValuation, SnapshotError> {
        self.converted.as_ref().map_err(|error| *error)
    }
    pub fn krw_selected(&self) -> Result<&KrwValuation, SnapshotError> {
        self.selected_converted.as_ref().map_err(|error| *error)
    }
    #[must_use]
    pub const fn krw_comparison(&self) -> Comparison {
        match self.reported {
            Some(_) => Comparison::Unavailable,
            None => Comparison::NotProvided,
        }
    }
    #[must_use]
    pub const fn previous(&self) -> SnapshotPrevious {
        self.previous
    }
    #[must_use]
    pub const fn current_source(&self) -> SnapshotSource {
        self.current_source
    }
    #[must_use]
    pub const fn reported(&self) -> Option<PortfolioDailyProfitLoss> {
        self.reported
    }
}
fn negative(value: Money) -> Money {
    let mut amount = value.amount().as_decimal();
    amount.set_sign_negative(!amount.is_sign_negative());
    Money::new(crate::Amount::new(amount), value.currency())
}
fn converted_difference(
    today: &ValuationSnapshot,
    previous: &ValuationSnapshot,
    flows: &SnapshotExternalFlows,
) -> Result<KrwValuation, SnapshotError> {
    let old = crate::valuation_snapshot::aggregate(
        negative(previous.currency_value(Currency::Krw)?),
        negative(previous.currency_value(Currency::Usd)?),
        previous.fx(),
        previous.dates().date(Currency::Usd),
    )?;
    let amounts = flows.amounts.ok_or(SnapshotError::MissingFlows)?;
    let external = crate::valuation_snapshot::aggregate(
        negative(amounts.resolved(Currency::Krw)),
        negative(amounts.resolved(Currency::Usd)),
        today.fx(),
        today.dates().date(Currency::Usd),
    )?;
    today
        .krw_value()?
        .checked_add(&old)
        .and_then(|value| value.checked_add(&external))
        .map_err(SnapshotError::Conversion)
}

fn currency_difference(
    current: Result<Money, SnapshotError>,
    previous: Result<Money, SnapshotError>,
    flow: Result<Money, SnapshotError>,
) -> Result<Money, SnapshotError> {
    let mut result = current?;
    for amount in [previous, flow] {
        result = result
            .checked_sub(&amount?)
            .map_err(SnapshotError::Arithmetic)?;
    }
    Ok(result)
}
