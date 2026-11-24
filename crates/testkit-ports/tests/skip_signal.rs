#[cfg(test)]
mod tests {
    use application::{
        AiRunId, EstimateAssignmentRepository, EstimateCommitOutcome, GetEstimateTask,
        GetEstimateTaskInput, SignalSkip, SignalSkipRepository, SkipReason, SkipReasonError,
        SkipSignal, SystemPortError, WriteCompleted,
    };
    use domain::{
        AdjustmentBasis, CalibrationEstimateId, Candle, CandleObservation, CandlePeriod,
        CandleStatus, CandleTime, CommissionRateFact, CommissionValidity, CommonShare,
        CostEligibleTask, CostRate, Currency, DisplayInstrumentName, EstimateAssignments,
        EstimateContext, EstimateCostPeriod, EstimateTask, Horizon, HorizonSelection,
        HoursPermission, InstrumentId, KoreanMasterDetail, LiquidationTrading, ListingStatus,
        Market, MasterCode, MasterInstrument, MasterInstrumentRow, MasterSecurityType, Nanosecond,
        NumericInput, NxtTrading, OffsetSeconds, Ohlcv, Price, PriceAdjustment, Quantity, Ratio,
        RiskAppetite, Symbol, Timestamp, TradeCost, TradeTaxFact, TradingSuspension, UnixSeconds,
        Volatility,
    };
    use std::{
        future::Future,
        sync::RwLock,
        task::{Context, Poll, Waker},
    };
    use testkit_ports::SteppedClock;

    fn price(value: &str) -> Price {
        Price::parse(&NumericInput::try_new(value).expect("입력"), Currency::Krw).expect("가격")
    }
    fn quantity(value: &str) -> Quantity {
        Quantity::parse(&NumericInput::try_new(value).expect("입력")).expect("수량")
    }
    fn ratio(value: &str) -> Ratio {
        Ratio::parse(&NumericInput::try_new(value).expect("입력")).expect("비율")
    }
    fn id(value: &str) -> InstrumentId {
        InstrumentId::try_new(value).expect("종목")
    }
    fn at(value: i64) -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(value),
            Nanosecond::new(0),
            OffsetSeconds::new(0),
        )
        .expect("시각")
    }
    fn day(value: i64) -> Timestamp {
        at(value.checked_mul(86400).expect("일"))
    }
    fn local(time: Timestamp) -> Timestamp {
        Timestamp::from_parts(
            time.utc_seconds(),
            time.nanosecond(),
            OffsetSeconds::new(32400),
        )
        .expect("현지 시각")
    }
    fn local_midnight(value: i64) -> Timestamp {
        local(at(day(value)
            .utc_seconds()
            .as_i64()
            .checked_sub(32400)
            .expect("현지 자정")))
    }
    fn master() -> MasterInstrument {
        MasterInstrument::try_new(MasterInstrumentRow {
            listing_market: MasterCode::try_new("KOSPI").expect("시장"),
            instrument_id: id("sample-a"),
            symbol: Symbol::try_new("005930").expect("코드"),
            name: DisplayInstrumentName::try_new("고정 종목").expect("이름"),
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
    fn eligible() -> CostEligibleTask {
        let source = master();
        let candles: Vec<_> = (0_i64..61)
            .map(|index| {
                let fixture_price = price(if index % 2 == 0 { "100" } else { "102" });
                Candle::try_new(
                    source.row().instrument_id.clone(),
                    CandleTime::try_new(local_midnight(index), CandlePeriod::Daily)
                        .expect("일봉 시각"),
                    PriceAdjustment::Adjusted(AdjustmentBasis::new(day(61))),
                    Ohlcv::try_new(
                        fixture_price,
                        fixture_price,
                        fixture_price,
                        fixture_price,
                        quantity("13"),
                    )
                    .expect("OHLCV"),
                    CandleObservation::new(day(61), CandleStatus::Finalized),
                )
                .expect("일봉")
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
        let zero = CostRate::try_new(ratio("0")).expect("비용");
        let period = EstimateCostPeriod::from_calendar_result(
            &task,
            at(day(65)
                .utc_seconds()
                .as_i64()
                .checked_add(3600)
                .expect("끝")),
        )
        .expect("만기");
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
    #[derive(Debug)]
    struct Store {
        assignments: RwLock<EstimateAssignments<AiRunId>>,
        skips: RwLock<Vec<SignalSkip>>,
    }
    impl EstimateAssignmentRepository for Store {
        fn load_assignments(
            &self,
        ) -> impl Future<Output = Result<EstimateAssignments<AiRunId>, SystemPortError>> + Send
        {
            std::future::ready(Ok(self.assignments.read().expect("잠금").clone()))
        }
        fn commit_assignments(
            &self,
            _previous: &EstimateAssignments<AiRunId>,
            next: &EstimateAssignments<AiRunId>,
        ) -> impl Future<Output = Result<EstimateCommitOutcome, SystemPortError>> + Send {
            *self.assignments.write().expect("잠금") = next.clone();
            std::future::ready(Ok(EstimateCommitOutcome::Committed))
        }
    }
    impl SignalSkipRepository for Store {
        fn record_skip(
            &self,
            skip: &SignalSkip,
        ) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send {
            self.skips.write().expect("잠금").push(skip.clone());
            std::future::ready(Ok(WriteCompleted))
        }
    }
    fn reason(value: &str) -> SkipReason {
        SkipReason::try_new(value).expect("사유")
    }

    #[test]
    fn skip_reason_is_recorded_but_never_reaches_the_next_estimate_input() {
        let run_id = AiRunId::try_new("alpha").expect("실행");
        let task_id = CalibrationEstimateId::try_new("next-task").expect("과제");
        let store = Store {
            assignments: RwLock::new(
                EstimateAssignments::default()
                    .issue(task_id.clone(), run_id.clone(), eligible())
                    .expect("발급"),
            ),
            skips: RwLock::new(Vec::new()),
        };
        let input = GetEstimateTaskInput::new(task_id);
        let before = run(GetEstimateTask::new(&store).execute(&run_id, &input)).expect("과제");
        let clock = SteppedClock::new(day(61));
        let skip = run(SkipSignal::new(&store, &clock)
            .execute(id("sample-a"), Some(reason("유일한사유문구 지금은 비싸다"))))
        .expect("넘기기");
        assert_eq!(*store.skips.read().expect("잠금"), vec![skip.clone()]);
        assert_eq!(skip.skipped_at, day(61));
        let after = run(GetEstimateTask::new(&store).execute(&run_id, &input)).expect("과제");
        assert_eq!(before, after);
        assert!(!format!("{after:?}").contains("유일한사유문구"));
    }

    #[test]
    fn skip_reason_is_optional_and_one_line() {
        assert_eq!(SkipReason::try_new("  "), Err(SkipReasonError::Empty));
        assert_eq!(
            Some(reason(" 비싸다 ").as_str().to_owned()),
            Some(String::from("비싸다"))
        );
        assert_eq!(
            SkipReason::try_new("첫 줄\n둘째 줄"),
            Err(SkipReasonError::MultipleLines)
        );
        assert_eq!(
            SkipReason::try_new(&"가".repeat(201)),
            Err(SkipReasonError::TooLong)
        );
        assert_eq!(
            SkipReason::try_new(&"가".repeat(200)).map(|value| value.as_str().chars().count()),
            Ok(200)
        );
    }
}
