use crate::performance_arithmetic::{Number, PerformanceValue};
use crate::{
    BuyingPower, CalendarDate, Candle, CandlePeriod, CandleStatus, CashLedger, CashLedgerInput,
    CashUnavailable, ConfirmedFlowPeriod, Currency, DatedSnapshotLedgers, ExternalFlows,
    FlowPeriod, FxRate, InstrumentId, LedgerEvidence, Money, Performance, PerformanceError,
    PriceAdjustment, RiskMetric, SettledRowSet, SettlementWindow, SnapshotCalendar, SnapshotError,
    SnapshotInstrumentHistory, SnapshotLedgers, SnapshotObservation, SnapshotPriceBasis, Timestamp,
    ValuationHistoryEvidence, ValuationPriceEvidence, ValuationRecomputeInput, ValuationSnapshot,
    valuation_time,
};
use rust_decimal::Decimal;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PerformanceSpan {
    OneMonth,
    ThreeMonths,
    SixMonths,
    OneYear,
}
impl PerformanceSpan {
    const fn days(self) -> crate::CalendarDays {
        crate::CalendarDays::new(match self {
            Self::OneMonth => 30,
            Self::ThreeMonths => 91,
            Self::SixMonths => 182,
            Self::OneYear => 365,
        })
    }
}

pub fn last_valuation_date(now: Timestamp) -> Result<CalendarDate, SnapshotError> {
    let today = crate::valuation_snapshot_calendar::kst_date(now)?;
    if valuation_time(today) <= now {
        Ok(today)
    } else {
        Ok(today.inspection_predecessor())
    }
}

pub fn performance_window(
    now: Timestamp,
    span: PerformanceSpan,
) -> Result<SettlementWindow, SnapshotError> {
    let end = last_valuation_date(now)?;
    SettlementWindow::try_new(end.days_before(span.days()), end)
        .map_err(|_error| SnapshotError::WrongPeriod)
}

#[must_use]
pub fn window_dates(window: SettlementWindow) -> Vec<CalendarDate> {
    let mut dates = Vec::new();
    let mut day = window.from();
    while day <= window.to() && dates.len() < 4096 {
        dates.push(day);
        day = day.inspection_successor();
    }
    dates
}

