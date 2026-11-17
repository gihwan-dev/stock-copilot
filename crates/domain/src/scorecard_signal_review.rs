#[cfg(test)]
pub(crate) mod tests {
    use crate::signal_return_contract::tests::{instant, key};
    use crate::signal_return_scenarios::tests::{bar, benchmark, calendar, task};
    use crate::{
        AcquisitionPrice, AdjustmentBasis, AiProbability, AiProbabilityInput, AllocationInstrument,
        Amount, BootstrapSeed, BuySignalRecord, BuyingPower, Calibration, CalibrationEstimateId,
        CandlePeriod, CollectedItem, CommissionRateFact, CommissionValidity, CostEligibleTask,
        CostRate, Currency, EdgeDirection, EstimateCostPeriod, EstimateTask, Evidence, EvidenceRef,
        EvidenceUrl, ExcludedSecurityType, Exclusions, Holding, HoldingObservation, Horizon,
        HoursPermission, InstrumentClassification, InvestorProfile, Margin, Market, MarketCalendar,
        Money, NormalizedUrl, NumericInput, OutcomeAdjustment, Policy, PolicySecurityFacts,
        Portfolio, PortfolioValuation, Quantity, RaisedSignal, Ratio, ReportedHolding,
        RiskAppetite, ScorecardObservation, ScorecardPromptId, ScorecardReturnObservation,
        ScorecardSignalDirection, ScorecardSignalIdentity, ScorecardSignalRecord, SectorAssignment,
        SectorAssignments, SectorId, SignalContext, SignalDecision, SignalEstimate,
        SignalReturnBasis, SignalReturnCost, SignalReturnError, SignalSourceLedger,
        SignalSubmission, SignalTaskKey, SignalTiming, Timestamp, TradeCost, TradeTaxFact,
        UntrustedText, VirtualExitSignal, WeightCaps, calibrate_signal_submission, scorecard_edge,
        scorecard_input, signal_return, signal_return_contract, signal_return_cost, virtual_exit,
    };
    use rust_decimal::Decimal;

