#[cfg(test)]
mod tests {
    use crate::candle::tests::price;
    use crate::signal_return_contract::tests::{
        adjustment, benchmark, calendars, candle, cost, entry_bars, exit_summary, instant, key,
        observation, task,
    };
    use crate::{
        AdjustmentBasis, BootstrapSeed, CandlePeriod, Currency, NumericInput, OutcomeUnavailable,
        VirtualExit, VirtualExitSignal, VirtualExitSignalError,
    };

    fn signal() -> VirtualExitSignal {
        VirtualExitSignal::try_new(
            task(),
            price("110", Currency::Krw),
            price("90", Currency::Krw),
            instant(66, 4),
        )
        .expect("신호")
    }
    #[test]
    fn d29_recorded_entry_is_not_the_forecast_reference_and_has_explicit_maturity() {
        let source = task();
        let entry = crate::PriceObservation::from_identified_quote_parts(
            source.volatility().instrument().clone(),
            source.reference().instrument().clone(),
            price("102", Currency::Krw),
            instant(61, 1),
        )
        .expect("실제 진입");
        let recorded = VirtualExitSignal::from_entry(
            source.clone(),
            entry.clone(),
            price("110", Currency::Krw),
            price("90", Currency::Krw),
            instant(66, 4),
        )
        .expect("신호");
        assert_eq!(recorded.entry(), &entry);
        assert_eq!(recorded.maturity(), instant(66, 4));
        let adjusted = crate::virtual_exit_adjustment(&recorded, adjustment().basis(), &[])
            .expect("진입 기준");
        assert_eq!(adjusted.reference_at(), instant(61, 1));
        let mut bars = entry_bars();
        bars[0] = candle(61, 1, CandlePeriod::Minute, ["200"; 4]);
        bars.extend((62_i64..=66).map(|day| candle(day, 0, CandlePeriod::Daily, ["100"; 4])));
        let exit = crate::virtual_exit(&recorded, &calendars(), &adjusted, &bars, instant(66, 4));
        assert_eq!(exit_summary(&exit).0, "maturity");
        let value = crate::signal_return(&source, exit, &cost("0", "0", "0"), &benchmark())
            .expect("수익률");
        let expected = crate::bootstrap::tests::reference_ratio(-2, 102);
        assert_eq!(value.net().number().as_decimal(), expected);
        assert_eq!(value.instrument(), source.volatility().instrument());
        assert_eq!(value.entry_at(), instant(61, 1));
        assert_eq!(value.entry_day(), instant(61, 1).offset_calendar_date());
        assert_eq!(value.cluster_day(), instant(61, 1).offset_calendar_date());
        assert_eq!(value.exit_day(), instant(66, 4).offset_calendar_date());
        assert_eq!(value.maturity(), instant(66, 4));
        assert_eq!(value.exit_at(), instant(66, 4));
        assert_eq!(value.currency(), Currency::Krw);
        assert_eq!(
            crate::virtual_exit(
                &recorded,
                &calendars(),
                &adjustment(),
                &bars,
                instant(66, 4)
            ),
            VirtualExit::Unavailable(OutcomeUnavailable::Adjustment)
        );
    }
    #[test]
    fn d29_signal_prices_and_entry_identity_reject_invalid_boundaries() {
        let source = task();
        for (target, stop, expected) in [
            (
                price("110", Currency::Usd),
                price("90", Currency::Krw),
                VirtualExitSignalError::CurrencyMismatch,
            ),
            (
                price("100", Currency::Krw),
                price("90", Currency::Krw),
                VirtualExitSignalError::InvalidBounds,
            ),
            (
                price("110", Currency::Krw),
                price("100", Currency::Krw),
                VirtualExitSignalError::InvalidBounds,
            ),
        ] {
            assert_eq!(
                VirtualExitSignal::try_new(source.clone(), target, stop, instant(66, 4)),
                Err(expected)
            );
        }
        let entry = crate::PriceObservation::from_quote_parts(
            source.reference().instrument().clone(),
            price("100", Currency::Krw),
            instant(61, 1),
        )
        .expect("미식별 관측");
        assert_eq!(
            VirtualExitSignal::from_entry(
                source.clone(),
                entry,
                price("110", Currency::Krw),
                price("90", Currency::Krw),
                instant(66, 4)
            ),
            Err(VirtualExitSignalError::InvalidEntry)
        );
        assert_eq!(
            VirtualExitSignal::try_new(
                source,
                price("110", Currency::Krw),
                price("90", Currency::Krw),
                instant(61, 0)
            ),
            Err(VirtualExitSignalError::InvalidEntry)
        );
        let wrong = VirtualExitSignal::try_new(
            task(),
            price("110", Currency::Krw),
            price("90", Currency::Krw),
            instant(65, 4),
        )
        .expect("만기 주장");
        assert_eq!(
            crate::virtual_exit(&wrong, &calendars(), &adjustment(), &[], instant(66, 4)),
            VirtualExit::Unavailable(OutcomeUnavailable::Calendar)
        );
    }
    #[test]
    fn d29_cost_facts_must_be_complete_valid_and_within_combined_rate_bounds() {
        let rate = |text: &str| {
            crate::CostRate::try_new(
                crate::Ratio::parse(&NumericInput::try_new(text).expect("입력")).expect("비율"),
            )
            .expect("요율")
        };
        let period =
            crate::EstimateCostPeriod::from_calendar_result(&task(), instant(66, 4)).expect("기간");
        let commission = crate::CommissionRateFact::from_commissions_result(
            crate::Market::Kr,
            rate("0.6"),
            crate::CommissionValidity::try_new(None, None).expect("유효"),
        );
        let tax =
            crate::TradeTaxFact::from_schedule_result(crate::Market::Kr, rate("0.5"), rate("0.5"));
        assert_eq!(
            crate::signal_return_cost(None, Some(tax.clone()), period.clone()),
            Err(crate::TradeCostError::UnknownComponent)
        );
        assert_eq!(
            crate::signal_return_cost(Some(commission.clone()), None, period.clone()),
            Err(crate::TradeCostError::UnknownComponent)
        );
        assert_eq!(
            crate::signal_return_cost(Some(commission.clone()), Some(tax), period.clone()),
            Err(crate::TradeCostError::InvalidRate)
        );
        let wrong =
            crate::TradeTaxFact::from_schedule_result(crate::Market::Us, rate("0"), rate("0"));
        assert_eq!(
            crate::signal_return_cost(Some(commission), Some(wrong), period),
            Err(crate::TradeCostError::MarketMismatch)
        );
        assert!(!format!("{:?}", cost("0.00015", "0", "0.002")).contains("0.00015"));
    }
    #[test]
    fn d29_missing_zero_volume_and_invalid_adjustment_cannot_produce_prices() {
        let recorded = signal();
        assert_eq!(
            crate::virtual_exit(&recorded, &[], &adjustment(), &[], instant(66, 4)),
            VirtualExit::Unavailable(OutcomeUnavailable::Calendar)
        );
        let first = candle(61, 1, CandlePeriod::Minute, ["100"; 4]);
        let zero = crate::Candle::try_new(
            first.instrument().clone(),
            first.time(),
            first.adjustment(),
            crate::Ohlcv::try_new(
                first.values().open(),
                first.values().high(),
                first.values().low(),
                first.values().close(),
                crate::Quantity::parse(&NumericInput::try_new("0").expect("입력")).expect("수량"),
            )
            .expect("OHLC"),
            crate::CandleObservation::new(first.collected_at(), first.status()),
        )
        .expect("봉");
        assert_eq!(
            crate::virtual_exit(
                &recorded,
                &calendars(),
                &adjustment(),
                &[zero],
                instant(66, 4)
            ),
            VirtualExit::Unavailable(OutcomeUnavailable::MissingCandles)
        );
        assert_eq!(
            crate::virtual_exit_adjustment(&recorded, AdjustmentBasis::new(instant(60, 0)), &[]),
            Err(OutcomeUnavailable::Adjustment)
        );
        let utc = crate::Timestamp::from_parts(
            instant(66, 4).utc_seconds(),
            crate::Nanosecond::new(0),
            crate::OffsetSeconds::new(0),
        )
        .expect("UTC");
        assert_eq!(
            crate::virtual_exit_adjustment(&recorded, AdjustmentBasis::new(utc), &[]),
            Err(OutcomeUnavailable::Adjustment)
        );
    }
    #[test]
    fn d29_intraday_benchmark_is_unavailable_and_bindings_follow_original_observation() {
        let source = task();
        let mut bars = entry_bars();
        bars.push(candle(62, 0, CandlePeriod::Daily, ["200"; 4]));
        bars.extend((1_i64..=4).map(|minute| candle(62, minute, CandlePeriod::Minute, ["200"; 4])));
        let outcome = crate::EstimateOutcome::evaluate(
            &source,
            &calendars(),
            &adjustment(),
            &bars,
            instant(66, 4),
        );
        let record = observation(outcome);
        let exit = crate::virtual_exit(
            &signal(),
            &calendars(),
            &adjustment(),
            &bars,
            instant(66, 4),
        );
        let value = crate::signal_return(&source, exit, &cost("0", "0", "0"), &benchmark())
            .expect("순수익률");
        assert_eq!(
            value.excess(),
            Err(crate::SignalReturnError::BenchmarkMismatch)
        );
        assert_eq!(value.entry_at(), instant(61, 0));
        assert_eq!(value.exit_at(), instant(62, 0));
        assert_eq!(value.maturity(), instant(66, 4));
        let bound =
            crate::ScorecardReturnObservation::try_new(record.clone(), value).expect("원본 결합");
        let edge = crate::scorecard_edge(
            &key(),
            std::slice::from_ref(&record),
            &[bound.clone(), bound],
            instant(66, 4),
            BootstrapSeed::new(1),
        )
        .expect("중복 동일 기록");
        assert_eq!(edge.unavailable().as_usize(), 1);
        let ongoing = observation(crate::EstimateOutcome::evaluate(
            &source,
            &calendars(),
            &adjustment(),
            &[],
            instant(61, 0),
        ));
        assert_eq!(
            crate::ScorecardReturnObservation::try_new(
                ongoing,
                crate::signal_return_scenarios::tests::sample(0, "100")
                    .1
                    .value()
                    .clone()
            ),
            Err(crate::SignalReturnError::ExitUnavailable)
        );
    }
}

