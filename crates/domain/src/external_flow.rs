use crate::cash_ledger::cash_direction;
use crate::cash_ledger_evidence::{CancellationRange, LedgerFlowFacts};
use crate::{
    CalendarDate, CashUnavailable, Currency, Money, Ratio, SettlementWindow, Timestamp,
    Transaction, TransactionKind, TransferClassification,
};
use std::collections::BTreeMap;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlowDirection {
    Inflow,
    Outflow,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExternalFlow {
    date: CalendarDate,
    transaction_time: Option<Timestamp>,
    exchange_rate: Option<Ratio>,
    direction: FlowDirection,
    amount: Money,
}
impl ExternalFlow {
    fn from_row(row: &Transaction, direction: FlowDirection, amount: Money) -> Self {
        Self {
            date: row.data().settlement_date,
            transaction_time: row.data().transaction_time,
            exchange_rate: row.data().exchange_rate,
            direction,
            amount,
        }
    }
    #[must_use]
    pub const fn date(self) -> CalendarDate {
        self.date
    }
    #[must_use]
    pub const fn transaction_time(self) -> Option<Timestamp> {
        self.transaction_time
    }
    #[must_use]
    pub const fn exchange_rate(self) -> Option<Ratio> {
        self.exchange_rate
    }
    #[must_use]
    pub fn currency(self) -> Currency {
        self.amount.currency()
    }
    #[must_use]
    pub const fn direction(self) -> FlowDirection {
        self.direction
    }
    #[must_use]
    pub const fn amount(self) -> Money {
        self.amount
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlowUncertainty {
    UnclassifiedTransfer,
    UnclassifiedTransaction,
    MissingAmount,
    UnmatchedCancellation,
    ConflictingClassification,
    OutsideCoverage,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfirmedFlowDay {
    date: CalendarDate,
    flows: Vec<ExternalFlow>,
}
impl ConfirmedFlowDay {
    #[must_use]
    pub const fn date(&self) -> CalendarDate {
        self.date
    }
    #[must_use]
    pub fn flows(&self) -> &[ExternalFlow] {
        &self.flows
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FlowDay {
    Confirmed(ConfirmedFlowDay),
    Uncertain(FlowUncertainty),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfirmedFlowPeriod {
    window: SettlementWindow,
    currency: Currency,
    flows: Vec<ExternalFlow>,
}
impl ConfirmedFlowPeriod {
    #[must_use]
    pub const fn window(&self) -> SettlementWindow {
        self.window
    }
    #[must_use]
    pub const fn currency(&self) -> Currency {
        self.currency
    }
    #[must_use]
    pub fn flows(&self) -> &[ExternalFlow] {
        &self.flows
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FlowPeriod {
    Confirmed(ConfirmedFlowPeriod),
    Uncertain(FlowUncertainty),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExternalFlows {
    Reconciled(FlowCalendar),
    Unavailable(CashUnavailable),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlowCalendar {
    window: SettlementWindow,
    currency: Currency,
    days: BTreeMap<CalendarDate, Result<Vec<ExternalFlow>, FlowUncertainty>>,
    cancellations: Vec<CancellationRange>,
}
impl FlowCalendar {
    #[must_use]
    pub fn day(&self, date: CalendarDate) -> FlowDay {
        if date < self.window.from() || date > self.window.to() {
            return FlowDay::Uncertain(FlowUncertainty::OutsideCoverage);
        }
        if self
            .cancellations
            .iter()
            .any(|range| range.from <= date && date <= range.to)
        {
            return FlowDay::Uncertain(FlowUncertainty::UnmatchedCancellation);
        }
        match self.days.get(&date) {
            Some(Ok(flows)) => FlowDay::Confirmed(ConfirmedFlowDay {
                date,
                flows: flows.clone(),
            }),
            Some(Err(reason)) => FlowDay::Uncertain(*reason),
            None => FlowDay::Confirmed(ConfirmedFlowDay {
                date,
                flows: Vec::new(),
            }),
        }
    }
    #[must_use]
    pub fn period(&self, window: SettlementWindow) -> FlowPeriod {
        if window != self.window {
            return FlowPeriod::Uncertain(FlowUncertainty::OutsideCoverage);
        }
        if self
            .cancellations
            .iter()
            .any(|range| range.from <= window.to() && window.from() <= range.to)
        {
            return FlowPeriod::Uncertain(FlowUncertainty::UnmatchedCancellation);
        }
        let mut flows = Vec::new();
        for result in self
            .days
            .range(window.from()..=window.to())
            .map(|(_, value)| value)
        {
            match result {
                Ok(values) => flows.extend_from_slice(values),
                Err(reason) => return FlowPeriod::Uncertain(*reason),
            }
        }
        FlowPeriod::Confirmed(ConfirmedFlowPeriod {
            window,
            currency: self.currency,
            flows,
        })
    }
}
pub(crate) fn collect(
    facts: &LedgerFlowFacts,
    classifications: &[TransferClassification],
) -> FlowCalendar {
    let mut days = BTreeMap::<CalendarDate, Result<Vec<ExternalFlow>, FlowUncertainty>>::new();
    for (position, row) in facts.rows.iter().enumerate() {
        let date = row.data().settlement_date;
        let occurrence = crate::TransferOccurrence::new(
            facts
                .rows
                .iter()
                .take(position)
                .filter(|previous| *previous == row)
                .count(),
        );
        let result = classify_row(row, classifications, occurrence);
        let day = days.entry(date).or_insert_with(|| Ok(Vec::new()));
        match result {
            Ok(Some(flow)) => {
                if let Ok(flows) = day {
                    flows.push(flow);
                }
            }
            Ok(None) => (),
            Err(reason) => *day = Err(reason),
        }
    }
    FlowCalendar {
        window: facts.window,
        currency: facts.currency,
        days,
        cancellations: facts.cancellations.clone(),
    }
}
fn classify_row(
    row: &Transaction,
    classifications: &[TransferClassification],
    occurrence: crate::TransferOccurrence,
) -> Result<Option<ExternalFlow>, FlowUncertainty> {
    match row.data().kind {
        TransactionKind::SecuritiesTransferIn | TransactionKind::SecuritiesTransferOut => {
            classify_transfer(row, classifications, occurrence)
        }
        TransactionKind::Deposit | TransactionKind::Withdrawal => {
            let direction = cash_direction(row.data())
                .map_err(|_error| FlowUncertainty::UnclassifiedTransaction)?;
            if row.data().asset_class.is_some() {
                return Ok(None);
            }
            let value = row
                .data()
                .net_amount
                .ok_or(FlowUncertainty::MissingAmount)?;
            Ok(Some(ExternalFlow::from_row(row, direction, value)))
        }
        TransactionKind::Buy
        | TransactionKind::Sell
        | TransactionKind::ExchangeIn
        | TransactionKind::ExchangeOut
        | TransactionKind::Dividend
        | TransactionKind::Interest
        | TransactionKind::Fee => Ok(None),
        TransactionKind::Unknown(_) => Err(FlowUncertainty::UnclassifiedTransaction),
    }
}
fn classify_transfer(
    row: &Transaction,
    classifications: &[TransferClassification],
    occurrence: crate::TransferOccurrence,
) -> Result<Option<ExternalFlow>, FlowUncertainty> {
    let mut matching = classifications
        .iter()
        .filter(|value| value.transaction() == row && value.occurrence() == occurrence);
    let classification = matching
        .next()
        .ok_or(FlowUncertainty::UnclassifiedTransfer)?;
    if matching.next().is_some() {
        return Err(FlowUncertainty::ConflictingClassification);
    }
    match classification.effect() {
        crate::external_flow_classification::TransferEffect::Internal => Ok(None),
        crate::external_flow_classification::TransferEffect::Unclassified => {
            Err(FlowUncertainty::UnclassifiedTransfer)
        }
        crate::external_flow_classification::TransferEffect::External(direction, value) => {
            Ok(Some(ExternalFlow::from_row(row, direction, value)))
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::{
        ConfirmedFlowDay, ConfirmedFlowPeriod, ExternalFlow, ExternalFlows, FlowCalendar, FlowDay,
        FlowDirection, FlowPeriod, FlowUncertainty, classify_row,
    };
    use crate::cash_ledger::tests::{at, date, input, ledger, money, row, set, window};
    use crate::{
        AccountId, AssetClass, Cancellation, CashLedger, StoredTransaction, TransactionBatch,
    };
    use crate::{Currency, Ratio, SettlementWindow, Transaction, TransferClassification};
    pub(crate) fn calendar(value: ExternalFlows) -> Option<FlowCalendar> {
        match value {
            ExternalFlows::Reconciled(result) => Some(result),
            ExternalFlows::Unavailable(_) => None,
        }
    }
    pub(crate) fn period_flows(value: FlowPeriod) -> Option<ConfirmedFlowPeriod> {
        match value {
            FlowPeriod::Confirmed(flows) => Some(flows),
            FlowPeriod::Uncertain(_) => None,
        }
    }
    #[test]
    fn dates_period_cash_gate_and_time_exchange_facts_are_preserved() {
        let mut data = row("DEPOSIT", "10", Currency::Usd).data().clone();
        data.exchange_rate =
            Some(Ratio::parse(&crate::NumericInput::try_new("1300").expect("입력")).expect("환율"));
        let rows = set(vec![Transaction::try_new(data.clone()).expect("행")]);
        let result =
            calendar(ledger(&rows, Currency::Usd, "110").external_flows(&[])).expect("흐름");
        let expected = ExternalFlow {
            date: date(2),
            transaction_time: data.transaction_time,
            exchange_rate: data.exchange_rate,
            direction: FlowDirection::Inflow,
            amount: money("10", Currency::Usd),
        };
        assert_eq!(expected.date(), date(2));
        assert_eq!(expected.transaction_time(), Some(at(86_410)));
        assert_eq!(expected.exchange_rate(), data.exchange_rate);
        assert_eq!(expected.currency(), Currency::Usd);
        assert_eq!(expected.direction(), FlowDirection::Inflow);
        assert_eq!(expected.amount(), money("10", Currency::Usd));
        assert_eq!(
            result.day(date(2)),
            FlowDay::Confirmed(ConfirmedFlowDay {
                date: date(2),
                flows: vec![expected]
            })
        );
        let confirmed = ConfirmedFlowPeriod {
            window: window(),
            currency: Currency::Usd,
            flows: vec![expected],
        };
        assert_eq!(confirmed.window(), window());
        assert_eq!(confirmed.currency(), Currency::Usd);
        assert_eq!(confirmed.flows(), &[expected]);
        assert_eq!(result.period(window()), FlowPeriod::Confirmed(confirmed));
        let empty_day = ConfirmedFlowDay {
            date: date(3),
            flows: vec![],
        };
        assert_eq!(empty_day.date(), date(3));
        assert_eq!(empty_day.flows(), &[]);
        assert_eq!(result.day(date(3)), FlowDay::Confirmed(empty_day));
        assert_eq!(
            result.day(date(11)),
            FlowDay::Uncertain(FlowUncertainty::OutsideCoverage)
        );
        assert_eq!(
            result.period(SettlementWindow::try_new(date(2), date(3)).expect("기간")),
            FlowPeriod::Uncertain(FlowUncertainty::OutsideCoverage)
        );
        assert_eq!(
            calendar(ledger(&rows, Currency::Usd, "100").external_flows(&[])),
            None
        );
    }
    #[test]
    fn unclassified_transfer_blocks_only_its_day_and_crossing_period() {
        let rows = set(vec![
            row("DEPOSIT", "10", Currency::Krw),
            row("SECURITIES_TRANSFER_IN", "0", Currency::Krw),
        ]);
        let result =
            calendar(ledger(&rows, Currency::Krw, "110").external_flows(&[])).expect("달력");
        assert_eq!(
            result.day(date(2)),
            FlowDay::Uncertain(FlowUncertainty::UnclassifiedTransfer)
        );
        assert_eq!(
            result.day(date(3)),
            FlowDay::Confirmed(ConfirmedFlowDay {
                date: date(3),
                flows: vec![]
            })
        );
        assert_eq!(
            result.period(window()),
            FlowPeriod::Uncertain(FlowUncertainty::UnclassifiedTransfer)
        );
        assert_eq!(period_flows(result.period(window())), None);
        let period = SettlementWindow::try_new(date(3), date(4)).expect("기간");
        let mut request = input(&rows, Currency::Krw, "100");
        request.window = period;
        request.evidence = Some(crate::cash_ledger::tests::evidence(&rows, period));
        let clear = calendar(CashLedger::reconstruct(&rows, request).external_flows(&[]))
            .expect("영향 없는 기간");
        assert_eq!(
            clear.period(period),
            FlowPeriod::Confirmed(ConfirmedFlowPeriod {
                window: period,
                currency: Currency::Krw,
                flows: vec![]
            })
        );
    }
    #[test]
    fn cancelled_deposit_outside_window_blocks_original_through_cancellation_date() {
        let original = row("DEPOSIT", "10", Currency::Krw);
        let mut cancellation = original.data().clone();
        cancellation.cancellation = Cancellation::Cancelled;
        cancellation.settlement_date = date(5);
        cancellation.transaction_time = Some(at(345_610));
        let rows = set(vec![
            original,
            Transaction::try_new(cancellation).expect("취소"),
        ]);
        let result =
            calendar(ledger(&rows, Currency::Krw, "110").external_flows(&[])).expect("달력");
        for day in [2, 3, 4, 5] {
            assert_eq!(
                result.day(date(day)),
                FlowDay::Uncertain(FlowUncertainty::UnmatchedCancellation)
            );
        }
        assert_eq!(
            result.day(date(6)),
            FlowDay::Confirmed(ConfirmedFlowDay {
                date: date(6),
                flows: vec![]
            })
        );
        assert_eq!(
            result.period(window()),
            FlowPeriod::Uncertain(FlowUncertainty::UnmatchedCancellation)
        );
        let period = SettlementWindow::try_new(date(5), date(6)).expect("기간");
        let mut request = input(&rows, Currency::Krw, "100");
        request.window = period;
        request.evidence = Some(crate::cash_ledger::tests::evidence(&rows, period));
        assert_eq!(
            calendar(CashLedger::reconstruct(&rows, request).external_flows(&[]))
                .expect("달력")
                .day(date(5)),
            FlowDay::Uncertain(FlowUncertainty::UnmatchedCancellation)
        );
    }
    #[test]
    fn disappeared_original_and_cancellation_are_fully_offset() {
        let original = row("DEPOSIT", "10", Currency::Krw);
        let mut cancellation = original.data().clone();
        cancellation.cancellation = Cancellation::Cancelled;
        cancellation.settlement_date = date(5);
        cancellation.transaction_time = Some(at(345_610));
        let restored = crate::SettledRowSet::try_restore(
            AccountId::new(1),
            vec![
                StoredTransaction::try_restore(original, at(900_000), Some(at(900_000)))
                    .expect("사라짐"),
                StoredTransaction::try_restore(
                    Transaction::try_new(cancellation).expect("취소"),
                    at(900_000),
                    None,
                )
                .expect("저장"),
            ],
            TransactionBatch::try_new(vec![]).expect("목록"),
            Some(at(900_000)),
        )
        .expect("복원");
        let paired =
            calendar(ledger(&restored, Currency::Krw, "100").external_flows(&[])).expect("상계");
        assert_eq!(
            paired.period(window()),
            FlowPeriod::Confirmed(ConfirmedFlowPeriod {
                window: window(),
                currency: Currency::Krw,
                flows: vec![]
            })
        );
    }
    #[test]
    fn internal_cash_events_and_bond_tax_never_become_external_flows() {
        for code in [
            "BUY",
            "SELL",
            "EXCHANGE_IN",
            "EXCHANGE_OUT",
            "DIVIDEND",
            "INTEREST",
            "FEE",
        ] {
            assert_eq!(
                classify_row(
                    &row(code, "10", Currency::Krw),
                    &[],
                    crate::TransferOccurrence::new(0)
                ),
                Ok(None)
            );
        }
        for code in ["DEPOSIT", "WITHDRAWAL"] {
            let mut data = row(code, "10", Currency::Krw).data().clone();
            data.asset_class = Some(AssetClass::Bond);
            assert_eq!(
                classify_row(
                    &Transaction::try_new(data).expect("채권"),
                    &[],
                    crate::TransferOccurrence::new(0)
                ),
                Ok(None)
            );
        }
        let withdrawal = row("WITHDRAWAL", "10", Currency::Krw);
        assert_eq!(
            classify_row(&withdrawal, &[], crate::TransferOccurrence::new(0)),
            Ok(Some(ExternalFlow::from_row(
                &withdrawal,
                FlowDirection::Outflow,
                money("10", Currency::Krw)
            )))
        );
    }
    #[test]
    fn invalid_classifications_and_unavailable_rows_cannot_yield_partial_periods() {
        let transfer = row("SECURITIES_TRANSFER_OUT", "0", Currency::Krw);
        let class = TransferClassification::unclassified(
            transfer.clone(),
            crate::TransferOccurrence::new(0),
        )
        .expect("미분류");
        assert_eq!(
            classify_row(
                &transfer,
                std::slice::from_ref(&class),
                crate::TransferOccurrence::new(0)
            ),
            Err(FlowUncertainty::UnclassifiedTransfer)
        );
        assert_eq!(
            classify_row(
                &transfer,
                &[class.clone(), class],
                crate::TransferOccurrence::new(0)
            ),
            Err(FlowUncertainty::ConflictingClassification)
        );
        assert_eq!(
            classify_row(
                &row("FUTURE", "0", Currency::Krw),
                &[],
                crate::TransferOccurrence::new(0)
            ),
            Err(FlowUncertainty::UnclassifiedTransaction)
        );
        let mut missing = row("DEPOSIT", "0", Currency::Krw).data().clone();
        missing.net_amount = None;
        assert_eq!(
            classify_row(
                &Transaction::try_new(missing).expect("행"),
                &[],
                crate::TransferOccurrence::new(0)
            ),
            Err(FlowUncertainty::MissingAmount)
        );
        let mut unknown = row("DEPOSIT", "0", Currency::Krw).data().clone();
        unknown.asset_class = Some(AssetClass::new(
            crate::AssetClassCode::try_new("FUTURE").expect("상품"),
        ));
        assert_eq!(
            classify_row(
                &Transaction::try_new(unknown).expect("행"),
                &[],
                crate::TransferOccurrence::new(0)
            ),
            Err(FlowUncertainty::UnclassifiedTransaction)
        );
    }
}
