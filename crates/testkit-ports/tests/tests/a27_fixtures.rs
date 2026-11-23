use application::{AiRunId, EstimateSubmissionInput, ReasoningEffort, SignalSourceFacts};
use domain::{
    AcquisitionPrice, AdjustmentBasis, AiProbability, AiProbabilityInput, BuyingPower, Calibration,
    CalibrationEstimateId, CalibrationKey, CalibrationLedgerHead, CalibrationModel,
    CalibrationPosition, CalibrationSnapshot, Candle, CandleObservation, CandlePeriod,
    CandleStatus, CandleTime, CollectedItem, CommissionRateFact, CommissionValidity, CommonShare,
    CostEligibleTask, CostRate, Currency, DisplayInstrumentName, EstimateAssignments,
    EstimateContext, EstimateCostPeriod, EstimateMetadata, EstimateNarrative, EstimatePrompt,
    EstimateTask, Evidence, EvidenceBundleId, EvidenceRef, EvidenceUrl, Holding,
    HoldingObservation, Horizon, HorizonSelection, HoursPermission, InstrumentId, InvestorProfile,
    KnownMarket, KoreanMasterDetail, LiquidationTrading, ListingStatus, Margin, Market,
    MarketCalendar, MarketSession, MasterCode, MasterInstrument, MasterInstrumentFacts,
    MasterInstrumentRow, MasterSecurityType, ModelIdentityPart, Money, Nanosecond, NormalizedUrl,
    NumericInput, NxtTrading, OffsetSeconds, Ohlcv, Policy, Portfolio, PortfolioValuation, Price,
    PriceAdjustment, PromptHash, Quantity, Ratio, ReportedHolding, RiskAppetite, SectorAssignments,
    SectorCatalog, SectorComposition, SectorDepth, SectorId, SectorMetadataRow, SectorStockRow,
    SessionData, SessionInformation, SignalAssessmentFacts, SignalContext, SignalDecision,
    SignalEstimate, SignalSubmission, SignalTaskKey, SignalTiming, Symbol, TimeInterval, Timestamp,
    TradeCost, TradeTaxFact, TradingSuspension, UnixSeconds, UntrustedText, Volatility,
    calibrate_signal_submission, sector_assignment_from_compositions,
};

pub(super) fn id(value: &str) -> InstrumentId {
    InstrumentId::try_new(value).expect("종목")
}

pub(super) fn at(value: i64) -> Timestamp {
    Timestamp::from_parts(
        UnixSeconds::new(value),
        Nanosecond::new(0),
        OffsetSeconds::new(0),
    )
    .expect("시각")
}

pub(super) fn price(value: &str) -> Price {
    Price::parse(&NumericInput::try_new(value).expect("입력"), Currency::Krw).expect("가격")
}

pub(super) fn money(value: &str, currency: Currency) -> Money {
    Money::parse(&NumericInput::try_new(value).expect("입력"), currency).expect("금액")
}

pub(super) fn quantity(value: &str) -> Quantity {
    Quantity::parse(&NumericInput::try_new(value).expect("입력")).expect("수량")
}

pub(super) fn ratio(value: &str) -> Ratio {
    Ratio::parse(&NumericInput::try_new(value).expect("입력")).expect("비율")
}

