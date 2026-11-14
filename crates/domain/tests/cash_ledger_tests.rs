#[cfg(test)]
mod tests {
    use domain::{
        AccountId, AssetClass, AssetClassCode, BuyingPower, CalendarDate, CalendarYear,
        Cancellation, CashComparison, CashLedger, CashLedgerInput, CashLedgerState,
        CashUnavailable, Currency, DayNumber, ExternalFlow, ExternalFlows, FlowDirection,
        LedgerEvidence, Market, Money, MoneyError, MonthNumber, Nanosecond, NumericInput,
        OffsetSeconds, SettledRowSet, Settlement, SettlementWindow, StoredTransaction, Timestamp,
        Transaction, TransactionBatch, TransactionCode, TransactionData, TransactionKind,
        UnixSeconds,
    };
    use proptest::{collection, prelude::any, prop_assert_eq, proptest, test_runner::Config};

    fn money(value: &str, currency: Currency) -> Money {
        Money::parse(&NumericInput::try_new(value).expect("숫자"), currency).expect("금액")
    }
    fn date(day: u8) -> CalendarDate {
        CalendarDate::from_parts(
            CalendarYear::new(2026),
            MonthNumber::new(11),
            DayNumber::new(day),
        )
        .expect("날짜")
    }
    fn time() -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(1),
            Nanosecond::new(0),
            OffsetSeconds::new(0),
        )
        .expect("시각")
    }
    fn window() -> SettlementWindow {
        SettlementWindow::try_new(date(1), date(30)).expect("기간")
    }
    fn row(code: &str, value: &str, currency: Currency) -> Transaction {
        Transaction::try_new(TransactionData {
            kind: TransactionKind::new(TransactionCode::try_new(code).expect("종류")),
            market: Market::Kr,
            currency,
            symbol: None,
            asset_class: None,
            isin: None,
            quantity: None,
            filled_price: None,
            amount: Some(money("999", currency)),
            commission: money("3", currency),
            tax: money("2", currency),
            net_amount: Some(money(value, currency)),
            exchange_rate: None,
            settlement_date: date(2),
            settlement: Settlement::Settled,
            transaction_time: Some(time()),
            cancellation: Cancellation::Regular,
        })
        .expect("행")
    }
    fn set(rows: Vec<Transaction>) -> SettledRowSet {
        SettledRowSet::try_restore(
            AccountId::new(1),
            rows.into_iter()
                .map(|item| StoredTransaction::try_restore(item, time(), None).expect("저장"))
                .collect(),
            TransactionBatch::try_new(vec![]).expect("목록"),
            Some(time()),
        )
        .expect("집합")
    }
    fn ledger(rows: &SettledRowSet, currency: Currency, expected: &str) -> CashLedger {
        CashLedger::reconstruct(
            rows,
            CashLedgerInput {
                window: window(),
                currency,
                opening: Some(money("100", currency)),
                evidence: LedgerEvidence::Complete,
                reported: Some(BuyingPower::new(money(expected, currency), time())),
            },
        )
    }
    #[test]
    fn cash_kind_table_uses_net_once_and_keeps_currencies_separate() {
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
        ] {
            for currency in [Currency::Krw, Currency::Usd] {
                let actual = ledger(&set(vec![row(code, "10", currency)]), currency, expected);
                assert_eq!(actual.comparison(), &CashComparison::Matches);
                let value = match actual.state() {
                    CashLedgerState::Reconciled(value) => Some(*value),
                    CashLedgerState::Unavailable(_) => None,
                }
                .expect("대조 잔액");
                assert_eq!(
                    value.compare_to(money(expected, currency)),
                    Ok(std::cmp::Ordering::Equal)
                );
                assert_eq!(actual.currency(), currency);
            }
        }
        let rows = set(vec![
            row("DEPOSIT", "10", Currency::Krw),
            row("WITHDRAWAL", "20", Currency::Usd),
        ]);
        assert_eq!(
            ledger(&rows, Currency::Krw, "110").comparison(),
            &CashComparison::Matches
        );
        assert_eq!(
            ledger(&rows, Currency::Usd, "80").comparison(),
            &CashComparison::Matches
        );
    }
    #[test]
    fn cash_low_trust_never_contains_a_value_and_exact_difference_is_preserved() {
        let rows = set(vec![row("DEPOSIT", "0.01", Currency::Usd)]);
        let actual = ledger(&rows, Currency::Usd, "100");
        assert_eq!(
            actual.state(),
            &CashLedgerState::Unavailable(CashUnavailable::ReportedDifference)
        );
        match actual.comparison() {
            CashComparison::Different(delta) => assert_eq!(
                delta.compare_to(money("0.01", Currency::Usd)),
                Ok(std::cmp::Ordering::Equal)
            ),
            CashComparison::Matches | CashComparison::Unavailable => assert_eq!(
                actual.state(),
                &CashLedgerState::Unavailable(CashUnavailable::UnknownOpening)
            ),
        }
    }
    #[test]
    fn incomplete_inputs_never_expose_cash() {
        let rows = set(vec![]);
        for (opening, evidence, reported, reason) in [
            (
                None,
                LedgerEvidence::Complete,
                Some(BuyingPower::new(money("100", Currency::Usd), time())),
                CashUnavailable::UnknownOpening,
            ),
            (
                Some(money("100", Currency::Usd)),
                LedgerEvidence::Incomplete,
                None,
                CashUnavailable::IncompleteResponse,
            ),
            (
                Some(money("100", Currency::Usd)),
                LedgerEvidence::UnknownCurrency,
                None,
                CashUnavailable::UnknownCurrency,
            ),
            (
                Some(money("100", Currency::Usd)),
                LedgerEvidence::UnconfirmedCancellation,
                None,
                CashUnavailable::UnconfirmedCancellation,
            ),
            (
                Some(money("100", Currency::Krw)),
                LedgerEvidence::Complete,
                None,
                CashUnavailable::CurrencyMismatch,
            ),
            (
                Some(money("100", Currency::Usd)),
                LedgerEvidence::Complete,
                None,
                CashUnavailable::MissingReportedCash,
            ),
        ] {
            let actual = CashLedger::reconstruct(
                &rows,
                CashLedgerInput {
                    window: window(),
                    currency: Currency::Usd,
                    opening,
                    evidence,
                    reported,
                },
            );
            assert_eq!(actual.state(), &CashLedgerState::Unavailable(reason));
            assert_eq!(actual.comparison(), &CashComparison::Unavailable);
        }
    }
    #[test]
    fn unknown_missing_and_transfer_cash_are_not_silently_zero() {
        for code in [
            "FUTURE",
            "TAX",
            "SECURITIES_TRANSFER_IN",
            "SECURITIES_TRANSFER_OUT",
        ] {
            assert_eq!(
                ledger(
                    &set(vec![row(code, "10", Currency::Krw)]),
                    Currency::Krw,
                    "100"
                )
                .state(),
                &CashLedgerState::Unavailable(CashUnavailable::UnclassifiedRow)
            );
        }
        let mut data = row("BUY", "10", Currency::Krw).data().clone();
        data.net_amount = None;
        assert_eq!(
            ledger(
                &set(vec![Transaction::try_new(data).expect("행")]),
                Currency::Krw,
                "100"
            )
            .state(),
            &CashLedgerState::Unavailable(CashUnavailable::MissingNetAmount)
        );
    }
    #[test]
    fn external_flows_use_settlement_date_and_bank_deposits_only() {
        for (code, direction) in [
            ("DEPOSIT", FlowDirection::Inflow),
            ("WITHDRAWAL", FlowDirection::Outflow),
        ] {
            let rows = set(vec![row(code, "10", Currency::Usd)]);
            let flows =
                match ExternalFlow::from_settled_rows(&rows, window(), LedgerEvidence::Complete) {
                    ExternalFlows::Available(flows) => Some(flows),
                    ExternalFlows::Unavailable(_) => None,
                }
                .expect("흐름");
            let flow = flows.first().expect("외부 흐름");
            assert_eq!(flow.date(), date(2));
            assert_eq!(flow.currency(), Currency::Usd);
            assert_eq!(flow.direction(), direction);
            assert_eq!(flow.amount(), money("10", Currency::Usd));
            let mut data = row(code, "10", Currency::Usd).data().clone();
            data.asset_class = Some(AssetClass::Bond);
            assert_eq!(
                ExternalFlow::from_settled_rows(
                    &set(vec![Transaction::try_new(data).expect("채권")]),
                    window(),
                    LedgerEvidence::Complete
                ),
                ExternalFlows::Available(vec![])
            );
        }
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
                ExternalFlow::from_settled_rows(
                    &set(vec![row(code, "10", Currency::Krw)]),
                    window(),
                    LedgerEvidence::Complete
                ),
                ExternalFlows::Available(vec![])
            );
        }
        for code in [
            "SECURITIES_TRANSFER_IN",
            "SECURITIES_TRANSFER_OUT",
            "FUTURE",
        ] {
            assert_eq!(
                ExternalFlow::from_settled_rows(
                    &set(vec![row(code, "10", Currency::Krw)]),
                    window(),
                    LedgerEvidence::Complete
                ),
                ExternalFlows::Unavailable(CashUnavailable::UnclassifiedRow)
            );
        }
    }
    proptest! {
        #![proptest_config(Config::with_cases(256))]
        #[test]
        fn independent_integer_sum_order_and_cancelled_row_invariance(values in collection::vec((-2_i64..3, any::<bool>()),0..40)) {
            let mut rows=vec![]; let mut krw=100_i64; let mut usd=100_i64;
            for (change,is_usd) in values {
                let currency=if is_usd {Currency::Usd} else {Currency::Krw};
                let target=if is_usd {&mut usd} else {&mut krw}; *target=target.checked_add(change).expect("和");
                rows.push(row(if change<0 {"WITHDRAWAL"} else {"DEPOSIT"}, &change.unsigned_abs().to_string(),currency));
            }
            let before=set(rows.clone()); rows.reverse();
            let mut cancelled=row("DEPOSIT","999",Currency::Krw).data().clone(); cancelled.cancellation=Cancellation::Cancelled;
            rows.push(Transaction::try_new(cancelled).expect("취소"));
            let after=set(rows);
            for (currency,expected) in [(Currency::Krw,krw),(Currency::Usd,usd)] {
                let expected_text=expected.to_string();
                prop_assert_eq!(ledger(&before,currency,&expected_text),ledger(&after,currency,&expected_text));
                let actual=ledger(&before,currency,&expected_text); prop_assert_eq!(actual.comparison(),&CashComparison::Matches);
            }
        }
    }
    #[test]
    fn arithmetic_failure_and_missing_reference_are_unavailable() {
        let rows = set(vec![row(
            "DEPOSIT",
            "79228162514264337593543950335",
            Currency::Krw,
        )]);
        assert_eq!(
            ledger(&rows, Currency::Krw, "100").state(),
            &CashLedgerState::Unavailable(CashUnavailable::Arithmetic(MoneyError::Overflow))
        );
        let empty = SettledRowSet::new(AccountId::new(1));
        assert_eq!(
            ledger(&empty, Currency::Krw, "100").state(),
            &CashLedgerState::Unavailable(CashUnavailable::IncompleteResponse)
        );
        let empty_rows = set(vec![]);
        for reported in [money("100", Currency::Usd), money("-1", Currency::Krw)] {
            assert_eq!(
                CashLedger::reconstruct(
                    &empty_rows,
                    CashLedgerInput {
                        window: window(),
                        currency: Currency::Krw,
                        opening: Some(money("100", Currency::Krw)),
                        evidence: LedgerEvidence::Complete,
                        reported: Some(BuyingPower::new(reported, time()))
                    }
                )
                .state(),
                &CashLedgerState::Unavailable(CashUnavailable::InvalidReportedCash)
            );
        }
    }
    #[test]
    fn outside_window_pending_disappeared_and_cancellation_rows_do_not_contribute() {
        let mut outside = row("DEPOSIT", "20", Currency::Krw).data().clone();
        outside.settlement_date = date(1);
        let mut cancelled = row("DEPOSIT", "30", Currency::Krw).data().clone();
        cancelled.cancellation = Cancellation::Cancelled;
        let gone = StoredTransaction::try_restore(
            row("DEPOSIT", "40", Currency::Krw),
            time(),
            Some(time()),
        )
        .expect("사라짐");
        let mut pending = row("DEPOSIT", "50", Currency::Krw).data().clone();
        pending.settlement = Settlement::Pending;
        let rows = SettledRowSet::try_restore(
            AccountId::new(1),
            vec![
                StoredTransaction::try_restore(
                    Transaction::try_new(outside).expect("기간 밖"),
                    time(),
                    None,
                )
                .expect("저장"),
                StoredTransaction::try_restore(
                    Transaction::try_new(cancelled).expect("취소"),
                    time(),
                    None,
                )
                .expect("저장"),
                gone,
            ],
            TransactionBatch::try_new(vec![Transaction::try_new(pending).expect("미결제")])
                .expect("목록"),
            Some(time()),
        )
        .expect("집합");
        let period = SettlementWindow::try_new(date(2), date(3)).expect("기간");
        let actual = CashLedger::reconstruct(
            &rows,
            CashLedgerInput {
                window: period,
                currency: Currency::Krw,
                opening: Some(money("100", Currency::Krw)),
                evidence: LedgerEvidence::Complete,
                reported: Some(BuyingPower::new(money("100", Currency::Krw), time())),
            },
        );
        assert_eq!(actual.comparison(), &CashComparison::Matches);
        assert_eq!(
            ExternalFlow::from_settled_rows(&rows, period, LedgerEvidence::Complete),
            ExternalFlows::Available(vec![])
        );
        assert_eq!(
            ExternalFlow::from_settled_rows(&rows, period, LedgerEvidence::Incomplete),
            ExternalFlows::Unavailable(CashUnavailable::IncompleteResponse)
        );
    }

    #[test]
    fn comparison_only_values_preserve_signed_delta_and_currency_errors() {
        let actual = ledger(
            &set(vec![row("WITHDRAWAL", "1", Currency::Krw)]),
            Currency::Krw,
            "100",
        );
        let difference = match actual.comparison() {
            CashComparison::Different(value) => Some(*value),
            CashComparison::Matches | CashComparison::Unavailable => None,
        }
        .expect("차액");
        assert_eq!(
            difference.compare_to(money("-1", Currency::Krw)),
            Ok(std::cmp::Ordering::Equal)
        );
        assert_eq!(
            difference.compare_to(money("0", Currency::Krw)),
            Ok(std::cmp::Ordering::Less)
        );
        assert_eq!(
            difference.compare_to(money("-2", Currency::Krw)),
            Ok(std::cmp::Ordering::Greater)
        );
        assert_eq!(
            difference.compare_to(money("-1", Currency::Usd)),
            Err(MoneyError::CurrencyMismatch)
        );
    }
    #[test]
    fn unknown_assets_missing_external_amount_and_both_window_edges_are_checked() {
        let mut unknown = row("DEPOSIT", "10", Currency::Krw).data().clone();
        unknown.asset_class = Some(AssetClass::new(
            AssetClassCode::try_new("FUTURE").expect("종류"),
        ));
        let rows = set(vec![Transaction::try_new(unknown).expect("행")]);
        assert_eq!(
            ledger(&rows, Currency::Krw, "110").state(),
            &CashLedgerState::Unavailable(CashUnavailable::UnclassifiedRow)
        );
        assert_eq!(
            ExternalFlow::from_settled_rows(&rows, window(), LedgerEvidence::Complete),
            ExternalFlows::Unavailable(CashUnavailable::UnclassifiedRow)
        );
        let mut absent = row("DEPOSIT", "10", Currency::Krw).data().clone();
        absent.net_amount = None;
        assert_eq!(
            ExternalFlow::from_settled_rows(
                &set(vec![Transaction::try_new(absent).expect("행")]),
                window(),
                LedgerEvidence::Complete
            ),
            ExternalFlows::Unavailable(CashUnavailable::MissingNetAmount)
        );
        let mut edge_rows = vec![];
        for day in [1, 2, 3, 4] {
            let mut data = row("DEPOSIT", "1", Currency::Krw).data().clone();
            data.settlement_date = date(day);
            edge_rows.push(Transaction::try_new(data).expect("행"));
        }
        let request = CashLedgerInput {
            window: SettlementWindow::try_new(date(2), date(3)).expect("기간"),
            currency: Currency::Krw,
            opening: Some(money("100", Currency::Krw)),
            evidence: LedgerEvidence::Complete,
            reported: Some(BuyingPower::new(money("102", Currency::Krw), time())),
        };
        let result = CashLedger::reconstruct(&set(edge_rows), request);
        assert_eq!(result.comparison(), &CashComparison::Matches);
    }
    #[test]
    fn exact_arithmetic_errors_never_expose_partial_cash() {
        for (code, opening, value, reported, error) in [
            (
                "WITHDRAWAL",
                "-79228162514264337593543950335",
                "1",
                "0",
                MoneyError::Overflow,
            ),
            (
                "DEPOSIT",
                "79228162514264337593543950334",
                "0.1",
                "0",
                MoneyError::PrecisionLoss,
            ),
            (
                "WITHDRAWAL",
                "0",
                "79228162514264337593543950335",
                "79228162514264337593543950335",
                MoneyError::Overflow,
            ),
        ] {
            let result = CashLedger::reconstruct(
                &set(vec![row(code, value, Currency::Krw)]),
                CashLedgerInput {
                    window: window(),
                    currency: Currency::Krw,
                    opening: Some(money(opening, Currency::Krw)),
                    evidence: LedgerEvidence::Complete,
                    reported: Some(BuyingPower::new(money(reported, Currency::Krw), time())),
                },
            );
            assert_eq!(
                result.state(),
                &CashLedgerState::Unavailable(CashUnavailable::Arithmetic(error))
            );
            assert_eq!(result.comparison(), &CashComparison::Unavailable);
        }
    }
    proptest! {
        #![proptest_config(Config::with_cases(256))]
        #[test]
        fn fractional_cash_matches_independent_minor_unit_sum(values in collection::vec((-4_i64..5, any::<bool>()),0..40)) {
            let mut rows=vec![]; let mut krw=10000_i64; let mut usd=10000_i64;
            for (change,is_usd) in values {
                let cents=change.checked_mul(25).expect("독립 정수 기준");
                let target=if is_usd {&mut usd} else {&mut krw};
                *target=target.checked_add(cents).expect("독립 합");
                let text=rust_decimal::Decimal::from_i128_with_scale(i128::from(cents.unsigned_abs()),2).to_string();
                rows.push(row(if change<0 {"WITHDRAWAL"} else {"DEPOSIT"},&text,if is_usd {Currency::Usd} else {Currency::Krw}));
            }
            let before=set(rows.clone());rows.reverse();let after=set(rows);
            for (currency,cents) in [(Currency::Krw,krw),(Currency::Usd,usd)] {
                let expected=rust_decimal::Decimal::from_i128_with_scale(i128::from(cents),2).to_string();
                let result=ledger(&before,currency,&expected);
                prop_assert_eq!(result.comparison(),&CashComparison::Matches);
                prop_assert_eq!(result,ledger(&after,currency,&expected));
            }
            prop_assert_eq!(ExternalFlow::from_settled_rows(&before,window(),LedgerEvidence::Complete),ExternalFlow::from_settled_rows(&after,window(),LedgerEvidence::Complete));
        }
    }
}
