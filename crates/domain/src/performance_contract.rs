#[cfg(test)]
mod tests {
    use crate::valuation_snapshot::tests::{date, fixture_window, observed, snapshot};
    use crate::{
        Currency, FlowPeriod, FlowUncertainty, Performance, PerformanceCurrency, PerformanceError,
        PerformancePeriod, SettlementWindow, TwrMethod,
    };
    use proptest::prelude::{prop_assert_eq, proptest};
    use proptest::test_runner::Config;
    fn fixture() -> crate::valuation_snapshot::tests::Fixture {
        fixture_window(
            vec![],
            ("100", "100"),
            ("100", "100"),
            SettlementWindow::try_new(date(2), date(5)).expect("기간"),
        )
    }
    #[test]
    fn sealed_inputs_preserve_currency_period_and_reject_untrusted_windows() {
        let data = fixture();
        let old = snapshot(&data, 2, &[], "1000");
        let today = snapshot(&data, 5, &[], "1000");
        let result = Performance::calculate(
            &[today, old],
            (&data.kp, &data.up),
            &[],
            PerformanceCurrency::Native(Currency::Krw),
        )
        .expect("성과");
        assert_eq!(result.start(), date(2));
        assert_eq!(result.end(), date(5));
        assert_eq!(
            result.currency(),
            PerformanceCurrency::Native(Currency::Krw)
        );
        assert_eq!(result.period(), PerformancePeriod::Cumulative);
        assert_eq!(result.method(), TwrMethod::LinkedValuations);
        assert_eq!(result.twr().to_string(), "0.000000");
        assert_eq!(result.mwr().expect("MWR").to_string(), "0.000000");
    }
    #[test]
    fn untrusted_periods_never_produce_values() {
        let data = fixture();
        let old = snapshot(&data, 2, &[], "1000");
        let today = snapshot(&data, 5, &[], "1000");
        let uncertain = FlowPeriod::Uncertain(FlowUncertainty::OutsideCoverage);
        assert_eq!(
            Performance::calculate(
                &[old.clone(), today.clone()],
                (&data.kp, &uncertain),
                &[],
                PerformanceCurrency::ConvertedKrw
            ),
            Err(PerformanceError::UncertainFlows)
        );
        assert_eq!(
            Performance::calculate(
                &[old.clone(), today.clone()],
                (&uncertain, &data.up),
                &[],
                PerformanceCurrency::ConvertedKrw
            ),
            Err(PerformanceError::UncertainFlows)
        );
        assert_eq!(
            Performance::calculate(
                &[old.clone(), today.clone()],
                (&data.up, &data.kp),
                &[],
                PerformanceCurrency::ConvertedKrw
            ),
            Err(PerformanceError::OutsideConfirmedPeriod)
        );
        assert_eq!(
            Performance::calculate(
                std::slice::from_ref(&old),
                (&data.kp, &data.up),
                &[],
                PerformanceCurrency::ConvertedKrw
            ),
            Err(PerformanceError::InvalidPeriod)
        );
        assert_eq!(
            Performance::calculate(
                &[],
                (&data.kp, &data.up),
                &[],
                PerformanceCurrency::ConvertedKrw
            ),
            Err(PerformanceError::InvalidPeriod)
        );
        assert_eq!(
            Performance::calculate(
                &[old, today, observed(3, "100", "100")],
                (&data.kp, &data.up),
                &[],
                PerformanceCurrency::ConvertedKrw
            ),
            Err(PerformanceError::CashBasisMismatch)
        );
    }
    proptest! {
        #![proptest_config(Config::with_cases(4096))]
        #[test]
        fn uncertain_and_incomplete_inputs_never_release_a_return(choice in 0_u8..3) {
            let data=fixture();
            let old=snapshot(&data,2,&[],"1000"); let today=snapshot(&data,5,&[],"1000");
            let reasons=[FlowUncertainty::OutsideCoverage,FlowUncertainty::UnclassifiedTransfer,FlowUncertainty::UnmatchedCancellation];
            let period=FlowPeriod::Uncertain(*reasons.get(usize::from(choice)).expect("사유"));
            prop_assert_eq!(Performance::calculate(&[old.clone(),today.clone()],(&period,&data.up),&[],PerformanceCurrency::ConvertedKrw),Err(PerformanceError::UncertainFlows));
            prop_assert_eq!(Performance::calculate(&[old,today,observed(3,"100","100")],(&data.kp,&data.up),&[],PerformanceCurrency::ConvertedKrw),Err(PerformanceError::CashBasisMismatch));
        }
        #[test]
        fn input_order_does_not_change_verified_currency_returns(rate in 1000_u32..1004) {
            let data=fixture(); let old=snapshot(&data,2,&[],"1000"); let today=snapshot(&data,5,&[],&rate.to_string());
            let forward=Performance::calculate(&[old.clone(),today.clone()],(&data.kp,&data.up),&[],PerformanceCurrency::ConvertedKrw).expect("순서");
            let reversed=Performance::calculate(&[today,old],(&data.kp,&data.up),&[],PerformanceCurrency::ConvertedKrw).expect("역순");
            prop_assert_eq!(forward,reversed);
        }
    }
}

