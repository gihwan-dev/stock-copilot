use crate::{
    AcquisitionPrice, AllocationInstrument, BuyingPower, Currency, ExcludedSecurityType, FundKind,
    Holding, HoldingObservation, InstrumentClassification, InvestorProfile, Market, Money,
    Nanosecond, NumericInput, OffsetSeconds, Policy, PolicyBuyDecision, PolicyCandidate,
    PolicySecurityFacts, PolicyViolation, Portfolio, PortfolioValuation, Quantity, Ratio,
    ReportedHolding, ResolvedOrderInstrument, SectorAssignment, SectorAssignments, Symbol,
    Timestamp, UnixSeconds,
};
use std::collections::BTreeMap;

fn money(text: &str) -> Money {
    Money::parse(&NumericInput::try_new(text).expect("입력"), Currency::Krw).expect("금액")
}

fn instrument(text: &str) -> ResolvedOrderInstrument {
    ResolvedOrderInstrument::from_master_lookup_result(
        Symbol::try_new(text).expect("심볼"),
        Market::Kr,
    )
    .expect("종목")
}

fn time() -> Timestamp {
    Timestamp::from_parts(
        UnixSeconds::new(1000),
        Nanosecond::new(0),
        OffsetSeconds::new(0),
    )
    .expect("시각")
}

fn holding(text: &str, value: &str) -> Holding {
    Holding::new(
        instrument(text),
        Quantity::parse(&NumericInput::try_new("1").expect("입력")).expect("수량"),
        AcquisitionPrice::try_new(money("0")).expect("원가"),
        HoldingObservation::new(
            None,
            Some(
                ReportedHolding::new(
                    money(value),
                    money("0"),
                    Ratio::parse(&NumericInput::try_new("0").expect("입력")).expect("비율"),
                )
                .expect("평가"),
            ),
            time(),
        ),
    )
    .expect("보유")
}

fn valuation(rows: Vec<Holding>, cash: &str) -> PortfolioValuation {
    Portfolio::new(
        rows,
        BuyingPower::new(money(cash), time()),
        BuyingPower::new(Money::new(money("0").amount(), Currency::Usd), time()),
    )
    .expect("포트폴리오")
    .evaluate(None)
}

fn etf_assignments() -> SectorAssignments {
    SectorAssignments::try_new(
        ["IDX", "LEV", "THEME"]
            .map(|symbol| {
                SectorAssignment::new(&instrument(symbol), InstrumentClassification::EtfFund)
            })
            .to_vec(),
    )
    .expect("분류")
}

fn facts() -> PolicySecurityFacts {
    let key = |symbol: &str| AllocationInstrument::new(&instrument(symbol));
    let types: BTreeMap<_, _> = ["IDX", "LEV", "THEME"]
        .map(|symbol| (key(symbol), ExcludedSecurityType::Etf))
        .into_iter()
        .collect();
    let kinds: BTreeMap<_, _> = [
        ("IDX", FundKind::BroadIndexFund),
        ("LEV", FundKind::LeveragedOrInverse),
        ("THEME", FundKind::SectorOrThemeFund),
    ]
    .map(|(symbol, kind)| (key(symbol), kind))
    .into_iter()
    .collect();
    PolicySecurityFacts::from_master_lookup_results(types).with_fund_kinds(kinds)
}

fn candidate(symbol: &str, amount: &str) -> PolicyCandidate {
    PolicyCandidate::new(&instrument(symbol), money(amount)).expect("후보")
}

fn findings_for(symbol: &str, value: &str, cash: &str) -> Vec<PolicyViolation> {
    Policy::new(InvestorProfile::default())
        .check_current(
            &valuation(vec![holding(symbol, value)], cash),
            &etf_assignments(),
            &facts(),
        )
        .expect("점검")
}

#[test]
fn a_broad_index_fund_at_37_percent_is_a_warning_not_an_excess() {
    let key = AllocationInstrument::new(&instrument("IDX"));
    assert_eq!(
        findings_for("IDX", "376", "624"),
        vec![PolicyViolation::InstrumentWarning(key.clone())]
    );
    assert_eq!(
        findings_for("IDX", "410", "590"),
        vec![PolicyViolation::InstrumentCap(key)]
    );
    assert_eq!(findings_for("IDX", "150", "850"), Vec::new());
}

#[test]
fn a_leveraged_fund_is_capped_at_the_warning_line() {
    let key = AllocationInstrument::new(&instrument("LEV"));
    assert_eq!(
        findings_for("LEV", "120", "880"),
        vec![PolicyViolation::InstrumentCap(key.clone())]
    );
    assert_eq!(
        findings_for("LEV", "100", "900"),
        vec![PolicyViolation::InstrumentWarning(key)]
    );
}

#[test]
fn a_theme_fund_keeps_the_single_instrument_cap() {
    let key = AllocationInstrument::new(&instrument("THEME"));
    assert_eq!(
        findings_for("THEME", "250", "750"),
        vec![PolicyViolation::InstrumentCap(key)]
    );
}

#[test]
fn buy_room_follows_the_fund_kind_cap() {
    let policy = Policy::new(InvestorProfile::default());
    let current = valuation(vec![holding("IDX", "300")], "700");
    assert_eq!(
        policy.check_buy(
            &current,
            &etf_assignments(),
            &facts(),
            &candidate("IDX", "100")
        ),
        PolicyBuyDecision::Pass
    );
    let PolicyBuyDecision::Reduce(limit) = policy.check_buy(
        &current,
        &etf_assignments(),
        &facts(),
        &candidate("IDX", "101"),
    ) else {
        panic!("지수 ETF는 업종 한도까지 줄인다")
    };
    assert_eq!(
        (
            limit.compare_krw_cost(money("100")),
            limit.compare_krw_cost(money("100.01"))
        ),
        (
            crate::PolicyCostFit::WithinLimit,
            crate::PolicyCostFit::AboveLimit
        )
    );
    let levered = valuation(vec![holding("LEV", "100")], "900");
    assert_eq!(
        policy.check_buy(
            &levered,
            &etf_assignments(),
            &facts(),
            &candidate("LEV", "1")
        ),
        PolicyBuyDecision::Block(PolicyViolation::InstrumentCap(AllocationInstrument::new(
            &instrument("LEV")
        )))
    );
}

#[test]
fn facts_without_names_classify_by_security_type_alone() {
    let key = AllocationInstrument::new(&instrument("IDX"));
    let plain = PolicySecurityFacts::from_master_lookup_results(BTreeMap::from([(
        key.clone(),
        ExcludedSecurityType::Etf,
    )]));
    assert_eq!(plain.fund_kind(&key), FundKind::SectorOrThemeFund);
    assert_eq!(
        PolicySecurityFacts::default().fund_kind(&key),
        FundKind::SingleSecurity
    );
}
