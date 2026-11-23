#[cfg(test)]
mod tests {
    use domain::{
        AdjustmentBasis, Candle, CandleObservation, CandlePeriod, CandleStatus, CandleTime,
        CommonShare, Currency, DisplayInstrumentName, EstimateContext, EstimateRestoreError,
        EstimateRestoreEvidence, EstimateTask, Horizon, HorizonSelection, HoursPermission,
        HoursPermissionState, InstrumentId, ListingStatus, MasterCode, MasterInstrument,
        MasterInstrumentRow, MasterSecurityType, Nanosecond, NumericInput, OffsetSeconds, Ohlcv,
        OrderSide, Price, PriceAdjustment, Quantity, RiskAppetite, Symbol, Timestamp, UnixSeconds,
        Volatility,
    };
    use proptest::{
        sample,
        test_runner::{Config, TestRunner},
    };

    fn at(day: i64, offset: i32) -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(
                1_767_225_600_i64
                    .checked_add(day.checked_mul(86_400).expect("일"))
                    .expect("시각"),
            ),
            Nanosecond::new(0),
            OffsetSeconds::new(offset),
        )
        .expect("시각")
    }
    fn price(text: &str) -> Price {
        Price::parse(&NumericInput::try_new(text).expect("입력"), Currency::Usd).expect("가격")
    }
    fn row() -> MasterInstrumentRow {
        MasterInstrumentRow {
            listing_market: MasterCode::try_new("NASDAQ").expect("시장"),
            symbol: Symbol::try_new("EXAMPLE").expect("심볼"),
            instrument_id: InstrumentId::try_new("sample").expect("식별자"),
            name: DisplayInstrumentName::try_new("가상 회사").expect("이름"),
            security_type: MasterSecurityType::from_code(
                MasterCode::try_new("STOCK").expect("유형"),
            ),
            currency: Currency::Usd,
            listing_status: ListingStatus::Active,
            common_share: CommonShare::Common,
            korean_detail: None,
            observed_at: at(61, -18_000),
        }
    }
    fn candles() -> Vec<Candle> {
        (0_i64..61)
            .map(|day| {
                let value = if day % 2 == 0 {
                    price("100")
                } else {
                    price("103")
                };
                Candle::try_new(
                    InstrumentId::try_new("sample").expect("식별자"),
                    CandleTime::try_new(at(day, 0), CandlePeriod::Daily).expect("일봉"),
                    PriceAdjustment::Adjusted(AdjustmentBasis::new(at(70, 0))),
                    Ohlcv::try_new(
                        value,
                        value,
                        value,
                        value,
                        Quantity::parse(&NumericInput::try_new("3").expect("입력")).expect("수량"),
                    )
                    .expect("봉 가격"),
                    CandleObservation::new(at(70, 0), CandleStatus::Finalized),
                )
                .expect("봉")
            })
            .collect()
    }
    fn evidence(selection: HorizonSelection, appetite: RiskAppetite) -> EstimateRestoreEvidence {
        EstimateRestoreEvidence {
            master: row(),
            side: OrderSide::Buy,
            candles: candles(),
            reference_price: price("104"),
            reference_at: at(61, -18_000),
            task_at: at(61, -18_000),
            horizon: Horizon::new(selection),
            permission: HoursPermissionState::Disabled,
            appetite,
        }
    }
    fn original(input: &EstimateRestoreEvidence, master: &MasterInstrument) -> EstimateTask {
        EstimateTask::create(
            EstimateContext::new(
                master
                    .price_observation(input.reference_price, input.reference_at)
                    .expect("기준 시세"),
                input.task_at,
            ),
            Volatility::calculate(&input.candles).expect("변동성"),
            input.horizon,
            HoursPermission::default(),
            input.appetite,
        )
        .expect("원래 과제")
    }
    fn prop_candles_for(instrument_id: &str, low: Price, high: Price) -> Vec<Candle> {
        (0_i64..61_i64)
            .map(|day| {
                let value = if day.checked_rem(2_i64) == Some(0_i64) {
                    low
                } else {
                    high
                };
                Candle::try_new(
                    InstrumentId::try_new(instrument_id).expect("식별자"),
                    CandleTime::try_new(at(day, 0), CandlePeriod::Daily).expect("일봉"),
                    PriceAdjustment::Adjusted(AdjustmentBasis::new(at(70, 0))),
                    Ohlcv::try_new(
                        value,
                        value,
                        value,
                        value,
                        Quantity::parse(&NumericInput::try_new("3").expect("입력")).expect("수량"),
                    )
                    .expect("봉"),
                    CandleObservation::new(at(70, 0), CandleStatus::Finalized),
                )
                .expect("봉")
            })
            .collect()
    }
    fn prop_master_for(market: domain::KnownMarket) -> (MasterInstrumentRow, Currency, i32) {
        let (currency, offset, listing, symbol, detail) = match market {
            domain::KnownMarket::Kr => (
                Currency::Krw,
                32_400_i32,
                "KOSPI",
                "005930",
                Some(domain::KoreanMasterDetail {
                    liquidation: domain::LiquidationTrading::Regular,
                    krx: domain::TradingSuspension::Clear,
                    nxt: domain::NxtTrading::Unsupported,
                }),
            ),
            domain::KnownMarket::Us => (Currency::Usd, -18_000_i32, "NASDAQ", "EXAMPLE", None),
        };
        let row = MasterInstrumentRow {
            listing_market: MasterCode::try_new(listing).expect("시장"),
            symbol: Symbol::try_new(symbol).expect("심볼"),
            instrument_id: InstrumentId::try_new("prop-est").expect("식별자"),
            name: DisplayInstrumentName::try_new("속성 종목").expect("이름"),
            security_type: MasterSecurityType::from_code(
                MasterCode::try_new("STOCK").expect("유형"),
            ),
            currency,
            listing_status: ListingStatus::Active,
            common_share: CommonShare::Common,
            korean_detail: detail,
            observed_at: at(61, offset),
        };
        (row, currency, offset)
    }
    #[derive(Clone, Copy)]
    struct EstimateCase {
        market: domain::KnownMarket,
        side: OrderSide,
        selection: HorizonSelection,
        appetite: RiskAppetite,
        base: u32,
        delta: u32,
    }
    fn check_estimate_property(case: EstimateCase) {
        let (master_row, currency, offset) = prop_master_for(case.market);
        let master = MasterInstrument::try_new(master_row.clone()).expect("조회");
        let mk_price = |val: u32| {
            Price::parse(
                &NumericInput::try_new(&val.to_string()).expect("입력"),
                currency,
            )
            .expect("가격")
        };
        let step = case
            .base
            .checked_mul(case.delta)
            .expect("변동폭")
            .checked_div(10_000)
            .expect("비율")
            .max(1);
        let low = mk_price(case.base);
        let high = mk_price(case.base.checked_add(step).expect("고가"));
        let ref_price = mk_price(
            case.base
                .checked_add(step.checked_div(2).expect("중간").max(1))
                .expect("기준"),
        );
        let input = EstimateRestoreEvidence {
            master: master_row,
            side: case.side,
            candles: prop_candles_for("prop-est", low, high),
            reference_price: ref_price,
            reference_at: at(61, offset),
            task_at: at(61, offset),
            horizon: Horizon::new(case.selection),
            permission: HoursPermissionState::Disabled,
            appetite: case.appetite,
        };
        let task = EstimateTask::create(
            EstimateContext::new(
                master
                    .price_observation_for(case.side, ref_price, at(61, offset))
                    .expect("시세"),
                at(61, offset),
            ),
            Volatility::calculate(&input.candles).expect("변동성"),
            input.horizon,
            HoursPermission::default(),
            case.appetite,
        )
        .expect("원래 과제");
        let saved = input.capture(&task, &master).expect("근거 수집");
        let historical = saved.restore_history(&master).expect("과거 판정");
        assert_eq!(historical.reference_price(), task.reference().price());
        assert_eq!(historical.created_at(), task.created_at());
        assert_eq!(historical.horizon(), task.horizon());
        assert_eq!(historical.target(), task.target());
        assert_eq!(historical.stop(), task.stop());
        assert_eq!(historical.base_probability(), task.base_probability());
    }
    #[test]
    fn replay_preserves_independently_created_task_facts() {
        let mut runner = TestRunner::new(Config::with_cases(testkit::property_cases()));
        runner
            .run(
                &(
                    sample::select(vec![domain::KnownMarket::Kr, domain::KnownMarket::Us]),
                    sample::select(vec![OrderSide::Buy, OrderSide::Sell]),
                    sample::select(vec![
                        HorizonSelection::FewDays,
                        HorizonSelection::WeeksToMonths,
                        HorizonSelection::HalfYearOrMore,
                    ]),
                    sample::select(vec![
                        RiskAppetite::Conservative,
                        RiskAppetite::Neutral,
                        RiskAppetite::Aggressive,
                    ]),
                    200_u32..=2000_u32,
                    50_u32..=400_u32,
                ),
                |(market, side, selection, appetite, base, delta)| {
                    check_estimate_property(EstimateCase {
                        market,
                        side,
                        selection,
                        appetite,
                        base,
                        delta,
                    });
                    Ok(())
                },
            )
            .expect("왕복");
    }
    #[test]
    fn replay_rejects_missing_or_mismatched_evidence_and_current_identity() {
        let input = evidence(HorizonSelection::FewDays, RiskAppetite::Neutral);
        let master = MasterInstrument::try_new(row()).expect("조회");
        let original = original(&input, &master);
        let saved = input.capture(&original, &master).expect("수집");
        let mut wrong_master = row();
        wrong_master.instrument_id = InstrumentId::try_new("different").expect("식별자");
        assert_eq!(
            saved.restore_history(&MasterInstrument::try_new(wrong_master).expect("조회")),
            Err(EstimateRestoreError::CurrentInstrumentMismatch)
        );
        let mut missing = saved.clone();
        missing.evidence.candles.clear();
        missing.restore_history(&master).expect_err("손상 거부");
        let mut wrong_price = saved.clone();
        wrong_price.evidence.reference_price = price("108");
        assert_eq!(
            wrong_price.restore_history(&master),
            Err(EstimateRestoreError::ChangedResult)
        );
        let mut wrong_horizon = saved.clone();
        wrong_horizon.evidence.horizon = Horizon::new(HorizonSelection::HalfYearOrMore);
        assert_eq!(
            wrong_horizon.restore_history(&master),
            Err(EstimateRestoreError::ChangedResult)
        );
        let mut wrong_symbol = saved;
        wrong_symbol.evidence.master.symbol = Symbol::try_new("OTHER").expect("심볼");
        assert_eq!(
            wrong_symbol.restore_history(&master),
            Err(EstimateRestoreError::CurrentInstrumentMismatch)
        );
    }

    fn draft_fixture() -> (
        domain::Proposal<domain::Draft>,
        domain::StoredEstimateEvidence,
        MasterInstrument,
        domain::ProposalLifetime,
        domain::PriceObservation,
    ) {
        let evidence = evidence(HorizonSelection::FewDays, RiskAppetite::Neutral);
        let master = MasterInstrument::try_new(row()).expect("현재 조회");
        let task = original(&evidence, &master);
        let stored = evidence.capture(&task, &master).expect("과제 근거");
        let card = master
            .price_observation(price("105"), at(61, -18_000))
            .expect("카드 시세");
        let range =
            domain::ProposalPriceRange::from_estimate_task(task.range_facts(), card.clone())
                .expect("범위");
        let id =
            domain::ProposalId::new(domain::ClientOrderId::try_new("restore-1").expect("주문 ID"));
        let intent = domain::OrderIntent::new(
            domain::AccountId::new(418),
            master.order_instrument().expect("종목"),
            id.as_client_order_id().clone(),
            domain::OrderTerms::UsMarketBuy {
                quantity: domain::WholeOrderQuantity::try_new(
                    Quantity::parse(&NumericInput::try_new("7").expect("입력")).expect("수량"),
                )
                .expect("정수 수량"),
            },
        )
        .expect("주문 의도");
        let lifetime =
            domain::ProposalLifetime::try_new(domain::SpanSeconds::new(379)).expect("기간");
        let draft =
            domain::Proposal::new(id, intent, at(61, -18_000), range, lifetime).expect("초안");
        (draft, stored, master, lifetime, card)
    }
    #[test]
    fn proposal_roundtrip_preserves_stored_expiry_and_report() {
        let (draft, estimate, master, lifetime, card) = draft_fixture();
        let saved =
            domain::DraftRestoreEvidence::capture(&draft, estimate, &master, lifetime, &card)
                .expect("초안 근거");
        let replayed = saved.restore(&master).expect("복원");
        assert_eq!(replayed.id(), draft.id());
        assert_eq!(replayed.intent(), draft.intent());
        assert_eq!(replayed.created_at(), draft.created_at());
        assert_eq!(replayed.expires_at(), draft.expires_at());
        assert_eq!(replayed.price_range(), draft.price_range());
        for value in ["50", "104", "105", "110", "150"] {
            let current = master
                .price_observation(price(value), at(61, -18_000))
                .expect("시세");
            assert_eq!(
                replayed.report(at(61, -18_000), &current),
                draft.report(at(61, -18_000), &current)
            );
        }
        saved.restore(&master).expect("시계 없이 복원");
    }
    #[test]
    fn proposal_replay_rejects_changed_expiry_or_range_evidence() {
        let (draft, estimate, master, lifetime, card) = draft_fixture();
        let saved =
            domain::DraftRestoreEvidence::capture(&draft, estimate, &master, lifetime, &card)
                .expect("근거");
        let mut wrong_expiry = saved.clone();
        wrong_expiry.expires_at = at(62, -18_000);
        assert_eq!(
            wrong_expiry.restore(&master).expect_err("만료 불일치"),
            domain::DraftRestoreError::ExpiryMismatch
        );
        let mut wrong_reference = saved.clone();
        wrong_reference.estimate.evidence.reference_price = price("111");
        wrong_reference.restore(&master).expect_err("손상 거부");
        let mut wrong_card = saved;
        wrong_card.card.price = price("109");
        assert_eq!(
            wrong_card.restore(&master).expect_err("시세 불일치"),
            domain::DraftRestoreError::ChangedRange
        );
    }

    fn shifted(day: i64, seconds: u64) -> Timestamp {
        at(day, -18_000)
            .checked_add(
                domain::TimeSpan::try_new(domain::SpanSeconds::new(seconds), Nanosecond::new(0))
                    .expect("기간"),
            )
            .expect("시각")
    }
    fn session(day: i64) -> domain::MarketSession {
        let start = shifted(day, 21_600);
        let end = shifted(day, 25_200);
        domain::MarketSession::try_new(
            at(day, -18_000).offset_calendar_date(),
            domain::SessionInformation::Us {
                day_market: None,
                pre_market: None,
                regular_market: Some(
                    domain::SessionData::try_new(
                        domain::TimeInterval::try_new(start, end).expect("구간"),
                        None,
                    )
                    .expect("장"),
                ),
                after_market: None,
            },
        )
        .expect("세션")
    }
    fn calendars() -> Vec<domain::MarketCalendar> {
        (60_i64..=68)
            .map(|day| {
                domain::MarketCalendar::try_new(
                    session(day.checked_sub(1).expect("전일")),
                    session(day),
                    session(day.checked_add(1).expect("다음날")),
                )
                .expect("달력")
            })
            .collect()
    }
    fn money(text: &str, currency: Currency) -> domain::Money {
        domain::Money::parse(&NumericInput::try_new(text).expect("입력"), currency).expect("금액")
    }
    struct SignalCostFixture {
        master: MasterInstrument,
        stored: domain::StoredEstimateEvidence,
        eligible: domain::CostEligibleTask,
        commission: domain::CommissionRateFact,
        tax: domain::TradeTaxFact,
        maturity: Timestamp,
    }
    struct SignalModelFixture {
        estimate: domain::SignalEstimate,
        snapshot: domain::CalibrationSnapshot,
        key: domain::CalibrationKey,
        head: domain::CalibrationLedgerHead,
        probability: domain::AiProbability,
        source: domain::Evidence,
        item: domain::CollectedItem,
        margin: domain::Margin,
    }
    fn signal_cost_fixture() -> SignalCostFixture {
        let mut input = evidence(HorizonSelection::FewDays, RiskAppetite::Neutral);
        input.master.security_type =
            MasterSecurityType::from_code(MasterCode::try_new("ETF").expect("유형"));
        let master = MasterInstrument::try_new(input.master.clone()).expect("조회");
        let task = original(&input, &master);
        let stored = input.capture(&task, &master).expect("과제 근거");
        let rate = domain::CostRate::try_new(
            domain::Ratio::parse(&NumericInput::try_new("0").expect("입력")).expect("비율"),
        )
        .expect("요율");
        let commission = domain::CommissionRateFact::from_commissions_result(
            domain::Market::Us,
            rate,
            domain::CommissionValidity::try_new(None, None).expect("기간"),
        );
        let tax = domain::TradeTaxFact::from_schedule_result(domain::Market::Us, rate, rate);
        let maturity = shifted(67, 25_200);
        let cost = domain::TradeCost::from_facts(
            Some(commission.clone()),
            Some(tax.clone()),
            domain::EstimateCostPeriod::from_calendar_result(&task, maturity).expect("기간"),
        )
        .expect("비용");
        let eligible = domain::CostEligibleTask::assess(task, &cost).expect("비용 적격");
        SignalCostFixture {
            master,
            stored,
            eligible,
            commission,
            tax,
            maturity,
        }
    }
    fn signal_model_fixture(cost: &SignalCostFixture, percent: &str) -> SignalModelFixture {
        let part = domain::ModelIdentityPart::try_new("model").expect("모델");
        let key = domain::CalibrationKey::new(
            domain::CalibrationModel::new(part.clone(), part.clone(), part),
            Horizon::new(HorizonSelection::FewDays),
        );
        let head =
            domain::CalibrationLedgerHead::new(key.clone(), domain::CalibrationPosition::new(0));
        let snapshot = domain::CalibrationSnapshot::try_new(
            domain::Calibration::try_new(key.clone(), &[]).expect("보정"),
            head.clone(),
        )
        .expect("스냅숏");
        let probability = domain::AiProbability::parse(
            &domain::AiProbabilityInput::try_new(percent).expect("입력"),
        )
        .expect("확률");
        let url = domain::NormalizedUrl::try_new("https://example.com/d56").expect("주소");
        let item = domain::CollectedItem::from_web(
            url.clone(),
            domain::UntrustedText::try_new("출처").expect("제목"),
            None,
        );
        let source = domain::Evidence::try_new(vec![domain::EvidenceRef::Web(
            domain::EvidenceUrl::try_new(url).expect("출처"),
        )])
        .expect("근거");
        let submission = domain::SignalSubmission::create(
            domain::SignalTaskKey::from_task(&cost.eligible),
            key.clone(),
            probability,
            Some(source.clone()),
            at(61, -18_000),
        )
        .expect("제출");
        let submitted =
            domain::calibrate_signal_submission(Some(&snapshot), &head, submission).expect("보정");
        let margin = domain::Margin::for_appetite(RiskAppetite::Neutral);
        let estimate =
            domain::SignalEstimate::create(&cost.eligible, &submitted, margin).expect("판정 입력");
        SignalModelFixture {
            estimate,
            snapshot,
            key,
            head,
            probability,
            source,
            item,
            margin,
        }
    }
    fn signal_portfolio(master: &MasterInstrument, percent: &str) -> domain::Portfolio {
        let instrument = master.order_instrument().expect("종목");
        let holdings = if percent == "1" {
            vec![
                domain::Holding::new(
                    instrument,
                    Quantity::parse(&NumericInput::try_new("4").expect("입력")).expect("수량"),
                    domain::AcquisitionPrice::try_new(money("90", Currency::Usd)).expect("원가"),
                    domain::HoldingObservation::new(Some(price("104")), None, at(61, -18_000)),
                )
                .expect("보유"),
            ]
        } else {
            vec![]
        };
        domain::Portfolio::new(
            holdings,
            domain::BuyingPower::new(money("1000000", Currency::Krw), at(61, -18_000)),
            domain::BuyingPower::new(money("0", Currency::Usd), at(61, -18_000)),
        )
        .expect("포트폴리오")
    }
    fn signal_classifications(
        master: &MasterInstrument,
    ) -> (domain::SectorAssignments, domain::PolicySecurityFacts) {
        let catalog = domain::SectorCatalog::from_response(vec![], at(61, -18_000)).expect("목록");
        let assignments = domain::SectorAssignments::try_new(vec![
            domain::sector_assignment_from_compositions(master, &catalog, &[]).expect("분류"),
        ])
        .expect("분류표");
        let security = domain::MasterInstrumentFacts::try_new(vec![master.clone()])
            .expect("마스터")
            .policy_security_facts();
        (assignments, security)
    }
    fn signal_assessment(
        master: &MasterInstrument,
        model: &SignalModelFixture,
        percent: &str,
    ) -> domain::HistoricalSignalContext {
        let (assignments, security) = signal_classifications(master);
        let target = master
            .price_observation(price("105"), at(61, -18_000))
            .expect("시세");
        domain::HistoricalSignalContext {
            portfolio: Some(
                domain::HistoricalPortfolio::capture(
                    &signal_portfolio(master, percent),
                    std::slice::from_ref(master),
                )
                .expect("포트폴리오 근거"),
            ),
            rate: None,
            policy: domain::Policy::new(domain::InvestorProfile::default()),
            classifications: domain::HistoricalClassifications::capture(&assignments, &security),
            items: vec![model.item.clone()],
            disclosures: vec![],
            tools: vec![],
            calendars: calendars(),
            evaluated_at: at(61, -18_000),
            target: Some(domain::StoredPriceObservation::from_price_observation(
                &target,
            )),
            calibration_head: Some(model.head.clone()),
        }
    }
    fn signal_fixture(
        percent: &str,
    ) -> (
        domain::RaisedSignal,
        domain::SignalRestoreEvidence,
        MasterInstrument,
    ) {
        let cost = signal_cost_fixture();
        let model = signal_model_fixture(&cost, percent);
        let assessment = signal_assessment(&cost.master, &model, percent);
        let valuation = signal_portfolio(&cost.master, percent).evaluate(None);
        let (assignments, security) = signal_classifications(&cost.master);
        let ledger = domain::SignalSourceLedger::new(
            &assessment.items,
            &assessment.disclosures,
            &assessment.tools,
        );
        let target = cost
            .master
            .price_observation(price("105"), at(61, -18_000))
            .expect("시세");
        let facts = domain::SignalAssessmentFacts::new(
            domain::SignalTiming::new(assessment.evaluated_at, &assessment.calendars),
            Some(target),
            Some(model.head),
        );
        let context = domain::SignalContext::new(
            Some(&valuation),
            &assessment.policy,
            (&assignments, &security),
            &ledger,
            facts,
        );
        let signal = domain::SignalDecision::assess(&model.estimate, &context)
            .into_signal()
            .expect("신호");
        let saved = domain::SignalRestoreEvidence {
            estimate: cost.stored,
            commission: cost.commission,
            tax: cost.tax,
            maturity: cost.maturity,
            calibration: model.snapshot,
            calibration_key: model.key,
            probability: model.probability,
            source: Some(model.source),
            submitted_at: at(61, -18_000),
            margin: model.margin,
            assessment,
        };
        (signal, saved, cost.master)
    }
    #[test]
    fn cost_and_classification_evidence_can_be_read_without_unsealing_signals() {
        let (_, evidence, _) = signal_fixture("99");
        let commission = &evidence.commission;
        let validity = commission.validity();
        assert_eq!(
            domain::CommissionValidity::try_new(validity.start(), validity.end()),
            Ok(validity)
        );
        assert_eq!(
            domain::CommissionRateFact::from_commissions_result(
                commission.market().clone(),
                commission.rate(),
                validity
            ),
            *commission
        );
        let tax = &evidence.tax;
        assert_eq!(
            domain::TradeTaxFact::from_schedule_result(
                tax.market().clone(),
                tax.buy_rate(),
                tax.sell_rate()
            ),
            *tax
        );
        assert_eq!(
            domain::Margin::try_new(evidence.margin.ratio()),
            Ok(evidence.margin)
        );
        assert_eq!(evidence.assessment.classifications.sectors.len(), 1);
        assert_eq!(evidence.assessment.classifications.security.len(), 1);
    }
    #[test]
    fn stored_portfolio_facts_require_matching_unique_master_to_rebuild_history() {
        let (signal, mut evidence, master) = signal_fixture("1");
        let historical = evidence.assessment.portfolio.as_ref().expect("포트");
        let facts = historical.facts().clone();
        assert_eq!(
            domain::HistoricalPortfolio::from_saved_facts(facts.clone(), &[]),
            Err(domain::HistoricalPortfolioError::MasterMissingOrAmbiguous)
        );
        assert_eq!(
            domain::HistoricalPortfolio::from_saved_facts(
                facts.clone(),
                &[master.clone(), master.clone()]
            ),
            Err(domain::HistoricalPortfolioError::MasterMissingOrAmbiguous)
        );
        let rebuilt = domain::HistoricalPortfolio::from_saved_facts(
            facts.clone(),
            std::slice::from_ref(&master),
        )
        .expect("새 마스터 조회로 과거 근거 복원");
        assert_eq!(rebuilt.facts(), &facts);
        evidence.assessment.portfolio = Some(rebuilt);
        evidence.capture(&signal, &master).expect("원래 판정");
        let mut invalid = facts;
        invalid.krw.currency = Currency::Usd;
        domain::HistoricalPortfolio::from_saved_facts(invalid, &[master]).expect_err("통화 불일치");
    }
    #[test]
    fn restored_signal_rejects_duplicate_historical_classifications() {
        let (signal, evidence, master) = signal_fixture("99");
        let mut stored = evidence.capture(&signal, &master).expect("근거");
        let duplicate = stored.evidence.assessment.classifications.sectors[0].clone();
        stored
            .evidence
            .assessment
            .classifications
            .sectors
            .push(duplicate);
        assert_eq!(
            stored.restore_history(&master),
            Err(domain::SignalRestoreError::Classification(
                domain::HistoricalClassificationError::DuplicateInstrument
            ))
        );
    }
    fn assert_sell_display(signal: &domain::RaisedSignal, history: &domain::HistoricalSignal) {
        let domain::RaisedSignal::Sell(sell) = signal else {
            return;
        };
        let domain::HistoricalSignalDirection::Sell {
            quantity,
            observed_at,
        } = history.display_facts().direction
        else {
            panic!("매도 표시");
        };
        assert_eq!(
            quantity,
            domain::HistoricalSignalQuantity::new(sell.held_quantity())
        );
        assert_eq!(
            quantity.display_text().as_str(),
            sell.held_quantity().as_decimal().to_string()
        );
        assert_eq!(observed_at, sell.observed_at());
    }
    #[test]
    fn signal_history_replays_buy_and_sell_facts_without_exposing_live_signal() {
        for percent in ["99", "1"] {
            let (signal, evidence, master) = signal_fixture(percent);
            let saved = evidence.capture(&signal, &master).expect("신호 근거");
            let history = saved.restore_history(&master).expect("과거 신호");
            assert_eq!(history.display_facts(), saved.expected);
            let (side, estimate) = match &signal {
                domain::RaisedSignal::Buy(value) => (OrderSide::Buy, value.facts()),
                domain::RaisedSignal::Sell(value) => (OrderSide::Sell, value.facts()),
            };
            assert_eq!(history.side(), side);
            assert_sell_display(&signal, &history);
            let decision = match &signal {
                domain::RaisedSignal::Buy(value) => domain::SignalDecision::Buy(value.clone()),
                domain::RaisedSignal::Sell(value) => domain::SignalDecision::Sell(value.clone()),
            };
            assert_eq!(
                domain::signal_transition_from_history(Some(&history), &decision),
                domain::signal_transition(Some(&signal), &decision)
            );
            assert_eq!(
                history.display_probability().expect("표시"),
                estimate.display_probability().expect("원래 표시")
            );
            for kind in [
                domain::ThresholdKind::Buy,
                domain::ThresholdKind::Sell,
                domain::ThresholdKind::Breakeven,
            ] {
                assert_eq!(
                    history.threshold_display(kind).expect("표시"),
                    estimate.thresholds().display(kind).expect("원래 표시")
                );
            }
        }
    }
    #[test]
    fn historical_signal_values_preserve_display_numbers_and_currency() {
        let quantity = domain::HistoricalSignalQuantity::new(
            Quantity::parse(&NumericInput::try_new("17.125").expect("입력")).expect("수량"),
        );
        let limit = domain::HistoricalSignalLimit::new(money("1357.89", Currency::Krw));
        assert_eq!(quantity.display_text().as_str(), "17.125");
        assert_eq!(limit.display_text().as_str(), "1357.89");
        assert_eq!(limit.currency(), Currency::Krw);
        assert!(!format!("{quantity:?}").contains("17.125"));
        assert!(!format!("{limit:?}").contains("1357.89"));
        assert!(!format!("{:?}", quantity.display_text()).contains("17.125"));
    }
    #[test]
    fn signal_history_rejects_missing_source_or_changed_calibration() {
        let (signal, evidence, master) = signal_fixture("99");
        let saved = evidence.capture(&signal, &master).expect("근거");
        let mut missing = saved.clone();
        missing.evidence.assessment.items.clear();
        missing.restore_history(&master).expect_err("손상 거부");
        let mut changed = saved;
        changed.evidence.probability =
            domain::AiProbability::parse(&domain::AiProbabilityInput::try_new("1").expect("입력"))
                .expect("확률");
        changed.restore_history(&master).expect_err("손상 거부");
    }

    fn with_offset(value: Timestamp, offset: i32) -> Timestamp {
        Timestamp::from_parts(
            value.utc_seconds(),
            value.nanosecond(),
            OffsetSeconds::new(offset),
        )
        .expect("오프셋")
    }
    #[test]
    fn capture_rejects_original_estimate_offset_mismatch() {
        let input = evidence(HorizonSelection::FewDays, RiskAppetite::Neutral);
        let master = MasterInstrument::try_new(row()).expect("마스터");
        let original_task = original(&input, &master);
        for change in [
            (with_offset(input.task_at, -14_400), input.reference_at),
            (input.task_at, with_offset(input.reference_at, -14_400)),
        ] {
            let mut altered = input.clone();
            altered.task_at = change.0;
            altered.reference_at = change.1;
            assert_eq!(
                altered
                    .capture(&original_task, &master)
                    .expect_err("시각 오프셋 불일치"),
                EstimateRestoreError::ChangedResult
            );
        }
    }
    #[test]
    fn capture_rejects_original_adjustment_basis_offset_mismatch() {
        let input = evidence(HorizonSelection::FewDays, RiskAppetite::Neutral);
        let master = MasterInstrument::try_new(row()).expect("마스터");
        let original_task = original(&input, &master);
        let mut altered = input.clone();
        altered.candles = input
            .candles
            .iter()
            .map(|candle| {
                let PriceAdjustment::Adjusted(basis) = candle.adjustment() else {
                    panic!("수정 봉");
                };
                Candle::try_new(
                    candle.instrument().clone(),
                    candle.time(),
                    PriceAdjustment::Adjusted(AdjustmentBasis::new(with_offset(
                        *basis.as_timestamp(),
                        -14_400,
                    ))),
                    candle.values(),
                    CandleObservation::new(candle.collected_at(), candle.status()),
                )
                .expect("봉")
            })
            .collect();
        assert_eq!(
            altered
                .capture(&original_task, &master)
                .expect_err("수정 기준 오프셋"),
            EstimateRestoreError::ChangedResult
        );
    }
    #[test]
    fn saved_estimate_display_offsets_match_replayed_task() {
        let input = evidence(HorizonSelection::FewDays, RiskAppetite::Neutral);
        let master = MasterInstrument::try_new(row()).expect("마스터");
        let saved = input
            .capture(&original(&input, &master), &master)
            .expect("과제 근거");
        let mut reference = saved.clone();
        reference.expected.reference_at = with_offset(reference.expected.reference_at, -14_400);
        assert_eq!(
            reference
                .restore_history(&master)
                .expect_err("기준 표시 오프셋"),
            EstimateRestoreError::ChangedResult
        );
        let mut created = saved;
        created.expected.created_at = with_offset(created.expected.created_at, -14_400);
        assert_eq!(
            created
                .restore_history(&master)
                .expect_err("생성 표시 오프셋"),
            EstimateRestoreError::ChangedResult
        );
    }
    #[test]
    fn capture_rejects_original_signal_offset_mismatch() {
        let (signal, input, master) = signal_fixture("1");
        let mut altered_submission = input.clone();
        altered_submission.submitted_at = with_offset(input.submitted_at, -14_400);
        assert_eq!(
            altered_submission
                .capture(&signal, &master)
                .expect_err("제출 오프셋"),
            domain::SignalRestoreError::ChangedSignal
        );
        let mut altered_task = input.clone();
        let task_at = with_offset(altered_task.estimate.evidence.task_at, -14_400);
        altered_task.estimate.evidence.task_at = task_at;
        altered_task.estimate.original.task_at = task_at;
        altered_task.estimate.expected.created_at = task_at;
        assert_eq!(
            altered_task
                .capture(&signal, &master)
                .expect_err("신호 과제 오프셋"),
            domain::SignalRestoreError::ChangedSignal
        );
        let mut altered_portfolio = input;
        let portfolio = altered_portfolio
            .assessment
            .portfolio
            .as_ref()
            .expect("포트");
        let mut facts = portfolio.facts().clone();
        facts.holdings[0].observed_at = with_offset(facts.holdings[0].observed_at, -14_400);
        altered_portfolio.assessment.portfolio = Some(
            domain::HistoricalPortfolio::from_saved_facts(facts, std::slice::from_ref(&master))
                .expect("과거 포트폴리오"),
        );
        assert_eq!(
            altered_portfolio
                .capture(&signal, &master)
                .expect_err("매도 표시 오프셋"),
            domain::SignalRestoreError::ChangedSignal
        );
    }
    #[test]
    fn stored_offsets_are_checked_even_when_the_instant_is_unchanged() {
        let (draft, estimate, master, lifetime, card) = draft_fixture();
        let saved =
            domain::DraftRestoreEvidence::capture(&draft, estimate, &master, lifetime, &card)
                .expect("근거");
        let mut reference = saved.clone();
        reference.estimate.evidence.reference_at =
            with_offset(reference.estimate.evidence.reference_at, -14_400);
        assert_eq!(
            reference.restore(&master).expect_err("기준 오프셋"),
            domain::DraftRestoreError::Estimate(EstimateRestoreError::ChangedEvidence)
        );
        let mut expiry = saved.clone();
        expiry.expires_at = with_offset(expiry.expires_at, -14_400);
        assert_eq!(
            expiry.restore(&master).expect_err("만료 오프셋"),
            domain::DraftRestoreError::ExpiryMismatch
        );
        let mut card_offset = saved;
        card_offset.card.observed_at = with_offset(card_offset.card.observed_at, -14_400);
        assert_eq!(
            card_offset.restore(&master).expect_err("시세 오프셋"),
            domain::DraftRestoreError::ChangedRange
        );
        let (signal, input, signal_master) = signal_fixture("99");
        let mut stored = input.capture(&signal, &signal_master).expect("신호");
        stored.evidence.submitted_at = with_offset(stored.evidence.submitted_at, -14_400);
        assert_eq!(
            stored
                .restore_history(&signal_master)
                .expect_err("제출 오프셋"),
            domain::SignalRestoreError::ChangedSignal
        );
    }
    #[test]
    fn signal_history_rejects_changed_offsets_in_maturity_and_portfolio() {
        let (signal, input, master) = signal_fixture("1");
        let saved = input.capture(&signal, &master).expect("신호 근거");
        let mut changed_maturity = saved.clone();
        changed_maturity.evidence.maturity =
            with_offset(changed_maturity.evidence.maturity, -14_400);
        changed_maturity
            .restore_history(&master)
            .expect_err("만기 오프셋");
        let mut changed_display = saved.clone();
        let domain::HistoricalSignalDirection::Sell {
            quantity,
            observed_at,
        } = changed_display.expected.direction
        else {
            panic!("매도 신호");
        };
        changed_display.expected.direction = domain::HistoricalSignalDirection::Sell {
            quantity,
            observed_at: with_offset(observed_at, -14_400),
        };
        changed_display
            .restore_history(&master)
            .expect_err("표시 오프셋");
        let mutators: [fn(&mut domain::HistoricalPortfolioFacts); 3] = [
            |facts| {
                facts.holdings[0].observed_at = with_offset(facts.holdings[0].observed_at, -14_400);
            },
            |facts| facts.krw.observed_at = with_offset(facts.krw.observed_at, -14_400),
            |facts| facts.usd.observed_at = with_offset(facts.usd.observed_at, -14_400),
        ];
        for change in mutators {
            let mut changed = saved.clone();
            let portfolio = changed
                .evidence
                .assessment
                .portfolio
                .as_ref()
                .expect("포트");
            let mut facts = portfolio.facts().clone();
            change(&mut facts);
            let rebuilt =
                domain::HistoricalPortfolio::from_saved_facts(facts, std::slice::from_ref(&master))
                    .expect("과거 포트폴리오");
            assert_ne!(&rebuilt, portfolio);
            changed.evidence.assessment.portfolio = Some(rebuilt);
            changed
                .restore_history(&master)
                .expect_err("포트폴리오 오프셋");
        }
    }
    fn rate_with_offsets(start_offset: i32, end_offset: i32) -> domain::FxRate {
        domain::FxRate::parse(
            &NumericInput::try_new("1350").expect("입력"),
            domain::FxPair::UsdKrw,
            domain::FxRateKind::Mid,
            domain::TimeInterval::try_new(
                with_offset(at(60, -18_000), start_offset),
                with_offset(at(63, -18_000), end_offset),
            )
            .expect("기간"),
            domain::FxSource::TossOpenApi,
        )
        .expect("환율")
    }
    fn session_with_offsets(day: i64, start_offset: i32, end_offset: i32) -> domain::MarketSession {
        let original = session(day);
        let interval = original.sessions()[0].session().interval();
        let adjusted = domain::TimeInterval::try_new(
            with_offset(interval.start(), start_offset),
            with_offset(interval.end(), end_offset),
        )
        .expect("구간");
        domain::MarketSession::try_new(
            original.date(),
            domain::SessionInformation::Us {
                day_market: None,
                pre_market: None,
                regular_market: Some(domain::SessionData::try_new(adjusted, None).expect("정규장")),
                after_market: None,
            },
        )
        .expect("시장")
    }
    #[test]
    fn signal_history_rejects_changed_offsets_in_calendar_and_rate() {
        let (signal, mut input, master) = signal_fixture("1");
        input.assessment.rate = Some(rate_with_offsets(-18_000, -18_000));
        let saved = input.capture(&signal, &master).expect("신호 근거");
        for (start, end) in [(-14_400_i32, -18_000_i32), (-18_000_i32, -14_400_i32)] {
            let mut rate = saved.clone();
            rate.evidence.assessment.rate = Some(rate_with_offsets(start, end));
            rate.restore_history(&master).expect_err("환율 오프셋");
            let mut calendar = saved.clone();
            calendar.evidence.assessment.calendars[0] = domain::MarketCalendar::try_new(
                session_with_offsets(59, start, end),
                session(60),
                session(61),
            )
            .expect("달력");
            calendar.restore_history(&master).expect_err("시장 오프셋");
        }
    }
    #[test]
    fn independently_recorded_evidence_rejects_valid_but_changed_fields() {
        let (draft, estimate, master, lifetime, card) = draft_fixture();
        let saved =
            domain::DraftRestoreEvidence::capture(&draft, estimate, &master, lifetime, &card)
                .expect("초안");
        let mut changed_account = saved.clone();
        changed_account.account = domain::AccountId::new(419);
        changed_account.restore(&master).expect_err("계좌 손상");
        let mut changed_master = saved;
        changed_master.estimate.evidence.master.name =
            DisplayInstrumentName::try_new("다른 회사").expect("이름");
        changed_master
            .restore(&master)
            .expect_err("과거 마스터 손상");
        let (signal, input, signal_master) = signal_fixture("99");
        let mut changed_source = input.capture(&signal, &signal_master).expect("신호");
        changed_source.evidence.assessment.items[0] = domain::CollectedItem::from_web(
            domain::NormalizedUrl::try_new("https://example.com/d56").expect("주소"),
            domain::UntrustedText::try_new("바뀐 출처").expect("제목"),
            None,
        );
        changed_source
            .restore_history(&signal_master)
            .expect_err("출처 손상");
    }
    #[test]
    fn individual_estimate_evidence_fields_are_required() {
        let input = evidence(HorizonSelection::FewDays, RiskAppetite::Neutral);
        let master = MasterInstrument::try_new(row()).expect("마스터");
        let saved = input
            .capture(&original(&input, &master), &master)
            .expect("근거");
        saved.restore_history(&master).expect("정상 대조군");
        let changes: [fn(&mut domain::StoredEstimateEvidence); 11] = [
            |value| {
                value.evidence.master.name =
                    DisplayInstrumentName::try_new("다른 이름").expect("이름");
            },
            |value| value.evidence.side = OrderSide::Sell,
            |value| {
                value.evidence.candles.pop();
            },
            |value| value.evidence.reference_price = price("111"),
            |value| value.evidence.reference_at = at(60, -18_000),
            |value| value.evidence.task_at = at(62, -18_000),
            |value| value.evidence.horizon = Horizon::new(HorizonSelection::WeeksToMonths),
            |value| value.evidence.permission = HoursPermissionState::Enabled,
            |value| value.evidence.appetite = RiskAppetite::Aggressive,
            |value| value.expected.reference_price = price("112"),
            |value| {
                value.original.master.name =
                    DisplayInstrumentName::try_new("과거 이름").expect("이름");
            },
        ];
        for change in changes {
            let mut damaged = saved.clone();
            change(&mut damaged);
            damaged
                .restore_history(&master)
                .expect_err("단일 필드 손상");
        }
    }
    #[test]
    fn individual_draft_evidence_fields_are_required() {
        let (draft, estimate, master, lifetime, card) = draft_fixture();
        let saved =
            domain::DraftRestoreEvidence::capture(&draft, estimate, &master, lifetime, &card)
                .expect("근거");
        saved.restore(&master).expect("정상 대조군");
        let changes: [fn(&mut domain::DraftRestoreEvidence); 11] = [
            |value| {
                value.id =
                    domain::ProposalId::new(domain::ClientOrderId::try_new("changed").expect("ID"));
            },
            |value| value.account = domain::AccountId::new(419),
            |value| {
                value.terms = domain::HistoricalOrderTerms::UsMarketBuy {
                    quantity: domain::HistoricalWholeQuantity::try_new(
                        Quantity::parse(&NumericInput::try_new("8").expect("입력")).expect("수량"),
                    )
                    .expect("주문 수량"),
                };
            },
            |value| value.created_at = at(60, -18_000),
            |value| {
                value.lifetime =
                    domain::ProposalLifetime::try_new(domain::SpanSeconds::new(380)).expect("기간");
            },
            |value| value.expires_at = at(62, -18_000),
            |value| value.card.price = price("109"),
            |value| value.card.identity = domain::StoredPriceIdentity::Unidentified,
            |value| value.expected_card.price = price("108"),
            |value| value.original.created_at = at(60, -18_000),
            |value| {
                value.original.lifetime =
                    domain::ProposalLifetime::try_new(domain::SpanSeconds::new(380)).expect("기간");
            },
        ];
        for change in changes {
            let mut damaged = saved.clone();
            change(&mut damaged);
            damaged.restore(&master).expect_err("단일 필드 손상");
        }
    }
    type SignalEvidenceChange = fn(&mut domain::StoredSignalEvidence);
    fn signal_model_changes() -> [SignalEvidenceChange; 8] {
        [
            |value| value.evidence.estimate.evidence.reference_price = price("111"),
            |value| {
                let rate = domain::CostRate::try_new(
                    domain::Ratio::parse(&NumericInput::try_new("0.01").expect("입력"))
                        .expect("비율"),
                )
                .expect("수수료");
                value.evidence.commission = domain::CommissionRateFact::from_commissions_result(
                    domain::Market::Us,
                    rate,
                    domain::CommissionValidity::try_new(None, None).expect("기간"),
                );
            },
            |value| {
                let rate = domain::CostRate::try_new(
                    domain::Ratio::parse(&NumericInput::try_new("0.02").expect("입력"))
                        .expect("비율"),
                )
                .expect("세율");
                value.evidence.tax =
                    domain::TradeTaxFact::from_schedule_result(domain::Market::Us, rate, rate);
            },
            |value| value.evidence.maturity = at(67, -18_000),
            |value| {
                let old = &value.evidence.calibration;
                let head = domain::CalibrationLedgerHead::new(
                    old.through().key().clone(),
                    domain::CalibrationPosition::new(42),
                );
                value.evidence.calibration =
                    domain::CalibrationSnapshot::try_new(old.calibration().clone(), head)
                        .expect("보정");
            },
            |value| {
                value.evidence.calibration_key = domain::CalibrationKey::new(
                    value.evidence.calibration_key.model().clone(),
                    Horizon::new(HorizonSelection::WeeksToMonths),
                );
            },
            |value| {
                value.evidence.probability = domain::AiProbability::parse(
                    &domain::AiProbabilityInput::try_new("1").expect("입력"),
                )
                .expect("확률");
            },
            |value| value.evidence.source = None,
        ]
    }
    fn signal_policy_changes() -> [SignalEvidenceChange; 7] {
        [
            |value| value.evidence.submitted_at = at(60, -18_000),
            |value| {
                value.evidence.margin = domain::Margin::for_appetite(RiskAppetite::Conservative);
            },
            |value| value.evidence.assessment.portfolio = None,
            |value| {
                value.evidence.assessment.rate = Some(
                    domain::FxRate::parse(
                        &NumericInput::try_new("1350").expect("입력"),
                        domain::FxPair::UsdKrw,
                        domain::FxRateKind::Mid,
                        domain::TimeInterval::try_new(at(60, -18_000), at(62, -18_000))
                            .expect("기간"),
                        domain::FxSource::TossOpenApi,
                    )
                    .expect("환율"),
                );
            },
            |value| {
                value.evidence.assessment.policy = value
                    .evidence
                    .assessment
                    .policy
                    .clone()
                    .with_context(domain::PolicyText::try_new("과거 정책").expect("정책"));
            },
            |value| value.evidence.assessment.classifications.sectors.clear(),
            |value| value.evidence.assessment.classifications.security.clear(),
        ]
    }
    fn signal_source_changes() -> [SignalEvidenceChange; 7] {
        [
            |value| value.evidence.assessment.items.clear(),
            |value| {
                value
                    .evidence
                    .assessment
                    .disclosures
                    .push(domain::Disclosure::new(
                        domain::ReceiptNo::Dart(
                            domain::DartReceiptNo::try_new("20261113000001").expect("접수"),
                        ),
                        domain::NormalizedUrl::try_new("https://example.com/disclosure")
                            .expect("주소"),
                        domain::UntrustedText::try_new("공시").expect("제목"),
                        None,
                    ));
            },
            |value| {
                value
                    .evidence
                    .assessment
                    .tools
                    .push(domain::ToolReference::try_new("tool:extra").expect("도구"));
            },
            |value| value.evidence.assessment.calendars.clear(),
            |value| value.evidence.assessment.evaluated_at = at(60, -18_000),
            |value| value.evidence.assessment.target = None,
            |value| value.evidence.assessment.calibration_head = None,
        ]
    }
    #[test]
    fn individual_signal_evidence_fields_are_required() {
        let (signal, input, master) = signal_fixture("99");
        let saved = input.capture(&signal, &master).expect("신호 근거");
        saved.restore_history(&master).expect("정상 대조군");
        let changes = signal_model_changes()
            .into_iter()
            .chain(signal_policy_changes())
            .chain(signal_source_changes());
        for change in changes {
            let mut damaged = saved.clone();
            change(&mut damaged);
            damaged
                .restore_history(&master)
                .expect_err("단일 필드 손상");
        }
    }
    #[test]
    fn historical_evidence_debug_redacts_account_and_portfolio_values() {
        let (draft, estimate, master, lifetime, card) = draft_fixture();
        let saved_draft =
            domain::DraftRestoreEvidence::capture(&draft, estimate, &master, lifetime, &card)
                .expect("초안 근거");
        assert!(!format!("{:?}", saved_draft.original).contains("418"));
        let quantity = domain::HistoricalWholeQuantity::try_new(
            Quantity::parse(&NumericInput::try_new("123456").expect("입력")).expect("수량"),
        )
        .expect("정수");
        assert!(!format!("{quantity:?}").contains("123456"));
        let (signal, evidence, signal_master) = signal_fixture("99");
        let saved_signal = evidence
            .capture(&signal, &signal_master)
            .expect("신호 근거");
        let limit = match saved_signal.expected.direction {
            domain::HistoricalSignalDirection::Buy { limit } => limit,
            domain::HistoricalSignalDirection::Sell { .. } => panic!("매수 신호"),
        };
        let value = limit.display_text();
        assert!(!format!("{saved_signal:?}").contains(value.as_str()));
        let portfolio = saved_signal
            .evidence
            .assessment
            .portfolio
            .as_ref()
            .expect("포트");
        let cash = portfolio.facts().krw;
        let cash_text = match cash.state {
            domain::HistoricalBuyingPowerState::Available(cash_money) => cash_money.display_text(),
            domain::HistoricalBuyingPowerState::NegativeBuyingPower
            | domain::HistoricalBuyingPowerState::CashLookupFailed(_) => panic!("현금"),
        };
        assert!(!format!("{cash:?}").contains(cash_text.as_str()));
        assert!(!format!("{:?}", cash.state).contains(cash_text.as_str()));
        assert!(!format!("{cash_text:?}").contains(cash_text.as_str()));
        let (sell, input, sell_master) = signal_fixture("1");
        let history = input
            .capture(&sell, &sell_master)
            .expect("근거")
            .restore_history(&sell_master)
            .expect("과거 신호");
        assert!(!format!("{history:?}").contains("held_quantity"));
    }
    #[test]
    fn signal_occurrence_replays_original_interval_and_rejects_damage() {
        let (signal, evidence, master) = signal_fixture("99");
        let buy = match &signal {
            domain::RaisedSignal::Buy(value) => value,
            domain::RaisedSignal::Sell(_) => panic!("매수 신호가 필요"),
        };
        let id = domain::BuySignalOccurrenceId::try_new("event-314").expect("발생 ID");
        let original = domain::BuySignalRecord::from_signal_checked(
            id,
            buy,
            at(61, -18_000),
            &evidence.assessment.calendars,
        )
        .expect("발생 이력");
        let saved_signal = evidence.capture(&signal, &master).expect("신호 근거");
        let occurrence = domain::StoredSignalOccurrence::capture(&original, saved_signal, &master)
            .expect("이력 근거");
        let historical = occurrence.restore(&master).expect("이력 복원");
        assert_eq!(historical.id(), original.id());
        assert_eq!(historical.instrument_id(), original.instrument_id());
        assert_eq!(historical.interval(), original.interval());
        assert_eq!(historical.raised_at(), original.raised_at());
        assert_eq!(
            domain::CurrentBuySignal::try_from_history(buy.clone(), historical),
            domain::CurrentBuySignal::try_new(buy.clone(), original),
        );
        let mut changed_offset = occurrence.clone();
        changed_offset.raised_at = with_offset(changed_offset.raised_at, -14_400);
        changed_offset.restore(&master).expect_err("발생 오프셋");
        let mut invalid = occurrence;
        invalid.raised_at = shifted(61, 2);
        invalid.restore(&master).expect_err("손상 거부");
    }

    #[derive(Debug, Clone, Copy)]
    enum PropOrderKind {
        KrLimitBuy,
        KrMarketSell,
        UsLimitBuy,
        UsMarketBuy,
        UsMarketSell,
        UsMarketAmountBuy,
    }
    #[derive(Debug, Clone, Copy)]
    enum ExpiryWindow {
        BeforeExpiry,
        AtExpiry,
    }
    #[derive(Clone, Copy)]
    struct ProposalCase {
        kind: PropOrderKind,
        account: i64,
        seconds: u64,
        card_value: u32,
        current_value: u32,
        identified: domain::StoredPriceIdentity,
        window: ExpiryWindow,
    }
    fn prop_order_terms(kind: PropOrderKind) -> domain::OrderTerms {
        let whole = domain::WholeOrderQuantity::try_new(
            Quantity::parse(&NumericInput::try_new("7").expect("입력")).expect("수량"),
        )
        .expect("정수");
        match kind {
            PropOrderKind::KrLimitBuy => domain::OrderTerms::KrLimit {
                side: OrderSide::Buy,
                quantity: whole,
                price: domain::KrOrderPrice::try_new(
                    Price::parse(
                        &NumericInput::try_new("51000").expect("입력"),
                        Currency::Krw,
                    )
                    .expect("가격"),
                )
                .expect("호가"),
            },
            PropOrderKind::KrMarketSell => domain::OrderTerms::KrMarket {
                side: OrderSide::Sell,
                quantity: whole,
            },
            PropOrderKind::UsLimitBuy => domain::OrderTerms::UsLimit {
                side: OrderSide::Buy,
                quantity: whole,
                price: domain::UsOrderPrice::try_new(price("105.25")).expect("달러 가격"),
            },
            PropOrderKind::UsMarketBuy => domain::OrderTerms::UsMarketBuy { quantity: whole },
            PropOrderKind::UsMarketSell => domain::OrderTerms::UsMarketSell {
                quantity: domain::SellOrderQuantity::try_new(
                    Quantity::parse(&NumericInput::try_new("2.5").expect("입력")).expect("수량"),
                )
                .expect("매도 수량"),
            },
            PropOrderKind::UsMarketAmountBuy => domain::OrderTerms::UsMarketAmountBuy {
                amount: domain::UsdOrderAmount::try_new(money("250.50", Currency::Usd))
                    .expect("금액"),
            },
        }
    }
    fn prop_market_fixture(
        kind: PropOrderKind,
    ) -> (
        Currency,
        i32,
        MasterInstrument,
        domain::StoredEstimateEvidence,
        EstimateTask,
    ) {
        match kind {
            PropOrderKind::KrLimitBuy | PropOrderKind::KrMarketSell => {
                let cost = kr_signal_cost_fixture();
                let task = cost.eligible.task().clone();
                (Currency::Krw, 32_400_i32, cost.master, cost.stored, task)
            }
            PropOrderKind::UsLimitBuy
            | PropOrderKind::UsMarketBuy
            | PropOrderKind::UsMarketSell
            | PropOrderKind::UsMarketAmountBuy => {
                let ev = evidence(HorizonSelection::FewDays, RiskAppetite::Neutral);
                let master = MasterInstrument::try_new(row()).expect("조회");
                let task = original(&ev, &master);
                let stored = ev.capture(&task, &master).expect("과제");
                (Currency::Usd, -18_000_i32, master, stored, task)
            }
        }
    }
    fn prop_price_for(currency: Currency, value: u32) -> Price {
        let scaled = match currency {
            Currency::Krw => value.checked_mul(500).expect("원화"),
            Currency::Usd => value,
        };
        Price::parse(
            &NumericInput::try_new(&scaled.to_string()).expect("입력"),
            currency,
        )
        .expect("가격")
    }
    fn check_proposal_property(case: ProposalCase) {
        let (currency, offset, master, estimate, base_task) = prop_market_fixture(case.kind);
        let card_price = prop_price_for(currency, case.card_value);
        let card = match case.identified {
            domain::StoredPriceIdentity::Identified => master
                .price_observation(card_price, at(61, offset))
                .expect("식별 시세"),
            domain::StoredPriceIdentity::Unidentified => {
                domain::PriceObservation::from_quote_parts(
                    master.order_instrument().expect("종목"),
                    card_price,
                    at(61, offset),
                )
                .expect("미식별 시세")
            }
        };
        let id = domain::ProposalId::new(
            domain::ClientOrderId::try_new(&format!("prop-{}", case.account)).expect("ID"),
        );
        let lifetime = domain::ProposalLifetime::try_new(domain::SpanSeconds::new(case.seconds))
            .expect("기간");
        let range =
            domain::ProposalPriceRange::from_estimate_task(base_task.range_facts(), card.clone())
                .expect("범위");
        let intent = domain::OrderIntent::new(
            domain::AccountId::new(case.account),
            master.order_instrument().expect("종목"),
            id.as_client_order_id().clone(),
            prop_order_terms(case.kind),
        )
        .expect("주문");
        let original =
            domain::Proposal::new(id, intent, at(61, offset), range, lifetime).expect("초안");
        let saved =
            domain::DraftRestoreEvidence::capture(&original, estimate, &master, lifetime, &card)
                .expect("근거 수집");
        let restored = saved.restore(&master).expect("초안 복원");
        let current = master
            .price_observation(prop_price_for(currency, case.current_value), at(61, offset))
            .expect("시세");
        let eval_seconds = match case.window {
            ExpiryWindow::BeforeExpiry => case.seconds.saturating_sub(1),
            ExpiryWindow::AtExpiry => case.seconds,
        };
        let now = at(61, offset)
            .checked_add(
                domain::TimeSpan::try_new(
                    domain::SpanSeconds::new(eval_seconds),
                    Nanosecond::new(0),
                )
                .expect("기간"),
            )
            .expect("시각");
        assert_eq!(restored.id(), original.id());
        assert_eq!(restored.intent(), original.intent());
        assert_eq!(restored.expires_at(), original.expires_at());
        assert_eq!(restored.price_range(), original.price_range());
        assert_eq!(
            restored.report(now, &current),
            original.report(now, &current)
        );
    }
    fn signal_cost_for_appetite(appetite: RiskAppetite) -> SignalCostFixture {
        let mut input = evidence(HorizonSelection::FewDays, appetite);
        input.master.security_type =
            MasterSecurityType::from_code(MasterCode::try_new("ETF").expect("유형"));
        let master = MasterInstrument::try_new(input.master.clone()).expect("조회");
        let task = original(&input, &master);
        let stored = input.capture(&task, &master).expect("과제 근거");
        let zero = domain::CostRate::try_new(
            domain::Ratio::parse(&NumericInput::try_new("0").expect("입력")).expect("비율"),
        )
        .expect("요율");
        let commission = domain::CommissionRateFact::from_commissions_result(
            domain::Market::Us,
            zero,
            domain::CommissionValidity::try_new(None, None).expect("기간"),
        );
        let tax = domain::TradeTaxFact::from_schedule_result(domain::Market::Us, zero, zero);
        let maturity = shifted(67, 25_200);
        let cost = domain::TradeCost::from_facts(
            Some(commission.clone()),
            Some(tax.clone()),
            domain::EstimateCostPeriod::from_calendar_result(&task, maturity).expect("기간"),
        )
        .expect("비용");
        let eligible = domain::CostEligibleTask::assess(task, &cost).expect("적격");
        SignalCostFixture {
            master,
            stored,
            eligible,
            commission,
            tax,
            maturity,
        }
    }
    fn prop_signal_portfolio(
        master: &MasterInstrument,
        side: OrderSide,
        cash_units: u32,
        held_shares: u32,
    ) -> domain::Portfolio {
        let holdings = match side {
            OrderSide::Buy => vec![],
            OrderSide::Sell => vec![
                domain::Holding::new(
                    master.order_instrument().expect("종목"),
                    Quantity::parse(
                        &NumericInput::try_new(&held_shares.to_string()).expect("입력"),
                    )
                    .expect("수량"),
                    domain::AcquisitionPrice::try_new(money("90", Currency::Usd)).expect("원가"),
                    domain::HoldingObservation::new(Some(price("104")), None, at(61, -18_000)),
                )
                .expect("보유"),
            ],
        };
        let krw_cash = u64::from(cash_units).checked_mul(10_000).expect("현금");
        domain::Portfolio::new(
            holdings,
            domain::BuyingPower::new(money(&krw_cash.to_string(), Currency::Krw), at(61, -18_000)),
            domain::BuyingPower::new(money("0", Currency::Usd), at(61, -18_000)),
        )
        .expect("포트")
    }
    fn assert_signal_roundtrip(
        signal: &domain::RaisedSignal,
        saved: domain::StoredSignalEvidence,
        master: &MasterInstrument,
        cals: &[domain::MarketCalendar],
        occ_tag: &str,
    ) {
        let history = saved.restore_history(master).expect("신호 복원");
        let (expected_side, facts, decision) = match signal {
            domain::RaisedSignal::Buy(value) => (
                OrderSide::Buy,
                value.facts(),
                domain::SignalDecision::Buy(value.clone()),
            ),
            domain::RaisedSignal::Sell(value) => (
                OrderSide::Sell,
                value.facts(),
                domain::SignalDecision::Sell(value.clone()),
            ),
        };
        assert_eq!(history.side(), expected_side);
        assert_eq!(history.display_facts(), saved.expected);
        assert_eq!(
            domain::signal_transition_from_history(Some(&history), &decision),
            domain::signal_transition(Some(signal), &decision)
        );
        assert_eq!(
            history.display_probability().expect("복원 확률"),
            facts.display_probability().expect("원래 확률")
        );
        for kind in [
            domain::ThresholdKind::Buy,
            domain::ThresholdKind::Sell,
            domain::ThresholdKind::Breakeven,
        ] {
            assert_eq!(
                history.threshold_display(kind).expect("복원 임계값"),
                facts.thresholds().display(kind).expect("원래 임계값")
            );
        }
        if let domain::RaisedSignal::Buy(buy) = signal {
            let record = domain::BuySignalRecord::from_signal_checked(
                domain::BuySignalOccurrenceId::try_new(occ_tag).expect("ID"),
                buy,
                at(61, -18_000),
                cals,
            )
            .expect("발생");
            let stored_occ =
                domain::StoredSignalOccurrence::capture(&record, saved, master).expect("이력 근거");
            let restored_occ = stored_occ.restore(master).expect("이력 복원");
            assert_eq!(restored_occ.interval(), record.interval());
            assert_eq!(restored_occ.raised_at(), record.raised_at());
        }
    }
    fn assess_prop_signal(
        cost_fix: &SignalCostFixture,
        model: &SignalModelFixture,
        portfolio: &domain::Portfolio,
        cals: &[domain::MarketCalendar],
    ) -> (domain::RaisedSignal, domain::HistoricalSignalContext) {
        let master = &cost_fix.master;
        let valuation = portfolio.evaluate(None);
        let (assignments, security) = signal_classifications(master);
        let target = master
            .price_observation(price("105"), at(61, -18_000))
            .expect("시세");
        let assessment = domain::HistoricalSignalContext {
            portfolio: Some(
                domain::HistoricalPortfolio::capture(portfolio, std::slice::from_ref(master))
                    .expect("포트폴리오 근거"),
            ),
            rate: None,
            policy: domain::Policy::new(domain::InvestorProfile::default()),
            classifications: domain::HistoricalClassifications::capture(&assignments, &security),
            items: vec![model.item.clone()],
            disclosures: vec![],
            tools: vec![],
            calendars: cals.to_vec(),
            evaluated_at: at(61, -18_000),
            target: Some(domain::StoredPriceObservation::from_price_observation(
                &target,
            )),
            calibration_head: Some(model.head.clone()),
        };
        let ledger = domain::SignalSourceLedger::new(&assessment.items, &[], &[]);
        let facts_ctx = domain::SignalAssessmentFacts::new(
            domain::SignalTiming::new(assessment.evaluated_at, cals),
            Some(target),
            Some(model.head.clone()),
        );
        let context = domain::SignalContext::new(
            Some(&valuation),
            &assessment.policy,
            (&assignments, &security),
            &ledger,
            facts_ctx,
        );
        let signal = domain::SignalDecision::assess(&model.estimate, &context)
            .into_signal()
            .expect("신호");
        (signal, assessment)
    }
    fn check_signal_property(
        appetite: RiskAppetite,
        side: OrderSide,
        prob_offset: u8,
        cash_units: u32,
        held_shares: u32,
    ) {
        let prob_value = match side {
            OrderSide::Buy => 95_u8.checked_add(prob_offset).expect("매수 확률"),
            OrderSide::Sell => 1_u8.checked_add(prob_offset).expect("매도 확률"),
        };
        let cost_fix = signal_cost_for_appetite(appetite);
        let master = cost_fix.master.clone();
        let model = signal_model_fixture(&cost_fix, &prob_value.to_string());
        let portfolio = prop_signal_portfolio(&master, side, cash_units, held_shares);
        let cals = calendars();
        let (signal, assessment) = assess_prop_signal(&cost_fix, &model, &portfolio, &cals);
        let saved = domain::SignalRestoreEvidence {
            estimate: cost_fix.stored,
            commission: cost_fix.commission,
            tax: cost_fix.tax,
            maturity: cost_fix.maturity,
            calibration: model.snapshot,
            calibration_key: model.key,
            probability: model.probability,
            source: Some(model.source),
            submitted_at: at(61, -18_000),
            margin: model.margin,
            assessment,
        }
        .capture(&signal, &master)
        .expect("신호 저장");
        assert_signal_roundtrip(
            &signal,
            saved,
            &master,
            &cals,
            &format!("occ-{prob_value}-{cash_units}"),
        );
    }
    #[test]
    fn proposal_and_signal_roundtrip_property_preserves_behavior() {
        let kinds = vec![
            PropOrderKind::KrLimitBuy,
            PropOrderKind::KrMarketSell,
            PropOrderKind::UsLimitBuy,
            PropOrderKind::UsMarketBuy,
            PropOrderKind::UsMarketSell,
            PropOrderKind::UsMarketAmountBuy,
        ];
        let appetites = vec![
            RiskAppetite::Conservative,
            RiskAppetite::Neutral,
            RiskAppetite::Aggressive,
        ];
        TestRunner::new(Config::with_cases(testkit::property_cases()))
            .run(
                &(
                    (
                        sample::select(kinds),
                        1_i64..=9_999_i64,
                        1_u64..=600_u64,
                        80_u32..=140_u32,
                        50_u32..=200_u32,
                        sample::select(vec![
                            domain::StoredPriceIdentity::Identified,
                            domain::StoredPriceIdentity::Unidentified,
                        ]),
                        sample::select(vec![ExpiryWindow::BeforeExpiry, ExpiryWindow::AtExpiry]),
                    ),
                    (
                        sample::select(appetites),
                        sample::select(vec![OrderSide::Buy, OrderSide::Sell]),
                        0_u8..=4_u8,
                        10_u32..=500_u32,
                        1_u32..=50_u32,
                    ),
                ),
                |(
                    (kind, account, seconds, card_value, current_value, identified, window),
                    (appetite, side, prob_offset, cash_units, held_shares),
                )| {
                    check_proposal_property(ProposalCase {
                        kind,
                        account,
                        seconds,
                        card_value,
                        current_value,
                        identified,
                        window,
                    });
                    check_signal_property(appetite, side, prob_offset, cash_units, held_shares);
                    Ok(())
                },
            )
            .expect("원래 판정과 복원 판정 일치");
    }

    fn kr_liquidation_row() -> MasterInstrumentRow {
        MasterInstrumentRow {
            listing_market: MasterCode::try_new("KOSPI").expect("시장"),
            symbol: Symbol::try_new("005930").expect("심볼"),
            instrument_id: InstrumentId::try_new("kr-liq").expect("식별자"),
            name: DisplayInstrumentName::try_new("정리매매 종목").expect("이름"),
            security_type: MasterSecurityType::from_code(
                MasterCode::try_new("STOCK").expect("유형"),
            ),
            currency: Currency::Krw,
            listing_status: ListingStatus::Active,
            common_share: CommonShare::Common,
            korean_detail: Some(domain::KoreanMasterDetail {
                liquidation: domain::LiquidationTrading::Liquidation,
                krx: domain::TradingSuspension::Clear,
                nxt: domain::NxtTrading::Unsupported,
            }),
            observed_at: at(61, 32_400),
        }
    }
    fn kr_candles(instrument_id: &str) -> Vec<Candle> {
        let p_even = Price::parse(
            &NumericInput::try_new("50000").expect("입력"),
            Currency::Krw,
        )
        .expect("원화 가격");
        let p_odd = Price::parse(
            &NumericInput::try_new("51500").expect("입력"),
            Currency::Krw,
        )
        .expect("원화 가격");
        (0_i64..61_i64)
            .map(|day| {
                let value = if day.checked_rem(2_i64) == Some(0_i64) {
                    p_even
                } else {
                    p_odd
                };
                Candle::try_new(
                    InstrumentId::try_new(instrument_id).expect("식별자"),
                    CandleTime::try_new(at(day, 0), CandlePeriod::Daily).expect("일봉"),
                    PriceAdjustment::Adjusted(AdjustmentBasis::new(at(70, 0))),
                    Ohlcv::try_new(
                        value,
                        value,
                        value,
                        value,
                        Quantity::parse(&NumericInput::try_new("3").expect("입력")).expect("수량"),
                    )
                    .expect("봉"),
                    CandleObservation::new(at(70, 0), CandleStatus::Finalized),
                )
                .expect("봉")
            })
            .collect()
    }
    #[test]
    fn draft_order_intent_accessors_and_debug_redaction_cover_all_types() {
        let (draft, estimate, master, lifetime, card) = draft_fixture();
        let intent = draft.intent();
        assert_eq!(intent.client_order_id(), draft.id().as_client_order_id());
        assert_eq!(
            intent.instrument(),
            &master.order_instrument().expect("종목")
        );
        assert_eq!(intent.symbol(), &master.row().symbol);
        assert_eq!(intent.market(), domain::Market::Us);
        assert!(!format!("{intent:?}").contains("418"));
        let sell_qty = domain::HistoricalSellQuantity::try_new(
            Quantity::parse(&NumericInput::try_new("19.75").expect("입력")).expect("수량"),
        )
        .expect("매도 수량");
        assert!(!format!("{sell_qty:?}").contains("19.75"));
        let order_amount =
            domain::HistoricalOrderAmount::try_new(money("888.50", Currency::Usd)).expect("금액");
        assert!(!format!("{order_amount:?}").contains("888.50"));
        let saved =
            domain::DraftRestoreEvidence::capture(&draft, estimate, &master, lifetime, &card)
                .expect("근거");
        assert!(!format!("{saved:?}").contains("418"));
        assert_eq!(PriceAdjustment::Raw.adjustment_basis(), None);
        let basis = AdjustmentBasis::new(at(70, 0));
        assert_eq!(
            PriceAdjustment::Adjusted(basis).adjustment_basis(),
            Some(basis)
        );
    }
    #[test]
    fn estimate_restore_rejects_master_and_capture_errors() {
        let input = evidence(HorizonSelection::FewDays, RiskAppetite::Neutral);
        let master = MasterInstrument::try_new(row()).expect("마스터");
        let task = original(&input, &master);
        let mut wrong_task_input = input.clone();
        wrong_task_input.appetite = RiskAppetite::Aggressive;
        assert_eq!(
            wrong_task_input.capture(&task, &master),
            Err(EstimateRestoreError::ChangedResult)
        );
        let mut bad_hist = input.clone();
        bad_hist.master.currency = Currency::Krw;
        assert_eq!(
            bad_hist.capture(&task, &master),
            Err(EstimateRestoreError::HistoricalMaster(
                domain::MasterInstrumentError::CurrencyMismatch
            ))
        );
        let mut unknown_hist = input.clone();
        unknown_hist.master.listing_market = MasterCode::try_new("XETRA").expect("미지");
        assert_eq!(
            unknown_hist.capture(&task, &master),
            Err(EstimateRestoreError::HistoricalMaster(
                domain::MasterInstrumentError::UnknownMarket
            ))
        );
        let mut unknown_cur_row = row();
        unknown_cur_row.listing_market = MasterCode::try_new("XETRA").expect("미지");
        let unknown_cur = MasterInstrument::try_new(unknown_cur_row).expect("미지");
        assert_eq!(
            input.capture(&task, &unknown_cur),
            Err(EstimateRestoreError::CurrentMaster(
                domain::MasterInstrumentError::UnknownMarket
            ))
        );
    }
    #[test]
    fn estimate_restore_rejects_delisted_reference_and_candle_currency_errors() {
        let input = evidence(HorizonSelection::FewDays, RiskAppetite::Neutral);
        let master = MasterInstrument::try_new(row()).expect("마스터");
        let task = original(&input, &master);
        let mut delisted_cur_row = row();
        delisted_cur_row.listing_status = ListingStatus::Delisted;
        let delisted_cur = MasterInstrument::try_new(delisted_cur_row).expect("폐지");
        assert_eq!(
            input.capture(&task, &delisted_cur),
            Err(EstimateRestoreError::CurrentMaster(
                domain::MasterInstrumentError::NotListed
            ))
        );
        let mut delisted_hist = input.clone();
        delisted_hist.master.listing_status = ListingStatus::Delisted;
        assert_eq!(
            delisted_hist.capture(&task, &master),
            Err(EstimateRestoreError::HistoricalMaster(
                domain::MasterInstrumentError::NotListed
            ))
        );
        let mut bad_ref = input.clone();
        bad_ref.reference_at = at(61, 0);
        assert_eq!(
            bad_ref.capture(&task, &master),
            Err(EstimateRestoreError::CurrentMaster(
                domain::MasterInstrumentError::InvalidPriceObservation
            ))
        );
        let mut stored_bad_ref = input.capture(&task, &master).expect("저장");
        stored_bad_ref.evidence.reference_at = at(61, 0);
        assert_eq!(
            stored_bad_ref.restore_history(&master),
            Err(EstimateRestoreError::CurrentMaster(
                domain::MasterInstrumentError::InvalidPriceObservation
            ))
        );
        let mut bad_candle = input;
        bad_candle.candles = kr_candles("sample");
        assert_eq!(
            bad_candle.capture(&task, &master),
            Err(EstimateRestoreError::CandleCurrencyMismatch)
        );
    }
    #[test]
    fn draft_restore_handles_unidentified_card_and_capture_mismatches() {
        let (draft, estimate, master, lifetime, card) = draft_fixture();
        let unid_card = domain::PriceObservation::from_quote_parts(
            master.order_instrument().expect("종목"),
            price("105"),
            at(61, -18_000),
        )
        .expect("미식별 시세");
        let base_task = original(
            &evidence(HorizonSelection::FewDays, RiskAppetite::Neutral),
            &master,
        );
        let unid_range = domain::ProposalPriceRange::from_estimate_task(
            base_task.range_facts(),
            unid_card.clone(),
        )
        .expect("범위");
        let id =
            domain::ProposalId::new(domain::ClientOrderId::try_new("unid-1").expect("주문 ID"));
        let intent = domain::OrderIntent::new(
            domain::AccountId::new(418),
            master.order_instrument().expect("종목"),
            id.as_client_order_id().clone(),
            domain::OrderTerms::UsMarketBuy {
                quantity: domain::WholeOrderQuantity::try_new(
                    Quantity::parse(&NumericInput::try_new("5").expect("입력")).expect("수량"),
                )
                .expect("정수"),
            },
        )
        .expect("의도");
        let unid_draft =
            domain::Proposal::new(id, intent, at(61, -18_000), unid_range, lifetime).expect("초안");
        let unid_saved = domain::DraftRestoreEvidence::capture(
            &unid_draft,
            estimate.clone(),
            &master,
            lifetime,
            &unid_card,
        )
        .expect("미식별 수집");
        assert_eq!(
            unid_saved.card.identity,
            domain::StoredPriceIdentity::Unidentified
        );
        let restored = unid_saved.restore(&master).expect("미식별 복원");
        assert_eq!(restored.price_range(), unid_draft.price_range());
        assert_eq!(
            domain::DraftRestoreEvidence::capture(&draft, estimate, &master, lifetime, &unid_card),
            Err(domain::DraftRestoreError::ChangedRange)
        );
        let alt_input = evidence(HorizonSelection::WeeksToMonths, RiskAppetite::Neutral);
        let alt_estimate = alt_input
            .capture(&original(&alt_input, &master), &master)
            .expect("다른 기간");
        assert_eq!(
            domain::DraftRestoreEvidence::capture(&draft, alt_estimate, &master, lifetime, &card),
            Err(domain::DraftRestoreError::ChangedDraft)
        );
    }
    #[test]
    fn draft_restore_rejects_mismatched_instrument_and_card_errors() {
        let (draft, estimate, master, lifetime, card) = draft_fixture();
        let mut other_row = row();
        other_row.symbol = Symbol::try_new("OTHER").expect("심볼");
        other_row.instrument_id = InstrumentId::try_new("other-id").expect("ID");
        let other_master = MasterInstrument::try_new(other_row.clone()).expect("다른 마스터");
        let mut other_ev = evidence(HorizonSelection::FewDays, RiskAppetite::Neutral);
        other_ev.master = other_row;
        other_ev.candles = candles()
            .into_iter()
            .map(|candle| {
                Candle::try_new(
                    InstrumentId::try_new("other-id").expect("ID"),
                    candle.time(),
                    candle.adjustment(),
                    candle.values(),
                    CandleObservation::new(candle.collected_at(), candle.status()),
                )
                .expect("봉")
            })
            .collect();
        let other_stored = other_ev
            .capture(&original(&other_ev, &other_master), &other_master)
            .expect("근거");
        assert_eq!(
            domain::DraftRestoreEvidence::capture(
                &draft,
                other_stored,
                &other_master,
                lifetime,
                &card
            ),
            Err(domain::DraftRestoreError::ChangedDraft)
        );
        let saved =
            domain::DraftRestoreEvidence::capture(&draft, estimate, &master, lifetime, &card)
                .expect("근거");
        let mut bad_card_obs = saved.clone();
        bad_card_obs.card.observed_at = at(61, 0);
        bad_card_obs.expected_card.observed_at = at(61, 0);
        assert_eq!(
            bad_card_obs.restore(&master).expect_err("시세 오프셋"),
            domain::DraftRestoreError::Master(
                domain::MasterInstrumentError::InvalidPriceObservation
            )
        );
        let mut bad_unid_cur = saved;
        let krw_price = Price::parse(&NumericInput::try_new("1000").expect("입력"), Currency::Krw)
            .expect("원화");
        bad_unid_cur.card.identity = domain::StoredPriceIdentity::Unidentified;
        bad_unid_cur.card.price = krw_price;
        bad_unid_cur.expected_card = bad_unid_cur.card;
        assert_eq!(
            bad_unid_cur.restore(&master).expect_err("통화 불일치"),
            domain::DraftRestoreError::Proposal(domain::ProposalError::CurrencyMismatch)
        );
    }
    #[test]
    fn draft_restore_rejects_mismatched_terms_and_time_overflow() {
        let (draft, estimate, master, lifetime, card) = draft_fixture();
        let saved =
            domain::DraftRestoreEvidence::capture(&draft, estimate, &master, lifetime, &card)
                .expect("근거");
        let mut bad_terms = saved.clone();
        let kr_terms = domain::HistoricalOrderTerms::KrMarket {
            side: OrderSide::Buy,
            quantity: domain::HistoricalWholeQuantity::try_new(
                Quantity::parse(&NumericInput::try_new("1").expect("입력")).expect("수량"),
            )
            .expect("정수"),
        };
        bad_terms.terms = kr_terms;
        bad_terms.original.terms = kr_terms;
        assert_eq!(
            bad_terms.restore(&master).expect_err("시장 불일치"),
            domain::DraftRestoreError::Intent(domain::OrderIntentError::MarketMismatch)
        );
        let max_ts = Timestamp::from_parts(
            UnixSeconds::new(253_402_300_799_i64),
            Nanosecond::new(0),
            OffsetSeconds::new(0),
        )
        .expect("최대 시각");
        let mut overflow_draft = saved.clone();
        overflow_draft.created_at = max_ts;
        overflow_draft.original.created_at = max_ts;
        assert_eq!(
            overflow_draft.restore(&master).expect_err("초안 시각 넘침"),
            domain::DraftRestoreError::Proposal(domain::ProposalError::Time(
                domain::TimeError::Overflow
            ))
        );
        let mut overflow_card = saved;
        overflow_card.card.identity = domain::StoredPriceIdentity::Unidentified;
        overflow_card.card.observed_at = max_ts;
        overflow_card.expected_card = overflow_card.card;
        assert_eq!(
            overflow_card.restore(&master).expect_err("시세 시각 넘침"),
            domain::DraftRestoreError::Time(domain::TimeError::Overflow)
        );
    }
    fn kr_liquidation_estimate(
        liq_master: &MasterInstrument,
        kr_price: Price,
    ) -> domain::StoredEstimateEvidence {
        let input = EstimateRestoreEvidence {
            master: kr_liquidation_row(),
            side: OrderSide::Sell,
            candles: kr_candles("kr-liq"),
            reference_price: kr_price,
            reference_at: at(61, 32_400),
            task_at: at(61, 32_400),
            horizon: Horizon::new(HorizonSelection::FewDays),
            permission: HoursPermissionState::Disabled,
            appetite: RiskAppetite::Neutral,
        };
        let sell_ref = liq_master
            .price_observation_for(OrderSide::Sell, kr_price, at(61, 32_400))
            .expect("매도 시세");
        let task = EstimateTask::create(
            EstimateContext::new(sell_ref, at(61, 32_400)),
            Volatility::calculate(&input.candles).expect("변동성"),
            input.horizon,
            HoursPermission::default(),
            input.appetite,
        )
        .expect("과제");
        input.capture(&task, liq_master).expect("과제 근거")
    }
    #[test]
    fn draft_restore_rejects_unidentified_card_on_liquidation_buy() {
        let liq_master = MasterInstrument::try_new(kr_liquidation_row()).expect("정리매매");
        let kr_price = Price::parse(
            &NumericInput::try_new("50000").expect("입력"),
            Currency::Krw,
        )
        .expect("원화");
        let stored_est = kr_liquidation_estimate(&liq_master, kr_price);
        let buy_terms = domain::HistoricalOrderTerms::KrMarket {
            side: OrderSide::Buy,
            quantity: domain::HistoricalWholeQuantity::try_new(
                Quantity::parse(&NumericInput::try_new("1").expect("입력")).expect("수량"),
            )
            .expect("정수"),
        };
        let id = domain::ProposalId::new(domain::ClientOrderId::try_new("liq-1").expect("ID"));
        let lifetime =
            domain::ProposalLifetime::try_new(domain::SpanSeconds::new(60)).expect("기간");
        let card = domain::StoredPriceObservation {
            price: kr_price,
            observed_at: at(61, 32_400),
            identity: domain::StoredPriceIdentity::Unidentified,
        };
        let saved = domain::DraftRestoreEvidence {
            estimate: stored_est,
            id: id.clone(),
            account: domain::AccountId::new(1),
            terms: buy_terms,
            created_at: at(61, 32_400),
            lifetime,
            expires_at: at(61, 32_400),
            card,
            expected_card: card,
            original: domain::DraftOriginalFacts {
                id,
                account: domain::AccountId::new(1),
                terms: buy_terms,
                created_at: at(61, 32_400),
                lifetime,
            },
        };
        assert_eq!(
            saved.restore(&liq_master).expect_err("정리매매 매수 거부"),
            domain::DraftRestoreError::Master(domain::MasterInstrumentError::LiquidationBuy)
        );
    }
    #[test]
    fn historical_portfolio_covers_reported_and_master_errors() {
        let (_, _, master) = signal_fixture("1");
        let amounts = domain::CurrencyAmounts::new(
            money("1000000", Currency::Krw),
            Some(money("416", Currency::Usd)),
        )
        .expect("통화 금액");
        let reported = domain::ReportedPortfolio::new(amounts, amounts, amounts, at(61, -18_000));
        let hist = domain::HistoricalPortfolio::capture(
            &signal_portfolio(&master, "1").with_reported(reported),
            std::slice::from_ref(&master),
        )
        .expect("과거 포트폴리오");
        assert!(!format!("{hist:?}").contains("1000000"));
        assert!(!format!("{:?}", hist.facts()).contains("1000000"));
        assert!(!format!("{:?}", hist.facts().holdings[0]).contains("90"));
        assert_eq!(
            domain::HistoricalPortfolio::from_saved_facts(
                hist.facts().clone(),
                std::slice::from_ref(&master),
            ),
            Ok(hist.clone())
        );
        let mut diff_rep = hist.facts().clone();
        diff_rep.reported = Some(domain::ReportedPortfolio::new(
            amounts,
            amounts,
            amounts,
            with_offset(at(61, -18_000), -14_400),
        ));
        assert_ne!(
            domain::HistoricalPortfolio::from_saved_facts(diff_rep, std::slice::from_ref(&master))
                .expect("다른 오프셋 보고"),
            hist
        );
        let mut delisted = master.row().clone();
        delisted.listing_status = ListingStatus::Delisted;
        let delisted_master = MasterInstrument::try_new(delisted).expect("폐지");
        assert_eq!(
            domain::HistoricalPortfolio::from_saved_facts(hist.facts().clone(), &[delisted_master]),
            Ok(hist.clone())
        );
        let mut unknown = master.row().clone();
        unknown.listing_market = MasterCode::try_new("XETRA").expect("미지");
        let unknown_master = MasterInstrument::try_new(unknown).expect("미지");
        assert_eq!(
            domain::HistoricalPortfolio::from_saved_facts(hist.facts().clone(), &[unknown_master]),
            Err(domain::HistoricalPortfolioError::Master(
                domain::MasterInstrumentError::UnknownMarket
            ))
        );
        let mut dup = hist.facts().clone();
        dup.holdings.push(dup.holdings[0].clone());
        assert_eq!(
            domain::HistoricalPortfolio::from_saved_facts(dup, &[master]),
            Err(domain::HistoricalPortfolioError::Portfolio(
                domain::PortfolioError::DuplicateInstrument
            ))
        );
    }
    #[test]
    fn historical_portfolio_covers_buying_power_variants_and_unsorted_facts() {
        let (_, _, master) = signal_fixture("1");
        let base_facts = domain::HistoricalPortfolio::capture(
            &signal_portfolio(&master, "1"),
            std::slice::from_ref(&master),
        )
        .expect("과거 포트폴리오")
        .facts()
        .clone();
        let mut bad_usd = base_facts.clone();
        bad_usd.usd.currency = Currency::Krw;
        assert_eq!(
            domain::HistoricalPortfolio::from_saved_facts(bad_usd, std::slice::from_ref(&master)),
            Err(domain::HistoricalPortfolioError::Portfolio(
                domain::PortfolioError::CurrencyMismatch
            ))
        );
        assert_eq!(
            domain::HistoricalCashMoney::try_new(money("-100", Currency::Krw)),
            Err(domain::PortfolioError::NegativeBuyingPower)
        );
        let zero_cash =
            domain::HistoricalCashMoney::try_new(money("0", Currency::Krw)).expect("영원");
        assert_eq!(zero_cash.currency(), Currency::Krw);
        assert_eq!(zero_cash.display_text().as_str(), "0");
        assert_eq!(format!("{zero_cash:?}"), "HistoricalCashMoney([비공개])");
        let mut wrong_cash = base_facts.clone();
        wrong_cash.krw.state = domain::HistoricalBuyingPowerState::Available(
            domain::HistoricalCashMoney::try_new(money("100", Currency::Usd)).expect("달러"),
        );
        assert_eq!(
            domain::HistoricalPortfolio::from_saved_facts(
                wrong_cash,
                std::slice::from_ref(&master)
            ),
            Err(domain::HistoricalPortfolioError::Portfolio(
                domain::PortfolioError::CurrencyMismatch
            ))
        );
        let mut err_preserved = base_facts;
        err_preserved.krw.state = domain::HistoricalBuyingPowerState::NegativeBuyingPower;
        err_preserved.usd.state = domain::HistoricalBuyingPowerState::CashLookupFailed(
            domain::CashObservationFailure::NotFound(domain::CashMissingData::Stock),
        );
        assert_eq!(
            format!("{:?}", err_preserved.krw.state),
            "NegativeBuyingPower"
        );
        assert!(format!("{:?}", err_preserved.usd.state).starts_with("CashLookupFailed("));
        assert_eq!(
            domain::HistoricalPortfolio::from_saved_facts(
                err_preserved.clone(),
                std::slice::from_ref(&master),
            )
            .expect("오류 현금 보존")
            .facts(),
            &err_preserved
        );
    }
    #[test]
    fn historical_portfolio_rejects_unsorted_holdings_facts() {
        let (_, _, master) = signal_fixture("1");
        let mut unsorted = domain::HistoricalPortfolio::capture(
            &signal_portfolio(&master, "1"),
            std::slice::from_ref(&master),
        )
        .expect("과거 포트폴리오")
        .facts()
        .clone();
        let mut second_row = master.row().clone();
        second_row.symbol = Symbol::try_new("AAA").expect("심볼");
        second_row.instrument_id = InstrumentId::try_new("sample-aaa").expect("ID");
        let second_master = MasterInstrument::try_new(second_row).expect("둘째 마스터");
        let mut second_holding = unsorted.holdings[0].clone();
        second_holding.instrument_id = InstrumentId::try_new("sample-aaa").expect("ID");
        second_holding.symbol = Symbol::try_new("AAA").expect("심볼");
        unsorted.holdings.push(second_holding);
        assert_eq!(
            domain::HistoricalPortfolio::from_saved_facts(unsorted, &[master, second_master]),
            Err(domain::HistoricalPortfolioError::ChangedFacts)
        );
    }
    #[test]
    fn signal_restore_covers_classifications_errors_and_details() {
        let (signal, input, master) = signal_fixture("99");
        let saved = input.capture(&signal, &master).expect("근거");
        let history = saved.restore_history(&master).expect("복원");
        assert_eq!(history.calibration_key(), &saved.evidence.calibration_key);
        assert!(!format!("{history:?}").contains("EXAMPLE"));
        let mut bad_sector_mkt = saved.clone();
        bad_sector_mkt.evidence.assessment.classifications.sectors[0].market =
            domain::Market::new(domain::MarketCode::try_new("XETRA").expect("미지"));
        assert_eq!(
            bad_sector_mkt.restore_history(&master),
            Err(domain::SignalRestoreError::Classification(
                domain::HistoricalClassificationError::InvalidMarket
            ))
        );
        let mut none_details = input;
        none_details.assessment.classifications.sectors[0].membership = None;
        none_details.assessment.classifications.sectors[0].reason = None;
        let none_saved = none_details.capture(&signal, &master).expect("세부 없음");
        none_saved.restore_history(&master).expect("세부 없음 복원");
        let mut mem_no_reason = saved.clone();
        mem_no_reason.evidence.assessment.classifications.sectors[0].membership =
            Some(domain::SectorMembership::Single);
        mem_no_reason.evidence.assessment.classifications.sectors[0].reason = None;
        assert_eq!(
            mem_no_reason.restore_history(&master),
            Err(domain::SignalRestoreError::Classification(
                domain::HistoricalClassificationError::MembershipWithoutReason
            ))
        );
        let mut bad_sec_mkt = saved.clone();
        bad_sec_mkt.evidence.assessment.classifications.security[0].market =
            domain::Market::new(domain::MarketCode::try_new("XETRA").expect("미지"));
        assert_eq!(
            bad_sec_mkt.restore_history(&master),
            Err(domain::SignalRestoreError::Classification(
                domain::HistoricalClassificationError::InvalidMarket
            ))
        );
        let mut dup_sec = saved;
        let sec = dup_sec.evidence.assessment.classifications.security[0].clone();
        dup_sec
            .evidence
            .assessment
            .classifications
            .security
            .push(sec);
        assert_eq!(
            dup_sec.restore_history(&master),
            Err(domain::SignalRestoreError::Classification(
                domain::HistoricalClassificationError::DuplicateInstrument
            ))
        );
    }
    fn kr_shifted(day: i64, seconds: u64) -> Timestamp {
        at(day, 32_400)
            .checked_add(
                domain::TimeSpan::try_new(domain::SpanSeconds::new(seconds), Nanosecond::new(0))
                    .expect("기간"),
            )
            .expect("시각")
    }
    fn kr_auction_session(day: i64, start_off: i32, end_off: i32) -> domain::MarketSession {
        let reg = domain::SessionData::try_new(
            domain::TimeInterval::try_new(kr_shifted(day, 0), kr_shifted(day, 21_600))
                .expect("구간"),
            Some(domain::AuctionBoundary::StartsAt(with_offset(
                kr_shifted(day, 0),
                start_off,
            ))),
        )
        .expect("정규장");
        let after = domain::SessionData::try_new(
            domain::TimeInterval::try_new(kr_shifted(day, 21_600), kr_shifted(day, 25_200))
                .expect("구간"),
            Some(domain::AuctionBoundary::EndsAt(with_offset(
                kr_shifted(day, 25_200),
                end_off,
            ))),
        )
        .expect("장후");
        domain::MarketSession::try_new(
            kr_shifted(day, 0)
                .date_at_offset(OffsetSeconds::new(32_400))
                .expect("날짜"),
            domain::SessionInformation::Kr {
                pre_market: None,
                regular_market: Some(reg),
                after_market: Some(after),
            },
        )
        .expect("세션")
    }
    #[test]
    fn signal_restore_covers_cost_target_and_sell_debug() {
        let (signal, input, master) = signal_fixture("99");
        let mut bad_maturity = input.clone();
        bad_maturity.maturity = at(50, -18_000);
        assert!(matches_signal_cost_err(
            &bad_maturity.capture(&signal, &master)
        ));
        let mut bad_comm = input.clone();
        bad_comm.commission = domain::CommissionRateFact::from_commissions_result(
            domain::Market::Kr,
            input.commission.rate(),
            input.commission.validity(),
        );
        assert!(matches_signal_cost_err(&bad_comm.capture(&signal, &master)));
        let mut bad_target = input;
        bad_target.assessment.target = Some(domain::StoredPriceObservation {
            price: Price::parse(&NumericInput::try_new("1000").expect("입력"), Currency::Krw)
                .expect("원화"),
            observed_at: at(61, -18_000),
            identity: domain::StoredPriceIdentity::Identified,
        });
        assert_eq!(
            bad_target.capture(&signal, &master),
            Err(domain::SignalRestoreError::Target(
                domain::DraftRestoreError::Master(domain::MasterInstrumentError::CurrencyMismatch)
            ))
        );
        let (sell, sell_input, sell_master) = signal_fixture("1");
        let sell_saved = sell_input.capture(&sell, &sell_master).expect("매도 근거");
        assert!(!format!("{:?}", sell_saved.expected.direction).contains('4'));
    }
    fn kr_active_row() -> MasterInstrumentRow {
        let mut row = kr_liquidation_row();
        row.instrument_id = InstrumentId::try_new("kr-auc").expect("식별자");
        row.security_type =
            MasterSecurityType::from_code(MasterCode::try_new("ETF").expect("유형"));
        row.korean_detail = Some(domain::KoreanMasterDetail {
            liquidation: domain::LiquidationTrading::Regular,
            krx: domain::TradingSuspension::Clear,
            nxt: domain::NxtTrading::Unsupported,
        });
        row
    }
    fn kr_auction_calendars(start_off: i32, end_off: i32) -> Vec<domain::MarketCalendar> {
        (60_i64..=68_i64)
            .map(|day| {
                let prev = day.checked_sub(1_i64).expect("전일");
                let prev_session = if day == 60_i64 {
                    kr_auction_session(prev, start_off, end_off)
                } else {
                    kr_auction_session(prev, 32_400_i32, 32_400_i32)
                };
                domain::MarketCalendar::try_new(
                    prev_session,
                    kr_auction_session(day, 32_400_i32, 32_400_i32),
                    kr_auction_session(
                        day.checked_add(1_i64).expect("익일"),
                        32_400_i32,
                        32_400_i32,
                    ),
                )
                .expect("달력")
            })
            .collect()
    }
    fn kr_signal_cost_fixture() -> SignalCostFixture {
        let row = kr_active_row();
        let master = MasterInstrument::try_new(row.clone()).expect("조회");
        let kr_price = Price::parse(
            &NumericInput::try_new("51000").expect("입력"),
            Currency::Krw,
        )
        .expect("원화");
        let input = EstimateRestoreEvidence {
            master: row,
            side: OrderSide::Buy,
            candles: kr_candles("kr-auc"),
            reference_price: kr_price,
            reference_at: at(61, 32_400),
            task_at: at(61, 32_400),
            horizon: Horizon::new(HorizonSelection::FewDays),
            permission: HoursPermissionState::Disabled,
            appetite: RiskAppetite::Neutral,
        };
        let task = original(&input, &master);
        let stored = input.capture(&task, &master).expect("과제 근거");
        let zero = domain::CostRate::try_new(
            domain::Ratio::parse(&NumericInput::try_new("0").expect("입력")).expect("비율"),
        )
        .expect("요율");
        let commission = domain::CommissionRateFact::from_commissions_result(
            domain::Market::Kr,
            zero,
            domain::CommissionValidity::try_new(None, None).expect("기간"),
        );
        let tax = domain::TradeTaxFact::from_schedule_result(domain::Market::Kr, zero, zero);
        let maturity = kr_shifted(67, 25_200);
        let cost = domain::TradeCost::from_facts(
            Some(commission.clone()),
            Some(tax.clone()),
            domain::EstimateCostPeriod::from_calendar_result(&task, maturity).expect("기간"),
        )
        .expect("비용");
        let eligible = domain::CostEligibleTask::assess(task, &cost).expect("적격");
        SignalCostFixture {
            master,
            stored,
            eligible,
            commission,
            tax,
            maturity,
        }
    }
    fn kr_signal_assessment(
        cost: &SignalCostFixture,
        model: &SignalModelFixture,
        cals: Vec<domain::MarketCalendar>,
    ) -> (
        domain::Portfolio,
        domain::PriceObservation,
        domain::HistoricalSignalContext,
    ) {
        let portfolio = domain::Portfolio::new(
            vec![],
            domain::BuyingPower::new(money("1000000", Currency::Krw), at(61, 32_400)),
            domain::BuyingPower::new(money("0", Currency::Usd), at(61, 32_400)),
        )
        .expect("포트폴리오");
        let (assignments, security) = signal_classifications(&cost.master);
        let target = cost
            .master
            .price_observation(
                Price::parse(
                    &NumericInput::try_new("51000").expect("입력"),
                    Currency::Krw,
                )
                .expect("원화"),
                at(61, 32_400),
            )
            .expect("시세");
        let historical = domain::HistoricalSignalContext {
            portfolio: Some(
                domain::HistoricalPortfolio::capture(
                    &portfolio,
                    std::slice::from_ref(&cost.master),
                )
                .expect("포트폴리오 근거"),
            ),
            rate: None,
            policy: domain::Policy::new(domain::InvestorProfile::default()),
            classifications: domain::HistoricalClassifications::capture(&assignments, &security),
            items: vec![model.item.clone()],
            disclosures: vec![],
            tools: vec![],
            calendars: cals,
            evaluated_at: at(61, 32_400),
            target: Some(domain::StoredPriceObservation::from_price_observation(
                &target,
            )),
            calibration_head: Some(model.head.clone()),
        };
        (portfolio, target, historical)
    }
    #[test]
    fn signal_restore_covers_kr_auction_boundary_offsets() {
        let cost = kr_signal_cost_fixture();
        let model = signal_model_fixture(&cost, "99");
        let (portfolio, target, historical) =
            kr_signal_assessment(&cost, &model, kr_auction_calendars(32_400_i32, 32_400_i32));
        let valuation = portfolio.evaluate(None);
        let (assignments, security) = signal_classifications(&cost.master);
        let ledger = domain::SignalSourceLedger::new(&historical.items, &[], &[]);
        let facts = domain::SignalAssessmentFacts::new(
            domain::SignalTiming::new(at(61, 32_400), &historical.calendars),
            Some(target),
            Some(model.head),
        );
        let context = domain::SignalContext::new(
            Some(&valuation),
            &historical.policy,
            (&assignments, &security),
            &ledger,
            facts,
        );
        let signal = domain::SignalDecision::assess(&model.estimate, &context)
            .into_signal()
            .expect("신호");
        let input = domain::SignalRestoreEvidence {
            estimate: cost.stored,
            commission: cost.commission,
            tax: cost.tax,
            maturity: cost.maturity,
            calibration: model.snapshot,
            calibration_key: model.key,
            probability: model.probability,
            source: Some(model.source),
            submitted_at: at(61, -18_000),
            margin: model.margin,
            assessment: historical,
        };
        let auction_saved = input.capture(&signal, &cost.master).expect("경매 근거");
        auction_saved
            .restore_history(&cost.master)
            .expect("경매 복원");
        let mut changed_start = auction_saved.clone();
        changed_start.evidence.assessment.calendars = kr_auction_calendars(0_i32, 32_400_i32);
        assert_eq!(
            changed_start.restore_history(&cost.master),
            Err(domain::SignalRestoreError::ChangedSignal)
        );
        let mut changed_end = auction_saved;
        changed_end.evidence.assessment.calendars = kr_auction_calendars(32_400_i32, 0_i32);
        assert_eq!(
            changed_end.restore_history(&cost.master),
            Err(domain::SignalRestoreError::ChangedSignal)
        );
    }
    fn matches_signal_cost_err(
        result: &Result<domain::StoredSignalEvidence, domain::SignalRestoreError>,
    ) -> bool {
        match result {
            Err(domain::SignalRestoreError::Cost(_)) => true,
            Ok(_) | Err(_) => false,
        }
    }
    fn alt_buy_record(id_text: &str) -> domain::BuySignalRecord {
        let mut cost = signal_cost_fixture();
        let mut alt_input = evidence(HorizonSelection::FewDays, RiskAppetite::Conservative);
        alt_input.master.security_type =
            MasterSecurityType::from_code(MasterCode::try_new("ETF").expect("유형"));
        let alt_task = original(&alt_input, &cost.master);
        let alt_cost = domain::TradeCost::from_facts(
            Some(cost.commission.clone()),
            Some(cost.tax.clone()),
            domain::EstimateCostPeriod::from_calendar_result(&alt_task, cost.maturity)
                .expect("기간"),
        )
        .expect("비용");
        cost.eligible = domain::CostEligibleTask::assess(alt_task, &alt_cost).expect("적격");
        let model = signal_model_fixture(&cost, "99");
        let assessment = signal_assessment(&cost.master, &model, "99");
        let valuation = signal_portfolio(&cost.master, "99").evaluate(None);
        let (assignments, security) = signal_classifications(&cost.master);
        let ledger = domain::SignalSourceLedger::new(&assessment.items, &[], &[]);
        let target = cost
            .master
            .price_observation(price("105"), at(61, -18_000))
            .expect("시세");
        let facts = domain::SignalAssessmentFacts::new(
            domain::SignalTiming::new(assessment.evaluated_at, &assessment.calendars),
            Some(target),
            Some(model.head),
        );
        let context = domain::SignalContext::new(
            Some(&valuation),
            &assessment.policy,
            (&assignments, &security),
            &ledger,
            facts,
        );
        let domain::SignalDecision::Buy(buy) =
            domain::SignalDecision::assess(&model.estimate, &context)
        else {
            panic!("매수 필요");
        };
        domain::BuySignalRecord::from_signal_checked(
            domain::BuySignalOccurrenceId::try_new(id_text).expect("ID"),
            &buy,
            at(61, -18_000),
            &assessment.calendars,
        )
        .expect("다른 발생")
    }
    #[test]
    fn occurrence_errors_cover_not_buy_changed_id_signal_and_calendar() {
        let (signal, input, master) = signal_fixture("99");
        let buy = match &signal {
            domain::RaisedSignal::Buy(value) => value.clone(),
            domain::RaisedSignal::Sell(_) => panic!("매수 필요"),
        };
        let id = domain::BuySignalOccurrenceId::try_new("occ-900").expect("ID");
        let record = domain::BuySignalRecord::from_signal_checked(
            id,
            &buy,
            at(61, -18_000),
            &input.assessment.calendars,
        )
        .expect("발생");
        let saved_sig = input.capture(&signal, &master).expect("신호");
        let (sell, sell_input, sell_master) = signal_fixture("1");
        let saved_sell = sell_input.capture(&sell, &sell_master).expect("매도");
        assert_eq!(
            domain::StoredSignalOccurrence::capture(&record, saved_sell, &sell_master),
            Err(domain::SignalOccurrenceRestoreError::NotBuy)
        );
        let other_record = alt_buy_record("occ-900");
        assert_eq!(
            domain::StoredSignalOccurrence::capture(&other_record, saved_sig.clone(), &master),
            Err(domain::SignalOccurrenceRestoreError::ChangedOccurrence)
        );
        let occ = domain::StoredSignalOccurrence::capture(&record, saved_sig, &master)
            .expect("발생 근거");
        let mut bad_id = occ.clone();
        bad_id.id = domain::BuySignalOccurrenceId::try_new("occ-901").expect("다른 ID");
        assert_eq!(
            bad_id.restore(&master),
            Err(domain::SignalOccurrenceRestoreError::ChangedOccurrence)
        );
        let mut bad_sig = occ.clone();
        bad_sig.signal.evidence.assessment.items.clear();
        assert_eq!(
            bad_sig.restore(&master),
            Err(domain::SignalOccurrenceRestoreError::Signal(
                domain::SignalRestoreError::NoSignal(domain::NoSignalReason::MissingSource)
            ))
        );
        let mut bad_cal = occ;
        bad_cal.raised_at = at(90, -18_000);
        assert_eq!(
            bad_cal.restore(&master),
            Err(domain::SignalOccurrenceRestoreError::Evaluation(
                domain::NoSignalReason::Expired
            ))
        );
    }
    #[test]
    fn buy_quantity_with_history_matches_direct_buy_quantity_calculation() {
        let (signal, input, master) = signal_fixture("99");
        let buy = match &signal {
            domain::RaisedSignal::Buy(value) => value.clone(),
            domain::RaisedSignal::Sell(_) => panic!("매수 필요"),
        };
        let id = domain::BuySignalOccurrenceId::try_new("occ-902").expect("ID");
        let record = domain::BuySignalRecord::from_signal_checked(
            id,
            &buy,
            at(61, -18_000),
            &input.assessment.calendars,
        )
        .expect("발생");
        let hist_occ = domain::HistoricalSignalOccurrence::from_record(&record);
        let current_buy =
            domain::CurrentBuySignal::try_from_history(buy, hist_occ.clone()).expect("현재 매수");
        let valuation = signal_portfolio(&master, "99").evaluate(None);
        let (assignments, security) = signal_classifications(&master);
        let target = master
            .price_observation(price("105"), at(61, -18_000))
            .expect("시세");
        let facts = domain::BuyQuantityFacts::new(
            &valuation,
            &input.assessment.policy,
            (&assignments, &security),
            &target,
            (domain::OrderFxRates::missing(), &input.commission),
        );
        let decision = domain::calculate_buy_quantity_with_history(
            &current_buy,
            &facts,
            at(61, -18_000),
            &input.assessment.calendars,
            std::slice::from_ref(&hist_occ),
        );
        assert_eq!(
            decision,
            domain::calculate_buy_quantity(
                &current_buy,
                &facts,
                at(61, -18_000),
                &input.assessment.calendars,
                std::slice::from_ref(&record),
            )
        );
    }

    fn non_tradable_kr_masters(base: &MasterInstrument) -> Vec<MasterInstrument> {
        let mut suspended = base.row().clone();
        suspended.korean_detail = Some(domain::KoreanMasterDetail {
            liquidation: domain::LiquidationTrading::Regular,
            krx: domain::TradingSuspension::Suspended,
            nxt: domain::NxtTrading::Unsupported,
        });
        let mut delisted = base.row().clone();
        delisted.listing_status = ListingStatus::Delisted;
        let mut scheduled = base.row().clone();
        scheduled.listing_status = ListingStatus::Scheduled;
        let mut liquidation = base.row().clone();
        liquidation.korean_detail = Some(domain::KoreanMasterDetail {
            liquidation: domain::LiquidationTrading::Liquidation,
            krx: domain::TradingSuspension::Clear,
            nxt: domain::NxtTrading::Unsupported,
        });
        let mut unknown_type = base.row().clone();
        unknown_type.security_type =
            MasterSecurityType::from_code(MasterCode::try_new("UNKNOWN_SEC").expect("유형"));
        let mut missing_status = base.row().clone();
        missing_status.korean_detail = None;
        [
            suspended,
            delisted,
            scheduled,
            liquidation,
            unknown_type,
            missing_status,
        ]
        .into_iter()
        .map(|row| MasterInstrument::try_new(row).expect("비거래 마스터"))
        .collect()
    }

    fn kr_non_tradable_bundle() -> (
        SignalCostFixture,
        domain::RaisedSignal,
        domain::SignalRestoreEvidence,
        domain::BuySignalRecord,
        domain::DraftRestoreEvidence,
    ) {
        let cost = kr_signal_cost_fixture();
        let model = signal_model_fixture(&cost, "99");
        let (portfolio, target, historical) =
            kr_signal_assessment(&cost, &model, kr_auction_calendars(32_400_i32, 32_400_i32));
        let valuation = portfolio.evaluate(None);
        let (assignments, security) = signal_classifications(&cost.master);
        let ledger = domain::SignalSourceLedger::new(&historical.items, &[], &[]);
        let facts = domain::SignalAssessmentFacts::new(
            domain::SignalTiming::new(at(61, 32_400), &historical.calendars),
            Some(target.clone()),
            Some(model.head),
        );
        let context = domain::SignalContext::new(
            Some(&valuation),
            &historical.policy,
            (&assignments, &security),
            &ledger,
            facts,
        );
        let signal = domain::SignalDecision::assess(&model.estimate, &context)
            .into_signal()
            .expect("신호");
        let buy = match &signal {
            domain::RaisedSignal::Buy(value) => value.clone(),
            domain::RaisedSignal::Sell(_) => panic!("매수 필요"),
        };
        let record = domain::BuySignalRecord::from_signal_checked(
            domain::BuySignalOccurrenceId::try_new("occ-nt-1").expect("ID"),
            &buy,
            at(61, 32_400),
            &historical.calendars,
        )
        .expect("발생");
        let input = domain::SignalRestoreEvidence {
            estimate: cost.stored.clone(),
            commission: cost.commission.clone(),
            tax: cost.tax.clone(),
            maturity: cost.maturity,
            calibration: model.snapshot,
            calibration_key: model.key,
            probability: model.probability,
            source: Some(model.source),
            submitted_at: at(61, -18_000),
            margin: model.margin,
            assessment: historical,
        };
        let saved_draft = kr_non_tradable_draft(&cost, &target);
        (cost, signal, input, record, saved_draft)
    }

    fn kr_non_tradable_draft(
        cost: &SignalCostFixture,
        target: &domain::PriceObservation,
    ) -> domain::DraftRestoreEvidence {
        let range = domain::ProposalPriceRange::from_estimate_task(
            cost.eligible.task().range_facts(),
            target.clone(),
        )
        .expect("범위");
        let id = domain::ProposalId::new(domain::ClientOrderId::try_new("nt-draft").expect("ID"));
        let intent = domain::OrderIntent::new(
            domain::AccountId::new(7),
            cost.master.order_instrument().expect("종목"),
            id.as_client_order_id().clone(),
            domain::OrderTerms::KrMarket {
                side: OrderSide::Buy,
                quantity: domain::WholeOrderQuantity::try_new(
                    Quantity::parse(&NumericInput::try_new("2").expect("입력")).expect("수량"),
                )
                .expect("정수"),
            },
        )
        .expect("의도");
        let lifetime = domain::ProposalLifetime::default();
        let draft =
            domain::Proposal::new(id, intent, at(61, 32_400), range, lifetime).expect("초안");
        domain::DraftRestoreEvidence::capture(
            &draft,
            cost.stored.clone(),
            &cost.master,
            lifetime,
            target,
        )
        .expect("초안 근거")
    }

    #[test]
    fn historical_restore_survives_non_tradable_current_master_while_live_draft_rejects_it() {
        let (cost, signal, input, record, saved_draft) = kr_non_tradable_bundle();
        let saved_signal = input.capture(&signal, &cost.master).expect("신호 근거");
        let saved_occ =
            domain::StoredSignalOccurrence::capture(&record, saved_signal.clone(), &cost.master)
                .expect("이력 근거");
        for non_tradable in non_tradable_kr_masters(&cost.master) {
            cost.stored
                .restore_history(&non_tradable)
                .expect("과거 과제 복원 허용");
            let history = saved_signal
                .restore_history(&non_tradable)
                .expect("과거 신호 복원 허용");
            assert_eq!(history.display_facts(), saved_signal.expected);
            let occ = saved_occ
                .restore(&non_tradable)
                .expect("과거 발생 복원 허용");
            assert_eq!(occ.interval(), record.interval());
            saved_draft
                .restore(&non_tradable)
                .expect_err("실거래 초안 복원 거부");
            input
                .capture(&signal, &non_tradable)
                .expect_err("실거래 신호 저장 거부");
        }
    }

    #[test]
    fn historical_target_observation_covers_unidentified_and_error_paths() {
        let (signal, input, master) = signal_fixture("99");
        let saved = input.capture(&signal, &master).expect("식별 저장");
        let mut delisted_row = master.row().clone();
        delisted_row.listing_status = ListingStatus::Delisted;
        let delisted = MasterInstrument::try_new(delisted_row).expect("폐지");
        let mut unid = saved.clone();
        unid.evidence.assessment.target = Some(domain::StoredPriceObservation {
            price: price("105"),
            observed_at: at(61, -18_000),
            identity: domain::StoredPriceIdentity::Unidentified,
        });
        assert_eq!(
            unid.restore_history(&delisted),
            Err(domain::SignalRestoreError::NoSignal(
                domain::NoSignalReason::Unavailable(domain::SignalUnavailable::InvalidTargetPrice)
            ))
        );
        let mut overflow = saved;
        let max_ts = Timestamp::from_parts(
            UnixSeconds::new(253_402_300_799_i64),
            Nanosecond::new(0),
            OffsetSeconds::new(0),
        )
        .expect("최대 시각");
        overflow.evidence.assessment.target = Some(domain::StoredPriceObservation {
            price: price("105"),
            observed_at: max_ts,
            identity: domain::StoredPriceIdentity::Unidentified,
        });
        assert_eq!(
            overflow.restore_history(&delisted),
            Err(domain::SignalRestoreError::Target(
                domain::DraftRestoreError::Time(domain::TimeError::Overflow)
            ))
        );
    }
    #[test]
    fn historical_identified_target_rejects_currency_and_offset_errors() {
        let (sig_id, input_id, master_id) = signal_fixture("99");
        let mut delisted_row = master_id.row().clone();
        delisted_row.listing_status = ListingStatus::Delisted;
        let delisted = MasterInstrument::try_new(delisted_row).expect("폐지");
        let saved_id = input_id.capture(&sig_id, &master_id).expect("식별 저장");
        let mut wrong_cur = saved_id.clone();
        wrong_cur.evidence.assessment.target = Some(domain::StoredPriceObservation {
            price: Price::parse(&NumericInput::try_new("1000").expect("입력"), Currency::Krw)
                .expect("원화"),
            observed_at: at(61, -18_000),
            identity: domain::StoredPriceIdentity::Identified,
        });
        assert_eq!(
            wrong_cur.restore_history(&delisted),
            Err(domain::SignalRestoreError::Target(
                domain::DraftRestoreError::Master(domain::MasterInstrumentError::CurrencyMismatch)
            ))
        );
        let mut wrong_offset = saved_id;
        wrong_offset.evidence.assessment.target = Some(domain::StoredPriceObservation {
            price: price("105"),
            observed_at: at(61, 0),
            identity: domain::StoredPriceIdentity::Identified,
        });
        assert_eq!(
            wrong_offset.restore_history(&delisted),
            Err(domain::SignalRestoreError::Target(
                domain::DraftRestoreError::Master(
                    domain::MasterInstrumentError::InvalidPriceObservation
                )
            ))
        );
    }

    fn dual_market_abcdef_rows() -> (MasterInstrumentRow, MasterInstrumentRow) {
        let kr_row = MasterInstrumentRow {
            listing_market: MasterCode::try_new("KOSPI").expect("시장"),
            symbol: Symbol::try_new("ABCDEF").expect("심볼"),
            instrument_id: InstrumentId::try_new("dual-id").expect("식별자"),
            name: DisplayInstrumentName::try_new("동일 심볼 한국").expect("이름"),
            security_type: MasterSecurityType::from_code(
                MasterCode::try_new("STOCK").expect("유형"),
            ),
            currency: Currency::Krw,
            listing_status: ListingStatus::Active,
            common_share: CommonShare::Common,
            korean_detail: Some(domain::KoreanMasterDetail {
                liquidation: domain::LiquidationTrading::Regular,
                krx: domain::TradingSuspension::Clear,
                nxt: domain::NxtTrading::Unsupported,
            }),
            observed_at: at(61, 32_400),
        };
        let us_row = MasterInstrumentRow {
            listing_market: MasterCode::try_new("NASDAQ").expect("시장"),
            symbol: Symbol::try_new("ABCDEF").expect("심볼"),
            instrument_id: InstrumentId::try_new("dual-id").expect("식별자"),
            name: DisplayInstrumentName::try_new("동일 심볼 미국").expect("이름"),
            security_type: MasterSecurityType::from_code(
                MasterCode::try_new("STOCK").expect("유형"),
            ),
            currency: Currency::Usd,
            listing_status: ListingStatus::Active,
            common_share: CommonShare::Common,
            korean_detail: None,
            observed_at: at(61, -18_000),
        };
        (kr_row, us_row)
    }

    #[test]
    fn historical_holding_identity_rejects_symbol_reuse_and_currency_mismatch() {
        let (_, _, master) = signal_fixture("1");
        let base_portfolio = signal_portfolio(&master, "1");
        assert_eq!(
            domain::HistoricalPortfolio::capture(&base_portfolio, &[]),
            Err(domain::HistoricalPortfolioError::MasterMissingOrAmbiguous)
        );
        assert_eq!(
            domain::HistoricalPortfolio::capture(
                &base_portfolio,
                &[master.clone(), master.clone()]
            ),
            Err(domain::HistoricalPortfolioError::MasterMissingOrAmbiguous)
        );
        let hist =
            domain::HistoricalPortfolio::capture(&base_portfolio, std::slice::from_ref(&master))
                .expect("캡처");
        let mut reused_row = master.row().clone();
        reused_row.instrument_id = InstrumentId::try_new("reused-id").expect("새 식별자");
        let reused_master = MasterInstrument::try_new(reused_row).expect("심볼 재사용 마스터");
        assert_eq!(
            domain::HistoricalPortfolio::from_saved_facts(
                hist.facts().clone(),
                std::slice::from_ref(&reused_master),
            ),
            Err(domain::HistoricalPortfolioError::MasterMissingOrAmbiguous)
        );
        assert_eq!(
            domain::HistoricalPortfolio::from_saved_facts(
                hist.facts().clone(),
                &[master.clone(), reused_master],
            ),
            Err(domain::HistoricalPortfolioError::MasterMissingOrAmbiguous)
        );
        let mut renamed_row = master.row().clone();
        renamed_row.symbol = Symbol::try_new("NEWNAME").expect("변경 심볼");
        assert_eq!(
            domain::HistoricalPortfolio::from_saved_facts(
                hist.facts().clone(),
                &[MasterInstrument::try_new(renamed_row).expect("변경")],
            ),
            Err(domain::HistoricalPortfolioError::InstrumentMismatch)
        );
    }
    #[test]
    fn historical_holding_rejects_acquisition_and_current_price_currency_mismatch() {
        let (_, _, master) = signal_fixture("1");
        let hist = domain::HistoricalPortfolio::capture(
            &signal_portfolio(&master, "1"),
            std::slice::from_ref(&master),
        )
        .expect("캡처");
        let mut wrong_acq = hist.facts().clone();
        wrong_acq.holdings[0].acquisition_price =
            domain::AcquisitionPrice::try_new(money("90", Currency::Krw)).expect("원화 원가");
        assert_eq!(
            domain::HistoricalPortfolio::from_saved_facts(wrong_acq, std::slice::from_ref(&master)),
            Err(domain::HistoricalPortfolioError::InstrumentMismatch)
        );
        let mut bad_cur_price = hist.facts().clone();
        bad_cur_price.holdings[0].current_price = Some(
            Price::parse(&NumericInput::try_new("90").expect("입력"), Currency::Krw).expect("원화"),
        );
        assert_eq!(
            domain::HistoricalPortfolio::from_saved_facts(
                bad_cur_price,
                std::slice::from_ref(&master),
            ),
            Err(domain::HistoricalPortfolioError::Holding(
                domain::HoldingError::CurrencyMismatch
            ))
        );
    }

    #[test]
    fn identical_symbol_across_kr_and_us_markets_is_rejected_for_estimate_and_holding() {
        let (kr_abcdef_row, us_abcdef_row) = dual_market_abcdef_rows();
        let kr_abcdef = MasterInstrument::try_new(kr_abcdef_row.clone()).expect("한국");
        let us_abcdef = MasterInstrument::try_new(us_abcdef_row).expect("미국");
        let kr_portfolio = domain::Portfolio::new(
            vec![
                domain::Holding::new(
                    kr_abcdef.order_instrument().expect("종목"),
                    Quantity::parse(&NumericInput::try_new("3").expect("입력")).expect("수량"),
                    domain::AcquisitionPrice::try_new(money("50000", Currency::Krw)).expect("원가"),
                    domain::HoldingObservation::new(None, None, at(61, 32_400)),
                )
                .expect("보유"),
            ],
            domain::BuyingPower::new(money("100000", Currency::Krw), at(61, 32_400)),
            domain::BuyingPower::new(money("0", Currency::Usd), at(61, 32_400)),
        )
        .expect("포트");
        let kr_hist =
            domain::HistoricalPortfolio::capture(&kr_portfolio, std::slice::from_ref(&kr_abcdef))
                .expect("한국 캡처");
        assert_eq!(
            domain::HistoricalPortfolio::from_saved_facts(
                kr_hist.facts().clone(),
                std::slice::from_ref(&us_abcdef),
            ),
            Err(domain::HistoricalPortfolioError::InstrumentMismatch)
        );
        let kr_price = Price::parse(
            &NumericInput::try_new("51000").expect("입력"),
            Currency::Krw,
        )
        .expect("가격");
        let est_input = EstimateRestoreEvidence {
            master: kr_abcdef_row,
            side: OrderSide::Buy,
            candles: kr_candles("dual-id"),
            reference_price: kr_price,
            reference_at: at(61, 32_400),
            task_at: at(61, 32_400),
            horizon: Horizon::new(HorizonSelection::FewDays),
            permission: HoursPermissionState::Disabled,
            appetite: RiskAppetite::Neutral,
        };
        let est_task = original(&est_input, &kr_abcdef);
        assert_eq!(
            est_input.capture(&est_task, &us_abcdef),
            Err(EstimateRestoreError::CurrentInstrumentMismatch)
        );
        let est_saved = est_input.capture(&est_task, &kr_abcdef).expect("저장");
        assert_eq!(
            est_saved.restore_history(&us_abcdef),
            Err(EstimateRestoreError::CurrentInstrumentMismatch)
        );
    }

    fn candle_offset_variants(first_candle: &Candle) -> [Candle; 2] {
        let PriceAdjustment::Adjusted(basis) = first_candle.adjustment() else {
            panic!("수정 봉");
        };
        [
            Candle::try_new(
                first_candle.instrument().clone(),
                first_candle.time(),
                first_candle.adjustment(),
                first_candle.values(),
                CandleObservation::new(
                    with_offset(first_candle.collected_at(), 32_400),
                    first_candle.status(),
                ),
            )
            .expect("봉"),
            Candle::try_new(
                first_candle.instrument().clone(),
                first_candle.time(),
                PriceAdjustment::Adjusted(AdjustmentBasis::new(with_offset(
                    *basis.as_timestamp(),
                    32_400,
                ))),
                first_candle.values(),
                CandleObservation::new(first_candle.collected_at(), first_candle.status()),
            )
            .expect("봉"),
        ]
    }
    fn twelve_hour_candles(offset: i32) -> Vec<Candle> {
        (0_i64..61_i64)
            .map(|day| {
                let value = if day.checked_rem(2_i64) == Some(0_i64) {
                    price("100")
                } else {
                    price("103")
                };
                let utc = 1_767_225_600_i64
                    .checked_add(day.checked_mul(86_400).expect("일"))
                    .expect("시각")
                    .checked_add(43_200)
                    .expect("정오");
                let ts = Timestamp::from_parts(
                    UnixSeconds::new(utc),
                    Nanosecond::new(0),
                    OffsetSeconds::new(offset),
                )
                .expect("시각");
                Candle::try_new(
                    InstrumentId::try_new("sample").expect("식별자"),
                    CandleTime::try_new(ts, CandlePeriod::Daily).expect("일봉"),
                    PriceAdjustment::Adjusted(AdjustmentBasis::new(at(70, 0))),
                    Ohlcv::try_new(
                        value,
                        value,
                        value,
                        value,
                        Quantity::parse(&NumericInput::try_new("3").expect("입력")).expect("수량"),
                    )
                    .expect("봉"),
                    CandleObservation::new(at(70, 0), CandleStatus::Finalized),
                )
                .expect("봉")
            })
            .collect()
    }
    #[test]
    fn estimate_restore_kills_candle_time_offset_mutation_on_same_utc_instant() {
        let mut input = evidence(HorizonSelection::FewDays, RiskAppetite::Neutral);
        input.candles = twelve_hour_candles(-43_200);
        input.reference_at = at(62, -18_000);
        input.task_at = at(62, -18_000);
        let master = MasterInstrument::try_new(row()).expect("마스터");
        let saved = input
            .capture(&original(&input, &master), &master)
            .expect("과제 근거");
        let mut shifted_candles = saved;
        shifted_candles.evidence.candles = twelve_hour_candles(43_200);
        assert_eq!(
            shifted_candles.restore_history(&master),
            Err(EstimateRestoreError::ChangedEvidence)
        );
    }
    #[test]
    fn estimate_restore_kills_task_master_and_candle_offset_mutations() {
        let input = evidence(HorizonSelection::FewDays, RiskAppetite::Neutral);
        let master = MasterInstrument::try_new(row()).expect("마스터");
        let saved = input
            .capture(&original(&input, &master), &master)
            .expect("과제 근거");
        let mut task_offset_both = saved.clone();
        let shifted_task_at = with_offset(task_offset_both.evidence.task_at, -14_400);
        task_offset_both.evidence.task_at = shifted_task_at;
        task_offset_both.expected.created_at = shifted_task_at;
        assert_eq!(
            task_offset_both.restore_history(&master),
            Err(EstimateRestoreError::ChangedEvidence)
        );
        let mut master_obs_offset = saved.clone();
        master_obs_offset.evidence.master.observed_at =
            with_offset(master_obs_offset.evidence.master.observed_at, -14_400);
        assert_eq!(
            master_obs_offset.restore_history(&master),
            Err(EstimateRestoreError::ChangedEvidence)
        );
        for candle in candle_offset_variants(&saved.evidence.candles[0]) {
            let mut mutated = saved.clone();
            mutated.evidence.candles[0] = candle;
            assert_eq!(
                mutated.restore_history(&master),
                Err(EstimateRestoreError::ChangedEvidence)
            );
        }
    }

    #[test]
    fn draft_restore_kills_created_and_expiry_paired_offset_mutation() {
        let (draft, estimate, draft_master, lifetime, card) = draft_fixture();
        let mut draft_created_both =
            domain::DraftRestoreEvidence::capture(&draft, estimate, &draft_master, lifetime, &card)
                .expect("초안 근거");
        draft_created_both.created_at = with_offset(draft_created_both.created_at, -14_400);
        draft_created_both.expires_at = with_offset(draft_created_both.expires_at, -14_400);
        assert_eq!(
            draft_created_both
                .restore(&draft_master)
                .expect_err("초안 생성 오프셋"),
            domain::DraftRestoreError::ChangedDraft
        );
    }

    #[test]
    fn signal_restore_kills_capture_eval_and_target_offset_mutations() {
        let (_, evidence_99, master) = signal_fixture("99");
        let (signal_98, _, _) = signal_fixture("98");
        assert_eq!(
            evidence_99.capture(&signal_98, &master),
            Err(domain::SignalRestoreError::ChangedSignal)
        );
        let (signal_99, input_99, master_99) = signal_fixture("99");
        let saved = input_99.capture(&signal_99, &master_99).expect("근거");
        let mut eval_offset = saved.clone();
        eval_offset.evidence.assessment.evaluated_at =
            with_offset(eval_offset.evidence.assessment.evaluated_at, -14_400);
        assert_eq!(
            eval_offset.restore_history(&master_99),
            Err(domain::SignalRestoreError::ChangedSignal)
        );
        let mut target_offset = saved;
        let old_target = target_offset.evidence.assessment.target.expect("시세");
        target_offset.evidence.assessment.target = Some(domain::StoredPriceObservation {
            observed_at: with_offset(old_target.observed_at, -14_400),
            ..old_target
        });
        assert_eq!(
            target_offset.restore_history(&master_99),
            Err(domain::SignalRestoreError::ChangedSignal)
        );
    }
    #[test]
    fn signal_restore_kills_expected_display_field_mutations() {
        let (signal_99, input_99, master_99) = signal_fixture("99");
        let saved = input_99.capture(&signal_99, &master_99).expect("근거");
        let expected_changes: [fn(&mut domain::StoredSignalEvidence); 6] = [
            |value| {
                value.expected.probability = domain::AiProbability::parse(
                    &domain::AiProbabilityInput::try_new("98").expect("입력"),
                )
                .expect("확률")
                .into_probability();
            },
            |value| value.expected.operation = domain::SignalOperation::EligibleBeforeE91,
            |value| {
                value.expected.buy_threshold =
                    domain::Ratio::parse(&NumericInput::try_new("0.51").expect("입력"))
                        .expect("비율");
            },
            |value| {
                value.expected.sell_threshold =
                    domain::Ratio::parse(&NumericInput::try_new("0.49").expect("입력"))
                        .expect("비율");
            },
            |value| {
                value.expected.breakeven_threshold =
                    domain::Ratio::parse(&NumericInput::try_new("0.505").expect("입력"))
                        .expect("비율");
            },
            |value| {
                value.expected.direction = domain::HistoricalSignalDirection::Buy {
                    limit: domain::HistoricalSignalLimit::new(money("1234", Currency::Krw)),
                };
            },
        ];
        for change in expected_changes {
            let mut mutated = saved.clone();
            change(&mut mutated);
            assert_eq!(
                mutated.restore_history(&master_99),
                Err(domain::SignalRestoreError::ChangedSignal)
            );
        }
    }

    #[test]
    fn occurrence_restore_kills_interval_start_and_end_offset_mutations() {
        let (signal_99, input_99, master_99) = signal_fixture("99");
        let saved = input_99.capture(&signal_99, &master_99).expect("근거");
        let buy = match &signal_99 {
            domain::RaisedSignal::Buy(value) => value,
            domain::RaisedSignal::Sell(_) => panic!("매수"),
        };
        let record = domain::BuySignalRecord::from_signal_checked(
            domain::BuySignalOccurrenceId::try_new("occ-s6").expect("ID"),
            buy,
            at(61, -18_000),
            &input_99.assessment.calendars,
        )
        .expect("발생");
        let occ =
            domain::StoredSignalOccurrence::capture(&record, saved, &master_99).expect("이력 근거");
        let mut end_offset = occ.clone();
        end_offset.interval = domain::TimeInterval::try_new(
            occ.interval.start(),
            with_offset(occ.interval.end(), -14_400),
        )
        .expect("종료 오프셋");
        assert_eq!(
            end_offset.restore(&master_99),
            Err(domain::SignalOccurrenceRestoreError::ChangedOccurrence)
        );
        let mut start_offset = occ;
        start_offset.interval = domain::TimeInterval::try_new(
            with_offset(start_offset.interval.start(), -14_400),
            start_offset.interval.end(),
        )
        .expect("시작 오프셋");
        assert_eq!(
            start_offset.restore(&master_99),
            Err(domain::SignalOccurrenceRestoreError::ChangedOccurrence)
        );
    }
}