#[cfg(test)]
mod flow_tests {
    use crate::valuation_snapshot::tests::{at, date, fixture_window, fx, snapshot, tx};
    use crate::{
        Currency, Performance, PerformanceCurrency, SettlementWindow, TransactionKind, TwrMethod,
    };
    #[test]
    fn external_inflows_outflows_and_exchanges_use_account_change() {
        for (kind, unit, opening, closing) in [
            (
                TransactionKind::Deposit,
                Currency::Usd,
                ("100", "100"),
                ("100", "110"),
            ),
            (
                TransactionKind::Withdrawal,
                Currency::Krw,
                ("100", "100"),
                ("90", "100"),
            ),
            (
                TransactionKind::ExchangeIn,
                Currency::Usd,
                ("100", "100"),
                ("100", "110"),
            ),
            (
                TransactionKind::ExchangeOut,
                Currency::Krw,
                ("100", "100"),
                ("90", "100"),
            ),
        ] {
            let data = fixture_window(
                vec![tx(kind.clone(), unit, 3, 1, "10")],
                opening,
                closing,
                SettlementWindow::try_new(date(2), date(5)).expect("기간"),
            );
            let snapshots = [
                snapshot(&data, 2, &[], "1000"),
                snapshot(&data, 5, &[], "1000"),
            ];
            let rates = [fx(at(3, 1), "1000")];
            let native = Performance::calculate(
                &snapshots,
                (&data.kp, &data.up),
                &rates,
                PerformanceCurrency::Native(unit),
            )
            .expect("통화별");
            assert_eq!(native.twr().to_string(), "0.000000");
            assert_eq!(native.method(), TwrMethod::LinkedModifiedDietz);
            if kind == TransactionKind::Deposit || kind == TransactionKind::Withdrawal {
                let converted = Performance::calculate(
                    &snapshots,
                    (&data.kp, &data.up),
                    &rates,
                    PerformanceCurrency::ConvertedKrw,
                )
                .expect("전체");
                assert_eq!(converted.twr().to_string(), "0.000000");
            }
        }
    }
}

#[cfg(test)]
mod period_tests {
    use crate::valuation_snapshot::tests::{
        at, calendar, date, fixture_window, fx, ledgers, snapshot, tx,
    };
    use crate::{
        Currency, Performance, PerformanceCurrency, PerformanceError, PerformancePeriod,
        SettlementWindow, SnapshotObservation, TransactionKind, ValuationSnapshot,
    };
    #[test]
    fn token_identity_zero_capital_and_subperiod_flow_exclusion_are_enforced() {
        let window = SettlementWindow::try_new(date(2), date(5)).expect("기간");
        let data = fixture_window(
            vec![tx(TransactionKind::Deposit, Currency::Krw, 3, 1, "10")],
            ("100", "100"),
            ("110", "100"),
            window,
        );
        let snapshots = [
            snapshot(&data, 2, &[], "1000"),
            snapshot(&data, 3, &[], "1000"),
            snapshot(&data, 5, &[], "1000"),
        ];
        assert_eq!(
            Performance::calculate(
                &snapshots,
                (&data.kp, &data.up),
                &[],
                PerformanceCurrency::Native(Currency::Krw)
            )
            .expect("구간")
            .twr()
            .to_string(),
            "0.000000"
        );
        let other = fixture_window(vec![], ("100", "100"), ("100", "100"), window);
        assert_eq!(
            Performance::calculate(
                &snapshots,
                (&other.kp, &other.up),
                &[],
                PerformanceCurrency::Native(Currency::Krw)
            ),
            Err(PerformanceError::OutsideConfirmedPeriod)
        );
        let zero = fixture_window(vec![], ("0", "0"), ("0", "0"), window);
        assert_eq!(
            Performance::calculate(
                &[
                    snapshot(&zero, 2, &[], "1000"),
                    snapshot(&zero, 5, &[], "1000")
                ],
                (&zero.kp, &zero.up),
                &[],
                PerformanceCurrency::Native(Currency::Krw)
            ),
            Err(PerformanceError::NonPositiveCapital)
        );
    }
    #[test]
    fn full_year_uses_annual_basis_and_preserves_fx_growth() {
        let window = SettlementWindow::try_new(date(2), date(367)).expect("기간");
        let data = fixture_window(vec![], ("0", "100"), ("0", "100"), window);
        let build = |day, rate| {
            ValuationSnapshot::recalculate(
                &calendar(day),
                &data.rows,
                &[],
                Some(ledgers(&data)),
                SnapshotObservation::new(at(day, 7), at(400, 0), Some(fx(at(day, 7), rate))),
            )
            .expect("평가")
        };
        let result = Performance::calculate(
            &[build(2, "1000"), build(367, "1100")],
            (&data.kp, &data.up),
            &[],
            PerformanceCurrency::ConvertedKrw,
        )
        .expect("연간 성과");
        assert_eq!(result.period(), PerformancePeriod::Annualized);
        assert_eq!(result.twr().to_string(), "0.100000");
        assert_eq!(result.mwr().expect("MWR").to_string(), "0.100000");
    }
}

