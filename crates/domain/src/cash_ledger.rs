use crate::cash_ledger_evidence::{LedgerFlowFacts, cancellation_ranges};
use crate::{
    Amount, AssetClass, BuyingPower, Currency, LedgerEvidence, Money, MoneyError, SettledRowSet,
    SettlementWindow, TransactionData, TransactionKind,
};
use std::cmp::Ordering;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CashUnavailable {
    UnknownOpening,
    IncompleteResponse,
    UnknownCurrency,
    UnconfirmedCancellation,
    UnclassifiedRow,
    MissingNetAmount,
    CurrencyMismatch,
    MissingReportedCash,
    InvalidReportedCash,
    ReportedDifference,
    Arithmetic(MoneyError),
    TransferCashUnknown,
    StaleReportedCash,
    MissingSettlementTime,
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct ComparisonCash(Money);
impl ComparisonCash {
    pub fn compare_to(self, reference: Money) -> Result<Ordering, MoneyError> {
        if self.0.currency() != reference.currency() {
            return Err(MoneyError::CurrencyMismatch);
        }
        Ok(self
            .0
            .amount()
            .as_decimal()
            .cmp(&reference.amount().as_decimal()))
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct CashDifference(Money);
impl CashDifference {
    pub fn compare_to(self, reference: Money) -> Result<Ordering, MoneyError> {
        ComparisonCash(self.0).compare_to(reference)
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CashLedgerState {
    Reconciled(ComparisonCash),
    Unavailable(CashUnavailable),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CashComparison {
    Matches,
    Different(CashDifference),
    Unavailable,
}
#[derive(Clone, PartialEq, Eq)]
pub struct CashLedger {
    currency: Currency,
    state: CashLedgerState,
    comparison: CashComparison,
    facts: LedgerFlowFacts,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CashLedgerInput {
    pub window: SettlementWindow,
    pub currency: Currency,
    pub opening: Option<Money>,
    pub evidence: Option<LedgerEvidence>,
    pub reported: Option<BuyingPower>,
}
impl CashLedger {
    #[must_use]
    pub fn reconstruct(rows: &SettledRowSet, input: CashLedgerInput) -> Self {
        let currency = input.currency;
        let window = input.window;
        match reconstruct(rows, input) {
            Ok(result) => {
                if result.delta.amount().as_decimal().is_zero() {
                    Self {
                        currency,
                        state: CashLedgerState::Reconciled(ComparisonCash(result.value)),
                        comparison: CashComparison::Matches,
                        facts: result.facts,
                    }
                } else {
                    Self {
                        currency,
                        state: CashLedgerState::Unavailable(CashUnavailable::ReportedDifference),
                        comparison: CashComparison::Different(CashDifference(result.delta)),
                        facts: result.facts,
                    }
                }
            }
            Err(reason) => Self {
                currency,
                state: CashLedgerState::Unavailable(reason),
                comparison: CashComparison::Unavailable,
                facts: LedgerFlowFacts {
                    window,
                    currency,
                    rows: Vec::new(),
                    cancellations: Vec::new(),
                },
            },
        }
    }
    pub(crate) fn snapshot_basis(
        &self,
        period: &crate::ConfirmedFlowPeriod,
        classifications: &[crate::TransferClassification],
    ) -> Result<(Money, &[crate::Transaction]), CashUnavailable> {
        if period.currency() != self.currency || period.window() != self.facts.window {
            return Err(CashUnavailable::IncompleteResponse);
        }
        match self.state {
            CashLedgerState::Reconciled(value) => {
                let expected = crate::external_flow::collect(&self.facts, classifications)
                    .period(self.facts.window);
                if expected != crate::FlowPeriod::Confirmed(period.clone()) {
                    return Err(CashUnavailable::IncompleteResponse);
                }
                Ok((value.0, &self.facts.rows))
            }
            CashLedgerState::Unavailable(reason) => Err(reason),
        }
    }
    #[must_use]
    pub fn external_flows(
        &self,
        classifications: &[crate::TransferClassification],
    ) -> crate::ExternalFlows {
        match self.state {
            CashLedgerState::Reconciled(_) => crate::ExternalFlows::Reconciled(
                crate::external_flow::collect(&self.facts, classifications),
            ),
            CashLedgerState::Unavailable(reason) => crate::ExternalFlows::Unavailable(reason),
        }
    }
    #[must_use]
    pub const fn state(&self) -> &CashLedgerState {
        &self.state
    }
    #[must_use]
    pub const fn comparison(&self) -> &CashComparison {
        &self.comparison
    }
    #[must_use]
    pub const fn currency(&self) -> Currency {
        self.currency
    }
}
fn reconstruct(
    rows: &SettledRowSet,
    input: CashLedgerInput,
) -> Result<Reconstruction, CashUnavailable> {
    let CashLedgerInput {
        window,
        currency,
        opening,
        evidence,
        reported,
    } = input;
    evidence
        .as_ref()
        .ok_or(CashUnavailable::IncompleteResponse)?
        .validate(rows, window)?;
    let mut total = opening.ok_or(CashUnavailable::UnknownOpening)?;
    if total.currency() != currency {
        return Err(CashUnavailable::CurrencyMismatch);
    }
    let mut selected = rows
        .active_settled_for_calculation()
        .filter(|row| {
            row.data().currency == currency
                && window.coverage(row.data().settlement_date)
                    == crate::settled_row_set::WindowCoverage::Inside
        })
        .collect::<Vec<_>>();
    selected.sort_unstable();
    for row in &selected {
        let data = row.data();
        total = total
            .checked_add(&cash_delta(data)?)
            .map_err(CashUnavailable::Arithmetic)?;
    }
    let observed = reported.ok_or(CashUnavailable::MissingReportedCash)?;
    if observed.currency() != currency {
        return Err(CashUnavailable::InvalidReportedCash);
    }
    let reference = observed
        .money()
        .map_err(|_error| CashUnavailable::InvalidReportedCash)?;
    let delta = total
        .checked_sub(&reference)
        .map_err(CashUnavailable::Arithmetic)?;
    validate_observation(rows, currency, observed)?;
    Ok(Reconstruction {
        value: total,
        delta,
        facts: LedgerFlowFacts {
            window,
            currency,
            rows: selected.into_iter().cloned().collect(),
            cancellations: cancellation_ranges(rows, currency),
        },
    })
}
fn validate_observation(
    rows: &SettledRowSet,
    currency: Currency,
    observed: BuyingPower,
) -> Result<(), CashUnavailable> {
    for stored in rows
        .history_rows()
        .iter()
        .filter(|stored| stored.transaction().data().currency == currency)
    {
        let timestamp = stored
            .transaction()
            .data()
            .transaction_time
            .ok_or(CashUnavailable::MissingSettlementTime)?;
        if observed.observed_at() < timestamp {
            return Err(CashUnavailable::StaleReportedCash);
        }
    }
    Ok(())
}
pub(crate) fn cash_direction(
    data: &TransactionData,
) -> Result<crate::FlowDirection, CashUnavailable> {
    if let Some(AssetClass::Unknown(_)) = data.asset_class {
        return Err(CashUnavailable::UnclassifiedRow);
    }
    match data.kind {
        TransactionKind::Buy
        | TransactionKind::Withdrawal
        | TransactionKind::ExchangeOut
        | TransactionKind::Fee => Ok(crate::FlowDirection::Outflow),
        TransactionKind::Sell
        | TransactionKind::Deposit
        | TransactionKind::ExchangeIn
        | TransactionKind::Dividend
        | TransactionKind::Interest => Ok(crate::FlowDirection::Inflow),
        TransactionKind::SecuritiesTransferIn
        | TransactionKind::SecuritiesTransferOut
        | TransactionKind::Unknown(_) => Err(CashUnavailable::UnclassifiedRow),
    }
}

#[derive(Debug)]
struct Reconstruction {
    value: Money,
    delta: Money,
    facts: LedgerFlowFacts,
}
impl std::fmt::Debug for ComparisonCash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ComparisonCash([redacted])")
    }
}
impl std::fmt::Debug for CashDifference {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("CashDifference([redacted])")
    }
}
impl std::fmt::Debug for CashLedger {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CashLedger")
            .field("currency", &self.currency)
            .field("state", &self.state)
            .field("comparison", &self.comparison)
            .finish_non_exhaustive()
    }
}
pub(crate) fn cash_delta(data: &TransactionData) -> Result<Money, CashUnavailable> {
    if let Some(AssetClass::Unknown(_)) = data.asset_class {
        return Err(CashUnavailable::UnclassifiedRow);
    }
    let zero = Money::new(Amount::new(rust_decimal::Decimal::ZERO), data.currency);
    match data.kind {
        TransactionKind::SecuritiesTransferIn | TransactionKind::SecuritiesTransferOut => {
            if data.net_amount.is_some() || data.amount.is_some() {
                return Err(CashUnavailable::TransferCashUnknown);
            }
            let costs = data
                .commission
                .checked_add(&data.tax)
                .map_err(CashUnavailable::Arithmetic)?;
            zero.checked_sub(&costs)
                .map_err(CashUnavailable::Arithmetic)
        }
        TransactionKind::Buy
        | TransactionKind::Sell
        | TransactionKind::Deposit
        | TransactionKind::Withdrawal
        | TransactionKind::ExchangeIn
        | TransactionKind::ExchangeOut
        | TransactionKind::Dividend
        | TransactionKind::Interest
        | TransactionKind::Fee
        | TransactionKind::Unknown(_) => {
            let direction = cash_direction(data)?;
            let value = data.net_amount.ok_or(CashUnavailable::MissingNetAmount)?;
            match direction {
                crate::FlowDirection::Inflow => Ok(value),
                crate::FlowDirection::Outflow => zero
                    .checked_sub(&value)
                    .map_err(CashUnavailable::Arithmetic),
            }
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::{
        CashComparison, CashDifference, CashLedger, CashLedgerInput, CashLedgerState,
        CashUnavailable, ComparisonCash,
    };
    use crate::{
        AccountId, Amount, AmountInput, CalendarDate, CalendarYear, Cancellation,
        CompleteTransactionResponse, CompletionAttestation, DayNumber, Market, MonthNumber,
        Nanosecond, OffsetSeconds, Settlement, StoredTransaction, Timestamp, Transaction,
        TransactionBatch, TransactionCode, TransactionFilter, UnixSeconds,
    };
    use crate::{
        AssetClass, BuyingPower, Currency, LedgerEvidence, Money, MoneyError, SettledRowSet,
        SettlementWindow, TransactionData, TransactionKind,
    };
    use proptest::{collection, prelude::any, prop_assert_eq, proptest, test_runner::Config};
    use std::cmp::Ordering;
    pub(crate) fn money(text: &str, currency: Currency) -> Money {
        Money::new(
            Amount::parse(&AmountInput::new(text)).expect("금액"),
            currency,
        )
    }
    pub(crate) fn date(day: u8) -> CalendarDate {
        CalendarDate::from_parts(
            CalendarYear::new(1970),
            MonthNumber::new(1),
            DayNumber::new(day),
        )
        .expect("날짜")
    }
    pub(crate) fn at(seconds: i64) -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(seconds),
            Nanosecond::new(0),
            OffsetSeconds::new(0),
        )
        .expect("시각")
    }
    pub(crate) fn window() -> SettlementWindow {
        SettlementWindow::try_new(date(1), date(10)).expect("기간")
    }
    pub(crate) fn row(code: &str, text: &str, currency: Currency) -> Transaction {
        let transfer = code.starts_with("SECURITIES_TRANSFER");
        Transaction::try_new(TransactionData {
            kind: TransactionKind::new(TransactionCode::try_new(code).expect("종류")),
            market: Market::Kr,
            currency,
            symbol: None,
            asset_class: None,
            isin: None,
            quantity: None,
            filled_price: None,
            amount: if transfer {
                None
            } else {
                Some(money("999", currency))
            },
            commission: money("0", currency),
            tax: money("0", currency),
            net_amount: if transfer {
                None
            } else {
                Some(money(text, currency))
            },
            exchange_rate: None,
            settlement_date: date(2),
            settlement: Settlement::Settled,
            transaction_time: Some(at(86_410)),
            cancellation: Cancellation::Regular,
        })
        .expect("거래")
    }
    pub(crate) fn set(rows: Vec<Transaction>) -> SettledRowSet {
        SettledRowSet::try_restore(
            AccountId::new(1),
            rows.into_iter()
                .map(|value| {
                    StoredTransaction::try_restore(value, at(900_000), None).expect("저장")
                })
                .collect(),
            TransactionBatch::try_new(vec![]).expect("목록"),
            Some(at(900_000)),
        )
        .expect("행 집합")
    }
    pub(crate) fn evidence(rows: &SettledRowSet, period: SettlementWindow) -> LedgerEvidence {
        let selected = rows
            .active_settled_rows()
            .map(StoredTransaction::transaction)
            .filter(|value| {
                value.data().settlement_date >= period.from()
                    && value.data().settlement_date <= period.to()
            })
            .cloned()
            .collect();
        LedgerEvidence::from_complete_response(
            CompleteTransactionResponse::after_last_page_without_quarantine(
                rows.account_id().clone(),
                period,
                TransactionFilter::all(),
                TransactionBatch::try_new(selected).expect("목록"),
                CompletionAttestation::AllPagesAndAllMatchingPendingReceived,
            )
            .expect("완전 응답"),
        )
        .expect("증거")
    }
    pub(crate) fn input(
        rows: &SettledRowSet,
        currency: Currency,
        expected: &str,
    ) -> CashLedgerInput {
        CashLedgerInput {
            window: window(),
            currency,
            opening: Some(money("100", currency)),
            evidence: Some(evidence(rows, window())),
            reported: Some(BuyingPower::new(money(expected, currency), at(900_000))),
        }
    }
    pub(crate) fn ledger(rows: &SettledRowSet, currency: Currency, expected: &str) -> CashLedger {
        CashLedger::reconstruct(rows, input(rows, currency, expected))
    }
    #[test]
    fn closed_cash_table_and_transfer_costs_follow_schema() {
        for currency in [Currency::Krw, Currency::Usd] {
            for (code, expected) in [
                ("BUY", "90"),
                ("SELL", "110"),
                ("DEPOSIT", "110"),
                ("WITHDRAWAL", "90"),
                ("EXCHANGE_IN", "110"),
                ("EXCHANGE_OUT", "90"),
                ("DIVIDEND", "110"),
                ("INTEREST", "110"),
                ("FEE", "90"),
                ("SECURITIES_TRANSFER_IN", "100"),
                ("SECURITIES_TRANSFER_OUT", "100"),
            ] {
                let rows = set(vec![row(code, "10", currency)]);
                let result = ledger(&rows, currency, expected);
                assert_eq!(
                    result.state(),
                    &CashLedgerState::Reconciled(ComparisonCash(money(expected, currency)))
                );
                assert_eq!(result.comparison(), &CashComparison::Matches);
                assert_eq!(result.currency(), currency);
            }
            let mut data = row("SECURITIES_TRANSFER_IN", "0", currency).data().clone();
            data.commission = money("3", currency);
            data.tax = money("2", currency);
            assert_eq!(
                ledger(
                    &set(vec![Transaction::try_new(data).expect("행")]),
                    currency,
                    "95"
                )
                .comparison(),
                &CashComparison::Matches
            );
        }
    }
    #[test]
    fn exact_difference_debug_and_low_trust_hide_cash() {
        let rows = set(vec![row("WITHDRAWAL", "1", Currency::Krw)]);
        let result = ledger(&rows, Currency::Krw, "100");
        assert_eq!(
            result.state(),
            &CashLedgerState::Unavailable(CashUnavailable::ReportedDifference)
        );
        assert_eq!(
            result.comparison(),
            &CashComparison::Different(CashDifference(money("-1", Currency::Krw)))
        );
        assert_eq!(
            ComparisonCash(money("99", Currency::Krw)).compare_to(money("99", Currency::Krw)),
            Ok(Ordering::Equal)
        );
        let delta = CashDifference(money("-1", Currency::Krw));
        assert_eq!(
            delta.compare_to(money("0", Currency::Krw)),
            Ok(Ordering::Less)
        );
        assert_eq!(
            delta.compare_to(money("-2", Currency::Krw)),
            Ok(Ordering::Greater)
        );
        assert_eq!(
            delta.compare_to(money("0", Currency::Usd)),
            Err(MoneyError::CurrencyMismatch)
        );
        assert_eq!(
            format!("{:?}", ComparisonCash(money("123456", Currency::Krw))),
            "ComparisonCash([redacted])"
        );
        assert_eq!(format!("{delta:?}"), "CashDifference([redacted])");
        assert!(!format!("{result:?}").contains("99"));
        assert_eq!(
            result.external_flows(&[]),
            crate::ExternalFlows::Unavailable(CashUnavailable::ReportedDifference)
        );
    }
    #[test]
    fn complete_response_is_required_and_bound_to_rows_account_and_window() {
        let rows = set(vec![]);
        let mut request = input(&rows, Currency::Krw, "100");
        request.evidence = None;
        assert_eq!(
            CashLedger::reconstruct(&rows, request).state(),
            &CashLedgerState::Unavailable(CashUnavailable::IncompleteResponse)
        );
        assert_eq!(
            ledger(&rows, Currency::Krw, "100").comparison(),
            &CashComparison::Matches
        );
        let populated = set(vec![row("DEPOSIT", "1", Currency::Krw)]);
        let mut mismatch = input(&populated, Currency::Krw, "101");
        mismatch.evidence = Some(evidence(&rows, window()));
        assert_eq!(
            CashLedger::reconstruct(&populated, mismatch).state(),
            &CashLedgerState::Unavailable(CashUnavailable::IncompleteResponse)
        );
        let empty = SettledRowSet::new(AccountId::new(1));
        assert_eq!(
            ledger(&empty, Currency::Krw, "100").state(),
            &CashLedgerState::Unavailable(CashUnavailable::IncompleteResponse)
        );
    }
    #[test]
    fn stale_reported_cash_missing_time_and_invalid_inputs_are_unavailable() {
        let rows = set(vec![row("DEPOSIT", "1", Currency::Krw)]);
        for (opening, reported, reason) in [
            (
                None,
                Some(BuyingPower::new(money("101", Currency::Krw), at(900_000))),
                CashUnavailable::UnknownOpening,
            ),
            (
                Some(money("100", Currency::Usd)),
                None,
                CashUnavailable::CurrencyMismatch,
            ),
            (
                Some(money("100", Currency::Krw)),
                None,
                CashUnavailable::MissingReportedCash,
            ),
            (
                Some(money("100", Currency::Krw)),
                Some(BuyingPower::new(money("1", Currency::Usd), at(900_000))),
                CashUnavailable::InvalidReportedCash,
            ),
            (
                Some(money("100", Currency::Krw)),
                Some(BuyingPower::new(money("-1", Currency::Krw), at(900_000))),
                CashUnavailable::InvalidReportedCash,
            ),
            (
                Some(money("100", Currency::Krw)),
                Some(BuyingPower::new(money("101", Currency::Krw), at(86_409))),
                CashUnavailable::StaleReportedCash,
            ),
        ] {
            let mut request = input(&rows, Currency::Krw, "101");
            request.opening = opening;
            request.reported = reported;
            assert_eq!(
                CashLedger::reconstruct(&rows, request).state(),
                &CashLedgerState::Unavailable(reason)
            );
        }
        let mut data = row("DEPOSIT", "1", Currency::Krw).data().clone();
        data.transaction_time = None;
        assert_eq!(
            ledger(
                &set(vec![Transaction::try_new(data).expect("행")]),
                Currency::Krw,
                "101"
            )
            .state(),
            &CashLedgerState::Unavailable(CashUnavailable::MissingSettlementTime)
        );
        let mut boundary = input(&rows, Currency::Krw, "101");
        boundary.reported = Some(BuyingPower::new(money("101", Currency::Krw), at(86_410)));
        assert_eq!(
            CashLedger::reconstruct(&rows, boundary).comparison(),
            &CashComparison::Matches
        );
    }
    #[test]
    fn first_error_is_stable_for_permutations_and_arithmetic_is_exact() {
        let mut missing = row("BUY", "1", Currency::Krw).data().clone();
        missing.net_amount = None;
        let left = Transaction::try_new(missing).expect("행");
        let right = row("FUTURE", "1", Currency::Krw);
        for rows in [vec![left.clone(), right.clone()], vec![right, left]] {
            assert_eq!(
                ledger(&set(rows), Currency::Krw, "100").state(),
                &CashLedgerState::Unavailable(CashUnavailable::MissingNetAmount)
            );
        }
        for (opening, value, error) in [
            ("79228162514264337593543950335", "1", MoneyError::Overflow),
            (
                "79228162514264337593543950334",
                "0.1",
                MoneyError::PrecisionLoss,
            ),
        ] {
            let rows = set(vec![row("DEPOSIT", value, Currency::Krw)]);
            let mut request = input(&rows, Currency::Krw, "0");
            request.opening = Some(money(opening, Currency::Krw));
            assert_eq!(
                CashLedger::reconstruct(&rows, request).state(),
                &CashLedgerState::Unavailable(CashUnavailable::Arithmetic(error))
            );
        }
    }
    proptest! {
        #![proptest_config(Config::with_cases(4096))]
        #[test]
        fn mixed_kinds_currencies_cancellations_and_transfers_match_independent_cents(values in collection::vec((0_usize..12,0_i64..100,any::<bool>()),0..24)) {
            let codes=["BUY","SELL","DEPOSIT","WITHDRAWAL","EXCHANGE_IN","EXCHANGE_OUT","DIVIDEND","INTEREST","FEE","SECURITIES_TRANSFER_IN","SECURITIES_TRANSFER_OUT","CANCELLED"];
            let mut rows=Vec::new();let mut krw=10000_i64;let mut usd=10000_i64;
            for (index,cents,is_usd) in values {
                let currency=if is_usd {Currency::Usd}else{Currency::Krw};let code=codes.get(index).expect("종류");
                let text=rust_decimal::Decimal::from_i128_with_scale(i128::from(cents),2).to_string();
                let mut data=row(if *code=="CANCELLED" {"DEPOSIT"}else{code},&text,currency).data().clone();
                if *code=="CANCELLED" {data.cancellation=Cancellation::Cancelled;}
                let signed=match index {0|3|5|8=>cents.checked_neg().expect("부호"),1|2|4|6|7=>cents,_=>0};
                let target=if is_usd {&mut usd}else{&mut krw};*target=target.checked_add(signed).expect("독립 합");rows.push(Transaction::try_new(data).expect("행"));
            }
            let original=set(rows.clone());rows.reverse();let reversed=set(rows);
            for (currency,cents) in [(Currency::Krw,krw),(Currency::Usd,usd)] {
                let expected=rust_decimal::Decimal::from_i128_with_scale(i128::from(cents),2).to_string();let result=ledger(&original,currency,&expected);
                let alternate=ledger(&reversed,currency,&expected);prop_assert_eq!(result.comparison(),&CashComparison::Matches);prop_assert_eq!(result.external_flows(&[]),alternate.external_flows(&[]));prop_assert_eq!(result,alternate);
            }
        }
    }
    #[test]
    fn unknown_rows_transfer_schema_conflicts_and_currency_deltas_are_closed_errors() {
        let mut asset = row("BUY", "1", Currency::Krw).data().clone();
        asset.asset_class = Some(AssetClass::new(
            crate::AssetClassCode::try_new("FUTURE").expect("상품"),
        ));
        let mut transfer = row("SECURITIES_TRANSFER_IN", "0", Currency::Krw)
            .data()
            .clone();
        transfer.net_amount = Some(money("0", Currency::Krw));
        for (data, reason) in [
            (asset, CashUnavailable::UnclassifiedRow),
            (transfer, CashUnavailable::TransferCashUnknown),
        ] {
            assert_eq!(
                ledger(
                    &set(vec![Transaction::try_new(data).expect("행")]),
                    Currency::Krw,
                    "100"
                )
                .state(),
                &CashLedgerState::Unavailable(reason)
            );
        }
        assert_eq!(
            ledger(
                &set(vec![row("FUTURE", "1", Currency::Krw)]),
                Currency::Krw,
                "100"
            )
            .state(),
            &CashLedgerState::Unavailable(CashUnavailable::UnclassifiedRow)
        );
        let rows = set(vec![row(
            "WITHDRAWAL",
            "79228162514264337593543950335",
            Currency::Krw,
        )]);
        let mut request = input(&rows, Currency::Krw, "79228162514264337593543950335");
        request.opening = Some(money("0", Currency::Krw));
        assert_eq!(
            CashLedger::reconstruct(&rows, request).state(),
            &CashLedgerState::Unavailable(CashUnavailable::Arithmetic(MoneyError::Overflow))
        );
        let fractional_rows = set(vec![row("DEPOSIT", "0.01", Currency::Usd)]);
        assert_eq!(
            ledger(&fractional_rows, Currency::Usd, "100").comparison(),
            &CashComparison::Different(CashDifference(money("0.01", Currency::Usd)))
        );
    }
    #[test]
    fn evidence_must_match_equal_length_multisets_and_calculation_excludes_inactive_rows() {
        let rows = set(vec![row("DEPOSIT", "1", Currency::Krw)]);
        let other = set(vec![row("DEPOSIT", "2", Currency::Krw)]);
        let mut request = input(&rows, Currency::Krw, "101");
        request.evidence = Some(evidence(&other, window()));
        assert_eq!(
            CashLedger::reconstruct(&rows, request).state(),
            &CashLedgerState::Unavailable(CashUnavailable::IncompleteResponse)
        );
        let mut cancelled = row("DEPOSIT", "20", Currency::Krw).data().clone();
        cancelled.cancellation = Cancellation::Cancelled;
        let mut pending = row("BUY", "30", Currency::Krw).data().clone();
        pending.settlement = Settlement::Pending;
        let mut outside = row("DEPOSIT", "40", Currency::Krw).data().clone();
        outside.settlement_date = date(11);
        let state = SettledRowSet::try_restore(
            AccountId::new(1),
            vec![
                StoredTransaction::try_restore(
                    row("DEPOSIT", "10", Currency::Krw),
                    at(900_000),
                    Some(at(900_000)),
                )
                .expect("사라짐"),
                StoredTransaction::try_restore(
                    Transaction::try_new(cancelled).expect("취소"),
                    at(900_000),
                    None,
                )
                .expect("저장"),
                StoredTransaction::try_restore(
                    Transaction::try_new(outside).expect("기간 밖"),
                    at(900_000),
                    None,
                )
                .expect("저장"),
            ],
            TransactionBatch::try_new(vec![Transaction::try_new(pending).expect("미결제")])
                .expect("목록"),
            Some(at(900_000)),
        )
        .expect("복원");
        assert_eq!(
            ledger(&state, Currency::Krw, "100").comparison(),
            &CashComparison::Matches
        );
    }
    #[test]
    fn transfer_amount_conflict_and_fee_overflow_are_not_silently_zero() {
        let mut data = row("SECURITIES_TRANSFER_OUT", "0", Currency::Krw)
            .data()
            .clone();
        data.amount = Some(money("1", Currency::Krw));
        assert_eq!(
            ledger(
                &set(vec![Transaction::try_new(data).expect("행")]),
                Currency::Krw,
                "100"
            )
            .state(),
            &CashLedgerState::Unavailable(CashUnavailable::TransferCashUnknown)
        );
        let mut costs = row("SECURITIES_TRANSFER_OUT", "0", Currency::Krw)
            .data()
            .clone();
        costs.commission = money("79228162514264337593543950335", Currency::Krw);
        costs.tax = money("1", Currency::Krw);
        assert_eq!(
            ledger(
                &set(vec![Transaction::try_new(costs).expect("행")]),
                Currency::Krw,
                "100"
            )
            .state(),
            &CashLedgerState::Unavailable(CashUnavailable::Arithmetic(MoneyError::Overflow))
        );
    }
}
