use crate::{EstimateStore, SignalStore};
use application::{
    AiRunId, EstimateRepository, EstimateSubmissionRepository, EstimateTaskRepository,
    ReasoningEffort, SignalCommitEvidence, SignalSourceFacts,
};
use domain::{
    AccountId, AcquisitionPrice, AdjustmentBasis, AiProbability, AiProbabilityInput,
    BuySignalOccurrenceId, BuySignalRecord, BuyingPower, Calibration, CalibrationEstimateId,
    CalibrationKey, CalibrationLedgerHead, CalibrationModel, CalibrationPosition,
    CalibrationSnapshot, Candle, CandleObservation, CandlePeriod, CandleStatus, CandleTime,
    ClientOrderId, CollectedItem, CommissionRateFact, CommissionValidity, CommonShare,
    CostEligibleTask, CostRate, Currency, DisplayInstrumentName, Draft, DraftRestoreEvidence,
    EstimateContext, EstimateCostPeriod, EstimateMetadata, EstimateNarrative, EstimatePrompt,
    EstimateRestoreEvidence, EstimateSubmissionRecord, EstimateTask, Evidence, EvidenceBundleId,
    EvidenceRef, EvidenceUrl, FxPair, FxRate, FxRateKind, FxSource, HistoricalClassifications,
    HistoricalPortfolio, HistoricalSignalContext, Holding, HoldingObservation, Horizon,
    HorizonSelection, HoursPermission, HoursPermissionState, InstrumentId, InvestorProfile,
    ListingStatus, Margin, Market, MarketCalendar, MarketSession, MasterCode, MasterInstrument,
    MasterInstrumentFacts, MasterInstrumentRow, MasterSecurityType, ModelIdentityPart, Money,
    Nanosecond, NormalizedUrl, NumericInput, OffsetSeconds, Ohlcv, OrderIntent, OrderSide,
    OrderTerms, Policy, PolicySecurityFacts, Portfolio, Price, PriceAdjustment, PriceObservation,
    PromptHash, Proposal, ProposalId, ProposalLifetime, ProposalPriceRange, Quantity, RaisedSignal,
    Ratio, RiskAppetite, ScorecardPromptId, SectorAssignments, SectorCatalog, SessionData,
    SessionInformation, SignalAssessmentFacts, SignalContext, SignalDecision, SignalEstimate,
    SignalRestoreEvidence, SignalSubmission, SignalTaskKey, SignalTiming, SpanSeconds,
    StoredEstimateEvidence, StoredPriceObservation, StoredSignalOccurrence, Symbol, TimeInterval,
    TimeSpan, Timestamp, TradeCost, TradeTaxFact, UnixSeconds, UntrustedText, UsOrderPrice,
    Volatility, WholeOrderQuantity, calibrate_signal_submission,
    sector_assignment_from_compositions,
};

pub(super) fn at_us(day: i64) -> Timestamp {
    let delta = day.checked_mul(86_400_i64).expect("day");
    Timestamp::from_parts(
        UnixSeconds::new(1_767_225_600_i64.checked_add(delta).expect("ts")),
        Nanosecond::new(0),
        OffsetSeconds::new(-18_000),
    )
    .expect("timestamp")
}

pub(super) fn at_utc(day: i64) -> Timestamp {
    let delta = day.checked_mul(86_400_i64).expect("day");
    Timestamp::from_parts(
        UnixSeconds::new(1_767_225_600_i64.checked_add(delta).expect("ts")),
        Nanosecond::new(0),
        OffsetSeconds::new(0),
    )
    .expect("timestamp")
}

pub(super) fn shifted_us(day: i64, seconds: u64) -> Timestamp {
    at_us(day)
        .checked_add(TimeSpan::try_new(SpanSeconds::new(seconds), Nanosecond::new(0)).expect("sp"))
        .expect("shifted")
}

pub(super) fn us_price(text: &str) -> Price {
    Price::parse(&NumericInput::try_new(text).expect("num"), Currency::Usd).expect("price")
}

pub(super) fn qty(text: &str) -> Quantity {
    Quantity::parse(&NumericInput::try_new(text).expect("num")).expect("qty")
}

pub(super) fn money(text: &str, currency: Currency) -> Money {
    Money::parse(&NumericInput::try_new(text).expect("num"), currency).expect("money")
}

