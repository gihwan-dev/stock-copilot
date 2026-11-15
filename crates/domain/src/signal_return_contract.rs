#[cfg(test)]
pub(crate) mod tests {
    use crate::candle::tests::{at, price};
    use crate::{
        AdjustmentBasis, BootstrapSeed, Candle, CandleObservation, CandlePeriod, CandleStatus,
        CandleTime, Currency, EstimateTask, Horizon, HorizonSelection, MarketCalendar,
        MarketSession, NumericInput, OffsetSeconds, Ohlcv, OutcomeAdjustment, Quantity,
        SessionData, SessionInformation, TimeInterval, Timestamp, VirtualExit,
    };
    pub(crate) fn instant(day: i64, minute: i64) -> Timestamp {
        Timestamp::from_parts(
            crate::UnixSeconds::new(
                at(day)
                    .utc_seconds()
                    .as_i64()
                    .checked_add(minute.checked_mul(60).expect("분"))
                    .expect("시각"),
            ),
            crate::Nanosecond::new(0),
            OffsetSeconds::new(32400),
        )
        .expect("현지 시각")
    }
    pub(crate) fn row(day: i64) -> MarketSession {
        MarketSession::try_new(
            instant(day, 0).offset_calendar_date(),
            SessionInformation::Kr {
                pre_market: None,
                regular_market: Some(
                    SessionData::try_new(
                        TimeInterval::try_new(instant(day, 0), instant(day, 4)).expect("장"),
                        None,
                    )
                    .expect("장"),
                ),
                after_market: None,
            },
        )
        .expect("거래일")
    }
    pub(crate) fn calendars() -> Vec<MarketCalendar> {
        (61_i64..=66)
            .map(|day| {
                MarketCalendar::try_new(
                    row(day.checked_sub(1).expect("전")),
                    row(day),
                    row(day.checked_add(1).expect("후")),
                )
                .expect("달력")
            })
            .collect()
    }
    pub(crate) fn exit_summary(
        outcome: &VirtualExit,
    ) -> (&'static str, Option<&crate::VirtualExitFill>) {
        match outcome {
            VirtualExit::Target(value) => ("target", Some(value.as_fill())),
            VirtualExit::Stop(value) => ("stop", Some(value.as_fill())),
            VirtualExit::Maturity(value) => ("maturity", Some(value.as_fill())),
            VirtualExit::Ongoing => ("ongoing", None),
            VirtualExit::Unavailable(_reason) => ("unavailable", None),
        }
    }
    fn virtual_exit(
        task: &EstimateTask,
        calendars: &[MarketCalendar],
        adjustment: &OutcomeAdjustment,
        candles: &[Candle],
        now: Timestamp,
    ) -> VirtualExit {
        let signal = crate::virtual_exit::tests::signal_fixture(
            task.clone(),
            price("110", Currency::Krw),
            price("90", Currency::Krw),
            instant(66, 4),
        )
        .expect("기록된 목표와 손절");
        crate::virtual_exit(&signal, calendars, adjustment, candles, now)
    }
    pub(crate) fn task() -> EstimateTask {
        crate::thresholds::tests::task_for(Horizon::new(HorizonSelection::FewDays))
    }
    pub(crate) fn adjustment() -> OutcomeAdjustment {
        OutcomeAdjustment::from_actions(&task(), AdjustmentBasis::new(instant(66, 4)), &[])
            .expect("수정 기준")
    }
    pub(crate) fn candle(day: i64, minute: i64, period: CandlePeriod, values: [&str; 4]) -> Candle {
        let [open, high, low, close] = values;
        let when = if period == CandlePeriod::Daily {
            Timestamp::from_parts(
                instant(day, 0).utc_seconds(),
                crate::Nanosecond::new(0),
                OffsetSeconds::new(32400),
            )
            .expect("일봉 시각")
        } else {
            instant(day, minute)
        };
        let stamp = if period == CandlePeriod::Daily {
            when.checked_sub(
                crate::TimeSpan::try_new(crate::SpanSeconds::new(32400), crate::Nanosecond::new(0))
                    .expect("9시"),
            )
            .expect("자정")
        } else {
            when
        };
        Candle::try_new(
            task().volatility().instrument().clone(),
            CandleTime::try_new(stamp, period).expect("봉 시각"),
            crate::PriceAdjustment::Adjusted(adjustment().basis()),
            Ohlcv::try_new(
                price(open, Currency::Krw),
                price(high, Currency::Krw),
                price(low, Currency::Krw),
                price(close, Currency::Krw),
                Quantity::parse(&NumericInput::try_new("1").expect("입력")).expect("수량"),
            )
            .expect("OHLC"),
            CandleObservation::new(instant(66, 4), CandleStatus::Finalized),
        )
        .expect("봉")
    }
    pub(crate) fn entry_bars() -> Vec<Candle> {
        (1_i64..=4)
            .map(|minute| candle(61, minute, CandlePeriod::Minute, ["100"; 4]))
            .collect()
    }
    #[test]
    fn d29_entry_session_close_is_not_task_maturity() {
        let result = virtual_exit(
            &task(),
            &calendars(),
            &adjustment(),
            &entry_bars(),
            instant(62, 0),
        );
        assert_eq!(result, VirtualExit::Ongoing);
        assert_eq!(exit_summary(&result), ("ongoing", None));
        assert_eq!(result.into_fill(), None);
        assert_eq!(format!("{:?}", BootstrapSeed::new(42)), "BootstrapSeed");
    }

