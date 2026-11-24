use crate::{
    CalendarDate, CashLedger, ConfirmedFlowPeriod, DatedSnapshotLedgers, InstrumentId,
    SettledRowSet, SnapshotCalendar, SnapshotError, SnapshotInstrumentHistory, SnapshotLedgers,
    SnapshotObservation, TransferClassification, ValuationHistoryEvidence, ValuationPriceEvidence,
    ValuationRecomputeInput, ValuationSnapshot,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DayCashLedger {
    ledger: CashLedger,
    period: ConfirmedFlowPeriod,
    classifications: Vec<TransferClassification>,
}

impl DayCashLedger {
    #[must_use]
    pub fn new(ledger: CashLedger, period: ConfirmedFlowPeriod) -> Self {
        Self {
            ledger,
            period,
            classifications: Vec::new(),
        }
    }
    #[must_use]
    pub fn with_transfer_classifications(
        mut self,
        classifications: Vec<TransferClassification>,
    ) -> Self {
        self.classifications = classifications;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValuationDayHistory {
    histories: Vec<SnapshotInstrumentHistory>,
    evidence: Vec<(InstrumentId, ValuationPriceEvidence)>,
}

impl ValuationDayHistory {
    #[must_use]
    pub fn new(
        histories: Vec<SnapshotInstrumentHistory>,
        evidence: Vec<(InstrumentId, ValuationPriceEvidence)>,
    ) -> Self {
        Self {
            histories,
            evidence,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValuationDayInputs {
    date: CalendarDate,
    rows: SettledRowSet,
    history: ValuationDayHistory,
    kr: DayCashLedger,
    us: DayCashLedger,
    observation: SnapshotObservation,
}

impl ValuationDayInputs {
    #[must_use]
    pub fn new(
        date: CalendarDate,
        rows: SettledRowSet,
        history: ValuationDayHistory,
        cash: (DayCashLedger, DayCashLedger),
        observation: SnapshotObservation,
    ) -> Self {
        let (kr, us) = cash;
        Self {
            date,
            rows,
            history,
            kr,
            us,
            observation,
        }
    }
    #[must_use]
    pub const fn date(&self) -> CalendarDate {
        self.date
    }
    pub fn recompute(
        &self,
        date: CalendarDate,
        calendar: &SnapshotCalendar,
    ) -> Result<ValuationSnapshot, SnapshotError> {
        if self.date != date {
            return Err(SnapshotError::WrongPeriod);
        }
        let ledgers = SnapshotLedgers::new(
            (&self.kr.ledger, &self.kr.period),
            (&self.us.ledger, &self.us.period),
        )
        .with_transfer_classifications(&self.kr.classifications, &self.us.classifications);
        let input = ValuationRecomputeInput::new(
            calendar,
            &self.rows,
            ValuationHistoryEvidence::new(&self.history.histories, &self.history.evidence),
            DatedSnapshotLedgers::new(self.date, ledgers),
            self.observation,
        )?;
        ValuationSnapshot::recompute(input)
    }
}

#[cfg(test)]
mod tests {
    use crate::valuation_snapshot::tests::{at, calendar, confirmed, date, fixture_window, fx, tx};
    use crate::{
        Currency, FlowPeriod, SettlementWindow, SnapshotError, SnapshotObservation, TransactionKind,
    };

    use super::{DayCashLedger, ValuationDayHistory, ValuationDayInputs};

    fn owned_period(item: &FlowPeriod) -> crate::ConfirmedFlowPeriod {
        confirmed(item).clone()
    }

    fn inputs(day: i64, opening: (&str, &str), ending: (&str, &str)) -> ValuationDayInputs {
        let window = SettlementWindow::try_new(date(1), date(day)).expect("기간");
        let fixture = fixture_window(
            vec![tx(TransactionKind::Deposit, Currency::Krw, 2, 1, "10")],
            opening,
            ending,
            window,
        );
        let kr = DayCashLedger::new(fixture.kr.clone(), owned_period(&fixture.kp));
        let us = DayCashLedger::new(fixture.us.clone(), owned_period(&fixture.up))
            .with_transfer_classifications(Vec::new());
        ValuationDayInputs::new(
            date(day),
            fixture.rows,
            ValuationDayHistory::new(Vec::new(), Vec::new()),
            (kr, us),
            SnapshotObservation::new(at(day, 7), at(20, 0), Some(fx(at(day, 7), "1300"))),
        )
    }

    #[test]
    fn owned_day_inputs_recompute_the_same_snapshot_as_the_borrowed_contract() {
        let day = inputs(5, ("100", "0"), ("110", "0"));
        assert_eq!(day.date(), date(5));
        let snapshot = day.recompute(date(5), &calendar(5)).expect("재계산");
        assert_eq!(snapshot.settlement_date(), date(5));
        assert_eq!(
            day.recompute(date(5), &calendar(5)).expect("반복"),
            snapshot
        );
    }

    #[test]
    fn day_inputs_reject_a_different_planned_date_or_calendar() {
        let day = inputs(5, ("100", "0"), ("110", "0"));
        assert_eq!(
            day.recompute(date(6), &calendar(6)),
            Err(SnapshotError::WrongPeriod)
        );
        assert_eq!(
            day.recompute(date(5), &calendar(6)),
            Err(SnapshotError::WrongPeriod)
        );
    }
}
