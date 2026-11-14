#[cfg(test)]
pub(crate) mod tests {
    use crate::candle::tests::{price, series};
    use crate::signal_return_contract::tests::{instant, key, row};
    use crate::{
        AdjustmentBasis, AiProbability, AiProbabilityInput, BootstrapSeed, BuyingPower, Candle,
        CandleObservation, CandlePeriod, CandleStatus, CandleTime, CommissionRateFact,
        CommissionValidity, CostRate, Currency, EdgeDirection, EstimateContext, EstimateCostPeriod,
        EstimateOutcome, EstimateTask, Horizon, HorizonSelection, HoursPermission, InstrumentId,
        Market, MarketCalendar, MarketSession, NumericInput, Ohlcv, OutcomeAdjustment, Portfolio,
        PriceAdjustment, PriceObservation, Quantity, Ratio, ResolvedOrderInstrument, RiskAppetite,
        ScorecardObservation, ScorecardPromptId, SessionData, SessionInformation, SnapshotCalendar,
        SpanSeconds, Symbol, TimeInterval, TimeSpan, TradeTaxFact, ValuationSnapshot, Volatility,
    };
    use rust_decimal::Decimal;

    pub(crate) fn task(shift: i64) -> EstimateTask {
        let values = (0_usize..61)
            .map(|index| {
                if index.checked_rem(2).expect("짝수") == 0 {
                    "100"
                } else {
                    "102"
                }
            })
            .collect::<Vec<_>>();
        let seconds = u64::try_from(shift)
            .expect("양수 이동")
            .checked_mul(86400)
            .expect("이동");
        let span =
            TimeSpan::try_new(SpanSeconds::new(seconds), crate::Nanosecond::new(0)).expect("기간");
        let bars = series(&values)
            .into_iter()
            .map(|bar| {
                Candle::try_new(
                    bar.instrument().clone(),
                    CandleTime::try_new(
                        bar.time().timestamp().checked_add(span).expect("이동"),
                        CandlePeriod::Daily,
                    )
                    .expect("일봉"),
                    bar.adjustment(),
                    bar.values(),
                    CandleObservation::new(
                        bar.collected_at().checked_add(span).expect("수집"),
                        CandleStatus::Finalized,
                    ),
                )
                .expect("이력")
            })
            .collect::<Vec<_>>();
        let volatility = Volatility::calculate(&bars).expect("변동성");
        let when = instant(61_i64.checked_add(shift).expect("진입일"), 4);
        let reference = PriceObservation::from_identified_quote_parts(
            volatility.instrument().clone(),
            ResolvedOrderInstrument::from_master_lookup_result(
                Symbol::try_new("ABC").expect("코드"),
                Market::Kr,
            )
            .expect("종목"),
            price("100", Currency::Krw),
            when,
        )
        .expect("관측");
        EstimateTask::create(
            EstimateContext::new(reference, when),
            volatility,
            Horizon::new(HorizonSelection::FewDays),
            HoursPermission::default(),
            RiskAppetite::Neutral,
        )
        .expect("과제")
    }
    pub(crate) fn calendar(day: i64) -> MarketCalendar {
        MarketCalendar::try_new(
            row(day.checked_sub(1).expect("전일")),
            row(day),
            row(day.checked_add(1).expect("익일")),
        )
        .expect("달력")
    }
    pub(crate) fn bar(
        day: i64,
        minute: i64,
        period: CandlePeriod,
        value: &str,
        basis: AdjustmentBasis,
    ) -> Candle {
        let stamp = if period == CandlePeriod::Daily {
            instant(day, -540)
        } else {
            instant(day, minute)
        };
        let close = price(value, Currency::Krw);
        Candle::try_new(
            InstrumentId::try_new("sample").expect("식별자"),
            CandleTime::try_new(stamp, period).expect("시각"),
            PriceAdjustment::Adjusted(basis),
            Ohlcv::try_new(
                close,
                close,
                close,
                close,
                Quantity::parse(&NumericInput::try_new("1").expect("수량")).expect("수량"),
            )
            .expect("OHLC"),
            CandleObservation::new(*basis.as_timestamp(), CandleStatus::Finalized),
        )
        .expect("봉")
    }
    pub(crate) fn snapshot(day: i64) -> ValuationSnapshot {
        let us_row = |value: i64| {
            MarketSession::try_new(
                row(value).date(),
                SessionInformation::Us {
                    day_market: None,
                    pre_market: None,
                    regular_market: Some(
                        SessionData::try_new(
                            TimeInterval::try_new(instant(value, 0), instant(value, 4))
                                .expect("장"),
                            None,
                        )
                        .expect("장"),
                    ),
                    after_market: None,
                },
            )
            .expect("미국 달력 사실")
        };
        let us = MarketCalendar::try_new(
            us_row(day.checked_sub(1).expect("전일")),
            us_row(day),
            us_row(day.checked_add(1).expect("익일")),
        )
        .expect("달력");
        let dates = SnapshotCalendar::new(&calendar(day), &us).expect("평가 달력");
        let now = instant(day, 4);
        let portfolio = Portfolio::new(
            Vec::new(),
            BuyingPower::new(price("100", Currency::Krw).as_money(), now),
            BuyingPower::new(
                crate::Money::new(crate::Amount::new(Decimal::ZERO), Currency::Usd),
                now,
            ),
        )
        .expect("가상 포트폴리오");
        ValuationSnapshot::observe(&dates, &portfolio.evaluate(None), now, None).expect("가격 날짜")
    }
    pub(crate) fn benchmark(
        shift: i64,
        last: &str,
        basis: AdjustmentBasis,
    ) -> crate::SignalBenchmark {
        let days = (61_i64..=67)
            .map(|day| day.checked_add(shift).expect("날짜"))
            .collect::<Vec<_>>();
        let candles = days
            .iter()
            .map(|day| {
                bar(
                    *day,
                    0,
                    CandlePeriod::Daily,
                    if *day == 67_i64.checked_add(shift).expect("끝") {
                        last
                    } else {
                        "100"
                    },
                    basis,
                )
            })
            .collect::<Vec<_>>();
        let dates = days.iter().map(|day| snapshot(*day)).collect::<Vec<_>>();
        crate::SignalBenchmark::from_candles(
            crate::BenchmarkKind::Kospi,
            &candles,
            &dates,
            &days.iter().map(|day| calendar(*day)).collect::<Vec<_>>(),
            Currency::Krw,
        )
        .expect("동일 종가 기간")
    }
    pub(crate) fn observation(
        task: &EstimateTask,
        calendars: &[MarketCalendar],
        adjustment: &OutcomeAdjustment,
        bars: &[Candle],
        id: &str,
    ) -> ScorecardObservation {
        let now = *adjustment.basis().as_timestamp();
        ScorecardObservation::try_new(
            crate::CalibrationEstimateId::try_new(id).expect("추정 ID"),
            key(),
            ScorecardPromptId::try_new("prompt").expect("프롬프트"),
            AiProbability::parse(&AiProbabilityInput::try_new("60").expect("입력")).expect("확률"),
            EstimateOutcome::evaluate(task, calendars, adjustment, bars, now),
        )
        .expect("관측")
    }
    pub(crate) fn sample(
        shift: i64,
        last: &str,
    ) -> (ScorecardObservation, crate::ScorecardReturnObservation) {
        let source = task(shift);
        let end = 67_i64.checked_add(shift).expect("만기일");
        let basis = AdjustmentBasis::new(instant(end, 4));
        let adjustment = OutcomeAdjustment::from_actions(&source, basis, &[]).expect("조정");
        let calendars = (61_i64..=67)
            .map(|day| calendar(day.checked_add(shift).expect("날짜")))
            .collect::<Vec<_>>();
        let mut bars = (62_i64..=67)
            .map(|day| {
                bar(
                    day.checked_add(shift).expect("날짜"),
                    0,
                    CandlePeriod::Daily,
                    "100",
                    basis,
                )
            })
            .collect::<Vec<_>>();
        bars.extend((1_i64..=4).map(|minute| {
            bar(
                62_i64.checked_add(shift).expect("첫 거래일"),
                minute,
                CandlePeriod::Minute,
                "100",
                basis,
            )
        }));
        let zero = CostRate::try_new(Ratio::new(Decimal::ZERO)).expect("요율");
        let cost = crate::signal_return_cost(
            Some(CommissionRateFact::from_commissions_result(
                Market::Kr,
                zero,
                CommissionValidity::try_new(None, None).expect("유효 기간"),
            )),
            Some(TradeTaxFact::from_schedule_result(Market::Kr, zero, zero)),
            EstimateCostPeriod::from_calendar_result(&source, instant(end, 4)).expect("비용 기간"),
        )
        .expect("거래 비용");
        let signal = crate::VirtualExitSignal::try_new(
            source.clone(),
            price("110", Currency::Krw),
            price("90", Currency::Krw),
            instant(end, 4),
        )
        .expect("신호 가격");
        let exit = crate::virtual_exit(&signal, &calendars, &adjustment, &bars, instant(end, 4));
        let value = crate::signal_return(&source, exit, &cost, &benchmark(shift, last, basis))
            .expect("순수익률");
        let record = observation(
            &source,
            &calendars,
            &adjustment,
            &bars,
            &format!("sample-{shift}"),
        );
        let bound =
            crate::ScorecardReturnObservation::try_new(record.clone(), value).expect("신호 결합");
        (record, bound)
    }
    #[test]
    fn d29_public_path_compares_same_closes_and_preserves_all_edge_directions() {
        for (last, expected, excess) in [
            ("99", EdgeDirection::Superior, "0.010000"),
            ("100", EdgeDirection::EdgeNotConfirmed, "0.000000"),
            ("101", EdgeDirection::Inferior, "-0.010000"),
        ] {
            let pairs = (0_i64..30)
                .map(|index| sample(index.checked_mul(7).expect("비겹침"), last))
                .collect::<Vec<_>>();
            let records = pairs
                .iter()
                .map(|(record, _)| record.clone())
                .collect::<Vec<_>>();
            let returns = pairs
                .into_iter()
                .map(|(_, value)| value)
                .collect::<Vec<_>>();
            assert_eq!(
                returns
                    .first()
                    .expect("수익률")
                    .value()
                    .excess()
                    .expect("동일 기간")
                    .to_string(),
                excess
            );
            let edge = crate::scorecard_edge(
                &key(),
                &records,
                &returns,
                instant(300, 4),
                BootstrapSeed::new(42),
            )
            .expect("성적표");
            assert_eq!(edge.unavailable().as_usize(), 0);
            assert_eq!(
                edge.scorecards()
                    .select(&key())
                    .expect("조합")
                    .counts()
                    .judged()
                    .as_usize(),
                30
            );
            let interval = edge.confidence().clone().into_ready().expect("준비된 구간");
            assert_eq!(interval.direction(), expected);
            assert_eq!(interval.lower().to_string(), excess);
            assert_eq!(interval.upper().to_string(), excess);
        }
    }
}