pub(super) fn us_master(id: &str, symbol: &str) -> MasterInstrument {
    MasterInstrument::try_new(MasterInstrumentRow {
        listing_market: MasterCode::try_new("NASDAQ").expect("market"),
        symbol: Symbol::try_new(symbol).expect("symbol"),
        instrument_id: InstrumentId::try_new(id).expect("id"),
        name: DisplayInstrumentName::try_new("테스트 종목").expect("name"),
        security_type: MasterSecurityType::from_code(MasterCode::try_new("ETF").expect("kind")),
        currency: Currency::Usd,
        listing_status: ListingStatus::Active,
        common_share: CommonShare::Common,
        korean_detail: None,
        observed_at: at_us(61),
    })
    .expect("master")
}

pub(super) fn us_candles(id: &str) -> Vec<Candle> {
    (0_i64..61_i64)
        .map(|day| {
            let value = if day.checked_rem(2_i64) == Some(0_i64) {
                us_price("100")
            } else {
                us_price("103")
            };
            Candle::try_new(
                InstrumentId::try_new(id).expect("id"),
                CandleTime::try_new(at_utc(day), CandlePeriod::Daily).expect("time"),
                PriceAdjustment::Adjusted(AdjustmentBasis::new(at_utc(70))),
                Ohlcv::try_new(value, value, value, value, qty("10")).expect("ohlcv"),
                CandleObservation::new(at_utc(70), CandleStatus::Finalized),
            )
            .expect("candle")
        })
        .collect()
}

pub(super) fn us_calendars() -> Vec<MarketCalendar> {
    let session = |day: i64| {
        MarketSession::try_new(
            at_us(day).offset_calendar_date(),
            SessionInformation::Us {
                day_market: None,
                pre_market: None,
                regular_market: Some(
                    SessionData::try_new(
                        TimeInterval::try_new(shifted_us(day, 21_600), shifted_us(day, 25_200))
                            .expect("interval"),
                        None,
                    )
                    .expect("session"),
                ),
                after_market: None,
            },
        )
        .expect("market session")
    };
    (60_i64..=68_i64)
        .map(|day| {
            MarketCalendar::try_new(
                session(day.checked_sub(1_i64).expect("prev")),
                session(day),
                session(day.checked_add(1_i64).expect("next")),
            )
            .expect("calendar")
        })
        .collect()
}

fn usd_krw_rate() -> FxRate {
    FxRate::parse(
        &NumericInput::try_new("1350").expect("rate"),
        FxPair::UsdKrw,
        FxRateKind::Mid,
        TimeInterval::try_new(at_us(60), at_us(65)).expect("valid"),
        FxSource::TossOpenApi,
    )
    .expect("fx")
}

pub(super) fn calibration_key() -> CalibrationKey {
    CalibrationKey::new(
        CalibrationModel::new(
            ModelIdentityPart::try_new("claude").expect("provider"),
            ModelIdentityPart::try_new("opus").expect("model"),
            ModelIdentityPart::try_new("5").expect("version"),
        ),
        Horizon::new(HorizonSelection::FewDays),
    )
}

pub(super) fn zero_cost_facts() -> (CommissionRateFact, TradeTaxFact) {
    let zero =
        CostRate::try_new(Ratio::parse(&NumericInput::try_new("0").expect("zero")).expect("ratio"))
            .expect("rate");
    (
        CommissionRateFact::from_commissions_result(
            Market::Us,
            zero,
            CommissionValidity::try_new(None, None).expect("validity"),
        ),
        TradeTaxFact::from_schedule_result(Market::Us, zero, zero),
    )
}

pub(super) fn web_sources() -> (Evidence, SignalSourceFacts) {
    let url = NormalizedUrl::try_new("https://example.com/news/1").expect("url");
    let evidence = Evidence::try_new(vec![EvidenceRef::Web(
        EvidenceUrl::try_new(url.clone()).expect("evidence url"),
    )])
    .expect("evidence");
    let item = CollectedItem::from_web(
        url,
        UntrustedText::try_new("근거 기사").expect("title"),
        None,
    );
    (
        evidence,
        SignalSourceFacts {
            items: vec![item],
            disclosures: Vec::new(),
            tools: Vec::new(),
        },
    )
}