#[cfg(test)]
mod adjustment_tests {
    use crate::candle::tests::price;
    use crate::signal_return_contract::tests::{adjustment, calendars, instant, row, task};
    use crate::{
        CorporateAction, CorporateActionId, CorporateActionKind, Currency, InstrumentId,
        OutcomeCorporateAction, OutcomeUnavailable, ShareCount, ShareRatio, VirtualExit,
        VirtualExitSignal,
    };
    fn action(id: &str, day: i64, kind: CorporateActionKind) -> OutcomeCorporateAction {
        OutcomeCorporateAction::new(
            task().volatility().instrument().clone(),
            CorporateAction::try_new(
                CorporateActionId::new(InstrumentId::try_new(id).expect("ID")),
                row(day).date(),
                kind,
            )
            .expect("기업 행위"),
        )
    }
    fn recorded(entry: &str, target: &str, stop: &str) -> VirtualExitSignal {
        let source = task();
        let observation = crate::PriceObservation::from_identified_quote_parts(
            source.volatility().instrument().clone(),
            source.reference().instrument().clone(),
            price(entry, Currency::Krw),
            instant(61, 0),
        )
        .expect("진입");
        VirtualExitSignal::from_entry(
            source,
            observation,
            price(target, Currency::Krw),
            price(stop, Currency::Krw),
            instant(66, 4),
        )
        .expect("신호")
    }
    fn split(old: u64, new: u64) -> ShareRatio {
        ShareRatio::new(
            ShareCount::try_new(old).expect("구주"),
            ShareCount::try_new(new).expect("신주"),
        )
    }
    #[test]
    fn d29_adjustment_actions_are_ordered_filtered_and_reject_unsupported_evidence() {
        let signal = recorded("100", "110", "90");
        let first = action("a", 62, CorporateActionKind::Split(split(1, 2)));
        let second = action("b", 62, CorporateActionKind::ReverseSplit(split(2, 1)));
        let past = action("past", 61, CorporateActionKind::Delisting);
        let future = action("future", 67, CorporateActionKind::Delisting);
        let value = crate::virtual_exit_adjustment(
            &signal,
            adjustment().basis(),
            &[second, future, past, first.clone()],
        )
        .expect("시점 필터");
        assert_eq!(value.ratios(), &[split(1, 2), split(2, 1)]);
        assert_eq!(
            crate::virtual_exit_adjustment(
                &signal,
                adjustment().basis(),
                &[first.clone(), first.clone()]
            ),
            Err(OutcomeUnavailable::Adjustment)
        );
        assert_eq!(
            crate::virtual_exit_adjustment(
                &signal,
                adjustment().basis(),
                &[action("unsupported", 62, CorporateActionKind::Delisting)]
            ),
            Err(OutcomeUnavailable::Adjustment)
        );
        let wrong = OutcomeCorporateAction::new(
            InstrumentId::try_new("other").expect("종목"),
            first.action().clone(),
        );
        assert_eq!(
            crate::virtual_exit_adjustment(&signal, adjustment().basis(), &[wrong]),
            Err(OutcomeUnavailable::Adjustment)
        );
        assert_eq!(format!("{signal:?}"), "VirtualExitSignal");
    }
    #[test]
    fn d29_nonterminating_adjusted_entry_target_stop_and_overflow_are_unavailable() {
        for signal in [
            recorded("100", "110", "90"),
            recorded("99", "110", "90"),
            recorded("99", "111", "91"),
        ] {
            let adjusted = crate::virtual_exit_adjustment(
                &signal,
                adjustment().basis(),
                &[action(
                    "thirds",
                    62,
                    CorporateActionKind::Split(split(1, 3)),
                )],
            )
            .expect("분할 근거");
            assert_eq!(
                crate::virtual_exit(&signal, &calendars(), &adjusted, &[], instant(61, 0)),
                VirtualExit::Unavailable(OutcomeUnavailable::Adjustment)
            );
        }
        let signal = recorded("100", "79228162514264337593543950335", "90");
        let adjusted = crate::virtual_exit_adjustment(
            &signal,
            adjustment().basis(),
            &[action(
                "reverse",
                62,
                CorporateActionKind::ReverseSplit(split(2, 1)),
            )],
        )
        .expect("병합 근거");
        assert_eq!(
            crate::virtual_exit(&signal, &calendars(), &adjusted, &[], instant(61, 0)),
            VirtualExit::Unavailable(OutcomeUnavailable::Adjustment)
        );
    }
}

