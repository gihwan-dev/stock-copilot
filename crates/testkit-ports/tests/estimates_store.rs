#[cfg(test)]
mod tests {
    use application::{AiRunId, ReasoningEffort};
    use domain::{
        AccountId, AdjustmentBasis, AiProbability, AiProbabilityInput, BuyingPower, CalendarDate,
        CalendarYear, CalibrationEstimateId, CalibrationKey, CalibrationModel, Candle,
        CandleObservation, CandlePeriod, CandleStatus, CandleTime, CommissionRateFact,
        CommissionValidity, CommonShare, CostEligibleTask, CostRate, Currency, DayNumber,
        DisplayInstrumentName, EstimateContext, EstimateCostPeriod, EstimateMetadata,
        EstimateNarrative, EstimateOutcome, EstimatePrompt, EstimateTask, EvidenceBundleId,
        Horizon, HorizonSelection, HoursPermission, InstrumentId, KoreanMasterDetail,
        LiquidationTrading, ListingStatus, Market, MarketCalendar, MarketSession, MasterCode,
        MasterInstrument, MasterInstrumentRow, MasterSecurityType, ModelIdentityPart, Money,
        MonthNumber, Nanosecond, NumericInput, NxtTrading, OffsetSeconds, Ohlcv, OutcomeAdjustment,
        Portfolio, Price, PriceAdjustment, PromptHash, Quantity, Ratio, RiskAppetite,
        ScorecardPromptId, SessionData, SessionInformation, SignalSubmission, SignalTaskKey,
        SnapshotCalendar, SnapshotError, Symbol, TimeInterval, Timestamp, TradeCost, TradeTaxFact,
        TradingSuspension, UnixSeconds, UntrustedText, ValuationDayResult, ValuationSnapshot,
        Volatility,
    };
    use std::{
        future::Future,
        task::{Context, Poll, Waker},
    };
    use testkit_ports::contracts::estimates::{
        EstimateLifecycleFixture, estimate_task_submission_resolution_lifecycle,
        valuation_day_append_only,
    };
    use testkit_ports::memory::{MemoryEstimateLedger, MemoryValuationDayRepository};

