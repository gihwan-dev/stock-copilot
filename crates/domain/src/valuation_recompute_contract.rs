#[cfg(test)]
mod tests {
    use crate::valuation_snapshot::tests::{
        at, calendar, date, fixture_window, fx, ledgers, money, tx,
    };
    use crate::{
        AssetClass, CalendarDate, Candle, CandleObservation, CandlePeriod, CandleStatus,
        CandleTime, CorporateAction, CorporateActionId, CorporateActionKind, Currency,
        DatedSnapshotLedgers, InstrumentId, LotIdentity, Market, MarketCalendar, MarketSession,
        NumericInput, Ohlcv, Price, PriceAdjustment, Quantity, SessionData, SessionInformation,
        SettlementWindow, ShareCount, ShareRatio, SnapshotCalendar, SnapshotError,
        SnapshotInstrumentHistory, SnapshotObservation, Symbol, TimeInterval, Timestamp,
        Transaction, TransactionKind, ValuationHistoryEvidence, ValuationPriceEvidence,
        ValuationRecomputeInput, ValuationSnapshot, valuation_time,
    };
    use proptest::prelude::ProptestConfig;
    use proptest::{prop_assert_eq, proptest};

    fn id(value: &str) -> InstrumentId {
        InstrumentId::try_new(value).expect("ID")
    }

    fn price(value: &str, currency: Currency) -> Price {
        Price::try_new(money(value, currency)).expect("가격")
    }

    fn trade(
        symbol: &str,
        currency: Currency,
        day: i64,
        quantity: &str,
        amount: &str,
    ) -> Transaction {
        let mut data = tx(TransactionKind::Buy, currency, day, 1, amount)
            .data()
            .clone();
        data.symbol = Some(Symbol::try_new(symbol).expect("심볼"));
        data.asset_class = Some(AssetClass::Stock);
        data.quantity =
            Some(Quantity::parse(&NumericInput::try_new(quantity).expect("입력")).expect("수량"));
        Transaction::try_new(data).expect("매매")
    }

    fn history(
        symbol: &str,
        currency: Currency,
        day: i64,
        close: &str,
        actions: Vec<CorporateAction>,
    ) -> SnapshotInstrumentHistory {
        let identity = LotIdentity::new(
            id(symbol),
            Symbol::try_new(symbol).expect("심볼"),
            match currency {
                Currency::Krw => Market::Kr,
                Currency::Usd => Market::Us,
            },
            currency,
        );
        let value = price(close, currency);
        let candle = Candle::try_new(
            id(symbol),
            CandleTime::try_new(at(day, 0), CandlePeriod::Daily).expect("시각"),
            PriceAdjustment::Raw,
            Ohlcv::try_new(
                value,
                value,
                value,
                value,
                Quantity::parse(&NumericInput::try_new("0").expect("입력")).expect("수량"),
            )
            .expect("봉"),
            CandleObservation::new(at(20, 0), CandleStatus::Finalized),
        )
        .expect("봉");
        SnapshotInstrumentHistory::new(identity, actions, vec![candle]).expect("이력")
    }

    fn close(
        symbol: &str,
        currency: Currency,
        day: i64,
        value: &str,
    ) -> (InstrumentId, ValuationPriceEvidence) {
        (
            id(symbol),
            ValuationPriceEvidence::SessionClose {
                price: price(value, currency),
                date: date(day),
            },
        )
    }

    fn observation(day: i64, rate: Option<&str>) -> SnapshotObservation {
        SnapshotObservation::new(
            at(day, 7),
            at(20, 0),
            rate.map(|value| fx(at(day, 7), value)),
        )
    }