#[cfg(test)]
mod rejected_period_tests {
    use crate::valuation_snapshot::tests::{
        at, calendar, date, fixture_window, fx, ledgers, snapshot, tx,
    };
    use crate::{
        Currency, Performance, PerformanceCurrency, PerformanceError, SettlementWindow,
        SnapshotObservation, TransactionKind, ValuationSnapshot,
    };
    #[test]
    fn inconsistent_settlement_timing_and_missing_fx_are_closed() {
        let window = SettlementWindow::try_new(date(2), date(5)).expect("기간");
        let mut raw = tx(TransactionKind::Deposit, Currency::Krw, 3, 1, "10")
            .data()
            .clone();
        raw.transaction_time = Some(at(1, 1));
        let inconsistent = fixture_window(
            vec![crate::Transaction::try_new(raw).expect("행")],
            ("100", "100"),
            ("110", "100"),
            window,
        );
        let snapshots = [
            snapshot(&inconsistent, 2, &[], "1000"),
            snapshot(&inconsistent, 5, &[], "1000"),
        ];
        assert_eq!(
            Performance::calculate(
                &snapshots,
                (&inconsistent.kp, &inconsistent.up),
                &[],
                PerformanceCurrency::Native(Currency::Krw)
            ),
            Err(PerformanceError::Snapshot(
                crate::SnapshotError::WrongPeriod
            ))
        );
        assert_eq!(
            Performance::calculate(
                &snapshots,
                (&inconsistent.kp, &inconsistent.up),
                &[],
                PerformanceCurrency::ConvertedKrw
            ),
            Err(PerformanceError::Snapshot(
                crate::SnapshotError::WrongPeriod
            ))
        );
        let usd = fixture_window(
            vec![tx(TransactionKind::Deposit, Currency::Usd, 3, 1, "10")],
            ("100", "100"),
            ("100", "110"),
            window,
        );
        assert_eq!(
            Performance::calculate(
                &[
                    snapshot(&usd, 2, &[], "1000"),
                    snapshot(&usd, 5, &[], "1000")
                ],
                (&usd.kp, &usd.up),
                &[],
                PerformanceCurrency::ConvertedKrw
            ),
            Err(PerformanceError::MissingFx)
        );
    }
    #[test]
    fn loss_of_all_capital_has_no_annualized_irr() {
        let window = SettlementWindow::try_new(date(2), date(367)).expect("기간");
        let data = fixture_window(
            vec![tx(TransactionKind::Fee, Currency::Krw, 3, 1, "100")],
            ("100", "0"),
            ("0", "0"),
            window,
        );
        let build = |day| {
            ValuationSnapshot::recalculate(
                &calendar(day),
                &data.rows,
                &[],
                Some(ledgers(&data)),
                SnapshotObservation::new(at(day, 7), at(400, 0), Some(fx(at(day, 7), "1000"))),
            )
            .expect("평가")
        };
        let result = Performance::calculate(
            &[build(2), build(367)],
            (&data.kp, &data.up),
            &[],
            PerformanceCurrency::Native(Currency::Krw),
        )
        .expect("전액 손실의 TWR");
        assert_eq!(result.twr().to_string(), "-1.000000");
        assert_eq!(result.mwr(), Err(PerformanceError::AmbiguousOrMissingRoot));
    }
}