    #[test]
    fn d29_exit_examples_cover_intraday_gaps_maturity_and_missing_data() {
        for (values, expected, exit_price, minute) in [
            (["100", "200", "100", "100"], "target", "110", 4_i64),
            (["100", "100", "1", "100"], "stop", "90", 4_i64),
            (["200", "210", "190", "200"], "target", "200", 0_i64),
            (["1", "2", "1", "1"], "stop", "1", 0_i64),
        ] {
            let mut bars = entry_bars();
            bars.push(candle(62, 0, CandlePeriod::Daily, values));
            let result = virtual_exit(&task(), &calendars(), &adjustment(), &bars, instant(66, 4));
            assert_eq!(exit_summary(&result).0, expected);
            let fill = result.into_fill().expect("닫힌 청산");
            assert_eq!(fill.price(), price(exit_price, Currency::Krw));
            let moment = if minute == 0 {
                crate::VirtualExitMoment::At(instant(62, minute))
            } else {
                crate::VirtualExitMoment::TradingDay(row(62).date())
            };
            assert_eq!(fill.at(), moment);
            assert_eq!(format!("{fill:?}"), "VirtualExitFill");
        }
        let mut flat = entry_bars();
        flat.extend((62_i64..=66).map(|day| candle(day, 0, CandlePeriod::Daily, ["100"; 4])));
        let result = virtual_exit(&task(), &calendars(), &adjustment(), &flat, instant(66, 4));
        assert_eq!(exit_summary(&result).0, "maturity");
        let fill = result.into_fill().expect("만기 청산");
        assert_eq!(fill.price(), price("100", Currency::Krw));
        assert_eq!(fill.at(), crate::VirtualExitMoment::At(instant(66, 4)));
        let missing = virtual_exit(&task(), &calendars(), &adjustment(), &[], instant(66, 4));
        assert_eq!(
            missing,
            VirtualExit::Unavailable(crate::OutcomeUnavailable::MissingCandles)
        );
        assert_eq!(exit_summary(&missing), ("unavailable", None));
        assert_eq!(missing.into_fill(), None);
    }
    #[test]
    fn review_zero_volume_is_rejected_in_an_otherwise_complete_maturity_path() {
        let mut flat = entry_bars();
        flat.extend((62_i64..=66).map(|day| candle(day, 0, CandlePeriod::Daily, ["100"; 4])));
        assert_eq!(
            exit_summary(&virtual_exit(
                &task(),
                &calendars(),
                &adjustment(),
                &flat,
                instant(66, 4)
            ))
            .0,
            "maturity"
        );
        let last = flat.pop().expect("만기 봉");
        let values = last.values();
        flat.push(
            Candle::try_new(
                last.instrument().clone(),
                last.time(),
                last.adjustment(),
                Ohlcv::try_new(
                    values.open(),
                    values.high(),
                    values.low(),
                    values.close(),
                    Quantity::parse(&NumericInput::try_new("0").expect("영 거래량")).expect("수량"),
                )
                .expect("OHLC"),
                CandleObservation::new(last.collected_at(), last.status()),
            )
            .expect("무거래 봉"),
        );
        assert_eq!(
            virtual_exit(&task(), &calendars(), &adjustment(), &flat, instant(66, 4)),
            VirtualExit::Unavailable(crate::OutcomeUnavailable::MissingCandles)
        );
        assert_eq!(
            virtual_exit(
                &task(),
                &calendars(),
                &adjustment(),
                &entry_bars(),
                instant(62, 0)
            ),
            VirtualExit::Ongoing
        );
    }
    pub(crate) fn benchmark() -> crate::SignalBenchmark {
        crate::signal_return_scenarios::tests::benchmark(
            0,
            "100",
            AdjustmentBasis::new(instant(67, 4)),
        )
    }
    pub(crate) fn cost(commission: &str, buy: &str, sell: &str) -> crate::SignalReturnCost {
        let rate = |text: &str| {
            crate::CostRate::try_new(
                crate::Ratio::parse(&NumericInput::try_new(text).expect("요율")).expect("비율"),
            )
            .expect("요율 범위")
        };
        crate::signal_return_cost(
            Some(crate::CommissionRateFact::from_commissions_result(
                crate::Market::Kr,
                rate(commission),
                crate::CommissionValidity::try_new(None, None).expect("유효 기간"),
            )),
            Some(crate::TradeTaxFact::from_schedule_result(
                crate::Market::Kr,
                rate(buy),
                rate(sell),
            )),
            crate::EstimateCostPeriod::from_calendar_result(&task(), instant(66, 4)).expect("만기"),
        )
        .expect("비용")
    }
    #[test]
    fn d29_r11_cost_example_and_daily_close_benchmark_preserve_net_return() {
        let source = task();
        let fill = crate::virtual_exit::tests::virtual_exit_fixture_fill(
            &source,
            price("100", Currency::Krw),
            instant(66, 4),
        );
        let rates = cost("0.00015", "0", "0.002");
        let result = crate::signal_return(&source, fill, &rates, &benchmark()).expect("순수익률");
        let expected = crate::bootstrap::tests::reference_ratio(7_184_520, 7_201_080)
            .checked_sub(rust_decimal::Decimal::ONE)
            .expect("순수익률");
        assert_eq!(result.net().number().as_decimal(), expected);
        let comparison = result.excess().expect("종가 근사");
        assert_eq!(comparison.value(), result.net());
        assert_eq!(
            comparison.alignment(),
            crate::BenchmarkAlignment::DailyCloseApproximation
        );
        assert_eq!(result.currency(), Currency::Krw);
        assert_eq!(result.entry_day(), instant(61, 0).offset_calendar_date());
        assert_eq!(result.exit_day(), instant(66, 4).offset_calendar_date());
        assert_eq!(format!("{rates:?}"), "SignalReturnCost");
        assert_eq!(
            crate::signal_return(&source, VirtualExit::Ongoing, &rates, &benchmark()),
            Err(crate::SignalReturnError::ExitUnavailable)
        );
        assert_eq!(
            crate::signal_return(
                &source,
                VirtualExit::Unavailable(crate::OutcomeUnavailable::MissingCandles),
                &rates,
                &benchmark()
            ),
            Err(crate::SignalReturnError::ExitUnavailable)
        );
    }
    #[test]
    fn d29_zero_cost_matches_independent_gross_return_for_4096_cases() {
        use proptest::test_runner::{Config, TestRunner};
        let source = task();
        let rates = cost("0", "0", "0");
        let index = benchmark();
        TestRunner::new(Config::with_cases(4096))
            .run(&(1_u32..1_000_000), |value| {
                let fill = crate::virtual_exit::tests::virtual_exit_fixture_fill(
                    &source,
                    price(&value.to_string(), Currency::Krw),
                    instant(66, 4),
                );
                let result = crate::signal_return(&source, fill, &rates, &index).expect("순수익률");
                let numerator = i64::from(value).checked_sub(100).expect("독립 손익");
                let expected = crate::bootstrap::tests::reference_ratio(numerator, 100);
                proptest::prop_assert_eq!(result.net().number().as_decimal(), expected);
                Ok(())
            })
            .expect("4096건");
    }
    #[test]
    fn d29_split_adjusts_entry_and_thresholds_without_false_loss() {
        let source = task();
        let action = crate::CorporateAction::try_new(
            crate::CorporateActionId::new(crate::InstrumentId::try_new("split").expect("행사")),
            row(62).date(),
            crate::CorporateActionKind::Split(crate::ShareRatio::new(
                crate::ShareCount::try_new(1).expect("구주"),
                crate::ShareCount::try_new(2).expect("신주"),
            )),
        )
        .expect("분할");
        let adjusted = OutcomeAdjustment::from_actions(
            &source,
            adjustment().basis(),
            &[crate::OutcomeCorporateAction::new(
                source.volatility().instrument().clone(),
                action,
            )],
        )
        .expect("환산 근거");
        let mut bars = (1_i64..=4)
            .map(|minute| candle(61, minute, CandlePeriod::Minute, ["50"; 4]))
            .collect::<Vec<_>>();
        bars.extend((62_i64..=66).map(|day| candle(day, 0, CandlePeriod::Daily, ["50"; 4])));
        let exit = virtual_exit(&source, &calendars(), &adjusted, &bars, instant(66, 4));
        assert_eq!(exit_summary(&exit).0, "maturity");
        let fill = exit_summary(&exit).1.expect("만기 청산");
        assert_eq!(fill.price(), price("50", Currency::Krw));
        let result = crate::signal_return(&source, exit, &cost("0", "0", "0"), &benchmark())
            .expect("분할 수익률");
        assert_eq!(result.net().to_string(), "0.000000");
    }
    pub(crate) fn key() -> crate::CalibrationKey {
        let part = |value: &str| crate::ModelIdentityPart::try_new(value).expect("모델");
        crate::CalibrationKey::new(
            crate::CalibrationModel::new(part("provider"), part("model"), part("version")),
            Horizon::new(HorizonSelection::FewDays),
        )
    }
    pub(crate) fn observation(outcome: crate::EstimateOutcome) -> crate::ScorecardObservation {
        crate::ScorecardObservation::try_new(
            crate::CalibrationEstimateId::try_new("one").expect("추정 ID"),
            key(),
            crate::ScorecardPromptId::try_new("prompt").expect("프롬프트"),
            crate::AiProbability::parse(
                &crate::AiProbabilityInput::try_new("60").expect("AI 입력"),
            )
            .expect("확률"),
            outcome,
        )
        .expect("성적표 입력")
    }
    #[test]
    fn d29_scorecard_reuses_maturity_exclusions_and_preserves_comparisons() {
        let source = task();
        let mut bars = entry_bars();
        bars.push(candle(
            62,
            0,
            CandlePeriod::Daily,
            ["200", "210", "190", "200"],
        ));
        bars.extend((1_i64..=4).map(|minute| candle(62, minute, CandlePeriod::Minute, ["200"; 4])));
        let outcome = crate::EstimateOutcome::evaluate(
            &source,
            &calendars(),
            &adjustment(),
            &bars,
            instant(62, 4),
        );
        let input = observation(outcome);
        let before = crate::scorecard_edge(
            &key(),
            std::slice::from_ref(&input),
            &[],
            instant(65, 4),
            BootstrapSeed::new(1),
        )
        .expect("만기 전");
        assert_eq!(
            before.confidence(),
            &crate::BootstrapInterval::InsufficientSamples
        );
        assert_eq!(before.unavailable().as_usize(), 0);
        assert_eq!(before.scorecards().comparison_count().as_usize(), 1);
        let card = before.scorecards().select(&key()).expect("조합");
        assert_eq!(card.counts().before_maturity().as_usize(), 1);
        let after =
            crate::scorecard_edge(&key(), &[input], &[], instant(66, 4), BootstrapSeed::new(1))
                .expect("만기 후 자료 부족");
        assert_eq!(after.unavailable().as_usize(), 1);
        assert_eq!(
            after
                .scorecards()
                .select(&key())
                .expect("조합")
                .counts()
                .judged()
                .as_usize(),
            1
        );
        assert_eq!(
            crate::scorecard_edge(&key(), &[], &[], instant(66, 4), BootstrapSeed::new(1)),
            Err(crate::SignalReturnError::Scorecard(
                crate::ScorecardError::EmptyInput
            ))
        );
    }
    #[test]
    fn d29_ongoing_observations_are_not_return_samples() {
        let source = task();
        let ongoing = observation(crate::EstimateOutcome::evaluate(
            &source,
            &calendars(),
            &adjustment(),
            &entry_bars(),
            instant(61, 2),
        ));
        let pending = crate::scorecard_edge(
            &key(),
            &[ongoing],
            &[],
            instant(66, 4),
            BootstrapSeed::new(1),
        )
        .expect("진행 중 제외");
        assert_eq!(pending.unavailable().as_usize(), 0);
        assert_eq!(
            pending
                .scorecards()
                .select(&key())
                .expect("조합")
                .counts()
                .ongoing()
                .as_usize(),
            1
        );
    }
}