pub(super) struct TaskBundle {
    pub(super) master: MasterInstrument,
    pub(super) task: EstimateTask,
    pub(super) eligible: CostEligibleTask,
    pub(super) stored_estimate: StoredEstimateEvidence,
    pub(super) calibration: CalibrationSnapshot,
}

pub(super) fn build_task_bundle(id: &str, symbol: &str, side: OrderSide) -> TaskBundle {
    let master = us_master(id, symbol);
    let candles = us_candles(id);
    let volatility = Volatility::calculate(&candles).expect("vol");
    let reference = master
        .price_observation_for(side, us_price("100"), shifted_us(61, 21_600))
        .expect("ref");
    let task = EstimateTask::create(
        EstimateContext::new(reference, shifted_us(61, 21_601)),
        volatility,
        Horizon::new(HorizonSelection::FewDays),
        HoursPermission::default(),
        RiskAppetite::Neutral,
    )
    .expect("task");
    let stored_estimate = EstimateRestoreEvidence {
        master: master.row().clone(),
        side,
        candles,
        reference_price: us_price("100"),
        reference_at: shifted_us(61, 21_600),
        task_at: shifted_us(61, 21_601),
        horizon: Horizon::new(HorizonSelection::FewDays),
        permission: HoursPermissionState::Disabled,
        appetite: RiskAppetite::Neutral,
    }
    .capture(&task, &master)
    .expect("stored estimate");
    let (commission, tax) = zero_cost_facts();
    let period = EstimateCostPeriod::from_calendar_result(&task, at_us(70)).expect("period");
    let cost = TradeCost::from_facts(Some(commission), Some(tax), period).expect("cost");
    let eligible = CostEligibleTask::assess(task.clone(), &cost).expect("eligible");
    let key = calibration_key();
    let cal = Calibration::try_new(key.clone(), &[]).expect("cal");
    let head = CalibrationLedgerHead::new(key, CalibrationPosition::new(0));
    let calibration = CalibrationSnapshot::try_new(cal, head).expect("snap");
    TaskBundle {
        master,
        task,
        eligible,
        stored_estimate,
        calibration,
    }
}

pub(super) fn build_draft_fixture(
    bundle: &TaskBundle,
    proposal_id: &str,
    card_price: &str,
    order_price: &str,
    account_id: i64,
) -> (Proposal<Draft>, DraftRestoreEvidence, PriceObservation) {
    let card = bundle
        .master
        .price_observation_for(OrderSide::Buy, us_price(card_price), shifted_us(61, 21_602))
        .expect("card");
    let range = ProposalPriceRange::from_estimate_task(bundle.task.range_facts(), card.clone())
        .expect("range");
    let pid = ProposalId::new(ClientOrderId::try_new(proposal_id).expect("pid"));
    let terms = OrderTerms::UsLimit {
        side: OrderSide::Buy,
        quantity: WholeOrderQuantity::try_new(qty("2")).expect("whole"),
        price: UsOrderPrice::try_new(us_price(order_price)).expect("us price"),
    };
    let intent = OrderIntent::new(
        AccountId::new(account_id),
        card.instrument().clone(),
        pid.as_client_order_id().clone(),
        terms,
    )
    .expect("intent");
    let lifetime = ProposalLifetime::try_new(SpanSeconds::new(300)).expect("lifetime");
    let proposal =
        Proposal::new(pid, intent, shifted_us(61, 21_602), range, lifetime).expect("proposal");
    let evidence = DraftRestoreEvidence::capture(
        &proposal,
        bundle.stored_estimate.clone(),
        &bundle.master,
        lifetime,
        &card,
    )
    .expect("draft evidence");
    (proposal, evidence, card)
}

struct CapturedSignal {
    submission: SignalSubmission,
    signal: RaisedSignal,
    stored: domain::StoredSignalEvidence,
    sources: SignalSourceFacts,
    calendars: Vec<MarketCalendar>,
}

fn master_classifications(master: &MasterInstrument) -> (SectorAssignments, PolicySecurityFacts) {
    let catalog = SectorCatalog::from_response(Vec::new(), at_us(61)).expect("catalog");
    let sectors = SectorAssignments::try_new(vec![
        sector_assignment_from_compositions(master, &catalog, &[]).expect("sector"),
    ])
    .expect("assignments");
    let security = MasterInstrumentFacts::try_new(vec![master.clone()])
        .expect("facts")
        .policy_security_facts();
    (sectors, security)
}

