#[cfg(test)]
mod tests {
    use application::{
        GetProfile, GetScorecard, GetScorecardError, ProfileView, ScorecardObservationRepository,
        ScorecardSelection, ScorecardView, SystemFailure, SystemPort, SystemPortError,
    };
    use domain::{
        AdjustmentBasis, AiProbability, AiProbabilityInput, CalibrationEstimateId, CalibrationKey,
        CalibrationModel, Candle, CandleObservation, CandlePeriod, CandleStatus, CandleTime,
        CommissionRateFact, CommissionValidity, CommonShare, CostEligibleTask, CostRate, Currency,
        DisplayInstrumentName, EstimateContext, EstimateCostPeriod, EstimateOutcome, EstimateTask,
        Horizon, HorizonSelection, HoursPermission, InstrumentId, InvestorProfile,
        KoreanMasterDetail, LiquidationTrading, ListingStatus, Market, MasterCode,
        MasterInstrument, MasterInstrumentRow, MasterSecurityType, ModelIdentityPart, Nanosecond,
        NumericInput, NxtTrading, OffsetSeconds, Ohlcv, OutcomeAdjustment, Price, PriceAdjustment,
        Quantity, Ratio, RiskAppetite, Scorecard, ScorecardObservation, ScorecardPromptId, Symbol,
        Timestamp, TradeCost, TradeTaxFact, TradingSuspension, UnixSeconds, Volatility,
    };
    use std::{
        future::Future,
        sync::RwLock,
        task::{Context, Poll, Waker},
    };
    use testkit_ports::{
        SteppedClock,
        memory::{MemoryReply, MemorySettingsRepository},
    };

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
    fn key(version: &str) -> CalibrationKey {
        CalibrationKey::new(
            CalibrationModel::new(
                ModelIdentityPart::try_new("claude").expect("제공자"),
                ModelIdentityPart::try_new("model").expect("이름"),
                ModelIdentityPart::try_new(version).expect("버전"),
            ),
            Horizon::new(HorizonSelection::FewDays),
        )
    }
    fn observations(count: usize) -> Vec<ScorecardObservation> {
        let eligible = eligible();
        let task = eligible.task();
        let adjustment =
            OutcomeAdjustment::from_actions(task, AdjustmentBasis::new(local(day(70))), &[])
                .expect("수정");
        let outcome = EstimateOutcome::evaluate(task, &[], &adjustment, &[], local(day(70)));
        (0..count)
            .map(|index| {
                ScorecardObservation::try_new(
                    CalibrationEstimateId::try_new(&format!("est-{index}")).expect("ID"),
                    key("v1"),
                    ScorecardPromptId::try_new("estimate-prompt").expect("프롬프트"),
                    AiProbability::parse(&AiProbabilityInput::try_new("60").expect("입력"))
                        .expect("확률"),
                    outcome.clone(),
                )
                .expect("관측")
            })
            .collect()
    }
    fn pending(card: &Scorecard) -> &'static str {
        match card {
            Scorecard::Pending(_) => "보류",
            Scorecard::Ready(_) => "판정",
        }
    }
    #[derive(Debug)]
    struct Observations(RwLock<Result<Vec<ScorecardObservation>, SystemPortError>>);
    impl ScorecardObservationRepository for Observations {
        fn scorecard_observations(
            &self,
        ) -> impl Future<Output = Result<Vec<ScorecardObservation>, SystemPortError>> + Send
        {
            std::future::ready(self.0.read().expect("잠금").clone())
        }
    }
    fn scorecard(
        source: Result<Vec<ScorecardObservation>, SystemPortError>,
        selection: ScorecardSelection,
    ) -> Result<ScorecardView, GetScorecardError> {
        let repository = Observations(RwLock::new(source));
        let clock = SteppedClock::new(day(70));
        run(GetScorecard::new(&repository, &clock).execute(selection))
    }

    #[test]
    fn profile_defaults_until_saved() {
        let repository = MemorySettingsRepository::default();
        repository
            .script_profile()
            .push(MemoryReply::Return(Ok(None)));
        assert_eq!(
            run(GetProfile::new(&repository).execute()),
            Ok(ProfileView::Default(InvestorProfile::default()))
        );
        repository
            .script_profile()
            .push(MemoryReply::Return(Ok(Some(InvestorProfile::default()))));
        assert_eq!(
            run(GetProfile::new(&repository).execute()),
            Ok(ProfileView::Saved(InvestorProfile::default()))
        );
    }

    #[test]
    fn fewer_than_thirty_judgments_stay_pending() {
        let view = scorecard(Ok(observations(3)), ScorecardSelection::All).expect("성적표");
        let ScorecardView::Cards(cards) = view else {
            panic!("성적표 묶음");
        };
        assert_eq!(cards.cards().len(), 1);
        assert_eq!(cards.cards().first().map(pending), Some("보류"));
        let single = scorecard(
            Ok(observations(3)),
            ScorecardSelection::Combination(key("v1")),
        )
        .expect("단일");
        let ScorecardView::Single(card) = single else {
            panic!("단일 성적표");
        };
        assert_eq!(pending(&card), "보류");
    }

    #[test]
    fn empty_unknown_and_failed_reads_are_distinct() {
        assert_eq!(
            scorecard(Ok(Vec::new()), ScorecardSelection::All),
            Ok(ScorecardView::NoObservations)
        );
        assert_eq!(
            scorecard(
                Ok(observations(2)),
                ScorecardSelection::Combination(key("v9"))
            ),
            Ok(ScorecardView::UnknownCombination)
        );
        let failure = SystemPortError::new(SystemPort::Repository, SystemFailure::InvalidData);
        assert_eq!(
            scorecard(Err(failure), ScorecardSelection::All),
            Err(GetScorecardError::Storage(failure))
        );
    }
}