    fn run<F: Future>(future: F) -> F::Output {
        let mut pinned = std::pin::pin!(future);
        match pinned
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
        {
            Poll::Ready(value) => value,
            Poll::Pending => panic!("즉시 완료"),
        }
    }
    fn at(seconds: i64) -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(seconds),
            Nanosecond::new(0),
            OffsetSeconds::new(0),
        )
        .expect("시각")
    }
    fn day(value: i64) -> Timestamp {
        at(value.checked_mul(86_400).expect("일"))
    }
    fn local(time: Timestamp) -> Timestamp {
        Timestamp::from_parts(
            time.utc_seconds(),
            time.nanosecond(),
            OffsetSeconds::new(32_400),
        )
        .expect("현지 시각")
    }
    fn local_midnight(value: i64) -> Timestamp {
        local(at(day(value)
            .utc_seconds()
            .as_i64()
            .checked_sub(32_400)
            .expect("자정")))
    }
    fn price(value: &str) -> Price {
        Price::parse(&NumericInput::try_new(value).expect("입력"), Currency::Krw).expect("가격")
    }
    fn money(value: &str, currency: Currency) -> Money {
        Money::parse(&NumericInput::try_new(value).expect("입력"), currency).expect("금액")
    }
    fn quantity(value: &str) -> Quantity {
        Quantity::parse(&NumericInput::try_new(value).expect("입력")).expect("수량")
    }
    fn ratio(value: &str) -> Ratio {
        Ratio::parse(&NumericInput::try_new(value).expect("입력")).expect("비율")
    }
    fn master() -> MasterInstrument {
        MasterInstrument::try_new(MasterInstrumentRow {
            listing_market: MasterCode::try_new("KOSPI").expect("시장"),
            instrument_id: InstrumentId::try_new("est-mem-a").expect("종목"),
            symbol: Symbol::try_new("005930").expect("코드"),
            name: DisplayInstrumentName::try_new("종목").expect("이름"),
            security_type: MasterSecurityType::from_code(
                MasterCode::try_new("STOCK").expect("유형"),
            ),
            currency: Currency::Krw,
            listing_status: ListingStatus::Active,
            common_share: CommonShare::Common,
            korean_detail: Some(KoreanMasterDetail {
                liquidation: LiquidationTrading::Regular,
                krx: TradingSuspension::Clear,
                nxt: NxtTrading::Unsupported,
            }),
            observed_at: day(60),
        })
        .expect("마스터")
    }
    fn eligible_task() -> CostEligibleTask {
        let source = master();
        let candles: Vec<_> = (0_i64..61)
            .map(|index| {
                let value = price(if index % 2 == 0 { "100" } else { "102" });
                Candle::try_new(
                    source.row().instrument_id.clone(),
                    CandleTime::try_new(local_midnight(index), CandlePeriod::Daily).expect("일봉"),
                    PriceAdjustment::Adjusted(AdjustmentBasis::new(day(61))),
                    Ohlcv::try_new(value, value, value, value, quantity("10")).expect("OHLCV"),
                    CandleObservation::new(day(61), CandleStatus::Finalized),
                )
                .expect("봉")
            })
            .collect();
        let volatility = Volatility::calculate(&candles).expect("변동성");
        let task = EstimateTask::create(
            EstimateContext::new(
                source
                    .price_observation(price("100"), local(day(60)))
                    .expect("관측"),
                day(60),
            ),
            volatility,
            Horizon::new(HorizonSelection::FewDays),
            HoursPermission::default(),
            RiskAppetite::Neutral,
        )
        .expect("과제");
        let zero = CostRate::try_new(ratio("0")).expect("비용률");
        let period = EstimateCostPeriod::from_calendar_result(
            &task,
            at(day(65)
                .utc_seconds()
                .as_i64()
                .checked_add(3600)
                .expect("만기")),
        )
        .expect("기간");
        let cost = TradeCost::from_facts(
            Some(CommissionRateFact::from_commissions_result(
                Market::Kr,
                zero,
                CommissionValidity::try_new(None, None).expect("유효"),
            )),
            Some(TradeTaxFact::from_schedule_result(Market::Kr, zero, zero)),
            period,
        )
        .expect("비용");
        CostEligibleTask::assess(task, &cost).expect("적격")
    }
    fn session(day_number: i64) -> MarketSession {
        let start = local(day(day_number));
        let end = local(at(day(day_number)
            .utc_seconds()
            .as_i64()
            .checked_add(240)
            .expect("종료")));
        MarketSession::try_new(
            start.offset_calendar_date(),
            SessionInformation::Kr {
                pre_market: None,
                regular_market: Some(
                    SessionData::try_new(TimeInterval::try_new(start, end).expect("구간"), None)
                        .expect("세션"),
                ),
                after_market: None,
            },
        )
        .expect("장")
    }
    fn outcome_for(task: &CostEligibleTask, high: &str, low: &str) -> EstimateOutcome {
        let calendars: Vec<_> = (60_i64..=65)
            .map(|index| {
                MarketCalendar::try_new(
                    session(index.checked_sub(1).expect("전일")),
                    session(index),
                    session(index.checked_add(1).expect("익일")),
                )
                .expect("달력")
            })
            .collect();
        let adjustment =
            OutcomeAdjustment::from_actions(task.task(), AdjustmentBasis::new(local(day(70))), &[])
                .expect("수정");
        let make_bar = |date: i64, minute: i64, bar_high: &str, bar_low: &str| {
            let period = if minute == 0 {
                CandlePeriod::Daily
            } else {
                CandlePeriod::Minute
            };
            let stamp = if minute == 0 {
                local_midnight(date)
            } else {
                local(at(day(date)
                    .utc_seconds()
                    .as_i64()
                    .checked_add(minute.checked_mul(60).expect("분"))
                    .expect("시각")))
            };
            Candle::try_new(
                task.task().volatility().instrument().clone(),
                CandleTime::try_new(stamp, period).expect("시각"),
                PriceAdjustment::Adjusted(adjustment.basis()),
                Ohlcv::try_new(
                    price(bar_low),
                    price(bar_high),
                    price(bar_low),
                    price(bar_low),
                    quantity("10"),
                )
                .expect("OHLCV"),
                CandleObservation::new(day(70), CandleStatus::Finalized),
            )
            .expect("봉")
        };
        let mut candles: Vec<_> = (1..=4)
            .map(|minute| make_bar(60, minute, "100", "100"))
            .collect();
        candles.extend((61..=65).map(|date| make_bar(date, 0, high, low)));
        EstimateOutcome::evaluate_with_complete_data(
            task.task(),
            &calendars,
            &adjustment,
            &candles,
            local(day(70)),
        )
    }

    #[test]
    fn memory_estimate_ledger_satisfies_shared_lifecycle_contract() {
        let task = eligible_task();
        let key = CalibrationKey::new(
            CalibrationModel::new(
                ModelIdentityPart::try_new("claude").expect("제공자"),
                ModelIdentityPart::try_new("opus").expect("모델"),
                ModelIdentityPart::try_new("v1").expect("버전"),
            ),
            Horizon::new(HorizonSelection::FewDays),
        );
        let submission = SignalSubmission::create(
            SignalTaskKey::from_task(&task),
            key,
            AiProbability::parse(&AiProbabilityInput::try_new("72").expect("입력")).expect("확률"),
            None,
            day(61),
        )
        .expect("제출");
        let fixture = EstimateLifecycleFixture {
            task_id: CalibrationEstimateId::try_new("task-1").expect("과제ID"),
            run_id: AiRunId::try_new("run-a").expect("실행"),
            other_run_id: AiRunId::try_new("run-b").expect("다른실행"),
            task: task.clone(),
            submission,
            narrative: EstimateNarrative {
                rationale: UntrustedText::try_new("근거").expect("근거"),
                counterargument: UntrustedText::try_new("반론").expect("반론"),
            },
            metadata: EstimateMetadata {
                effort: ReasoningEffort::High,
                cli_version: ModelIdentityPart::try_new("1.0.0").expect("CLI"),
                prompt: EstimatePrompt {
                    id: ScorecardPromptId::try_new("p-1").expect("프롬프트"),
                    version: ModelIdentityPart::try_new("v1").expect("버전"),
                    hash: PromptHash::try_new(&"ab".repeat(32)).expect("해시"),
                },
                evidence_bundle: EvidenceBundleId::try_new("bundle-1").expect("묶음"),
            },
            outcome: outcome_for(&task, "200", "100"),
            other_outcome: outcome_for(&task, "100", "50"),
        };
        let ledger = MemoryEstimateLedger::new();
        run(estimate_task_submission_resolution_lifecycle(
            &ledger, &fixture,
        ));
    }

    fn march_date(day_number: u8) -> CalendarDate {
        CalendarDate::from_parts(
            CalendarYear::new(2026),
            MonthNumber::new(3),
            DayNumber::new(day_number),
        )
        .expect("날짜")
    }

    fn market_session(date: CalendarDate, market: &Market) -> MarketSession {
        let day_offset = i64::from(*date.day().as_u8())
            .checked_sub(1)
            .expect("일 오프셋");
        let base_secs = 1_772_323_200_i64
            .checked_add(day_offset.checked_mul(86_400).expect("초"))
            .expect("시각");
        let base = Timestamp::from_parts(
            UnixSeconds::new(base_secs),
            Nanosecond::new(0),
            OffsetSeconds::new(32_400),
        )
        .expect("시각");
        let end = Timestamp::from_parts(
            UnixSeconds::new(base_secs.checked_add(3600).expect("끝")),
            Nanosecond::new(0),
            OffsetSeconds::new(32_400),
        )
        .expect("끝");
        let seg = Some(
            SessionData::try_new(TimeInterval::try_new(base, end).expect("구간"), None)
                .expect("세션"),
        );
        MarketSession::try_new(
            date,
            match market {
                Market::Kr => SessionInformation::Kr {
                    pre_market: None,
                    regular_market: seg,
                    after_market: None,
                },
                Market::Us | Market::Unknown(_) => SessionInformation::Us {
                    day_market: None,
                    pre_market: None,
                    regular_market: seg,
                    after_market: None,
                },
            },
        )
        .expect("장")
    }

    fn sample_snapshot() -> ValuationSnapshot {
        let prev = march_date(9);
        let day_date = march_date(10);
        let next = march_date(11);
        let kr_cal = MarketCalendar::try_new(
            market_session(prev, &Market::Kr),
            market_session(day_date, &Market::Kr),
            market_session(next, &Market::Kr),
        )
        .expect("국내");
        let us_cal = MarketCalendar::try_new(
            market_session(prev, &Market::Us),
            market_session(day_date, &Market::Us),
            market_session(next, &Market::Us),
        )
        .expect("미국");
        let snap_cal = SnapshotCalendar::new(&kr_cal, &us_cal).expect("스냅숏 달력");
        let at_time = Timestamp::from_parts(
            UnixSeconds::new(1_773_126_000),
            Nanosecond::new(0),
            OffsetSeconds::new(32_400),
        )
        .expect("평가시각");
        let portfolio = Portfolio::new(
            Vec::new(),
            BuyingPower::new(money("1000", Currency::Krw), at_time),
            BuyingPower::new(money("0", Currency::Usd), at_time),
        )
        .expect("포트폴리오");
        ValuationSnapshot::observe(&snap_cal, &portfolio.evaluate(None), at_time, None)
            .expect("스냅숏")
    }

    #[test]
    fn memory_valuation_day_repository_satisfies_shared_contract() {
        let snapshot = sample_snapshot();
        let first = ValuationDayResult::restore_from_saved_state(
            snapshot.settlement_date(),
            Ok(snapshot.clone()),
        )
        .expect("성공 평가");
        let conflicting = ValuationDayResult::restore_from_saved_state(
            snapshot.settlement_date(),
            Err(SnapshotError::MissingHistory),
        )
        .expect("결측 평가");
        let repo = MemoryValuationDayRepository::new();
        let account_a = AccountId::new(1);
        let account_b = AccountId::new(2);
        run(valuation_day_append_only(
            &repo,
            (&account_a, &account_b),
            &first,
            &conflicting,
        ));
        assert_eq!(
            repo.day_result(&account_a, first.date()),
            Some(first),
            "원래 결과 보존"
        );
    }
}