fn build_restore_context(
    bundle: &TaskBundle,
    portfolio: &Portfolio,
    sources: &SignalSourceFacts,
    calendars: &[MarketCalendar],
    target: &PriceObservation,
) -> HistoricalSignalContext {
    let hist_portfolio =
        HistoricalPortfolio::capture(portfolio, std::slice::from_ref(&bundle.master))
            .expect("hist");
    let (sectors, security) = master_classifications(&bundle.master);
    HistoricalSignalContext {
        portfolio: Some(hist_portfolio),
        rate: Some(usd_krw_rate()),
        policy: Policy::new(InvestorProfile::default()),
        classifications: HistoricalClassifications::capture(&sectors, &security),
        items: sources.items.clone(),
        disclosures: sources.disclosures.clone(),
        tools: sources.tools.clone(),
        calendars: calendars.to_vec(),
        evaluated_at: shifted_us(61, 21_604),
        target: Some(StoredPriceObservation::from_price_observation(target)),
        calibration_head: Some(bundle.calibration.through().clone()),
    }
}

fn build_estimate(
    bundle: &TaskBundle,
    probability: AiProbability,
    evidence: &Evidence,
) -> (SignalSubmission, SignalEstimate) {
    let submission = SignalSubmission::create(
        SignalTaskKey::from_task(&bundle.eligible),
        calibration_key(),
        probability,
        Some(evidence.clone()),
        shifted_us(61, 21_602),
    )
    .expect("submission");
    let calibrated = calibrate_signal_submission(
        Some(&bundle.calibration),
        bundle.calibration.through(),
        submission.clone(),
    )
    .expect("cal");
    let estimate =
        SignalEstimate::create(&bundle.eligible, &calibrated, Margin::default()).expect("est");
    (submission, estimate)
}

fn evaluate_and_capture(
    bundle: &TaskBundle,
    probability: AiProbability,
    side: OrderSide,
    portfolio: &Portfolio,
) -> CapturedSignal {
    let (evidence, sources) = web_sources();
    let (submission, estimate) = build_estimate(bundle, probability, &evidence);
    let policy = Policy::new(InvestorProfile::default());
    let (sectors, security) = master_classifications(&bundle.master);
    let calendars = us_calendars();
    let target = bundle
        .master
        .price_observation_for(side, us_price("100"), shifted_us(61, 21_603))
        .expect("target");
    let ledger = sources.ledger();
    let valuation = portfolio.evaluate(Some(usd_krw_rate()));
    let context = SignalContext::new(
        Some(&valuation),
        &policy,
        (&sectors, &security),
        &ledger,
        SignalAssessmentFacts::new(
            SignalTiming::new(shifted_us(61, 21_604), &calendars),
            Some(target.clone()),
            Some(bundle.calibration.through().clone()),
        ),
    );
    let signal = SignalDecision::assess(&estimate, &context)
        .into_signal()
        .expect("signal");
    let (commission, tax) = zero_cost_facts();
    let stored = SignalRestoreEvidence {
        estimate: bundle.stored_estimate.clone(),
        commission,
        tax,
        maturity: at_us(70),
        calibration: bundle.calibration.clone(),
        calibration_key: calibration_key(),
        probability,
        source: Some(evidence),
        submitted_at: shifted_us(61, 21_602),
        margin: Margin::default(),
        assessment: build_restore_context(bundle, portfolio, &sources, &calendars, &target),
    }
    .capture(&signal, &bundle.master)
    .expect("capture");
    CapturedSignal {
        submission,
        signal,
        stored,
        sources,
        calendars,
    }
}

pub(super) fn build_sell_signal(
    bundle: &TaskBundle,
    prob_text: &str,
    held_qty: &str,
) -> (SignalSubmission, RaisedSignal, SignalCommitEvidence) {
    let probability =
        AiProbability::parse(&AiProbabilityInput::try_new(prob_text).expect("in")).expect("prob");
    let holding = Holding::new(
        bundle.master.order_instrument().expect("order inst"),
        qty(held_qty),
        AcquisitionPrice::try_new(money("90", Currency::Usd)).expect("acq"),
        HoldingObservation::new(Some(us_price("100")), None, shifted_us(61, 21_603)),
    )
    .expect("holding");
    let portfolio = Portfolio::new(
        vec![holding],
        BuyingPower::new(money("1000000", Currency::Krw), shifted_us(61, 21_603)),
        BuyingPower::new(money("0", Currency::Usd), shifted_us(61, 21_603)),
    )
    .expect("portfolio");
    let captured = evaluate_and_capture(bundle, probability, OrderSide::Sell, &portfolio);
    let commit_evidence = SignalCommitEvidence {
        master: bundle.master.clone(),
        holding_masters: Vec::new(),
        signal: captured.stored,
        occurrence: None,
    };
    (captured.submission, captured.signal, commit_evidence)
}