    pub(crate) fn costs(
        source: &EstimateTask,
        end: Timestamp,
        fee: &str,
    ) -> (TradeCost, SignalReturnCost) {
        let commission = CostRate::try_new(
            Ratio::parse(&NumericInput::try_new(fee).expect("요율 입력")).expect("요율"),
        )
        .expect("수수료");
        let zero = CostRate::try_new(Ratio::new(Decimal::ZERO)).expect("영");
        let fact = Some(CommissionRateFact::from_commissions_result(
            Market::Kr,
            commission,
            CommissionValidity::try_new(None, None).expect("유효"),
        ));
        let tax = Some(TradeTaxFact::from_schedule_result(Market::Kr, zero, zero));
        let period = EstimateCostPeriod::from_calendar_result(source, end).expect("기간");
        (
            TradeCost::from_facts(fact.clone(), tax.clone(), period.clone()).expect("거래 비용"),
            signal_return_cost(fact, tax, period).expect("수익 비용"),
        )
    }
    fn valuation(source: &EstimateTask) -> PortfolioValuation {
        let now = source.created_at();
        let instrument = source.reference().instrument();
        let money = |value| {
            Money::parse(
                &NumericInput::try_new(value).expect("금액 입력"),
                Currency::Krw,
            )
            .expect("금액")
        };
        let holding = Holding::new(
            instrument.clone(),
            Quantity::parse(&NumericInput::try_new("1").expect("수량")).expect("수량"),
            AcquisitionPrice::try_new(money("1")).expect("원가"),
            HoldingObservation::new(
                None,
                Some(
                    ReportedHolding::new(money("10"), money("0"), Ratio::new(Decimal::ZERO))
                        .expect("보유 평가"),
                ),
                now,
            ),
        )
        .expect("보유");
        Portfolio::new(
            vec![holding],
            BuyingPower::new(money("100"), now),
            BuyingPower::new(Money::new(Amount::new(Decimal::ZERO), Currency::Usd), now),
        )
        .expect("포트폴리오")
        .evaluate(None)
    }
    pub(crate) fn signal_facts(
        source: &EstimateTask,
        shift: i64,
        direction: ScorecardSignalDirection,
        fee: &str,
    ) -> (SignalEstimate, CollectedItem) {
        let now = source.created_at();
        let end = instant(67_i64.checked_add(shift).expect("만기"), 4);
        let (cost, _) = costs(source, end, fee);
        let eligible = CostEligibleTask::assess(source.clone(), &cost).expect("적격");
        let item = CollectedItem::from_web(
            NormalizedUrl::try_new("https://example.com/a").expect("URL"),
            UntrustedText::try_new("근거").expect("제목"),
            None,
        );
        let evidence = Evidence::try_new(vec![EvidenceRef::Web(
            EvidenceUrl::try_new(item.url().clone()).expect("URL"),
        )])
        .expect("근거");
        let percent = match direction {
            ScorecardSignalDirection::Buy => "99",
            ScorecardSignalDirection::SellAvoidance => "1",
        };
        let submission = SignalSubmission::create(
            SignalTaskKey::from_task(&eligible),
            key(),
            AiProbability::parse(&AiProbabilityInput::try_new(percent).expect("확률"))
                .expect("확률"),
            Some(evidence),
            now,
        )
        .expect("제출");
        let head = crate::CalibrationLedgerHead::new(key(), crate::CalibrationPosition::new(0));
        let snapshot = crate::CalibrationSnapshot::try_new(
            Calibration::try_new(key(), &[]).expect("보정"),
            head.clone(),
        )
        .expect("완전한 표");
        let submitted =
            calibrate_signal_submission(Some(&snapshot), &head, submission).expect("보정");
        let facts = SignalEstimate::create(
            &eligible,
            &submitted,
            Margin::try_new(Ratio::new(Decimal::new(10, 2))).expect("여유"),
        )
        .expect("신호 사실");
        (facts, item)
    }
    pub(crate) fn raised(
        source: &EstimateTask,
        facts: &SignalEstimate,
        item: CollectedItem,
        calendars: &[MarketCalendar],
    ) -> RaisedSignal {
        let now = source.created_at();
        let instrument = source.reference().instrument();
        let valuation = valuation(source);
        let policy = Policy::new(
            InvestorProfile::try_new(
                Horizon::default(),
                HoursPermission::default(),
                RiskAppetite::Neutral,
                Exclusions::try_new(vec![], vec![]).expect("제외"),
                WeightCaps::default(),
            )
            .expect("정책"),
        );
        let assignments = SectorAssignments::try_new(vec![SectorAssignment::new(
            instrument,
            InstrumentClassification::Sector(SectorId::try_new("Tech").expect("섹터")),
        )])
        .expect("분류");
        let security = PolicySecurityFacts::from_master_lookup_results(
            std::iter::once((
                AllocationInstrument::new(instrument),
                ExcludedSecurityType::Stock,
            ))
            .collect(),
        );
        let items = [item];
        let sources = SignalSourceLedger::new(&items, &[], &[]);
        let context = SignalContext::new(
            Some(&valuation),
            &policy,
            (&assignments, &security),
            &sources,
            crate::SignalAssessmentFacts::new(
                SignalTiming::new(now, calendars),
                Some(
                    crate::PriceObservation::from_quote_parts(
                        facts.task().reference().instrument().clone(),
                        facts.task().reference().price(),
                        now,
                    )
                    .expect("대상 시세"),
                ),
                Some(crate::CalibrationLedgerHead::new(
                    facts.submission().calibration_key().clone(),
                    crate::CalibrationPosition::new(0),
                )),
            ),
        );
        SignalDecision::assess(facts, &context)
            .into_signal()
            .expect("방향 신호")
    }
    pub(crate) fn record(
        shift: i64,
        direction: ScorecardSignalDirection,
        fee: &str,
    ) -> ScorecardSignalRecord {
        let source = task(shift);
        let now = source.created_at();
        let calendars = (61_i64..=67)
            .map(|day| calendar(day.checked_add(shift).expect("날짜")))
            .collect::<Vec<_>>();
        let (facts, item) = signal_facts(&source, shift, direction, fee);
        let signal = raised(&source, &facts, item, &calendars);
        let identity = ScorecardSignalIdentity::new(
            CalibrationEstimateId::try_new(&format!("sealed-{shift}")).expect("ID"),
            ScorecardPromptId::try_new("prompt").expect("프롬프트"),
        );
        match signal {
            RaisedSignal::Buy(buy) => {
                assert_eq!(direction, ScorecardSignalDirection::Buy);
                let history = BuySignalRecord::from_signal(
                    crate::BuySignalOccurrenceId::try_new("history").expect("발생 ID"),
                    &buy,
                    now,
                    &calendars,
                )
                .expect("매수 이력");
                ScorecardSignalRecord::from_buy(identity, &buy, source.reference(), &history)
                    .expect("봉인 매수")
            }
            RaisedSignal::Sell(sell) => {
                assert_eq!(direction, ScorecardSignalDirection::SellAvoidance);
                ScorecardSignalRecord::from_sell(
                    identity,
                    &sell,
                    source.reference(),
                    now,
                    &calendars,
                )
                .expect("봉인 매도")
            }
        }
    }
    pub(crate) fn sample(
        shift: i64,
        direction: ScorecardSignalDirection,
        fee: &str,
        close: &str,
        index: &str,
    ) -> (ScorecardObservation, ScorecardReturnObservation) {
        let record = record(shift, direction, fee);
        let signal = VirtualExitSignal::from_record(&record).expect("기록된 경계");
        let end = 67_i64.checked_add(shift).expect("만기일");
        let now = instant(end, 4);
        let basis = AdjustmentBasis::new(now);
        let calendars = (61_i64..=67)
            .map(|day| calendar(day.checked_add(shift).expect("날짜")))
            .collect::<Vec<_>>();
        let mut bars = (62_i64..=67)
            .map(|day| {
                bar(
                    day.checked_add(shift).expect("날짜"),
                    0,
                    CandlePeriod::Daily,
                    close,
                    basis,
                )
            })
            .collect::<Vec<_>>();
        bars.extend((1_i64..=4).map(|minute| {
            bar(
                62_i64.checked_add(shift).expect("첫날"),
                minute,
                CandlePeriod::Minute,
                close,
                basis,
            )
        }));
        let adjustment = OutcomeAdjustment::from_actions(record.task(), basis, &[]).expect("조정");
        let exit = virtual_exit(&signal, &calendars, &adjustment, &bars, now);
        if direction == ScorecardSignalDirection::SellAvoidance {
            assert_eq!(
                signal_return_contract::tests::exit_summary(&exit).0,
                "maturity"
            );
            assert_eq!(signal.target(), None);
            assert_eq!(signal.stop(), None);
        }
        let (_, cost) = costs(record.task(), now, fee);
        let value = signal_return(record.task(), exit, &cost, &benchmark(shift, index, basis))
            .expect("성과");
        assert_eq!(value.estimate_id(), Some(record.estimate_id()));
        assert_eq!(value.task_id(), &record.task_id());
        assert_eq!(value.direction(), direction);
        let observed = record
            .observation(&calendars, &adjustment, &bars, now)
            .expect("관측");
        let paired =
            ScorecardReturnObservation::try_new(observed.clone(), value).expect("동일 식별자 결합");
        (observed, paired)
    }
    #[test]
    fn review_sealed_public_fee_path_retains_28_place_excess_and_superiority() {
        let pairs = (0_i64..30)
            .map(|index| {
                sample(
                    index.checked_mul(7).expect("간격"),
                    ScorecardSignalDirection::Buy,
                    "0.00015",
                    "100",
                    "70",
                )
            })
            .collect::<Vec<_>>();
        let observations = pairs
            .iter()
            .map(|(record, _)| record.clone())
            .collect::<Vec<_>>();
        let returns = pairs
            .into_iter()
            .map(|(_, value)| value)
            .collect::<Vec<_>>();
        let edge = scorecard_edge(
            &key(),
            &observations,
            &returns,
            instant(500, 4),
            BootstrapSeed::new(42),
        )
        .expect("공개 성적표");
        let interval = edge.confidence().clone().into_ready().expect("신뢰구간");
        assert_eq!(interval.direction(), EdgeDirection::Superior);
        assert_eq!(interval.lower().to_string(), "0.299700044993");
        assert_eq!(interval.upper(), interval.lower());
        assert_eq!(edge.approximate_samples().as_usize(), 0);
    }
    #[test]
    fn review_mixed_stop_and_maturity_returns_cannot_select_only_winners() {
        let pairs = (0_i64..60)
            .map(|index| {
                sample(
                    index.checked_mul(7).expect("간격"),
                    ScorecardSignalDirection::Buy,
                    "0",
                    if index < 30 { "101" } else { "80" },
                    "100",
                )
            })
            .collect::<Vec<_>>();
        let observations = pairs
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
                .expect("만기")
                .value()
                .excess()
                .expect("비교")
                .to_string(),
            "0.010000"
        );
        assert_eq!(
            returns
                .last()
                .expect("손절")
                .value()
                .excess()
                .expect("비교")
                .to_string(),
            "-0.200000"
        );
        let edge = scorecard_edge(
            &key(),
            &observations,
            &returns,
            instant(600, 4),
            BootstrapSeed::new(42),
        )
        .expect("전체 성적표");
        let interval = edge.confidence().clone().into_ready().expect("신뢰구간");
        assert_eq!(interval.direction(), EdgeDirection::Inferior);
        assert_eq!(edge.unavailable().as_usize(), 0);
        assert_eq!(edge.approximate_samples().as_usize(), 30);
        assert_eq!(interval.lower().to_string(), "-0.119500000000");
        assert_eq!(interval.upper().to_string(), "-0.070500000000");
    }
    #[test]
    fn review_sell_avoidance_negates_gross_excess_without_transaction_costs() {
        for (close, expected) in [("80", "0.200000"), ("120", "-0.200000")] {
            let (_, paired) = sample(
                0,
                ScorecardSignalDirection::SellAvoidance,
                "0.00015",
                close,
                "100",
            );
            assert_eq!(
                paired.value().basis(),
                SignalReturnBasis::GrossSellAvoidance
            );
            assert_eq!(paired.value().net().to_string(), expected);
            assert_eq!(paired.value().excess().expect("초과").to_string(), expected);
        }
    }
    #[test]
    fn review_other_estimate_id_cannot_attach_even_with_same_task_and_period() {
        let (observed, paired) = sample(0, ScorecardSignalDirection::Buy, "0", "100", "100");
        let different = ScorecardObservation::try_new(
            CalibrationEstimateId::try_new("other").expect("ID"),
            key(),
            ScorecardPromptId::try_new("prompt").expect("프롬프트"),
            AiProbability::parse(&AiProbabilityInput::try_new("99").expect("확률")).expect("확률"),
            scorecard_input::scorecard_return_identity(&observed)
                .2
                .clone(),
        )
        .expect("다른 관측");
        let identified_different =
            scorecard_input::identified_scorecard(different, paired.value().task_id().clone());
        assert_eq!(
            ScorecardReturnObservation::try_new(identified_different, paired.value().clone()),
            Err(SignalReturnError::ConflictingReturn)
        );
    }
}