#[cfg(test)]
mod selection_tests {
    use crate::signal_return_contract::tests::{instant, key};
    #[test]
    fn d29_duplicate_observations_and_overlaps_preserve_d17_selection() {
        let pairs =
            [0_i64, 1, 7].map(|shift| crate::signal_return_scenarios::tests::sample(shift, "100"));
        let mut records = pairs
            .iter()
            .map(|(record, _)| record.clone())
            .collect::<Vec<_>>();
        records.push(records[0].clone());
        let values = pairs
            .into_iter()
            .map(|(_, value)| value)
            .collect::<Vec<_>>();
        let edge = crate::scorecard_edge(
            &key(),
            &records,
            &values,
            instant(300, 4),
            crate::BootstrapSeed::new(0),
        )
        .expect("기존 선택");
        let counts = edge.scorecards().select(&key()).expect("조합").counts();
        assert_eq!(counts.total().as_usize(), 3);
        assert_eq!(counts.judged().as_usize(), 2);
        assert_eq!(counts.overlapping().as_usize(), 1);
        assert_eq!(edge.unavailable().as_usize(), 0);
        let absent = crate::CalibrationKey::new(
            key().model().clone(),
            crate::Horizon::new(crate::HorizonSelection::WeeksToMonths),
        );
        assert_eq!(
            crate::scorecard_edge(
                &absent,
                &records,
                &[],
                instant(300, 4),
                crate::BootstrapSeed::new(0)
            ),
            Err(crate::SignalReturnError::Scorecard(
                crate::ScorecardError::MismatchedCombination
            ))
        );
    }
}