#[cfg(test)]
mod calendar_tests {
    use crate::OffsetSeconds;
    use crate::valuation_snapshot::tests::at;
    use proptest::prelude::{prop_assert_eq, proptest};
    use proptest::test_runner::Config;
    #[test]
    fn year_boundaries_use_elapsed_calendar_days() {
        let make_date = |year, month, day| {
            crate::CalendarDate::from_parts(
                crate::CalendarYear::new(year),
                crate::MonthNumber::new(month),
                crate::DayNumber::new(day),
            )
            .expect("달력")
        };
        for (from, to, expected) in [
            (
                make_date(2024_i32, 2_u8, 28_u8),
                make_date(2025_i32, 2_u8, 28_u8),
                366_u32,
            ),
            (
                make_date(2024_i32, 2_u8, 29_u8),
                make_date(2025_i32, 2_u8, 28_u8),
                365_u32,
            ),
            (
                make_date(2024_i32, 3_u8, 1_u8),
                make_date(2025_i32, 2_u8, 28_u8),
                364_u32,
            ),
        ] {
            assert_eq!(
                *crate::performance::days(from, to).expect("기간").as_u32(),
                expected
            );
        }
    }
    proptest! {
        #![proptest_config(Config::with_cases(4096))]
        #[test]
        fn flow_day_weights_match_independent_calendar_projection(day in -700_000_i64..2_800_000,hour in 0_i64..24) {
            let start=at(2,7); let end=at(day,hour);
            let first=start.date_at_offset(OffsetSeconds::new(32400)).expect("KST");
            let last=end.date_at_offset(OffsetSeconds::new(32400)).expect("KST");
            if end<start { prop_assert_eq!(crate::performance::timestamp_days(start,end),Err(crate::PerformanceError::InvalidPeriod)); }
            else { let span=first.days_until(last).expect("날짜 차이"); let expected=u64::try_from(*span.as_i64()).expect("정순"); prop_assert_eq!(u64::from(*crate::performance::timestamp_days(start,end).expect("기간").as_u32()),expected); }
        }
    }
}

#[cfg(test)]
mod incomplete_tests {
    use crate::valuation_snapshot::tests::{
        at, calendar, confirmed, date, fixture_window, fx, ledgers, observed, snapshot,
    };
    use crate::{
        Currency, Performance, PerformanceCurrency, PerformanceError, SettlementWindow,
        SnapshotObservation, ValuationSnapshot,
    };
    #[test]
    fn incomplete_first_last_and_other_currency_never_form_a_period() {
        let window = SettlementWindow::try_new(date(2), date(5)).expect("기간");
        let data = fixture_window(vec![], ("100", "100"), ("100", "100"), window);
        let old = snapshot(&data, 2, &[], "1000");
        let today = snapshot(&data, 5, &[], "1000");
        for snapshots in [
            [observed(2, "100", "100"), today.clone()],
            [old, observed(5, "100", "100")],
        ] {
            assert_eq!(
                Performance::calculate(
                    &snapshots,
                    (&data.kp, &data.up),
                    &[],
                    PerformanceCurrency::Native(Currency::Krw)
                ),
                Err(PerformanceError::CashBasisMismatch)
            );
        }
        let missing_fx = ValuationSnapshot::recalculate(
            &calendar(2),
            &data.rows,
            &[],
            Some(ledgers(&data)),
            SnapshotObservation::new(at(2, 7), at(20, 0), None),
        )
        .expect("불완전");
        assert_eq!(
            Performance::calculate(
                &[missing_fx, today],
                (&data.kp, &data.up),
                &[],
                PerformanceCurrency::ConvertedKrw
            ),
            Err(PerformanceError::Snapshot(crate::SnapshotError::MissingFx))
        );
    }
    #[test]
    fn incomplete_other_currency_blocks_a_native_return() {
        let window = SettlementWindow::try_new(date(2), date(5)).expect("기간");
        let data = fixture_window(vec![], ("100", "100"), ("100", "100"), window);
        let old = snapshot(&data, 2, &[], "1000");
        let today = snapshot(&data, 5, &[], "1000");
        let broken = crate::CashLedger::reconstruct(
            &data.rows,
            crate::CashLedgerInput {
                window,
                currency: Currency::Usd,
                opening: None,
                evidence: None,
                reported: None,
            },
        );
        let pair = crate::SnapshotLedgers::new(
            (&data.kr, confirmed(&data.kp)),
            (&broken, confirmed(&data.up)),
        );
        let partial = ValuationSnapshot::recalculate(
            &calendar(3),
            &data.rows,
            &[],
            Some(pair),
            SnapshotObservation::new(at(3, 7), at(20, 0), Some(fx(at(3, 7), "1000"))),
        )
        .expect("불완전");
        assert_eq!(
            Performance::calculate(
                &[old, today, partial],
                (&data.kp, &data.up),
                &[],
                PerformanceCurrency::Native(Currency::Krw)
            ),
            Err(PerformanceError::CashBasisMismatch)
        );
    }
}