pub(super) fn master() -> MasterInstrument {
    MasterInstrument::try_new(MasterInstrumentRow {
        listing_market: MasterCode::try_new("KOSPI").expect("시장"),
        instrument_id: id("sample-a"),
        symbol: Symbol::try_new("005930").expect("코드"),
        name: DisplayInstrumentName::try_new("고정 종목").expect("이름"),
        security_type: MasterSecurityType::from_code(MasterCode::try_new("STOCK").expect("유형")),
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

pub(super) fn other_master() -> MasterInstrument {
    let mut row = master().row().clone();
    row.instrument_id = id("sample-b");
    row.symbol = Symbol::try_new("000660").expect("다른 코드");
    row.name = DisplayInstrumentName::try_new("다른 고정 종목").expect("이름");
    MasterInstrument::try_new(row).expect("다른 마스터")
}

pub(super) fn local(time: Timestamp) -> Timestamp {
    Timestamp::from_parts(
        time.utc_seconds(),
        time.nanosecond(),
        OffsetSeconds::new(32400),
    )
    .expect("현지 시각")
}

pub(super) fn local_midnight(value: i64) -> Timestamp {
    local(at(day(value)
        .utc_seconds()
        .as_i64()
        .checked_sub(32400)
        .expect("현지 자정")))
}

pub(super) fn day(value: i64) -> Timestamp {
    at(value.checked_mul(86400).expect("일"))
}

pub(super) fn session(value: i64, market: &Market) -> MarketSession {
    let start = day(value);
    let end = at(start.utc_seconds().as_i64().checked_add(3600).expect("끝"));
    let data = Some(
        SessionData::try_new(TimeInterval::try_new(start, end).expect("구간"), None).expect("장"),
    );
    let information = match market {
        Market::Kr => SessionInformation::Kr {
            pre_market: None,
            regular_market: data,
            after_market: None,
        },
        Market::Us | Market::Unknown(_) => SessionInformation::Us {
            day_market: None,
            pre_market: None,
            regular_market: data,
            after_market: None,
        },
    };
    MarketSession::try_new(start.offset_calendar_date(), information).expect("세션")
}

pub(super) fn calendar(value: i64, market: &Market) -> MarketCalendar {
    MarketCalendar::try_new(
        session(value.checked_sub(1).expect("전일"), market),
        session(value, market),
        session(value.checked_add(1).expect("익일"), market),
    )
    .expect("달력")
}

pub(super) fn eligible_for_master(high: &str, source: &MasterInstrument) -> CostEligibleTask {
    eligible_for_horizon(high, source, HorizonSelection::FewDays)
}

fn last_day(selection: HorizonSelection) -> i64 {
    match selection {
        HorizonSelection::Hours => 61,
        HorizonSelection::FewDays => 65,
        HorizonSelection::WeeksToMonths => 100,
        HorizonSelection::HalfYearOrMore => 186,
    }
}

pub(super) fn eligible_for_horizon(
    high: &str,
    source: &MasterInstrument,
    selection: HorizonSelection,
) -> CostEligibleTask {
    let candles: Vec<_> = (0_i64..61)
        .map(|index| {
            let fixture_price = price(if index % 2 == 0 { "100" } else { high });
            Candle::try_new(
                source.row().instrument_id.clone(),
                CandleTime::try_new(local_midnight(index), CandlePeriod::Daily).expect("일봉 시각"),
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
        Horizon::new(selection),
        HoursPermission::default(),
        RiskAppetite::Neutral,
    )
    .expect("과제");
    let zero = CostRate::try_new(ratio("0")).expect("비용");
    let period = EstimateCostPeriod::from_calendar_result(
        &task,
        at(day(last_day(selection))
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

pub(super) fn calibration_key() -> CalibrationKey {
    calibration_key_for(HorizonSelection::FewDays)
}

pub(super) fn calibration_key_for(selection: HorizonSelection) -> CalibrationKey {
    let part = ModelIdentityPart::try_new("fixture").expect("모델");
    CalibrationKey::new(
        CalibrationModel::new(part.clone(), part.clone(), part),
        Horizon::new(selection),
    )
}

pub(super) fn run_id() -> AiRunId {
    AiRunId::try_new("run-a").expect("실행")
}
pub(super) fn task_id(text: &str) -> CalibrationEstimateId {
    CalibrationEstimateId::try_new(text).expect("과제")
}
pub(super) fn assignments() -> EstimateAssignments<AiRunId> {
    let first = eligible_for_master("102", &master());
    let other = eligible_for_master("104", &other_master());
    EstimateAssignments::default()
        .issue(task_id("task-old"), run_id(), first.clone())
        .expect("과제")
        .issue(task_id("task-new"), run_id(), first)
        .expect("과제")
        .issue(task_id("task-other"), run_id(), other)
        .expect("과제")
}
pub(super) fn item() -> CollectedItem {
    CollectedItem::from_web(
        NormalizedUrl::try_new("https://example.com/evidence").expect("주소"),
        UntrustedText::try_new("독립 원장").expect("제목"),
        None,
    )
}
pub(super) fn submission(task: &CostEligibleTask, probability: &str) -> SignalSubmission {
    submission_with_key(task, probability, calibration_key())
}

pub(super) fn submission_with_key(
    task: &CostEligibleTask,
    probability: &str,
    key: CalibrationKey,
) -> SignalSubmission {
    SignalSubmission::create(
        SignalTaskKey::from_task(task),
        key,
        AiProbability::parse(&AiProbabilityInput::try_new(probability).expect("입력"))
            .expect("확률"),
        Some(
            Evidence::try_new(vec![EvidenceRef::Web(
                EvidenceUrl::try_new(item().url().clone()).expect("근거"),
            )])
            .expect("출처"),
        ),
        task.task().created_at(),
    )
    .expect("제출")
}
pub(super) fn input(
    name: &str,
    probability: &str,
    source: &MasterInstrument,
) -> EstimateSubmissionInput {
    let part = ModelIdentityPart::try_new("model-v1").expect("모델");
    EstimateSubmissionInput {
        task_id: task_id(name),
        submission: submission(
            &eligible_for_master(
                if source.row().instrument_id == master().row().instrument_id {
                    "102"
                } else {
                    "104"
                },
                source,
            ),
            probability,
        ),
        narrative: EstimateNarrative {
            rationale: UntrustedText::try_new("근거").expect("근거"),
            counterargument: UntrustedText::try_new("반론").expect("반론"),
        },
        metadata: EstimateMetadata {
            effort: ReasoningEffort::High,
            cli_version: part.clone(),
            prompt: EstimatePrompt {
                id: domain::ScorecardPromptId::try_new("prompt").expect("ID"),
                version: part,
                hash: PromptHash::try_new(
                    "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789",
                )
                .expect("해시"),
            },
            evidence_bundle: EvidenceBundleId::try_new("bundle-1").expect("묶음"),
        },
    }
}
pub(super) fn head(position: u64) -> CalibrationLedgerHead {
    CalibrationLedgerHead::new(calibration_key(), CalibrationPosition::new(position))
}
pub(super) fn decision(
    source: &MasterInstrument,
    probability: &str,
    held: Option<&str>,
) -> SignalDecision {
    decision_with_sources(source, probability, held, &[item()])
}
fn portfolio_for(
    source: &MasterInstrument,
    held: Option<&str>,
    at: Timestamp,
) -> PortfolioValuation {
    let instrument = source.order_instrument().expect("종목");
    let holdings = held
        .into_iter()
        .map(|value| {
            Holding::new(
                instrument.clone(),
                quantity(value),
                AcquisitionPrice::try_new(money("87", Currency::Krw)).expect("원가"),
                HoldingObservation::new(
                    None,
                    Some(
                        ReportedHolding::new(
                            money("113", Currency::Krw),
                            money("7", Currency::Krw),
                            ratio("0.07"),
                        )
                        .expect("평가"),
                    ),
                    at,
                ),
            )
            .expect("보유")
        })
        .collect();
    Portfolio::new(
        holdings,
        BuyingPower::new(money("10000", Currency::Krw), at),
        BuyingPower::new(money("0", Currency::Usd), at),
    )
    .expect("포트")
    .evaluate(None)
}
fn mapping_for(source: &MasterInstrument, at: Timestamp) -> SectorAssignments {
    let sector = SectorId::try_new("tech").expect("섹터");
    let catalog = SectorCatalog::from_response(
        vec![SectorMetadataRow {
            id: sector.clone(),
            name: DisplayInstrumentName::try_new("기술").expect("이름"),
            depth: SectorDepth::Major,
            parent: None,
        }],
        at,
    )
    .expect("목록");
    let composition = SectorComposition::from_stocks_response(
        sector,
        KnownMarket::Kr,
        vec![SectorStockRow {
            symbol: source.row().symbol.clone(),
            currency: Currency::Krw,
            market_capitalization: Some(NumericInput::try_new("100000").expect("시총")),
            trading_amount: None,
        }],
        at,
    )
    .expect("구성");
    let assignment =
        sector_assignment_from_compositions(source, &catalog, &[composition]).expect("분류");
    SectorAssignments::try_new(vec![assignment]).expect("분류")
}
pub(super) fn decision_with_sources(
    source: &MasterInstrument,
    probability: &str,
    held: Option<&str>,
    items: &[CollectedItem],
) -> SignalDecision {
    decision_with_horizon(
        source,
        probability,
        held,
        items,
        (HorizonSelection::FewDays, None),
    )
}

pub(super) fn decision_with_horizon(
    source: &MasterInstrument,
    probability: &str,
    held: Option<&str>,
    items: &[CollectedItem],
    setup: (HorizonSelection, Option<&str>),
) -> SignalDecision {
    let (selection, high) = setup;
    let task = eligible_for_horizon(
        high.unwrap_or_else(|| {
            if source.row().instrument_id == master().row().instrument_id {
                "102"
            } else {
                "104"
            }
        }),
        source,
        selection,
    );
    let key = calibration_key_for(selection);
    let submitted = submission_with_key(&task, probability, key.clone());
    let calibration = Calibration::try_new(key.clone(), &[]).expect("보정");
    let position = CalibrationLedgerHead::new(key, CalibrationPosition::new(0));
    let snapshot = CalibrationSnapshot::try_new(calibration, position.clone()).expect("표");
    let calibrated =
        calibrate_signal_submission(Some(&snapshot), &position, submitted).expect("보정");
    let estimate = SignalEstimate::create(&task, &calibrated, Margin::default()).expect("입력");
    let at = task.task().created_at();
    let portfolio = portfolio_for(source, held, at);
    let policy = Policy::new(InvestorProfile::default());
    let mapping = mapping_for(source, at);
    let security = MasterInstrumentFacts::try_new(vec![source.clone()])
        .expect("마스터")
        .policy_security_facts();
    let facts = SignalSourceFacts {
        items: items.to_vec(),
        disclosures: vec![],
        tools: vec![],
    };
    let ledger = facts.ledger();
    let calendars: Vec<_> = (60..=last_day(selection).checked_add(1).expect("마지막 달력"))
        .map(|value| calendar(value, &Market::Kr))
        .collect();
    let context = SignalContext::new(
        Some(&portfolio),
        &policy,
        (&mapping, &security),
        &ledger,
        SignalAssessmentFacts::new(
            SignalTiming::new(at, &calendars),
            Some(estimate.task().reference().clone()),
            Some(position),
        ),
    );
    SignalDecision::assess(&estimate, &context)
}

fn restore_candles(high: &str, source: &MasterInstrument) -> Vec<Candle> {
    (0_i64..61)
        .map(|index| {
            let value = price(if index % 2 == 0 { "100" } else { high });
            Candle::try_new(
                source.row().instrument_id.clone(),
                CandleTime::try_new(local_midnight(index), CandlePeriod::Daily).expect("일봉"),
                PriceAdjustment::Adjusted(AdjustmentBasis::new(day(61))),
                Ohlcv::try_new(value, value, value, value, quantity("13")).expect("봉"),
                CandleObservation::new(day(61), CandleStatus::Finalized),
            )
            .expect("봉")
        })
        .collect()
}

pub(super) fn candidate_stored_task(
    task: &EstimateTask,
    source: &MasterInstrument,
    horizon: Horizon,
) -> Option<domain::StoredEstimateEvidence> {
    for high in [
        "101", "102", "103", "104", "105", "106", "107", "108", "110", "112", "113", "114", "116",
    ] {
        let input = domain::EstimateRestoreEvidence {
            master: source.row().clone(),
            side: domain::OrderSide::Buy,
            candles: restore_candles(high, source),
            reference_price: price("100"),
            reference_at: local(day(60)),
            task_at: day(60),
            horizon,
            permission: domain::HoursPermissionState::Disabled,
            appetite: RiskAppetite::Neutral,
        };
        if let Ok(result) = input.capture(task, source) {
            return Some(result);
        }
    }
    None
}

fn portfolio_for_signal(
    source: &MasterInstrument,
    signal: &domain::RaisedSignal,
    cash: &str,
) -> Portfolio {
    let holdings = match signal {
        domain::RaisedSignal::Buy(_) => vec![],
        domain::RaisedSignal::Sell(sell) => vec![
            Holding::new(
                source.order_instrument().expect("종목"),
                sell.held_quantity(),
                AcquisitionPrice::try_new(money("87", Currency::Krw)).expect("원가"),
                HoldingObservation::new(
                    None,
                    Some(
                        ReportedHolding::new(
                            money("113", Currency::Krw),
                            money("7", Currency::Krw),
                            ratio("0.07"),
                        )
                        .expect("평가"),
                    ),
                    sell.observed_at(),
                ),
            )
            .expect("보유"),
        ],
    };
    Portfolio::new(
        holdings,
        BuyingPower::new(money(cash, Currency::Krw), day(60)),
        BuyingPower::new(money("0", Currency::Usd), day(60)),
    )
    .expect("포트")
}

pub(super) fn evidence_for_signal(
    signal: &domain::RaisedSignal,
    occurrence: Option<&domain::BuySignalRecord>,
    cash: &str,
    item: CollectedItem,
    classification: &(SectorAssignments, domain::PolicySecurityFacts),
) -> application::SignalCommitEvidence {
    let estimate = match signal {
        domain::RaisedSignal::Buy(value) => value.facts(),
        domain::RaisedSignal::Sell(value) => value.facts(),
    };
    let source = if estimate.task().volatility().instrument() == &master().row().instrument_id {
        master()
    } else {
        other_master()
    };
    let submission = estimate.submission();
    let key = submission.calibration_key().clone();
    let horizon = key.horizon();
    let stored = candidate_stored_task(estimate.task(), &source, horizon).expect("과제 근거");
    let head = estimate.calibration_head().clone();
    let snapshot = CalibrationSnapshot::try_new(
        Calibration::try_new(key.clone(), &[]).expect("보정"),
        head.clone(),
    )
    .expect("스냅숏");
    let zero = CostRate::try_new(ratio("0")).expect("비용");
    let maturity = at(day(last_day(horizon.selection()))
        .utc_seconds()
        .as_i64()
        .checked_add(3600)
        .expect("만기"));
    let evidence = domain::SignalRestoreEvidence {
        estimate: stored,
        commission: CommissionRateFact::from_commissions_result(
            Market::Kr,
            zero,
            CommissionValidity::try_new(None, None).expect("기간"),
        ),
        tax: TradeTaxFact::from_schedule_result(Market::Kr, zero, zero),
        maturity,
        calibration: snapshot,
        calibration_key: key,
        probability: submission.probability(),
        source: submission.evidence().cloned(),
        submitted_at: submission.submitted_at(),
        margin: Margin::default(),
        assessment: signal_restore_context(
            &source,
            signal,
            (cash, item, classification),
            (horizon, head),
        ),
    }
    .capture(signal, &source)
    .expect("신호 복원 근거");
    let recorded = occurrence.and_then(|value| {
        domain::StoredSignalOccurrence::capture(value, evidence.clone(), &source).ok()
    });
    application::SignalCommitEvidence {
        master: source,
        signal: evidence,
        occurrence: recorded,
    }
}

fn signal_restore_context(
    source: &MasterInstrument,
    signal: &domain::RaisedSignal,
    facts: (
        &str,
        CollectedItem,
        &(SectorAssignments, domain::PolicySecurityFacts),
    ),
    calibration: (Horizon, CalibrationLedgerHead),
) -> domain::HistoricalSignalContext {
    let (cash, item, classification) = facts;
    let (horizon, head) = calibration;
    let estimate = match signal {
        domain::RaisedSignal::Buy(value) => value.facts(),
        domain::RaisedSignal::Sell(value) => value.facts(),
    };
    domain::HistoricalSignalContext {
        portfolio: Some(domain::HistoricalPortfolio::capture(&portfolio_for_signal(
            source, signal, cash,
        ))),
        rate: None,
        policy: Policy::new(InvestorProfile::default()),
        classifications: domain::HistoricalClassifications::capture(
            &classification.0,
            &classification.1,
        ),
        items: vec![item],
        disclosures: vec![],
        tools: vec![],
        calendars: (60..=last_day(horizon.selection())
            .checked_add(1)
            .expect("마지막"))
            .map(|value| calendar(value, &Market::Kr))
            .collect(),
        evaluated_at: day(60),
        target: Some(domain::StoredPriceObservation::from_price_observation(
            estimate.task().reference(),
        )),
        calibration_head: Some(head),
    }
}

pub(super) fn evidence_for_decision(
    decision: &SignalDecision,
    occurrence: Option<&domain::BuySignalRecord>,
) -> Option<application::SignalCommitEvidence> {
    let signal = decision.clone().into_signal()?;
    let facts = match &signal {
        domain::RaisedSignal::Buy(value) => value.facts(),
        domain::RaisedSignal::Sell(value) => value.facts(),
    };
    let source = if facts.task().volatility().instrument() == &master().row().instrument_id {
        master()
    } else {
        other_master()
    };
    let assignments = mapping_for(&source, day(60));
    let security = MasterInstrumentFacts::try_new(vec![source])
        .expect("마스터")
        .policy_security_facts();
    Some(evidence_for_signal(
        &signal,
        occurrence,
        "10000",
        item(),
        &(assignments, security),
    ))
}