pub fn settled_cash_through(
    rows: &SettledRowSet,
    currency: Currency,
    through: CalendarDate,
) -> Result<Money, CashUnavailable> {
    let mut selected = rows
        .active_settled_for_calculation()
        .filter(|row| row.data().currency == currency && row.data().settlement_date <= through)
        .collect::<Vec<_>>();
    selected.sort_unstable();
    selected.iter().try_fold(
        Money::new(crate::Amount::new(Decimal::ZERO), currency),
        |total, row| {
            total
                .checked_add(&crate::cash_ledger::cash_delta(row.data())?)
                .map_err(CashUnavailable::Arithmetic)
        },
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowLedger {
    ledger: CashLedger,
    period: ConfirmedFlowPeriod,
}
impl WindowLedger {
    #[must_use]
    pub const fn ledger(&self) -> &CashLedger {
        &self.ledger
    }
    #[must_use]
    pub const fn period(&self) -> &ConfirmedFlowPeriod {
        &self.period
    }
    #[must_use]
    pub fn flow_period(&self) -> FlowPeriod {
        FlowPeriod::Confirmed(self.period.clone())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CashAnchor {
    TransactionsOnly,
    ReportedMatches,
    ReportedWithOtherFlows,
}

fn settled_cash_total(rows: &SettledRowSet, currency: Currency) -> Result<Money, CashUnavailable> {
    rows.active_settled_for_calculation()
        .filter(|row| row.data().currency == currency)
        .try_fold(
            Money::new(crate::Amount::new(Decimal::ZERO), currency),
            |total, row| {
                total
                    .checked_add(&crate::cash_ledger::cash_delta(row.data())?)
                    .map_err(CashUnavailable::Arithmetic)
            },
        )
}

fn window_ledger(
    rows: &SettledRowSet,
    window: SettlementWindow,
    currency: Currency,
    evidence: LedgerEvidence,
    (opening, ending, observed_at): (Money, Money, Timestamp),
) -> Result<WindowLedger, CashUnavailable> {
    let ledger = CashLedger::reconstruct(
        rows,
        CashLedgerInput {
            window,
            currency,
            opening: Some(opening),
            evidence: Some(evidence),
            reported: Some(BuyingPower::new(ending, observed_at)),
        },
    );
    match ledger.external_flows(&[]) {
        ExternalFlows::Reconciled(flows) => match flows.period(window) {
            FlowPeriod::Confirmed(period) => Ok(WindowLedger { ledger, period }),
            FlowPeriod::Uncertain(_) => Err(CashUnavailable::IncompleteResponse),
        },
        ExternalFlows::Unavailable(reason) => Err(reason),
    }
}

pub fn anchored_window_ledger(
    rows: &SettledRowSet,
    window: SettlementWindow,
    currency: Currency,
    evidence: LedgerEvidence,
    reported: BuyingPower,
) -> Result<(WindowLedger, CashAnchor), CashUnavailable> {
    let now = reported
        .money()
        .map_err(|_error| CashUnavailable::InvalidReportedCash)?;
    if now.currency() != currency {
        return Err(CashUnavailable::CurrencyMismatch);
    }
    let total = settled_cash_total(rows, currency)?;
    let through_end = settled_cash_through(rows, currency, window.to())?;
    let before_start =
        settled_cash_through(rows, currency, window.from().inspection_predecessor())?;
    let arithmetic = CashUnavailable::Arithmetic;
    let ending = now
        .checked_sub(&total.checked_sub(&through_end).map_err(arithmetic)?)
        .map_err(arithmetic)?;
    let opening = ending
        .checked_sub(&through_end.checked_sub(&before_start).map_err(arithmetic)?)
        .map_err(arithmetic)?;
    let anchor = if total == now {
        CashAnchor::ReportedMatches
    } else {
        CashAnchor::ReportedWithOtherFlows
    };
    window_ledger(
        rows,
        window,
        currency,
        evidence,
        (opening, ending, reported.observed_at()),
    )
    .map(|ledger| (ledger, anchor))
}

pub fn derived_window_ledger(
    rows: &SettledRowSet,
    window: SettlementWindow,
    currency: Currency,
    evidence: LedgerEvidence,
    observed_at: Timestamp,
) -> Result<WindowLedger, CashUnavailable> {
    let opening = settled_cash_through(rows, currency, window.from().inspection_predecessor())?;
    let ending = settled_cash_through(rows, currency, window.to())?;
    window_ledger(
        rows,
        window,
        currency,
        evidence,
        (opening, ending, observed_at),
    )
}

fn usable(
    candle: &Candle,
    instrument: &InstrumentId,
    currency: Currency,
) -> Result<(), SnapshotError> {
    if candle.instrument() == instrument
        && candle.values().close().as_money().currency() == currency
        && candle.time().period() == CandlePeriod::Daily
        && candle.status() == CandleStatus::Finalized
        && candle.adjustment() == PriceAdjustment::Raw
    {
        Ok(())
    } else {
        Err(SnapshotError::InvalidCandle)
    }
}

pub fn day_price_evidence(
    calendar: SnapshotCalendar,
    history: &SnapshotInstrumentHistory,
    at: Timestamp,
) -> Result<ValuationPriceEvidence, SnapshotError> {
    let identity = history.identity();
    let currency = identity.currency();
    let (date, basis) = calendar.price(currency, at)?;
    let candle = history
        .candles()
        .iter()
        .find(|candle| {
            candle.time().date() == date && usable(candle, identity.instrument(), currency).is_ok()
        })
        .ok_or(SnapshotError::MissingPrice)?;
    let price = candle.values().close();
    Ok(match basis {
        SnapshotPriceBasis::CurrentSession => ValuationPriceEvidence::SessionClose { price, date },
        SnapshotPriceBasis::HolidayCarry => ValuationPriceEvidence::HolidayCarry { price, date },
        SnapshotPriceBasis::AwaitingSession => {
            ValuationPriceEvidence::AwaitingSession { price, date }
        }
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowDay {
    pub calendar: SnapshotCalendar,
    pub fx: Option<FxRate>,
}

pub fn recompute_window_day(
    day: &WindowDay,
    rows: &SettledRowSet,
    histories: &[SnapshotInstrumentHistory],
    ledgers: (&WindowLedger, &WindowLedger),
    recorded_at: Timestamp,
) -> Result<ValuationSnapshot, SnapshotError> {
    let date = day.calendar.dates().date(Currency::Krw);
    let at = valuation_time(date);
    let observation = SnapshotObservation::new(at, recorded_at, day.fx);
    let pair = SnapshotLedgers::new(
        (&ledgers.0.ledger, &ledgers.0.period),
        (&ledgers.1.ledger, &ledgers.1.period),
    );
    let first =
        ValuationSnapshot::recalculate(&day.calendar, rows, histories, Some(pair), observation)?;
    let mut evidence = Vec::new();
    for position in first.positions() {
        if position.value()?.amount().as_decimal().is_zero() {
            continue;
        }
        let history = histories
            .iter()
            .find(|history| history.identity().instrument() == position.instrument())
            .ok_or(SnapshotError::MissingHistory)?;
        evidence.push((
            position.instrument().clone(),
            day_price_evidence(day.calendar, history, at)?,
        ));
    }
    let input = ValuationRecomputeInput::new(
        &day.calendar,
        rows,
        ValuationHistoryEvidence::new(histories, &evidence),
        DatedSnapshotLedgers::new(date, pair),
        observation,
    )?;
    ValuationSnapshot::recompute(input)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DayFx {
    OfTheDay,
    StaleWithSmallDollarShare,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DollarShare {
    Small,
    Material,
}

fn dollar_share(snapshot: &ValuationSnapshot, rate: FxRate) -> Result<DollarShare, SnapshotError> {
    let dollars = snapshot.account_value(Currency::Usd)?.amount().as_decimal();
    let total = snapshot
        .account_krw_value()?
        .clone()
        .into_parts()
        .0
        .amount()
        .as_decimal();
    if total <= Decimal::ZERO {
        return Ok(DollarShare::Material);
    }
    let converted = dollars
        .abs()
        .checked_mul(rate.quote_per_base().as_money().amount().as_decimal())
        .ok_or(SnapshotError::MissingFx)?;
    let share = converted
        .checked_div(total)
        .ok_or(SnapshotError::MissingFx)?;
    let small = Decimal::new(1, 3);
    Ok(if share < small {
        DollarShare::Small
    } else {
        DollarShare::Material
    })
}

pub fn recompute_window_day_with_fallback(
    day: &WindowDay,
    rows: &SettledRowSet,
    histories: &[SnapshotInstrumentHistory],
    ledgers: (&WindowLedger, &WindowLedger),
    (recorded_at, fallback): (Timestamp, Option<FxRate>),
) -> Result<(ValuationSnapshot, DayFx), SnapshotError> {
    let original = match recompute_window_day(day, rows, histories, ledgers, recorded_at) {
        Ok(snapshot) => return Ok((snapshot, DayFx::OfTheDay)),
        Err(reason @ (SnapshotError::MissingFx | SnapshotError::InvalidFx)) => reason,
        Err(other) => return Err(other),
    };
    let at = valuation_time(day.calendar.dates().date(Currency::Krw));
    let carried = fallback.and_then(|rate| {
        let until = at.checked_add(crate::TimeSpan::DAY).ok()?;
        rate.carried_to(crate::TimeInterval::try_new(at, until).ok()?)
    });
    let Some(rate) = carried else {
        return Err(original);
    };
    let retried = WindowDay {
        calendar: day.calendar,
        fx: Some(rate),
    };
    let snapshot = recompute_window_day(&retried, rows, histories, ledgers, recorded_at)?;
    match dollar_share(&snapshot, rate)? {
        DollarShare::Small => Ok((snapshot, DayFx::StaleWithSmallDollarShare)),
        DollarShare::Material => Err(original),
    }
}

#[must_use]
pub fn performance_max_drawdown(performance: &Performance) -> RiskMetric {
    if performance.returns.is_empty() {
        return RiskMetric::InsufficientSamples;
    }
    match drawdown(&performance.returns) {
        Ok(value) => RiskMetric::Value(value),
        Err(cause) => RiskMetric::Unavailable(cause),
    }
}

fn drawdown(returns: &[PerformanceValue]) -> Result<PerformanceValue, PerformanceError> {
    let one = Number::new(Decimal::ONE);
    let mut wealth = one;
    let mut peak = one;
    let mut worst = Number::new(Decimal::ZERO);
    for value in returns {
        wealth = wealth.mul(one.add(value.number())?)?;
        if wealth > peak {
            peak = wealth;
        }
        let current = wealth.div(peak)?.sub(one)?;
        if current < worst {
            worst = current;
        }
    }
    Ok(PerformanceValue::from_number(worst))
}

#[cfg(test)]
mod tests;