#[cfg(test)]
mod candle_tests {
    #[test]
    fn d29_entry_currency_cannot_disagree_with_the_identified_market() {
        use crate::signal_return_contract::tests::{instant, task};
        let source = task();
        let entry = crate::PriceObservation::from_identified_quote_parts(
            source.volatility().instrument().clone(),
            source.reference().instrument().clone(),
            crate::candle::tests::price("100", crate::Currency::Usd),
            instant(61, 0),
        )
        .expect("잘못된 통화의 관측");
        assert_eq!(
            crate::VirtualExitSignal::from_entry(
                source,
                entry,
                crate::candle::tests::price("110", crate::Currency::Usd),
                crate::candle::tests::price("90", crate::Currency::Usd),
                instant(66, 4)
            ),
            Err(crate::VirtualExitSignalError::CurrencyMismatch)
        );
    }
    #[test]
    fn d29_foreign_instrument_candle_is_not_accepted_as_signal_evidence() {
        use crate::signal_return_contract::tests::{adjustment, calendars, candle, instant, task};
        let first = candle(61, 1, crate::CandlePeriod::Minute, ["100"; 4]);
        let invalid = crate::Candle::try_new(
            crate::InstrumentId::try_new("other").expect("다른 종목"),
            first.time(),
            first.adjustment(),
            first.values(),
            crate::CandleObservation::new(first.collected_at(), first.status()),
        )
        .expect("다른 종목의 유효 봉");
        let signal = crate::VirtualExitSignal::try_new(
            task(),
            crate::candle::tests::price("110", crate::Currency::Krw),
            crate::candle::tests::price("90", crate::Currency::Krw),
            instant(66, 4),
        )
        .expect("신호");
        assert_eq!(
            crate::virtual_exit(
                &signal,
                &calendars(),
                &adjustment(),
                &[invalid],
                instant(66, 4)
            ),
            crate::VirtualExit::Unavailable(crate::OutcomeUnavailable::InvalidCandle)
        );
    }
}
