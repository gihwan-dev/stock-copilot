use crate::{
    Comparison, Currency, Money, PortfolioDailyProfitLoss, SettledRowSet, SnapshotError,
    SnapshotSource, TotalCompleteness, TransactionKind, ValuationSnapshot, ValuationSource,
};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SnapshotPrevious {
    source: SnapshotSource,
    stocks: TotalCompleteness,
    account: TotalCompleteness,
}
impl SnapshotPrevious {
    #[must_use]
    pub const fn source(self) -> SnapshotSource {
        self.source
    }
    #[must_use]
    pub const fn stock_completeness(self) -> TotalCompleteness {
        self.stocks
    }
    #[must_use]
    pub const fn account_completeness(self) -> TotalCompleteness {
        self.account
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HoldingsDailyOrigin {
    TossSummary,
    TossHoldings,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HoldingsDailySelection {
    amount: Money,
    origin: HoldingsDailyOrigin,
    previous: Result<SnapshotSource, SnapshotError>,
}
impl HoldingsDailySelection {
    #[must_use]
    pub const fn amount(self) -> Money {
        self.amount
    }
    #[must_use]
    pub const fn origin(self) -> HoldingsDailyOrigin {
        self.origin
    }
    #[must_use]
    pub const fn source(self) -> ValuationSource {
        match self.origin {
            HoldingsDailyOrigin::TossSummary | HoldingsDailyOrigin::TossHoldings => {
                ValuationSource::TossOpenApi
            }
        }
    }
    pub const fn previous_source(self) -> Result<SnapshotSource, SnapshotError> {
        self.previous
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HoldingsDailyCurrency {
    computed: Result<Money, SnapshotError>,
    selected: Result<HoldingsDailySelection, SnapshotError>,
    comparison: Comparison,
}
impl HoldingsDailyCurrency {
    fn select(
        computed: Result<Money, SnapshotError>,
        reported: Result<(Money, HoldingsDailyOrigin), SnapshotError>,
        previous: Result<SnapshotSource, SnapshotError>,
        override_comparison: Option<Comparison>,
    ) -> Self {
        let comparison = match reported {
            Ok((value, _origin)) => match override_comparison {
                Some(comparison) => comparison,
                None => computed.map_or(Comparison::Unavailable, |calculated| {
                    crate::holding_valuation::compare_money(calculated, value)
                }),
            },
            Err(_) => Comparison::NotProvided,
        };
        Self {
            computed,
            selected: reported.map(|(amount, origin)| HoldingsDailySelection {
                amount,
                origin,
                previous,
            }),
            comparison,
        }
    }
    pub const fn computed(self) -> Result<Money, SnapshotError> {
        self.computed
    }
    pub const fn selected(self) -> Result<HoldingsDailySelection, SnapshotError> {
        self.selected
    }
    #[must_use]
    pub const fn comparison(self) -> Comparison {
        self.comparison
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HoldingsDailyProfit {
    kr: HoldingsDailyCurrency,
    us: HoldingsDailyCurrency,
    previous: Result<SnapshotPrevious, SnapshotError>,
    reported: Option<PortfolioDailyProfitLoss>,
    krw_cmp: Comparison,
}
impl HoldingsDailyProfit {
    #[must_use]
    pub fn compare(
        today: &ValuationSnapshot,
        previous: Option<&ValuationSnapshot>,
        rows: &SettledRowSet,
        reported: Option<PortfolioDailyProfitLoss>,
    ) -> Self {
        let pair = validated_previous(today, previous);
        let state = pair.map(|(state, _old)| state);
        let one = |currency| {
            let computed =
                pair.and_then(|(_state, old)| holding_change(today, old, rows, currency));
            let provided = match reported.and_then(|record| record.amounts().amount(currency)) {
                Some(amount) => Ok((amount, HoldingsDailyOrigin::TossSummary)),
                None => match reported {
                    Some(_) => Err(SnapshotError::MissingReportedValue),
                    None => today
                        .reported_stock_daily(currency)
                        .map(|entry| (entry, HoldingsDailyOrigin::TossHoldings)),
                },
            };
            let uncertain_krw = reported.is_some()
                && currency == Currency::Krw
                && (!today
                    .stock_value(Currency::Usd)
                    .is_ok_and(|entry| entry.amount().as_decimal().is_zero())
                    || previous.is_some_and(|old| {
                        !old.stock_value(Currency::Usd)
                            .is_ok_and(|entry| entry.amount().as_decimal().is_zero())
                    }));
            HoldingsDailyCurrency::select(
                computed,
                provided,
                state.map(SnapshotPrevious::source),
                uncertain_krw.then_some(Comparison::Unavailable),
            )
        };
        Self {
            kr: one(Currency::Krw),
            us: one(Currency::Usd),
            previous: state,
            reported,
            krw_cmp: Comparison::Unavailable,
        }
    }
    #[must_use]
    pub const fn currency(&self, currency: Currency) -> HoldingsDailyCurrency {
        match currency {
            Currency::Krw => self.kr,
            Currency::Usd => self.us,
        }
    }
    pub const fn previous(&self) -> Result<SnapshotPrevious, SnapshotError> {
        self.previous
    }
    pub fn krw_selected(&self) -> Result<Money, SnapshotError> {
        self.reported
            .and_then(|record| record.amounts().amount(Currency::Krw))
            .ok_or(SnapshotError::MissingReportedValue)
    }
    #[must_use]
    pub const fn krw_comparison(&self) -> Comparison {
        self.krw_cmp
    }
    #[must_use]
    pub const fn reported(&self) -> Option<PortfolioDailyProfitLoss> {
        self.reported
    }
}
pub(crate) fn validated_previous<'action>(
    today: &ValuationSnapshot,
    old: Option<&'action ValuationSnapshot>,
) -> Result<(SnapshotPrevious, &'action ValuationSnapshot), SnapshotError> {
    let previous = old.ok_or(SnapshotError::MissingPrevious)?;
    if previous.valuation_at() >= today.valuation_at() {
        return Err(SnapshotError::WrongPeriod);
    }
    Ok((
        SnapshotPrevious {
            source: previous.source(),
            stocks: previous.stock_completeness(),
            account: previous.completeness(),
        },
        previous,
    ))
}
fn holding_change(
    today: &ValuationSnapshot,
    previous: &ValuationSnapshot,
    rows: &SettledRowSet,
    currency: Currency,
) -> Result<Money, SnapshotError> {
    let trade = |row: &crate::Transaction| {
        row.data().currency == currency
            && (row.data().kind == TransactionKind::Buy || row.data().kind == TransactionKind::Sell)
    };
    if rows.last_synchronized_at().is_none()
        || rows.active_settled_for_calculation().any(trade)
        || rows.pending_rows().as_slice().iter().any(trade)
    {
        return Err(SnapshotError::MissingExecutionDate);
    }
    today
        .stock_value(currency)?
        .checked_sub(&previous.stock_value(currency)?)
        .map_err(SnapshotError::Arithmetic)
}
