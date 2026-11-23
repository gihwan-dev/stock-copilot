use super::a27_fixtures as fixtures;
use domain::{
    AcquisitionPrice, BuyingPower, Calibration, CalibrationLedgerHead, CalibrationSnapshot,
    CollectedItem, CommissionRateFact, CommissionValidity, CostRate, Currency, Holding,
    HoldingObservation, Horizon, InvestorProfile, KnownMarket, Margin, Market, MasterInstrument,
    MasterInstrumentFacts, NumericInput, Policy, Portfolio, ReportedHolding, SectorAssignments,
    SectorCatalog, SectorComposition, SectorDepth, SectorId, SectorMetadataRow, SectorStockRow,
    SignalDecision, Timestamp, TradeTaxFact, sector_assignment_from_compositions,
};

fn last_day(selection: domain::HorizonSelection) -> i64 {
    match selection {
        domain::HorizonSelection::Hours => 61,
        domain::HorizonSelection::FewDays => 65,
        domain::HorizonSelection::WeeksToMonths => 100,
        domain::HorizonSelection::HalfYearOrMore => 186,
    }
}

fn mapping_for(source: &MasterInstrument, at: Timestamp) -> SectorAssignments {
    let sector = SectorId::try_new("tech").expect("섹터");
    let catalog = SectorCatalog::from_response(
        vec![SectorMetadataRow {
            id: sector.clone(),
            name: domain::DisplayInstrumentName::try_new("기술").expect("이름"),
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
                AcquisitionPrice::try_new(fixtures::money("87", Currency::Krw)).expect("원가"),
                HoldingObservation::new(
                    None,
                    Some(
                        ReportedHolding::new(
                            fixtures::money("113", Currency::Krw),
                            fixtures::money("7", Currency::Krw),
                            fixtures::ratio("0.07"),
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
        BuyingPower::new(fixtures::money(cash, Currency::Krw), fixtures::day(60)),
        BuyingPower::new(fixtures::money("0", Currency::Usd), fixtures::day(60)),
    )
    .expect("포트")
}

fn source_for_instrument(instrument: &domain::InstrumentId) -> MasterInstrument {
    if instrument == &fixtures::master().row().instrument_id {
        fixtures::master()
    } else {
        fixtures::other_master()
    }
}

fn maturity_for(horizon: Horizon) -> Timestamp {
    fixtures::at(
        fixtures::day(last_day(horizon.selection()))
            .utc_seconds()
            .as_i64()
            .checked_add(3600)
            .expect("만기"),
    )
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
    let source = source_for_instrument(estimate.task().volatility().instrument());
    let submission = estimate.submission();
    let key = submission.calibration_key().clone();
    let horizon = key.horizon();
    let stored =
        fixtures::candidate_stored_task(estimate.task(), &source, horizon).expect("과제 근거");
    let head = estimate.calibration_head().clone();
    let snapshot = CalibrationSnapshot::try_new(
        Calibration::try_new(key.clone(), &[]).expect("보정"),
        head.clone(),
    )
    .expect("스냅숏");
    let zero = CostRate::try_new(fixtures::ratio("0")).expect("비용");
    let evidence = domain::SignalRestoreEvidence {
        estimate: stored,
        commission: CommissionRateFact::from_commissions_result(
            Market::Kr,
            zero,
            CommissionValidity::try_new(None, None).expect("기간"),
        ),
        tax: TradeTaxFact::from_schedule_result(Market::Kr, zero, zero),
        maturity: maturity_for(horizon),
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
        holding_masters: vec![],
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
        portfolio: Some(
            domain::HistoricalPortfolio::capture(
                &portfolio_for_signal(source, signal, cash),
                std::slice::from_ref(source),
            )
            .expect("포트폴리오 근거"),
        ),
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
            .map(|value| fixtures::calendar(value, &Market::Kr))
            .collect(),
        evaluated_at: fixtures::day(60),
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
    let source =
        if facts.task().volatility().instrument() == &fixtures::master().row().instrument_id {
            fixtures::master()
        } else {
            fixtures::other_master()
        };
    let assignments = mapping_for(&source, fixtures::day(60));
    let security = MasterInstrumentFacts::try_new(vec![source])
        .expect("마스터")
        .policy_security_facts();
    Some(evidence_for_signal(
        &signal,
        occurrence,
        "10000",
        fixtures::item(),
        &(assignments, security),
    ))
}
