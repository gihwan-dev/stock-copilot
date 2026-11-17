use crate::{
    CalendarDate, Candle, CandlePeriod, CandleStatus, Currency, InstrumentId, PriceAdjustment,
    SettledRowSet, SnapshotCalendar, SnapshotError, SnapshotInstrumentHistory, SnapshotLedgers,
    SnapshotObservation, ValuationPriceEvidence, ValuationSnapshot, valuation_time,
};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy)]
pub struct ValuationHistoryEvidence<'action> {
    histories: &'action [SnapshotInstrumentHistory],
    evidence: &'action [(InstrumentId, ValuationPriceEvidence)],
}

impl<'action> ValuationHistoryEvidence<'action> {
    #[must_use]
    pub const fn new(
        histories: &'action [SnapshotInstrumentHistory],
        evidence: &'action [(InstrumentId, ValuationPriceEvidence)],
    ) -> Self {
        Self {
            histories,
            evidence,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct DatedSnapshotLedgers<'action> {
    date: CalendarDate,
    ledgers: SnapshotLedgers<'action>,
}

impl<'action> DatedSnapshotLedgers<'action> {
    #[must_use]
    pub const fn new(date: CalendarDate, ledgers: SnapshotLedgers<'action>) -> Self {
        Self { date, ledgers }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ValuationRecomputeInput<'action> {
    calendar: &'action SnapshotCalendar,
    rows: &'action SettledRowSet,
    history: ValuationHistoryEvidence<'action>,
    ledgers: SnapshotLedgers<'action>,
    observation: SnapshotObservation,
}

impl<'action> ValuationRecomputeInput<'action> {
    pub fn new(
        calendar: &'action SnapshotCalendar,
        rows: &'action SettledRowSet,
        history: ValuationHistoryEvidence<'action>,
        ledgers: DatedSnapshotLedgers<'action>,
        observation: SnapshotObservation,
    ) -> Result<Self, SnapshotError> {
        let date = calendar.dates().date(Currency::Krw);
        let at = valuation_time(date);
        if observation.valuation_at() != at || ledgers.date != date {
            return Err(SnapshotError::WrongPeriod);
        }
        let checked_observation =
            SnapshotObservation::new(at, observation.recorded_at(), observation.fx());
        let input = Self {
            calendar,
            rows,
            history,
            ledgers: ledgers.ledgers,
            observation: checked_observation,
        };
        input.calculate()?;
        Ok(input)
    }

    pub(crate) fn calculate(&self) -> Result<ValuationSnapshot, SnapshotError> {
        let snapshot = ValuationSnapshot::recalculate(
            self.calendar,
            self.rows,
            self.history.histories,
            Some(self.ledgers),
            self.observation,
        )?;
        let mut seen = BTreeSet::new();
        for (instrument, evidence) in self.history.evidence {
            if !seen.insert(instrument) {
                return Err(SnapshotError::DuplicateInstrument);
            }
            let position = snapshot
                .positions()
                .iter()
                .find(|item| item.instrument() == instrument)
                .ok_or(SnapshotError::MissingHistory)?;
            let value = position.value()?;
            let price = evidence.closing_price(
                *self.calendar,
                value.currency(),
                self.observation.valuation_at(),
            )?;
            let candle = position.candle().ok_or(SnapshotError::InvalidCandle)?;
            validate_candle(candle, instrument, value.currency())?;
            if candle.values().close() != price {
                return Err(SnapshotError::InvalidCandle);
            }
        }
        for position in snapshot.positions() {
            let value = position.value()?;
            if !value.amount().as_decimal().is_zero() && !seen.contains(position.instrument()) {
                return Err(SnapshotError::MissingPrice);
            }
            if !seen.contains(position.instrument())
                && let Some(candle) = position.candle()
            {
                validate_candle(candle, position.instrument(), value.currency())?;
            }
        }
        snapshot.stock_krw_value()?;
        snapshot.account_krw_value()?;
        Ok(snapshot)
    }
}

fn validate_candle(
    candle: &Candle,
    instrument: &InstrumentId,
    currency: Currency,
) -> Result<(), SnapshotError> {
    if candle.instrument() != instrument
        || candle.values().close().as_money().currency() != currency
        || candle.time().period() != CandlePeriod::Daily
        || candle.status() != CandleStatus::Finalized
        || candle.adjustment() != PriceAdjustment::Raw
    {
        return Err(SnapshotError::InvalidCandle);
    }
    Ok(())
}