#[cfg(test)]
mod extreme_period_tests {
    use crate::valuation_snapshot::tests::{date, fixture_window, snapshot, tx};
    use crate::{
        Currency, Performance, PerformanceCurrency, PerformanceError, SettlementWindow,
        TransactionKind,
    };
    #[test]
    fn linked_dietz_overflow_never_becomes_a_complete_return() {
        let data = fixture_window(
            vec![
                tx(TransactionKind::Dividend, Currency::Krw, 3, 1, "300"),
                tx(
                    TransactionKind::Withdrawal,
                    Currency::Krw,
                    3,
                    1,
                    "199.99999999999999999999999998",
                ),
                tx(TransactionKind::Dividend, Currency::Krw, 5, 1, "500"),
            ],
            ("100", "0"),
            ("700.00000000000000000000000002", "0"),
            SettlementWindow::try_new(date(2), date(6)).expect("기간"),
        );
        let snapshots = [
            snapshot(&data, 2, &[], "1000"),
            snapshot(&data, 4, &[], "1000"),
            snapshot(&data, 6, &[], "1000"),
        ];
        assert_eq!(
            Performance::calculate(
                &snapshots,
                (&data.kp, &data.up),
                &[],
                PerformanceCurrency::Native(Currency::Krw)
            ),
            Err(PerformanceError::InsufficientPrecision)
        );
    }
}

#[cfg(test)]
mod boundary_flow_tests {
    use crate::valuation_snapshot::tests::{date, fixture_window, snapshot, tx};
    use crate::{
        Currency, Performance, PerformanceCurrency, PerformanceError, SettlementWindow,
        TransactionKind, TwrMethod,
    };
    #[test]
    fn endpoint_flow_foreign_currency_and_exchange_subperiods_are_distinct() {
        let window = SettlementWindow::try_new(date(2), date(5)).expect("기간");
        let exact = fixture_window(
            vec![tx(TransactionKind::Deposit, Currency::Krw, 3, 1, "10")],
            ("100", "100"),
            ("110", "100"),
            window,
        );
        let exact_result = Performance::calculate(
            &[
                snapshot(&exact, 2, &[], "1000"),
                snapshot(&exact, 3, &[], "1000"),
                snapshot(&exact, 5, &[], "1000"),
            ],
            (&exact.kp, &exact.up),
            &[],
            PerformanceCurrency::Native(Currency::Krw),
        )
        .expect("흐름 시점 평가");
        assert_eq!(exact_result.method(), TwrMethod::LinkedValuations);
        assert_eq!(exact_result.twr().to_string(), "0.000000");
        let foreign = fixture_window(
            vec![tx(TransactionKind::Deposit, Currency::Usd, 3, 1, "10")],
            ("100", "100"),
            ("100", "110"),
            window,
        );
        let foreign_result = Performance::calculate(
            &[
                snapshot(&foreign, 2, &[], "1000"),
                snapshot(&foreign, 5, &[], "1000"),
            ],
            (&foreign.kp, &foreign.up),
            &[],
            PerformanceCurrency::Native(Currency::Krw),
        )
        .expect("다른 통화");
        assert_eq!(foreign_result.method(), TwrMethod::LinkedValuations);
        assert_eq!(foreign_result.twr().to_string(), "0.000000");
        let exchange = fixture_window(
            vec![tx(TransactionKind::ExchangeIn, Currency::Usd, 3, 1, "10")],
            ("100", "100"),
            ("100", "110"),
            window,
        );
        let result = Performance::calculate(
            &[
                snapshot(&exchange, 2, &[], "1000"),
                snapshot(&exchange, 3, &[], "1000"),
                snapshot(&exchange, 5, &[], "1000"),
            ],
            (&exchange.kp, &exchange.up),
            &[],
            PerformanceCurrency::Native(Currency::Usd),
        )
        .expect("환전 구간");
        assert_eq!(result.twr().to_string(), "0.000000");
    }
    #[test]
    fn exact_inputs_can_still_exceed_the_linked_decimal_range() {
        let data = fixture_window(
            vec![
                tx(
                    TransactionKind::Dividend,
                    Currency::Krw,
                    3,
                    1,
                    "0.9999999999999999999999999999",
                ),
                tx(TransactionKind::Dividend, Currency::Krw, 5, 1, "999"),
            ],
            ("0.0000000000000000000000000001", "0"),
            ("1000", "0"),
            SettlementWindow::try_new(date(2), date(6)).expect("기간"),
        );
        assert_eq!(
            Performance::calculate(
                &[
                    snapshot(&data, 2, &[], "1000"),
                    snapshot(&data, 4, &[], "1000"),
                    snapshot(&data, 6, &[], "1000")
                ],
                (&data.kp, &data.up),
                &[],
                PerformanceCurrency::Native(Currency::Krw)
            ),
            Err(PerformanceError::Arithmetic)
        );
    }
}