    struct TestCash<'data> {
        rows: Vec<Transaction>,
        opening: (&'data str, &'data str),
        ending: (&'data str, &'data str),
    }
    impl<'data> TestCash<'data> {
        fn new(
            rows: Vec<Transaction>,
            opening: (&'data str, &'data str),
            ending: (&'data str, &'data str),
        ) -> Self {
            Self {
                rows,
                opening,
                ending,
            }
        }
    }
    fn test_window(day: i64) -> SettlementWindow {
        SettlementWindow::try_new(date(1), date(day)).expect("기간")
    }
    fn snapshot(
        day: i64,
        cash: TestCash<'_>,
        histories: &[SnapshotInstrumentHistory],
        evidence: &[(InstrumentId, ValuationPriceEvidence)],
        rate: Option<&str>,
    ) -> Result<ValuationSnapshot, SnapshotError> {
        let fixture_data = fixture_window(cash.rows, cash.opening, cash.ending, test_window(day));
        let cal = calendar(day);
        let input = ValuationRecomputeInput::new(
            &cal,
            &fixture_data.rows,
            ValuationHistoryEvidence::new(histories, evidence),
            DatedSnapshotLedgers::new(date(day), ledgers(&fixture_data)),
            observation(day, rate),
        )?;
        ValuationSnapshot::recompute(input)
    }

    #[test]
    fn valuation_time_is_16_kst_and_kst_midnight_crosses_utc_date() {
        let day = CalendarDate::from_parts(
            crate::CalendarYear::new(2026),
            crate::MonthNumber::new(1),
            crate::DayNumber::new(1),
        )
        .expect("날짜");
        let time = valuation_time(day);
        assert_eq!(*time.utc_seconds().as_i64(), 1_767_250_800);
        assert_eq!(*time.offset_seconds().as_i32(), 0_i32);
        assert_eq!(
            time.date_at_offset(crate::OffsetSeconds::new(32_400)),
            Ok(day)
        );
        let previous_utc = Timestamp::from_parts(
            crate::UnixSeconds::new(1_767_193_199),
            crate::Nanosecond::new(0),
            crate::OffsetSeconds::new(32_400),
        )
        .expect("자정 전");
        let next_utc = Timestamp::from_parts(
            crate::UnixSeconds::new(1_767_193_200),
            crate::Nanosecond::new(0),
            crate::OffsetSeconds::new(32_400),
        )
        .expect("자정");
        assert_eq!(
            previous_utc
                .date_at_offset(crate::OffsetSeconds::new(32_400))
                .expect("날짜"),
            CalendarDate::from_parts(
                crate::CalendarYear::new(2025),
                crate::MonthNumber::new(12),
                crate::DayNumber::new(31)
            )
            .expect("전일")
        );
        assert_eq!(
            next_utc
                .date_at_offset(crate::OffsetSeconds::new(32_400))
                .expect("날짜"),
            day
        );
        assert_eq!(
            next_utc.date_at_offset(crate::OffsetSeconds::new(0)),
            CalendarDate::from_parts(
                crate::CalendarYear::new(2025),
                crate::MonthNumber::new(12),
                crate::DayNumber::new(31)
            )
        );
    }

    #[test]
    fn recalculated_us_stock_and_cash_match_d31_expected_values() {
        let rows = vec![trade("A", Currency::Usd, 3, "10", "100")];
        let histories = [history("A", Currency::Usd, 3, "10", vec![])];
        let evidence = [close("A", Currency::Usd, 3, "10")];
        let result = snapshot(
            3,
            TestCash::new(rows, ("0", "1000"), ("0", "900")),
            &histories,
            &evidence,
            Some("1375"),
        )
        .expect("재계산");
        assert_eq!(
            result.stock_value(Currency::Usd),
            Ok(money("100", Currency::Usd))
        );
        assert_eq!(
            result.account_value(Currency::Usd),
            Ok(money("1000", Currency::Usd))
        );
        assert_eq!(
            result
                .account_krw_value()
                .expect("환산")
                .clone()
                .into_parts()
                .0,
            money("1375000", Currency::Krw)
        );
        assert_eq!(result.valuation_at(), at(3, 7));
    }

    #[test]
    fn evidence_must_match_the_actual_candle_currency_date_and_kind() {
        let histories = [history("A", Currency::Krw, 3, "12", vec![])];
        let rows = vec![trade("A", Currency::Krw, 1, "10", "100")];
        let run = |evidence: &[(InstrumentId, ValuationPriceEvidence)]| {
            snapshot(
                3,
                TestCash::new(rows.clone(), ("1000", "0"), ("900", "0")),
                &histories,
                evidence,
                None,
            )
        };
        assert_eq!((run(&[])).err(), Some(SnapshotError::MissingPrice));
        assert_eq!(
            (run(&[close("A", Currency::Krw, 3, "13")])).err(),
            Some(SnapshotError::InvalidCandle)
        );
        assert_eq!(
            (run(&[close("A", Currency::Usd, 3, "12")])).err(),
            Some(SnapshotError::InvalidCandle)
        );
        assert_eq!(
            (run(&[close("A", Currency::Krw, 2, "12")])).err(),
            Some(SnapshotError::InvalidCandle)
        );
        assert_eq!(
            (run(&[(
                id("A"),
                ValuationPriceEvidence::Intraday {
                    price: price("12", Currency::Krw),
                    observed_at: at(3, 1)
                }
            )]))
            .err(),
            Some(SnapshotError::InvalidCandle)
        );
        assert_eq!(
            (run(&[
                close("A", Currency::Krw, 3, "12"),
                close("A", Currency::Krw, 3, "12")
            ]))
            .err(),
            Some(SnapshotError::DuplicateInstrument)
        );
    }

    #[test]
    fn a_later_reconciled_period_rewinds_cash_to_the_valuation_day() {
        let fixture_data = fixture_window(
            vec![
                trade("A", Currency::Usd, 1, "10", "100"),
                tx(TransactionKind::Withdrawal, Currency::Usd, 4, 1, "100"),
            ],
            ("0", "1000"),
            ("0", "800"),
            SettlementWindow::try_new(date(1), date(4)).expect("기간"),
        );
        let cal = calendar(3);
        let histories = [history("A", Currency::Usd, 3, "10", vec![])];
        let evidence = [close("A", Currency::Usd, 3, "10")];
        let input = ValuationRecomputeInput::new(
            &cal,
            &fixture_data.rows,
            ValuationHistoryEvidence::new(&histories, &evidence),
            DatedSnapshotLedgers::new(date(3), ledgers(&fixture_data)),
            observation(3, Some("1375")),
        )
        .expect("과거일 현금");
        let result = ValuationSnapshot::recompute(input).expect("재계산");
        assert_eq!(
            result.stock_value(Currency::Usd),
            Ok(money("100", Currency::Usd))
        );
        assert_eq!(
            result.account_value(Currency::Usd),
            Ok(money("1000", Currency::Usd))
        );
        assert_eq!(result.valuation_at(), valuation_time(date(3)));
    }

    #[test]
    fn daily_recomputations_share_the_period_needed_for_performance() {
        use crate::{Performance, PerformanceCurrency};
        let fixture_data = fixture_window(
            vec![],
            ("1000", "0"),
            ("1000", "0"),
            SettlementWindow::try_new(date(2), date(3)).expect("기간"),
        );
        let make = |day| {
            let cal = calendar(day);
            ValuationRecomputeInput::new(
                &cal,
                &fixture_data.rows,
                ValuationHistoryEvidence::new(&[], &[]),
                DatedSnapshotLedgers::new(date(day), ledgers(&fixture_data)),
                observation(day, None),
            )
            .and_then(ValuationSnapshot::recompute)
            .expect("평가")
        };
        let old = make(2);
        let current = make(3);
        let performance = Performance::calculate(
            &[old, current],
            (&fixture_data.kp, &fixture_data.up),
            &[],
            PerformanceCurrency::Native(Currency::Krw),
        )
        .expect("성과");
        assert_eq!(performance.start(), date(2));
        assert_eq!(performance.end(), date(3));
        assert_eq!(performance.twr().to_string(), "0.000000");
    }

    #[test]
    fn mismatched_ledger_date_is_rejected_when_created() {
        let fixture_data = fixture_window(
            vec![],
            ("100", "0"),
            ("100", "0"),
            SettlementWindow::try_new(date(1), date(4)).expect("기간"),
        );
        let cal = calendar(3);
        assert_eq!(
            (ValuationRecomputeInput::new(
                &cal,
                &fixture_data.rows,
                ValuationHistoryEvidence::new(&[], &[]),
                DatedSnapshotLedgers::new(date(4), ledgers(&fixture_data)),
                observation(3, None)
            ))
            .err(),
            Some(SnapshotError::WrongPeriod)
        );
    }

    #[test]
    fn ledger_period_must_cover_the_requested_cash_date() {
        let fixture_data = fixture_window(
            vec![],
            ("100", "0"),
            ("100", "0"),
            SettlementWindow::try_new(date(1), date(2)).expect("기간"),
        );
        let cal = calendar(3);
        assert_eq!(
            ValuationRecomputeInput::new(
                &cal,
                &fixture_data.rows,
                ValuationHistoryEvidence::new(&[], &[]),
                DatedSnapshotLedgers::new(date(3), ledgers(&fixture_data)),
                observation(3, None),
            )
            .err(),
            Some(SnapshotError::OutsideConfirmedPeriod)
        );
    }

    #[test]
    fn corporate_action_is_applied_only_from_its_date() {
        let ratio = ShareRatio::new(
            ShareCount::try_new(1).expect("구주"),
            ShareCount::try_new(2).expect("신주"),
        );
        let action = CorporateAction::try_new(
            CorporateActionId::new(id("split")),
            date(3),
            CorporateActionKind::Split(ratio),
        )
        .expect("분할");
        let rows = vec![trade("A", Currency::Krw, 1, "10", "100")];
        let before = [history("A", Currency::Krw, 2, "12", vec![action.clone()])];
        let after = [history("A", Currency::Krw, 3, "7", vec![action])];
        let before_snapshot = snapshot(
            2,
            TestCash::new(rows.clone(), ("1000", "0"), ("900", "0")),
            &before,
            &[close("A", Currency::Krw, 2, "12")],
            None,
        )
        .expect("전");
        let after_snapshot = snapshot(
            3,
            TestCash::new(rows, ("1000", "0"), ("900", "0")),
            &after,
            &[close("A", Currency::Krw, 3, "7")],
            None,
        )
        .expect("후");
        assert_eq!(
            before_snapshot.stock_value(Currency::Krw),
            Ok(money("120", Currency::Krw))
        );
        assert_eq!(
            after_snapshot.stock_value(Currency::Krw),
            Ok(money("140", Currency::Krw))
        );
    }

    #[derive(Clone, Copy)]
    enum TestSession {
        Open,
        Closed,
    }

    fn market_row(day: i64, market: &Market, state: TestSession) -> MarketSession {
        let session = match state {
            TestSession::Open => Some({
                SessionData::try_new(
                    TimeInterval::try_new(at(day, 0), at(day, 1)).expect("구간"),
                    None,
                )
                .expect("장")
            }),
            TestSession::Closed => None,
        };
        let info = match market {
            Market::Kr => SessionInformation::Kr {
                pre_market: None,
                regular_market: session,
                after_market: None,
            },
            Market::Us | Market::Unknown(_) => SessionInformation::Us {
                day_market: None,
                pre_market: None,
                regular_market: session,
                after_market: None,
            },
        };
        MarketSession::try_new(date(day), info).expect("시장")
    }

    #[test]
    fn holiday_carry_uses_the_previous_confirmed_close() {
        let kr = MarketCalendar::try_new(
            market_row(1, &Market::Kr, TestSession::Open),
            market_row(3, &Market::Kr, TestSession::Closed),
            market_row(4, &Market::Kr, TestSession::Open),
        )
        .expect("한국");
        let us = MarketCalendar::try_new(
            market_row(2, &Market::Us, TestSession::Open),
            market_row(3, &Market::Us, TestSession::Open),
            market_row(4, &Market::Us, TestSession::Open),
        )
        .expect("미국");
        let cal = SnapshotCalendar::new(&kr, &us).expect("달력");
        let fixture_data = fixture_window(
            vec![trade("A", Currency::Krw, 1, "10", "100")],
            ("1000", "0"),
            ("900", "0"),
            SettlementWindow::try_new(date(1), date(3)).expect("기간"),
        );
        let histories = [history("A", Currency::Krw, 1, "12", vec![])];
        let evidence = [(
            id("A"),
            ValuationPriceEvidence::HolidayCarry {
                price: price("12", Currency::Krw),
                date: date(1),
            },
        )];
        let input = ValuationRecomputeInput::new(
            &cal,
            &fixture_data.rows,
            ValuationHistoryEvidence::new(&histories, &evidence),
            DatedSnapshotLedgers::new(date(3), ledgers(&fixture_data)),
            observation(3, None),
        )
        .expect("입력");
        let result = ValuationSnapshot::recompute(input).expect("재계산");
        assert_eq!(
            result.stock_value(Currency::Krw),
            Ok(money("120", Currency::Krw))
        );
        assert_eq!(result.price_date(Currency::Krw), date(1));
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(testkit::property_cases()))]
        #[test]
        fn valid_cash_only_days_keep_the_exact_valuation_time(day in 2_i64..9) {
            let value = snapshot(day, TestCash::new(vec![], ("123", "0"), ("123", "0")), &[], &[], None).expect("현금만");
            prop_assert_eq!(value.valuation_at(), valuation_time(date(day)));
            prop_assert_eq!(value.account_value(Currency::Krw), Ok(money("123", Currency::Krw)));
        }
        #[test]
        fn a_missing_price_never_becomes_a_snapshot(quantity in 1_u32..100) {
            let quantity_text = quantity.to_string();
            let histories = [history("A", Currency::Krw, 3, "12", vec![])];
            let result = snapshot(3, TestCash::new(vec![trade("A", Currency::Krw, 1, &quantity_text, "100")], ("1000", "0"), ("900", "0")), &histories, &[], None);
            prop_assert_eq!(result.err(), Some(SnapshotError::MissingPrice));
        }
    }

    #[test]
    fn zero_quantity_after_sale_needs_no_price_but_pending_action_is_not_zero() {
        let buy = trade("A", Currency::Krw, 1, "10", "100");
        let mut sale = trade("A", Currency::Krw, 3, "10", "100").data().clone();
        sale.kind = TransactionKind::Sell;
        let rows = vec![buy, Transaction::try_new(sale).expect("매도")];
        let source = history("A", Currency::Krw, 4, "12", vec![]);
        let empty = [
            SnapshotInstrumentHistory::new(source.identity().clone(), vec![], vec![])
                .expect("이력"),
        ];
        let result = snapshot(
            4,
            TestCash::new(rows.clone(), ("1000", "0"), ("1000", "0")),
            &empty,
            &[],
            None,
        )
        .expect("0주");
        assert_eq!(
            result.stock_value(Currency::Krw),
            Ok(money("0", Currency::Krw))
        );
        assert_eq!(
            result.account_value(Currency::Krw),
            Ok(money("1000", Currency::Krw))
        );
        let ratio = ShareRatio::new(
            ShareCount::try_new(10).expect("구주"),
            ShareCount::try_new(1).expect("신주"),
        );
        let action = CorporateAction::try_new(
            CorporateActionId::new(id("dividend")),
            date(2),
            CorporateActionKind::StockDividend {
                ratio,
                receipt: None,
                basis: crate::DividendBasis::Zero,
            },
        )
        .expect("배당");
        let pending =
            [
                SnapshotInstrumentHistory::new(source.identity().clone(), vec![action], vec![])
                    .expect("이력"),
            ];
        assert_eq!(
            (snapshot(
                4,
                TestCash::new(rows, ("1000", "0"), ("1000", "0")),
                &pending,
                &[],
                None
            ))
            .err(),
            Some(SnapshotError::PendingCorporateCash)
        );
    }

    #[test]
    fn first_evaluation_after_consecutive_holidays_is_not_skipped() {
        use crate::{ValuationCalendarDay, ValuationGapPlan};
        let calendar_day = |number: i64, state: TestSession| {
            let kr = MarketCalendar::try_new(
                market_row(1, &Market::Kr, TestSession::Open),
                market_row(number, &Market::Kr, state),
                market_row(5, &Market::Kr, TestSession::Open),
            )
            .expect("한국");
            let us = MarketCalendar::try_new(
                market_row(1, &Market::Us, TestSession::Open),
                market_row(number, &Market::Us, state),
                market_row(5, &Market::Us, TestSession::Open),
            )
            .expect("미국");
            ValuationCalendarDay::new(&kr, &us).expect("날짜")
        };
        let dates = [
            calendar_day(2, TestSession::Closed),
            calendar_day(3, TestSession::Closed),
            calendar_day(4, TestSession::Open),
        ];
        let plan = ValuationGapPlan::new(date(1), at(4, 8), &dates).expect("계획");
        let fixture_data = fixture_window(
            vec![trade("A", Currency::Krw, 1, "10", "100")],
            ("1000", "0"),
            ("900", "0"),
            SettlementWindow::try_new(date(1), date(4)).expect("기간"),
        );
        let histories = [history("A", Currency::Krw, 4, "13", vec![])];
        let prices = [close("A", Currency::Krw, 4, "13")];
        let results = plan.resolve(|day, cal| {
            assert_eq!(day, date(4));
            ValuationRecomputeInput::new(
                &cal,
                &fixture_data.rows,
                ValuationHistoryEvidence::new(&histories, &prices),
                DatedSnapshotLedgers::new(date(4), ledgers(&fixture_data)),
                observation(4, None),
            )
            .and_then(ValuationSnapshot::recompute)
        });
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].date(), date(4));
        assert_eq!(
            results[0]
                .result()
                .expect("평가")
                .stock_value(Currency::Krw),
            Ok(money("130", Currency::Krw))
        );
    }

    #[test]
    fn failed_recompute_is_a_day_result_with_its_reason() {
        use crate::{ValuationCalendarDay, ValuationGapPlan};
        let kr = MarketCalendar::try_new(
            market_row(2, &Market::Kr, TestSession::Open),
            market_row(3, &Market::Kr, TestSession::Open),
            market_row(4, &Market::Kr, TestSession::Open),
        )
        .expect("한국");
        let us = MarketCalendar::try_new(
            market_row(2, &Market::Us, TestSession::Open),
            market_row(3, &Market::Us, TestSession::Open),
            market_row(4, &Market::Us, TestSession::Open),
        )
        .expect("미국");
        let days = [ValuationCalendarDay::new(&kr, &us).expect("날짜")];
        let plan = ValuationGapPlan::new(date(2), at(3, 8), &days).expect("계획");
        let fixture_data = fixture_window(
            vec![trade("A", Currency::Krw, 1, "10", "100")],
            ("1000", "0"),
            ("900", "0"),
            SettlementWindow::try_new(date(1), date(3)).expect("기간"),
        );
        let histories = [history("A", Currency::Krw, 3, "12", vec![])];
        let results = plan.resolve(|day, cal| {
            assert_eq!(day, date(3));
            ValuationRecomputeInput::new(
                &cal,
                &fixture_data.rows,
                ValuationHistoryEvidence::new(&histories, &[]),
                DatedSnapshotLedgers::new(date(3), ledgers(&fixture_data)),
                observation(3, None),
            )
            .and_then(ValuationSnapshot::recompute)
        });
        assert_eq!(results[0].date(), date(3));
        assert_eq!(results[0].result(), Err(&SnapshotError::MissingPrice));
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(testkit::property_cases()))]
        #[test]
        fn removing_either_of_two_instruments_evidence_never_succeeds(missing in 0_usize..2) {
            let rows = vec![trade("A", Currency::Krw, 1, "10", "100"), trade("B", Currency::Usd, 2, "3", "90")];
            let histories = [history("A", Currency::Krw, 3, "12", vec![]), history("B", Currency::Usd, 3, "41", vec![])];
            let prices = [close("A", Currency::Krw, 3, "12"), close("B", Currency::Usd, 3, "41")];
            let good = snapshot(3, TestCash::new(rows.clone(), ("1000", "1000"), ("900", "910")), &histories, &prices, Some("1375")).expect("완전한 증거");
            prop_assert_eq!(good.valuation_at(), valuation_time(date(3)));
            prop_assert_eq!(good.stock_value(Currency::Krw), Ok(money("120", Currency::Krw)));
            prop_assert_eq!(good.stock_value(Currency::Usd), Ok(money("123", Currency::Usd)));
            let remaining = [prices[missing].clone()];
            prop_assert_eq!(snapshot(3, TestCash::new(rows, ("1000", "1000"), ("900", "910")), &histories, &remaining, Some("1375")).err(), Some(SnapshotError::MissingPrice));
        }
    }

    fn awaiting_calendar() -> SnapshotCalendar {
        let waiting = SessionData::try_new(
            TimeInterval::try_new(at(3, 6), at(3, 8)).expect("진행 중 장"),
            None,
        )
        .expect("장");
        let current = MarketSession::try_new(
            date(3),
            SessionInformation::Kr {
                pre_market: None,
                regular_market: Some(waiting),
                after_market: None,
            },
        )
        .expect("당일");
        let kr = MarketCalendar::try_new(
            market_row(2, &Market::Kr, TestSession::Open),
            current,
            market_row(4, &Market::Kr, TestSession::Open),
        )
        .expect("한국");
        let us = MarketCalendar::try_new(
            market_row(2, &Market::Us, TestSession::Open),
            market_row(3, &Market::Us, TestSession::Open),
            market_row(4, &Market::Us, TestSession::Open),
        )
        .expect("미국");
        SnapshotCalendar::new(&kr, &us).expect("달력")
    }

    #[test]
    fn incomplete_market_uses_previous_final_close_not_today_intraday() {
        let cal = awaiting_calendar();
        let fixture_data = fixture_window(
            vec![trade("A", Currency::Krw, 1, "10", "100")],
            ("1000", "0"),
            ("900", "0"),
            SettlementWindow::try_new(date(1), date(3)).expect("기간"),
        );
        let histories = [history("A", Currency::Krw, 2, "12", vec![])];
        let evidence = [(
            id("A"),
            ValuationPriceEvidence::AwaitingSession {
                price: price("12", Currency::Krw),
                date: date(2),
            },
        )];
        let input = ValuationRecomputeInput::new(
            &cal,
            &fixture_data.rows,
            ValuationHistoryEvidence::new(&histories, &evidence),
            DatedSnapshotLedgers::new(date(3), ledgers(&fixture_data)),
            observation(3, None),
        )
        .expect("직전 종가");
        let result = ValuationSnapshot::recompute(input).expect("평가");
        assert_eq!(
            result.stock_value(Currency::Krw),
            Ok(money("120", Currency::Krw))
        );
        assert_eq!(
            result.price_basis(Currency::Krw),
            crate::SnapshotPriceBasis::AwaitingSession
        );
        let bad = [close("A", Currency::Krw, 3, "12")];
        assert_eq!(
            (ValuationRecomputeInput::new(
                &cal,
                &fixture_data.rows,
                ValuationHistoryEvidence::new(&histories, &bad),
                DatedSnapshotLedgers::new(date(3), ledgers(&fixture_data)),
                observation(3, None)
            ))
            .err(),
            Some(SnapshotError::InvalidCandle)
        );
    }

    #[test]
    fn each_ledger_period_and_observation_time_is_checked_at_construction() {
        use crate::valuation_snapshot::tests::confirmed;
        let short = fixture_window(
            vec![],
            ("100", "0"),
            ("100", "0"),
            SettlementWindow::try_new(date(1), date(3)).expect("기간"),
        );
        let long = fixture_window(
            vec![],
            ("100", "0"),
            ("100", "0"),
            SettlementWindow::try_new(date(1), date(4)).expect("기간"),
        );
        let cal = calendar(3);
        let kr_late = crate::SnapshotLedgers::new(
            (&long.kr, confirmed(&long.kp)),
            (&short.us, confirmed(&short.up)),
        );
        let us_late = crate::SnapshotLedgers::new(
            (&short.kr, confirmed(&short.kp)),
            (&long.us, confirmed(&long.up)),
        );
        for ledgers in [kr_late, us_late] {
            assert_eq!(
                (ValuationRecomputeInput::new(
                    &cal,
                    &short.rows,
                    ValuationHistoryEvidence::new(&[], &[]),
                    DatedSnapshotLedgers::new(date(3), ledgers),
                    observation(3, None)
                ))
                .err(),
                Some(SnapshotError::OutsideConfirmedPeriod)
            );
        }
        let wrong_time = SnapshotObservation::new(at(3, 6), at(20, 0), None);
        let early_record = SnapshotObservation::new(at(3, 7), at(3, 6), None);
        for observation in [wrong_time, early_record] {
            assert_eq!(
                (ValuationRecomputeInput::new(
                    &cal,
                    &short.rows,
                    ValuationHistoryEvidence::new(&[], &[]),
                    DatedSnapshotLedgers::new(date(3), ledgers(&short)),
                    observation
                ))
                .err(),
                Some(SnapshotError::WrongPeriod)
            );
        }
    }

    #[test]
    fn constructor_rejects_missing_evidence_unknown_id_and_missing_fx() {
        let fixture_data = fixture_window(
            vec![trade("A", Currency::Usd, 1, "10", "100")],
            ("0", "1000"),
            ("0", "900"),
            SettlementWindow::try_new(date(1), date(3)).expect("기간"),
        );
        let cal = calendar(3);
        let histories = [history("A", Currency::Usd, 3, "12", vec![])];
        let evidence = [close("A", Currency::Usd, 3, "12")];
        let unknown = [close("B", Currency::Usd, 3, "12")];
        assert_eq!(
            (ValuationRecomputeInput::new(
                &cal,
                &fixture_data.rows,
                ValuationHistoryEvidence::new(&histories, &[]),
                DatedSnapshotLedgers::new(date(3), ledgers(&fixture_data)),
                observation(3, Some("1375"))
            ))
            .err(),
            Some(SnapshotError::MissingPrice)
        );
        assert_eq!(
            (ValuationRecomputeInput::new(
                &cal,
                &fixture_data.rows,
                ValuationHistoryEvidence::new(&histories, &unknown),
                DatedSnapshotLedgers::new(date(3), ledgers(&fixture_data)),
                observation(3, Some("1375"))
            ))
            .err(),
            Some(SnapshotError::MissingHistory)
        );
        assert_eq!(
            (ValuationRecomputeInput::new(
                &cal,
                &fixture_data.rows,
                ValuationHistoryEvidence::new(&histories, &evidence),
                DatedSnapshotLedgers::new(date(3), ledgers(&fixture_data)),
                observation(3, None)
            ))
            .err(),
            Some(SnapshotError::MissingFx)
        );
    }

    #[test]
    fn even_a_zero_position_cannot_label_an_incomplete_candle_as_a_close() {
        let value = price("12", Currency::Krw);
        let candle = Candle::try_new(
            id("A"),
            CandleTime::try_new(at(3, 0), CandlePeriod::Daily).expect("시간"),
            PriceAdjustment::Raw,
            Ohlcv::try_new(
                value,
                value,
                value,
                value,
                Quantity::parse(&NumericInput::try_new("0").expect("입력")).expect("수량"),
            )
            .expect("가격"),
            CandleObservation::new(at(3, 6), CandleStatus::InProgress),
        )
        .expect("장중 봉");
        let identity = LotIdentity::new(
            id("A"),
            Symbol::try_new("A").expect("심볼"),
            Market::Kr,
            Currency::Krw,
        );
        let histories =
            [SnapshotInstrumentHistory::new(identity, vec![], vec![candle]).expect("이력")];
        let evidence = [close("A", Currency::Krw, 3, "12")];
        assert_eq!(
            (snapshot(
                3,
                TestCash::new(vec![], ("100", "0"), ("100", "0")),
                &histories,
                &evidence,
                None
            ))
            .err(),
            Some(SnapshotError::InvalidCandle)
        );
    }

    #[test]
    fn zero_position_rejects_wrong_instrument_adjustment_and_period() {
        let value = price("12", Currency::Krw);
        let candle = |instrument: &str, period, adjustment| {
            Candle::try_new(
                id(instrument),
                CandleTime::try_new(at(3, 0), period).expect("시각"),
                adjustment,
                Ohlcv::try_new(
                    value,
                    value,
                    value,
                    value,
                    Quantity::parse(&NumericInput::try_new("0").expect("입력")).expect("수량"),
                )
                .expect("가격"),
                CandleObservation::new(at(3, 6), CandleStatus::Finalized),
            )
            .expect("봉")
        };
        let run = |item| {
            let identity = LotIdentity::new(
                id("A"),
                Symbol::try_new("A").expect("심볼"),
                Market::Kr,
                Currency::Krw,
            );
            let histories =
                [SnapshotInstrumentHistory::new(identity, vec![], vec![item]).expect("이력")];
            snapshot(
                3,
                TestCash::new(vec![], ("100", "0"), ("100", "0")),
                &histories,
                &[close("A", Currency::Krw, 3, "12")],
                None,
            )
        };
        assert_eq!(
            run(candle("A", CandlePeriod::Daily, PriceAdjustment::Raw))
                .expect("정상 봉")
                .account_value(Currency::Krw),
            Ok(money("100", Currency::Krw))
        );
        for invalid in [
            candle("B", CandlePeriod::Daily, PriceAdjustment::Raw),
            candle(
                "A",
                CandlePeriod::Daily,
                PriceAdjustment::Adjusted(crate::AdjustmentBasis::new(at(3, 0))),
            ),
            candle("A", CandlePeriod::Minute, PriceAdjustment::Raw),
        ] {
            assert_eq!(run(invalid).err(), Some(SnapshotError::InvalidCandle));
        }
    }

    fn zero_candle(
        instrument: &str,
        currency: Currency,
        period: CandlePeriod,
        status: CandleStatus,
        adjustment: PriceAdjustment,
    ) -> Candle {
        let value = price("12", currency);
        Candle::try_new(
            id(instrument),
            CandleTime::try_new(at(3, 0), period).expect("시각"),
            adjustment,
            Ohlcv::try_new(
                value,
                value,
                value,
                value,
                Quantity::parse(&NumericInput::try_new("0").expect("입력")).expect("수량"),
            )
            .expect("가격"),
            CandleObservation::new(at(3, 6), status),
        )
        .expect("봉")
    }

    fn zero_history(item: Option<Candle>) -> SnapshotInstrumentHistory {
        let identity = LotIdentity::new(
            id("A"),
            Symbol::try_new("A").expect("심볼"),
            Market::Kr,
            Currency::Krw,
        );
        SnapshotInstrumentHistory::new(identity, vec![], item.into_iter().collect()).expect("이력")
    }

    fn zero_without_evidence(item: Option<Candle>) -> Result<ValuationSnapshot, SnapshotError> {
        let histories = [zero_history(item)];
        snapshot(
            3,
            TestCash::new(vec![], ("100", "0"), ("100", "0")),
            &histories,
            &[],
            None,
        )
    }

    #[test]
    fn zero_without_evidence_accepts_absence_and_rejects_wrong_identity_or_currency() {
        let absent = zero_without_evidence(None).expect("봉 없는 0주");
        assert!(absent.positions()[0].candle().is_none());
        assert_eq!(
            absent.account_value(Currency::Krw),
            Ok(money("100", Currency::Krw))
        );
        let valid = zero_without_evidence(Some(zero_candle(
            "A",
            Currency::Krw,
            CandlePeriod::Daily,
            CandleStatus::Finalized,
            PriceAdjustment::Raw,
        )))
        .expect("정상 봉");
        assert!(valid.positions()[0].candle().is_some());
        for invalid in [
            zero_candle(
                "B",
                Currency::Krw,
                CandlePeriod::Daily,
                CandleStatus::Finalized,
                PriceAdjustment::Raw,
            ),
            zero_candle(
                "A",
                Currency::Usd,
                CandlePeriod::Daily,
                CandleStatus::Finalized,
                PriceAdjustment::Raw,
            ),
        ] {
            assert_eq!(
                zero_without_evidence(Some(invalid)).err(),
                Some(SnapshotError::InvalidCandle)
            );
        }
    }

    #[test]
    fn zero_without_evidence_rejects_unfinished_nondaily_or_adjusted_candles() {
        for invalid in [
            zero_candle(
                "A",
                Currency::Krw,
                CandlePeriod::Daily,
                CandleStatus::InProgress,
                PriceAdjustment::Raw,
            ),
            zero_candle(
                "A",
                Currency::Krw,
                CandlePeriod::Minute,
                CandleStatus::Finalized,
                PriceAdjustment::Raw,
            ),
            zero_candle(
                "A",
                Currency::Krw,
                CandlePeriod::Daily,
                CandleStatus::Finalized,
                PriceAdjustment::Adjusted(crate::AdjustmentBasis::new(at(3, 0))),
            ),
        ] {
            assert_eq!(
                zero_without_evidence(Some(invalid)).err(),
                Some(SnapshotError::InvalidCandle)
            );
        }
    }

    #[test]
    fn equivalent_instant_with_previous_local_date_is_canonicalized() {
        let canonical = valuation_time(date(3));
        let original = Timestamp::from_parts(
            crate::UnixSeconds::new(*canonical.utc_seconds().as_i64()),
            crate::Nanosecond::new(0),
            crate::OffsetSeconds::new(-28_800),
        )
        .expect("오프셋");
        assert_eq!(original, canonical);
        assert_eq!(original.offset_calendar_date(), date(2));
        let data = fixture_window(vec![], ("100", "0"), ("100", "0"), test_window(3));
        let cal = calendar(3);
        let input = ValuationRecomputeInput::new(
            &cal,
            &data.rows,
            ValuationHistoryEvidence::new(&[], &[]),
            DatedSnapshotLedgers::new(date(3), ledgers(&data)),
            SnapshotObservation::new(original, at(20, 0), None),
        )
        .expect("같은 시각");
        let result = ValuationSnapshot::recompute(input).expect("평가");
        assert_eq!(*result.valuation_at().offset_seconds().as_i32(), 0_i32);
        assert_eq!(result.valuation_at().offset_calendar_date(), date(3));
    }

    #[test]
    fn us_market_date_can_differ_from_kst_valuation_date() {
        let kr = MarketCalendar::try_new(
            market_row(2, &Market::Kr, TestSession::Open),
            market_row(3, &Market::Kr, TestSession::Open),
            market_row(4, &Market::Kr, TestSession::Open),
        )
        .expect("한국");
        let us = MarketCalendar::try_new(
            market_row(1, &Market::Us, TestSession::Open),
            market_row(2, &Market::Us, TestSession::Open),
            market_row(3, &Market::Us, TestSession::Open),
        )
        .expect("미국");
        let cal = SnapshotCalendar::new(&kr, &us).expect("달력");
        let fixture_data = fixture_window(
            vec![trade("A", Currency::Usd, 3, "10", "100")],
            ("0", "1000"),
            ("0", "900"),
            SettlementWindow::try_new(date(1), date(3)).expect("기간"),
        );
        let histories = [history("A", Currency::Usd, 2, "10", vec![])];
        let evidence = [close("A", Currency::Usd, 2, "10")];
        let input = ValuationRecomputeInput::new(
            &cal,
            &fixture_data.rows,
            ValuationHistoryEvidence::new(&histories, &evidence),
            DatedSnapshotLedgers::new(date(3), ledgers(&fixture_data)),
            observation(3, Some("1375")),
        )
        .expect("입력");
        let result = ValuationSnapshot::recompute(input).expect("평가");
        assert_eq!(result.settlement_date(), date(3));
        assert_eq!(result.price_date(Currency::Usd), date(2));
        assert_eq!(
            result.account_value(Currency::Usd),
            Ok(money("1000", Currency::Usd))
        );
        assert_eq!(result.valuation_at().offset_calendar_date(), date(3));
    }

    #[test]
    fn zero_position_cannot_attach_evidence_without_a_candle() {
        let identity = LotIdentity::new(
            id("A"),
            Symbol::try_new("A").expect("심볼"),
            Market::Kr,
            Currency::Krw,
        );
        let histories = [SnapshotInstrumentHistory::new(identity, vec![], vec![]).expect("이력")];
        let evidence = [close("A", Currency::Krw, 3, "12")];
        let cash = TestCash::new(vec![], ("100", "0"), ("100", "0"));
        assert_eq!(
            snapshot(3, cash, &histories, &evidence, None).err(),
            Some(SnapshotError::InvalidCandle)
        );
    }
    #[test]
    fn tiny_nonzero_quantity_and_price_cannot_turn_precision_loss_into_zero() {
        let tiny = "0.0000000000000000000000000001";
        let cash = TestCash::new(
            vec![trade("A", Currency::Krw, 1, tiny, "0")],
            ("100", "0"),
            ("100", "0"),
        );
        let histories = [history("A", Currency::Krw, 3, tiny, vec![])];
        let evidence = [close("A", Currency::Krw, 3, tiny)];
        assert_eq!(
            snapshot(3, cash, &histories, &evidence, None).err(),
            Some(SnapshotError::Arithmetic(crate::MoneyError::PrecisionLoss))
        );
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(testkit::property_cases()))]
        #[test]
        fn valuation_time_covers_the_supported_gregorian_range(year in 1_i32..10_000_i32, month in 1_u8..13_u8, day in 1_u8..29_u8) {
            let date = CalendarDate::from_parts(crate::CalendarYear::new(year), crate::MonthNumber::new(month), crate::DayNumber::new(day)).expect("날짜");
            let instant = valuation_time(date);
            prop_assert_eq!(instant.date_at_offset(crate::OffsetSeconds::new(32_400)), Ok(date));
            prop_assert_eq!(*instant.offset_seconds().as_i32(), 0_i32);
            prop_assert_eq!(instant.utc_seconds().as_i64().checked_rem_euclid(86_400), Some(25_200_i64));
        }
    }
    #[test]
    fn offsetting_usd_cash_does_not_hide_missing_stock_fx() {
        let rows = vec![
            trade("A", Currency::Usd, 1, "10", "100"),
            tx(TransactionKind::Deposit, Currency::Usd, 3, 8, "120"),
        ];
        let fixture_data = fixture_window(rows, ("0", "-20"), ("0", "0"), test_window(3));
        let cal = calendar(3);
        let histories = [history("A", Currency::Usd, 3, "12", vec![])];
        let evidence = [close("A", Currency::Usd, 3, "12")];
        let source = ValuationSnapshot::recalculate(
            &cal,
            &fixture_data.rows,
            &histories,
            Some(ledgers(&fixture_data)),
            observation(3, None),
        )
        .expect("기존 계산");
        assert_eq!(
            source.stock_value(Currency::Usd),
            Ok(money("120", Currency::Usd))
        );
        assert_eq!(
            source.account_value(Currency::Usd),
            Ok(money("0", Currency::Usd))
        );
        assert_eq!(
            source.stock_krw_value().err(),
            Some(SnapshotError::MissingFx)
        );
        assert_eq!(
            source
                .account_krw_value()
                .expect("계좌 0")
                .clone()
                .into_parts()
                .0,
            money("0", Currency::Krw)
        );
        let input = ValuationRecomputeInput::new(
            &cal,
            &fixture_data.rows,
            ValuationHistoryEvidence::new(&histories, &evidence),
            DatedSnapshotLedgers::new(date(3), ledgers(&fixture_data)),
            observation(3, None),
        );
        assert_eq!(input.err(), Some(SnapshotError::MissingFx));
    }
    #[test]
    fn unreconciled_cash_cannot_form_an_input() {
        use crate::valuation_snapshot::tests::confirmed;
        let good = fixture_window(vec![], ("100", "0"), ("100", "0"), test_window(3));
        let bad = fixture_window(vec![], ("100", "0"), ("101", "0"), test_window(3));
        let pair = crate::SnapshotLedgers::new(
            (&bad.kr, confirmed(&good.kp)),
            (&good.us, confirmed(&good.up)),
        );
        let cal = calendar(3);
        let input = ValuationRecomputeInput::new(
            &cal,
            &good.rows,
            ValuationHistoryEvidence::new(&[], &[]),
            DatedSnapshotLedgers::new(date(3), pair),
            observation(3, None),
        );
        assert_eq!(
            input.err(),
            Some(SnapshotError::Cash(
                crate::CashUnavailable::ReportedDifference
            ))
        );
    }
}
