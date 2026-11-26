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

pub fn derived_window_ledger(
    rows: &SettledRowSet,
    window: SettlementWindow,
    currency: Currency,
    evidence: LedgerEvidence,
    observed_at: Timestamp,
) -> Result<WindowLedger, CashUnavailable> {
    let opening = settled_cash_through(rows, currency, window.from().inspection_predecessor())?;
    let ending = settled_cash_through(rows, currency, window.to())?;
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

fn usable(candle: &Candle, instrument: &InstrumentId, currency: Currency) -> bool {
    candle.instrument() == instrument
        && candle.values().close().as_money().currency() == currency
        && candle.time().period() == CandlePeriod::Daily
        && candle.status() == CandleStatus::Finalized
        && candle.adjustment() == PriceAdjustment::Raw
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
            candle.time().date() == date && usable(candle, identity.instrument(), currency)
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