#[cfg(test)]
mod annual_precision_tests {
    use crate::valuation_snapshot::tests::{at, calendar, date, fixture_window, fx, ledgers, tx};
    use crate::{
        Currency, Performance, PerformanceCurrency, PerformanceError, SettlementWindow,
        SnapshotObservation, TransactionKind, ValuationSnapshot,
    };
    #[test]
    fn annualized_twr_rejects_uncertified_cumulative_growth() {
        let rows = [(3, "99"), (5, "0.99"), (7, "0.0099"), (9, "0.000099")]
            .map(|(day, amount)| tx(TransactionKind::Fee, Currency::Krw, day, 1, amount));
        let data = fixture_window(
            rows.into(),
            ("100", "0"),
            ("0.000001", "0"),
            SettlementWindow::try_new(date(2), date(368)).expect("기간"),
        );
        let snapshots = [2, 4, 6, 8, 368].map(|day| {
            ValuationSnapshot::recalculate(
                &calendar(day),
                &data.rows,
                &[],
                Some(ledgers(&data)),
                SnapshotObservation::new(at(day, 7), at(400, 0), Some(fx(at(day, 7), "1000"))),
            )
            .expect("평가")
        });
        let value = Performance::calculate(
            &snapshots,
            (&data.kp, &data.up),
            &[],
            PerformanceCurrency::Native(Currency::Krw),
        )
        .expect("누적 성과");
        assert_eq!(
            value.annual_twr(),
            Err(PerformanceError::InsufficientPrecision)
        );
        assert_eq!(value.twr().to_string(), "-1.000000");
    }
}

#[cfg(test)]
mod holiday_sample_tests {
    use crate::valuation_snapshot::tests::{at, date, fixture_window, fx, ledgers, snapshot};
    use crate::{
        Currency, MarketCalendar, MarketSession, PerformanceCurrency, SessionData,
        SessionInformation, SettlementWindow, SnapshotCalendar, SnapshotObservation, TimeInterval,
        ValuationSnapshot,
    };
    fn holiday_snapshot() -> ValuationSnapshot {
        let open = |day| {
            Some(
                SessionData::try_new(
                    TimeInterval::try_new(at(day, 0), at(day, 1)).expect("구간"),
                    None,
                )
                .expect("장"),
            )
        };
        let kr = |day, session| {
            MarketSession::try_new(
                date(day),
                SessionInformation::Kr {
                    pre_market: None,
                    regular_market: session,
                    after_market: None,
                },
            )
            .expect("세션")
        };
        let us = |day, session| {
            MarketSession::try_new(
                date(day),
                SessionInformation::Us {
                    day_market: None,
                    pre_market: None,
                    regular_market: session,
                    after_market: None,
                },
            )
            .expect("세션")
        };
        let calendar = SnapshotCalendar::new(
            &MarketCalendar::try_new(kr(3, open(3)), kr(4, None), kr(5, open(5))).expect("달력"),
            &MarketCalendar::try_new(us(3, open(3)), us(4, open(4)), us(5, open(5))).expect("달력"),
        )
        .expect("평가 달력");
        let data = fixture_window(
            vec![],
            ("100", "100"),
            ("100", "100"),
            SettlementWindow::try_new(date(2), date(5)).expect("기간"),
        );
        ValuationSnapshot::recalculate(
            &calendar,
            &data.rows,
            &[],
            Some(ledgers(&data)),
            SnapshotObservation::new(at(4, 7), at(20, 0), Some(fx(at(4, 7), "1000"))),
        )
        .expect("휴장 평가")
    }
    #[test]
    fn holiday_carries_are_not_trading_day_samples() {
        use crate::performance::{DailySample, daily_sample};
        let data = fixture_window(
            vec![],
            ("100", "100"),
            ("100", "100"),
            SettlementWindow::try_new(date(2), date(5)).expect("기간"),
        );
        let current = snapshot(&data, 3, &[], "1000");
        let holiday = holiday_snapshot();
        assert_eq!(
            daily_sample(
                &current,
                &holiday,
                PerformanceCurrency::Native(Currency::Krw)
            ),
            DailySample::Trading
        );
        assert_eq!(
            daily_sample(&holiday, &current, PerformanceCurrency::ConvertedKrw),
            DailySample::Unverified
        );
        assert_eq!(
            daily_sample(
                &current,
                &holiday,
                PerformanceCurrency::Native(Currency::Usd)
            ),
            DailySample::Trading
        );
        let mut portfolio = crate::performance::tests::series(60);
        *portfolio.daily_samples.first_mut().expect("표본") = DailySample::Unverified;
        assert_eq!(
            crate::performance_volatility(&portfolio).daily(),
            crate::RiskMetric::InsufficientSamples
        );
        let mut enough = crate::performance::tests::series(61);
        *enough.daily_samples.first_mut().expect("표본") = DailySample::Unverified;
        assert_eq!(
            crate::performance_volatility(&enough).daily(),
            crate::RiskMetric::Unavailable(crate::PerformanceError::InvalidPeriod)
        );
    }
}