pub(super) struct BuyFixture {
    pub(super) submission: SignalSubmission,
    pub(super) signal: RaisedSignal,
    pub(super) occurrence: BuySignalRecord,
    pub(super) evidence: SignalCommitEvidence,
    pub(super) sources: SignalSourceFacts,
}

pub(super) fn build_buy_signal(bundle: &TaskBundle, occurrence_id: &str) -> BuyFixture {
    let probability =
        AiProbability::parse(&AiProbabilityInput::try_new("90").expect("in")).expect("prob");
    let portfolio = Portfolio::new(
        Vec::new(),
        BuyingPower::new(money("10000000", Currency::Krw), shifted_us(61, 21_603)),
        BuyingPower::new(money("0", Currency::Usd), shifted_us(61, 21_603)),
    )
    .expect("portfolio");
    let captured = evaluate_and_capture(bundle, probability, OrderSide::Buy, &portfolio);
    let RaisedSignal::Buy(buy_ref) = &captured.signal else {
        panic!("expected buy");
    };
    let occurrence = BuySignalRecord::from_signal(
        BuySignalOccurrenceId::try_new(occurrence_id).expect("occ id"),
        buy_ref,
        shifted_us(61, 21_604),
        &captured.calendars,
    )
    .expect("occ record");
    let stored_occ =
        StoredSignalOccurrence::capture(&occurrence, captured.stored.clone(), &bundle.master)
            .expect("stored occ");
    let evidence = SignalCommitEvidence {
        master: bundle.master.clone(),
        holding_masters: Vec::new(),
        signal: captured.stored,
        occurrence: Some(stored_occ),
    };
    BuyFixture {
        submission: captured.submission,
        signal: captured.signal,
        occurrence,
        evidence,
        sources: captured.sources,
    }
}

pub(super) async fn seed_submission(
    estimates: &EstimateStore<'_>,
    signals: &SignalStore<'_>,
    task_id_str: &str,
    bundle: &TaskBundle,
    submission: SignalSubmission,
) -> domain::IssuedEstimateTaskId {
    let run = AiRunId::try_new("run-sig-1").expect("run");
    let task_id = CalibrationEstimateId::try_new(task_id_str).expect("task id");
    estimates
        .append_calibration(&calibration_key(), bundle.calibration.calibration())
        .await
        .expect("cal");
    estimates
        .issue_task(&task_id, &run, &bundle.eligible)
        .await
        .expect("issue");
    let before = estimates.assignments().await.expect("before");
    let issued = before.lookup(&task_id).expect("lookup");
    let issued_id = issued.id().clone();
    let record = EstimateSubmissionRecord::create(
        issued,
        submission,
        EstimateNarrative {
            rationale: UntrustedText::try_new("상승 근거").expect("r"),
            counterargument: UntrustedText::try_new("반론").expect("c"),
        },
        EstimateMetadata {
            effort: ReasoningEffort::High,
            cli_version: ModelIdentityPart::try_new("1.0").expect("v"),
            prompt: EstimatePrompt {
                id: ScorecardPromptId::try_new("prompt-1").expect("pid"),
                version: ModelIdentityPart::try_new("1").expect("pv"),
                hash: PromptHash::try_new(&"a".repeat(64)).expect("ph"),
            },
            evidence_bundle: EvidenceBundleId::try_new("bundle-1").expect("eb"),
        },
    )
    .expect("record");
    let after = before
        .consume(&run, std::slice::from_ref(&task_id))
        .expect("consume");
    estimates
        .commit_submission(&before, &after, std::slice::from_ref(&record))
        .await
        .expect("commit");
    signals.save_master(&bundle.master).await.expect("master");
    issued_id
}