#[cfg(test)]
mod exact_window_tests {
    use crate::valuation_snapshot::tests::{date, fixture_window, snapshot};
    use crate::{Currency, Performance, PerformanceCurrency, PerformanceError, SettlementWindow};
    #[test]
    fn a_confirmed_larger_window_cannot_authorize_a_partial_period_return() {
        let data = fixture_window(
            vec![],
            ("100", "100"),
            ("100", "100"),
            SettlementWindow::try_new(date(2), date(5)).expect("확정 기간"),
        );
        for (start, end) in [(2, 4), (3, 5), (3, 4)] {
            assert_eq!(
                Performance::calculate(
                    &[
                        snapshot(&data, start, &[], "1000"),
                        snapshot(&data, end, &[], "1000")
                    ],
                    (&data.kp, &data.up),
                    &[],
                    PerformanceCurrency::Native(Currency::Krw)
                ),
                Err(PerformanceError::OutsideConfirmedPeriod)
            );
        }
    }
}

#[cfg(test)]
mod review_regression_tests {
    use crate::valuation_snapshot::tests::{Fixture, at, calendar, date, fx, ledgers, money, tx};
    use crate::{
        BuyingPower, CashLedger, CashLedgerInput, Currency, ExternalFlows, Performance,
        PerformanceCurrency, SettlementWindow, SnapshotObservation, Transaction, TransactionKind,
        TwrMethod, ValuationSnapshot,
    };
    fn data(transactions: Vec<Transaction>, opening: &str, closing: &str, end: i64) -> Fixture {
        let rows = crate::cash_ledger::tests::set(transactions);
        let window = SettlementWindow::try_new(date(2), date(end)).expect("기간");
        let ledger = |unit, first, last| {
            CashLedger::reconstruct(
                &rows,
                CashLedgerInput {
                    window,
                    currency: unit,
                    opening: Some(money(first, unit)),
                    evidence: Some(crate::cash_ledger::tests::evidence(&rows, window)),
                    reported: Some(BuyingPower::new(money(last, unit), at(400, 0))),
                },
            )
        };
        let kr = ledger(Currency::Krw, opening, closing);
        let us = ledger(Currency::Usd, "0", "0");
        let period = |record: &CashLedger| match record.external_flows(&[]) {
            ExternalFlows::Reconciled(value) => value.period(window),
            ExternalFlows::Unavailable(_) => panic!("확정 흐름"),
        };
        let kp = period(&kr);
        let up = period(&us);
        Fixture {
            rows,
            kr,
            us,
            kp,
            up,
        }
    }
    fn build(data: &Fixture, day: i64) -> ValuationSnapshot {
        ValuationSnapshot::recalculate(
            &calendar(day),
            &data.rows,
            &[],
            Some(ledgers(data)),
            SnapshotObservation::new(at(day, 7), at(400, 0), Some(fx(at(day, 7), "1000"))),
        )
        .expect("평가")
    }
    #[test]
    fn research_one_through_sealed_inputs_keeps_later_flow_offset_and_endpoint() {
        let data = data(
            vec![
                tx(TransactionKind::Dividend, Currency::Krw, 181, 1, "1000000"),
                tx(TransactionKind::Deposit, Currency::Krw, 182, 7, "5000000"),
                tx(TransactionKind::Fee, Currency::Krw, 364, 1, "600000"),
            ],
            "10000000",
            "15400000",
            365,
        );
        let snapshots = [2, 100, 182, 365].map(|day| build(&data, day));
        let value = Performance::calculate(
            &snapshots,
            (&data.kp, &data.up),
            &[],
            PerformanceCurrency::Native(Currency::Krw),
        )
        .expect("성과");
        assert!(format!("{value:?}").contains("PerformanceSnapshots([REDACTED])"));
        assert_eq!(value.twr().to_string(), "0.058750");
        assert_eq!(value.mwr().expect("MWR").to_string(), "0.031997");
        assert_eq!(value.method(), TwrMethod::LinkedValuations);
        assert_eq!(
            value.annual_twr(),
            Err(crate::PerformanceError::NotAnnualized)
        );
    }
    #[test]
    fn outgoing_exchange_with_income_keeps_investor_cash_sign() {
        let data = data(
            vec![
                tx(TransactionKind::ExchangeOut, Currency::Krw, 3, 1, "10"),
                tx(TransactionKind::Dividend, Currency::Krw, 4, 1, "1"),
            ],
            "100",
            "91",
            5,
        );
        let value = Performance::calculate(
            &[build(&data, 2), build(&data, 5)],
            (&data.kp, &data.up),
            &[],
            PerformanceCurrency::Native(Currency::Krw),
        )
        .expect("성과");
        assert_eq!(value.twr().to_string(), "0.010714");
        assert_eq!(value.mwr().expect("MWR").to_string(), "0.010713");
    }
    #[test]
    fn full_withdrawal_and_reinvestment_do_not_hide_a_zero_middle_valuation() {
        let data = data(
            vec![
                tx(TransactionKind::Withdrawal, Currency::Krw, 3, 1, "100"),
                tx(TransactionKind::Deposit, Currency::Krw, 4, 1, "100"),
            ],
            "100",
            "100",
            5,
        );
        assert_eq!(
            Performance::calculate(
                &[build(&data, 2), build(&data, 3), build(&data, 5)],
                (&data.kp, &data.up),
                &[],
                PerformanceCurrency::Native(Currency::Krw)
            ),
            Err(crate::PerformanceError::NonPositiveCapital)
        );
    }
    #[test]
    fn bad_cash_evidence_cannot_mint_a_fixture_period() {
        assert!(std::panic::catch_unwind(|| data(vec![], "100", "200", 5)).is_err());
    }
    #[test]
    fn same_calendar_day_flows_have_zero_weight_and_reverse_times_are_errors() {
        let data = data(
            vec![
                tx(TransactionKind::ExchangeOut, Currency::Krw, 3, 1, "10"),
                tx(TransactionKind::Dividend, Currency::Krw, 3, 2, "1"),
            ],
            "100",
            "91",
            3,
        );
        let value = Performance::calculate(
            &[build(&data, 2), build(&data, 3)],
            (&data.kp, &data.up),
            &[],
            PerformanceCurrency::Native(Currency::Krw),
        )
        .expect("당일 환전");
        assert_eq!(value.method(), TwrMethod::LinkedValuations);
        assert_eq!(value.twr().to_string(), "0.010000");
        assert_eq!(value.mwr().expect("MWR").to_string(), "0.010000");
        assert_eq!(
            crate::performance::timestamp_days(at(3, 7), at(3, 1)),
            Err(crate::PerformanceError::InvalidPeriod)
        );
        let limit = crate::Timestamp::from_parts(
            crate::UnixSeconds::new(253_402_300_799),
            crate::Nanosecond::new(0),
            crate::OffsetSeconds::new(0),
        )
        .expect("최대 시각");
        assert_eq!(
            crate::performance::timestamp_days(at(2, 7), limit),
            Err(crate::PerformanceError::InvalidPeriod)
        );
    }
}

#[cfg(test)]
mod awaiting_session_tests {
    use crate::valuation_snapshot::tests::{at, date, fixture_window, fx, ledgers};
    use crate::{
        Currency, MarketCalendar, MarketSession, PerformanceCurrency, SessionData,
        SessionInformation, SettlementWindow, SnapshotCalendar, SnapshotObservation,
        SnapshotPriceBasis, TimeInterval, ValuationSnapshot,
    };
    fn snapshot(day: i64) -> ValuationSnapshot {
        let session = |session_day, unit| {
            let (open, close) = match unit {
                Currency::Krw => (0, 6),
                Currency::Usd => (17, 21),
            };
            let data = Some(
                SessionData::try_new(
                    TimeInterval::try_new(at(session_day, open), at(session_day, close))
                        .expect("시간"),
                    None,
                )
                .expect("장"),
            );
            let information = match unit {
                Currency::Krw => SessionInformation::Kr {
                    pre_market: None,
                    regular_market: data,
                    after_market: None,
                },
                Currency::Usd => SessionInformation::Us {
                    day_market: None,
                    pre_market: None,
                    regular_market: data,
                    after_market: None,
                },
            };
            MarketSession::try_new(date(session_day), information).expect("세션")
        };
        let market = |unit| {
            MarketCalendar::try_new(
                session(day.wrapping_sub(1), unit),
                session(day, unit),
                session(day.wrapping_add(1), unit),
            )
            .expect("달력")
        };
        let calendar =
            SnapshotCalendar::new(&market(Currency::Krw), &market(Currency::Usd)).expect("달력");
        let data = fixture_window(
            vec![],
            ("100", "100"),
            ("100", "100"),
            SettlementWindow::try_new(date(2), date(5)).expect("기간"),
        );
        ValuationSnapshot::recalculate(
            &calendar,
            &data.rows,
            &[],
            Some(ledgers(&data)),
            SnapshotObservation::new(at(day, 7), at(20, 0), Some(fx(at(day, 7), "1000"))),
        )
        .expect("16시 평가")
    }
    #[test]
    fn kst_sixteen_uses_confirmed_previous_us_close_as_a_daily_sample() {
        let old = snapshot(3);
        let today = snapshot(4);
        assert_eq!(
            today.price_basis(Currency::Usd),
            SnapshotPriceBasis::AwaitingSession
        );
        assert_eq!(today.price_date(Currency::Usd), date(3));
        for unit in [
            PerformanceCurrency::Native(Currency::Usd),
            PerformanceCurrency::ConvertedKrw,
        ] {
            assert_eq!(
                crate::performance::daily_sample(&old, &today, unit),
                crate::performance::DailySample::Trading
            );
        }
    }
}
