use crate::{
    Allocation, AllocationError, AllocationInstrument, AllocationSector, AssetAllocation,
    AssetInstrument, Currency, ExcludedSecurityType, ExclusionDecision, InvestorProfile, Money,
    PolicyConcentration, PolicyHoldingCount, PolicyPrinciple, PolicySecurityFacts, PolicyText,
    PortfolioValuation, ProfileWeight, ResolvedOrderInstrument, SectorAssignments, SectorId, Total,
};
use std::{cmp::Ordering, fmt};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicyViolation {
    InstrumentCap(AllocationInstrument),
    InstrumentWarning(AllocationInstrument),
    SectorCap(SectorId),
    ExcludedSector(SectorId),
    ExcludedType(ExcludedSecurityType),
    CandidateSectorUnassigned,
    SectorUnassignedHoldings(PolicyHoldingCount),
    UnknownSecurityType,
    MinimumCash,
    ConcentrationWarning,
    ConcentrationUnavailable,
    UnresolvedPrinciples,
    Allocation(AllocationError),
    Arithmetic(crate::MoneyError),
    InvalidCandidate,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicyBuyDecision {
    Pass,
    Reduce(PolicyBuyLimit),
    Block(PolicyViolation),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicySellDecision {
    Allowed,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyCostFit {
    WithinLimit,
    AboveLimit,
    InvalidCost,
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct PolicyBuyLimit(Money);
impl fmt::Debug for PolicyBuyLimit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("PolicyBuyLimit([비공개])")
    }
}
impl PolicyBuyLimit {
    #[must_use]
    pub fn compare_krw_cost(self, cost: Money) -> PolicyCostFit {
        if cost.currency() != Currency::Krw || cost.amount().as_decimal().is_sign_negative() {
            return PolicyCostFit::InvalidCost;
        }
        if cost.amount().as_decimal() > self.0.amount().as_decimal() {
            PolicyCostFit::AboveLimit
        } else {
            PolicyCostFit::WithinLimit
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyCandidate {
    instrument: AllocationInstrument,
    requested_krw: Money,
}
impl PolicyCandidate {
    pub fn new(
        instrument: &ResolvedOrderInstrument,
        requested_krw: Money,
    ) -> Result<Self, PolicyViolation> {
        if requested_krw.currency() != Currency::Krw
            || requested_krw.amount().as_decimal() <= rust_decimal::Decimal::ZERO
        {
            return Err(PolicyViolation::InvalidCandidate);
        }
        Ok(Self {
            instrument: AllocationInstrument::new(instrument),
            requested_krw,
        })
    }
    #[must_use]
    pub const fn instrument(&self) -> &AllocationInstrument {
        &self.instrument
    }
    #[must_use]
    pub const fn requested_krw(&self) -> Money {
        self.requested_krw
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Policy {
    profile: InvestorProfile,
    unresolved: Vec<PolicyPrinciple>,
    context: Vec<PolicyText>,
    minimum_cash: Option<ProfileWeight>,
    concentration: PolicyConcentration,
}
impl Policy {
    #[must_use]
    pub const fn new(profile: InvestorProfile) -> Self {
        Self {
            profile,
            unresolved: Vec::new(),
            context: Vec::new(),
            minimum_cash: None,
            concentration: PolicyConcentration::NotApplicable,
        }
    }
    #[must_use]
    pub fn with_unresolved_principle(mut self, principle: PolicyPrinciple) -> Self {
        self.unresolved.push(principle);
        self
    }
    #[must_use]
    pub fn with_context(mut self, text: PolicyText) -> Self {
        self.context.push(text);
        self
    }
    #[must_use]
    pub fn with_minimum_cash(mut self, minimum: ProfileWeight) -> Self {
        self.minimum_cash = Some(minimum);
        self
    }
    #[must_use]
    pub fn with_concentration(mut self, concentration: PolicyConcentration) -> Self {
        self.concentration = concentration;
        self
    }
    #[must_use]
    pub const fn profile(&self) -> &InvestorProfile {
        &self.profile
    }
    #[must_use]
    pub fn unresolved_principles(&self) -> &[PolicyPrinciple] {
        &self.unresolved
    }
    #[must_use]
    pub fn context(&self) -> &[PolicyText] {
        &self.context
    }
    #[must_use]
    pub const fn minimum_cash(&self) -> Option<ProfileWeight> {
        self.minimum_cash
    }
    #[must_use]
    pub const fn concentration(&self) -> PolicyConcentration {
        self.concentration
    }
    #[must_use]
    pub fn check_sell() -> PolicySellDecision {
        PolicySellDecision::Allowed
    }
    pub fn check_current(
        &self,
        valuation: &PortfolioValuation,
        assignments: &SectorAssignments,
        facts: &PolicySecurityFacts,
    ) -> Result<Vec<PolicyViolation>, PolicyViolation> {
        self.require_resolved()?;
        let snapshot = snapshot(valuation, assignments)?;
        let mut findings =
            current_instruments(&self.profile, &snapshot.instruments, assignments, facts)?;
        for row in snapshot.sectors.entries() {
            if let AllocationSector::Classified(sector) = row.key()
                && row
                    .weight()
                    .cmp_profile_weight(&self.profile.weight_caps().sector_cap())
                    == Ordering::Greater
            {
                findings.push(PolicyViolation::SectorCap(sector.clone()));
            }
        }
        match self.concentration {
            PolicyConcentration::Warning => findings.push(PolicyViolation::ConcentrationWarning),
            PolicyConcentration::Unavailable => {
                findings.push(PolicyViolation::ConcentrationUnavailable);
            }
            PolicyConcentration::Clear | PolicyConcentration::NotApplicable => {}
        }
        if let Some(minimum) = self.minimum_cash {
            findings.extend(current_cash_finding(
                snapshot.total,
                cash_value(snapshot.total, &snapshot.rows),
                minimum,
            )?);
        }
        Ok(findings)
    }
    #[must_use]
    pub fn check_buy(
        &self,
        valuation: &PortfolioValuation,
        assignments: &SectorAssignments,
        facts: &PolicySecurityFacts,
        candidate: &PolicyCandidate,
    ) -> PolicyBuyDecision {
        match self.buy_limit(valuation, assignments, facts, candidate) {
            Ok(limit) => {
                if limit.compare_krw_cost(candidate.requested_krw) == PolicyCostFit::AboveLimit {
                    PolicyBuyDecision::Reduce(limit)
                } else {
                    PolicyBuyDecision::Pass
                }
            }
            Err(violation) => PolicyBuyDecision::Block(violation),
        }
    }
    fn require_resolved(&self) -> Result<(), PolicyViolation> {
        if self.unresolved.is_empty() {
            Ok(())
        } else {
            Err(PolicyViolation::UnresolvedPrinciples)
        }
    }
    fn buy_limit(
        &self,
        valuation: &PortfolioValuation,
        assignments: &SectorAssignments,
        facts: &PolicySecurityFacts,
        candidate: &PolicyCandidate,
    ) -> Result<PolicyBuyLimit, PolicyViolation> {
        self.require_resolved()?;
        if let Some(violation) = type_violation(&self.profile, facts, &candidate.instrument)? {
            return Err(violation);
        }
        let sector = candidate_sector(&self.profile, assignments, &candidate.instrument)?;
        let snapshot = snapshot(valuation, assignments)?;
        let caps = self.profile.weight_caps();
        reject_at_cap(
            &snapshot.instruments,
            &AssetInstrument::Holding(candidate.instrument.clone()),
            caps.instrument_cap(),
            PolicyViolation::InstrumentCap(candidate.instrument.clone()),
        )?;
        if let AllocationSector::Classified(id) = &sector {
            reject_at_cap(
                &snapshot.sectors,
                &sector,
                caps.sector_cap(),
                PolicyViolation::SectorCap(id.clone()),
            )?;
        }
        let values = exposure_values(
            snapshot.rows.clone(),
            assignments,
            &candidate.instrument,
            &sector,
        );
        let limit =
            limit_from_values(snapshot.total, values, caps, &candidate.instrument, &sector)?;
        match self.minimum_cash {
            None => Ok(limit),
            Some(minimum) => cash_value(snapshot.total, &snapshot.rows)
                .and_then(|cash| cash_headroom(snapshot.total, cash, minimum))
                .map(|cash_room| minimum_room(limit.0, cash_room)),
        }
    }
}
struct PolicySnapshot {
    instruments: AssetAllocation<AssetInstrument>,
    sectors: AssetAllocation<AllocationSector>,
    total: Money,
    rows: Vec<(AllocationInstrument, Money)>,
}
fn snapshot(
    valuation: &PortfolioValuation,
    assignments: &SectorAssignments,
) -> Result<PolicySnapshot, PolicyViolation> {
    let (total, rates, _) = match valuation.assets() {
        Total::Complete(value) => value.value().clone().into_parts(),
        Total::Incomplete(value) => value.subtotal().clone().into_parts(),
    };
    let rows = crate::allocation::policy_krw_holding_values(valuation, &rates)
        .map_err(PolicyViolation::Allocation)?;
    let (instruments, sectors) = asset_axes(&Allocation::new(valuation, assignments))?;
    Ok(PolicySnapshot {
        instruments,
        sectors,
        total,
        rows,
    })
}
fn type_violation(
    profile: &InvestorProfile,
    facts: &PolicySecurityFacts,
    instrument: &AllocationInstrument,
) -> Result<Option<PolicyViolation>, PolicyViolation> {
    let kind = facts
        .security_type(instrument)
        .ok_or(PolicyViolation::UnknownSecurityType)?;
    Ok(match profile.exclusions().security_type_decision(kind) {
        ExclusionDecision::Excluded => Some(PolicyViolation::ExcludedType(kind)),
        ExclusionDecision::Included => None,
    })
}
fn candidate_sector(
    profile: &InvestorProfile,
    assignments: &SectorAssignments,
    instrument: &AllocationInstrument,
) -> Result<AllocationSector, PolicyViolation> {
    let sector = assignments.sector(instrument);
    match &sector {
        AllocationSector::Unclassified => return Err(PolicyViolation::CandidateSectorUnassigned),
        AllocationSector::Classified(id)
            if profile.exclusions().sector_decision(id) == ExclusionDecision::Excluded =>
        {
            return Err(PolicyViolation::ExcludedSector(id.clone()));
        }
        AllocationSector::Classified(_) | AllocationSector::EtfFund | AllocationSector::Cash => {}
    }
    Ok(sector)
}
fn current_instruments(
    profile: &InvestorProfile,
    instruments: &AssetAllocation<AssetInstrument>,
    assignments: &SectorAssignments,
    facts: &PolicySecurityFacts,
) -> Result<Vec<PolicyViolation>, PolicyViolation> {
    let mut findings = Vec::new();
    let mut unassigned = 0_usize;
    let caps = profile.weight_caps();
    for row in instruments.entries() {
        let AssetInstrument::Holding(instrument) = row.key() else {
            continue;
        };
        if let Some(violation) = type_violation(profile, facts, instrument)? {
            findings.push(violation);
        }
        match assignments.sector(instrument) {
            AllocationSector::Classified(id) => {
                if profile.exclusions().sector_decision(&id) == ExclusionDecision::Excluded {
                    findings.push(PolicyViolation::ExcludedSector(id));
                }
            }
            AllocationSector::Unclassified => unassigned = unassigned.saturating_add(1),
            AllocationSector::EtfFund | AllocationSector::Cash => {}
        }
        if row.weight().cmp_profile_weight(&caps.instrument_cap()) == Ordering::Greater {
            findings.push(PolicyViolation::InstrumentCap(instrument.clone()));
        } else if row.weight().cmp_profile_weight(&caps.instrument_warning()) != Ordering::Less {
            findings.push(PolicyViolation::InstrumentWarning(instrument.clone()));
        }
    }
    if unassigned > 0 {
        findings.push(PolicyViolation::SectorUnassignedHoldings(
            PolicyHoldingCount::new(unassigned),
        ));
    }
    Ok(findings)
}
fn cash_value(
    total: Money,
    rows: &[(AllocationInstrument, Money)],
) -> Result<Money, PolicyViolation> {
    rows.iter()
        .try_fold(crate::holding::zero(Currency::Krw), |sum, (_, value)| {
            sum.checked_add(value)
        })
        .and_then(|holdings| total.checked_sub(&holdings))
        .map_err(PolicyViolation::Arithmetic)
}
fn current_cash_finding(
    total: Money,
    cash: Result<Money, PolicyViolation>,
    minimum: ProfileWeight,
) -> Result<Option<PolicyViolation>, PolicyViolation> {
    let checked_cash = cash?;
    let threshold = total
        .checked_mul(&minimum.as_ratio())
        .map_err(PolicyViolation::Arithmetic)?;
    Ok(
        (checked_cash.amount().as_decimal() < threshold.amount().as_decimal())
            .then_some(PolicyViolation::MinimumCash),
    )
}
fn cash_headroom(
    total: Money,
    cash: Money,
    minimum: ProfileWeight,
) -> Result<Money, PolicyViolation> {
    let remaining = total
        .checked_mul(&minimum.as_ratio())
        .and_then(|threshold| cash.checked_sub(&threshold))
        .map_err(PolicyViolation::Arithmetic)?;
    if remaining.amount().as_decimal() <= rust_decimal::Decimal::ZERO {
        Err(PolicyViolation::MinimumCash)
    } else {
        Ok(remaining)
    }
}
fn limit_from_values(
    total: Money,
    values: Result<(Money, Money), PolicyViolation>,
    caps: crate::WeightCaps,
    instrument: &AllocationInstrument,
    sector: &AllocationSector,
) -> Result<PolicyBuyLimit, PolicyViolation> {
    let checked_values = values?;
    let instrument_room = room(
        total,
        checked_values.0,
        caps.instrument_cap(),
        PolicyViolation::InstrumentCap(instrument.clone()),
    )?;
    match sector {
        AllocationSector::Classified(id) => room(
            total,
            checked_values.1,
            caps.sector_cap(),
            PolicyViolation::SectorCap(id.clone()),
        )
        .map(|sector_room| minimum_room(instrument_room, sector_room)),
        AllocationSector::EtfFund | AllocationSector::Unclassified | AllocationSector::Cash => {
            Ok(PolicyBuyLimit(instrument_room))
        }
    }
}
fn minimum_room(instrument: Money, sector: Money) -> PolicyBuyLimit {
    PolicyBuyLimit(
        match instrument
            .amount()
            .as_decimal()
            .cmp(&sector.amount().as_decimal())
        {
            Ordering::Less | Ordering::Equal => instrument,
            Ordering::Greater => sector,
        },
    )
}
fn asset_axes(
    allocation: &Allocation,
) -> Result<
    (
        AssetAllocation<AssetInstrument>,
        AssetAllocation<AllocationSector>,
    ),
    PolicyViolation,
> {
    allocation
        .assets()
        .instruments()
        .clone()
        .and_then(|instruments| {
            allocation
                .assets()
                .sectors()
                .clone()
                .map(|sectors| (instruments, sectors))
        })
        .map_err(PolicyViolation::Allocation)
}
fn exposure_values(
    rows: Vec<(AllocationInstrument, Money)>,
    assignments: &SectorAssignments,
    candidate: &AllocationInstrument,
    sector: &AllocationSector,
) -> Result<(Money, Money), PolicyViolation> {
    let mut instrument_value = crate::holding::zero(Currency::Krw);
    let mut sector_value = crate::holding::zero(Currency::Krw);
    for (instrument, value) in rows {
        if &instrument == candidate {
            instrument_value = value;
        }
        if &assignments.sector(&instrument) == sector {
            sector_value = sector_value
                .checked_add(&value)
                .map_err(PolicyViolation::Arithmetic)?;
        }
    }
    Ok((instrument_value, sector_value))
}
fn reject_at_cap<K: PartialEq>(
    allocation: &AssetAllocation<K>,
    key: &K,
    cap: ProfileWeight,
    violation: PolicyViolation,
) -> Result<(), PolicyViolation> {
    match allocation.entries().iter().find(|row| row.key() == key) {
        Some(row) if row.weight().cmp_profile_weight(&cap) != Ordering::Less => Err(violation),
        _ => Ok(()),
    }
}
fn room(
    total: Money,
    current: Money,
    cap: ProfileWeight,
    violation: PolicyViolation,
) -> Result<Money, PolicyViolation> {
    let remaining = total
        .checked_mul(&cap.as_ratio())
        .and_then(|maximum| maximum.checked_sub(&current))
        .map_err(PolicyViolation::Arithmetic)?;
    if remaining.amount().as_decimal() <= rust_decimal::Decimal::ZERO {
        Err(violation)
    } else {
        Ok(remaining)
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        AcquisitionPrice, AllocationInstrument, AllocationSector, BuyingPower, Currency,
        ExcludedSecurityType, Exclusions, Holding, HoldingObservation, InstrumentClassification,
        InvestorProfile, Market, Money, Nanosecond, NumericInput, OffsetSeconds, Policy,
        PolicyBuyDecision, PolicyCandidate, PolicyConcentration, PolicyCostFit, PolicyPrinciple,
        PolicySecurityFacts, PolicyText, PolicyViolation, Portfolio, PortfolioValuation, Quantity,
        Ratio, ReportedHolding, ResolvedOrderInstrument, SectorAssignment, SectorAssignments,
        SectorId, Symbol, Timestamp, UnixSeconds,
    };
    use proptest::test_runner::{Config, TestRunner};
    fn assert_block(decision: &PolicyBuyDecision) {
        assert_eq!(
            std::mem::discriminant(decision),
            std::mem::discriminant(&PolicyBuyDecision::Block(PolicyViolation::InvalidCandidate))
        );
    }
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
    fn sector() -> SectorId {
        SectorId::try_new("Tech").expect("섹터")
    }
    fn time() -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(1000),
            Nanosecond::new(0),
            OffsetSeconds::new(0),
        )
        .expect("시각")
    }
    fn holding(text: &str, value: Option<&str>) -> Holding {
        Holding::new(
            instrument(text),
            Quantity::parse(&NumericInput::try_new("1").expect("입력")).expect("수량"),
            AcquisitionPrice::try_new(money("0")).expect("원가"),
            HoldingObservation::new(
                None,
                value.map(|text_value| {
                    ReportedHolding::new(
                        money(text_value),
                        money("0"),
                        Ratio::parse(&NumericInput::try_new("0").expect("입력")).expect("비율"),
                    )
                    .expect("평가")
                }),
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
    fn assignments() -> SectorAssignments {
        SectorAssignments::try_new(
            ["A", "B", "NEW"]
                .map(|value| {
                    SectorAssignment::new(
                        &instrument(value),
                        InstrumentClassification::Sector(sector()),
                    )
                })
                .to_vec(),
        )
        .expect("분류")
    }
    fn candidate(symbol: &str, amount: &str) -> PolicyCandidate {
        PolicyCandidate::new(&instrument(symbol), money(amount)).expect("후보")
    }
    fn policy() -> Policy {
        Policy::new(InvestorProfile::default())
    }
    fn known_types() -> PolicySecurityFacts {
        PolicySecurityFacts::from_master_lookup_results(
            ["A", "B", "C", "NEW", "ETF", "X"]
                .map(|name| {
                    (
                        AllocationInstrument::new(&instrument(name)),
                        if name == "ETF" {
                            ExcludedSecurityType::Etf
                        } else {
                            ExcludedSecurityType::Stock
                        },
                    )
                })
                .into_iter()
                .collect(),
        )
    }
    fn custom_policy(instrument: u32, sector: u32) -> Policy {
        let cap = crate::ProfileWeight::parse_percent(
            &NumericInput::try_new(&instrument.to_string()).expect("입력"),
        )
        .expect("상한");
        let sector_cap = crate::ProfileWeight::parse_percent(
            &NumericInput::try_new(&sector.to_string()).expect("입력"),
        )
        .expect("상한");
        Policy::new(
            InvestorProfile::default()
                .with_weight_caps(crate::WeightCaps::try_new(cap, cap, sector_cap).expect("관계")),
        )
    }
    fn reference_room(
        total: u32,
        cap_percent: u32,
        sector_percent: u32,
        first: u32,
        sibling: u32,
    ) -> i64 {
        let instrument_room = i64::from(total)
            .checked_mul(i64::from(cap_percent))
            .and_then(|value| value.checked_sub(i64::from(first).checked_mul(100).expect("분자")))
            .expect("종목 여유 분자");
        let sector_room = i64::from(total)
            .checked_mul(i64::from(sector_percent))
            .and_then(|value| {
                value.checked_sub(
                    i64::from(first.checked_add(sibling).expect("합"))
                        .checked_mul(100)
                        .expect("분자"),
                )
            })
            .expect("섹터 여유 분자");
        instrument_room.min(sector_room)
    }
    #[test]
    fn buy_pass_reduce_and_block_at_both_caps() {
        let initial = valuation(
            vec![holding("A", Some("10")), holding("B", Some("25"))],
            "65",
        );
        assert_eq!(
            policy().check_buy(
                &initial,
                &assignments(),
                &known_types(),
                &candidate("A", "5")
            ),
            PolicyBuyDecision::Pass
        );
        let limit = super::PolicyBuyLimit(money("5"));
        assert_eq!(
            policy().check_buy(
                &initial,
                &assignments(),
                &known_types(),
                &candidate("A", "10")
            ),
            PolicyBuyDecision::Reduce(limit)
        );
        assert_eq!(
            limit.compare_krw_cost(money("5")),
            PolicyCostFit::WithinLimit
        );
        assert_eq!(
            limit.compare_krw_cost(money("5.000000000000000000000000001")),
            PolicyCostFit::AboveLimit
        );
        let at_instrument = valuation(vec![holding("A", Some("20"))], "80");
        assert_eq!(
            policy().check_buy(
                &at_instrument,
                &assignments(),
                &known_types(),
                &candidate("A", "1")
            ),
            PolicyBuyDecision::Block(PolicyViolation::InstrumentCap(AllocationInstrument::new(
                &instrument("A")
            )))
        );
        let at_sector = valuation(
            vec![holding("A", Some("19")), holding("B", Some("21"))],
            "60",
        );
        assert_eq!(
            policy().check_buy(
                &at_sector,
                &assignments(),
                &known_types(),
                &candidate("A", "1")
            ),
            PolicyBuyDecision::Block(PolicyViolation::SectorCap(sector()))
        );
    }
    #[test]
    fn current_warning_and_excess_are_distinct_from_preflight() {
        let warning_values = valuation(vec![holding("A", Some("10"))], "90");
        assert_eq!(
            policy()
                .check_current(&warning_values, &assignments(), &known_types())
                .expect("완전"),
            vec![PolicyViolation::InstrumentWarning(
                AllocationInstrument::new(&instrument("A"))
            )]
        );
        let excess_values = valuation(
            vec![holding("A", Some("25")), holding("B", Some("25"))],
            "50",
        );
        let result = policy()
            .check_current(&excess_values, &assignments(), &known_types())
            .expect("상태");
        assert!(
            result.contains(&PolicyViolation::InstrumentCap(AllocationInstrument::new(
                &instrument("A")
            )))
        );
        assert!(result.contains(&PolicyViolation::SectorCap(sector())));
        assert_eq!(Policy::check_sell(), crate::PolicySellDecision::Allowed);
    }
    #[test]
    fn integer_reference_headroom_is_maximal_and_order_independent() {
        TestRunner::new(Config::with_cases(4096))
            .run(&(0_u32..20, 0_u32..40), |(first_value, second_value)| {
                let cash = 100_u32
                    .checked_sub(first_value)
                    .and_then(|value| value.checked_sub(second_value))
                    .expect("현금");
                let rows = vec![
                    holding("A", Some(&first_value.to_string())),
                    holding("B", Some(&second_value.to_string())),
                ];
                let first = valuation(rows.clone(), &cash.to_string());
                let second = valuation(rows.into_iter().rev().collect(), &cash.to_string());
                let actual = policy().check_buy(
                    &first,
                    &assignments(),
                    &known_types(),
                    &candidate("A", "100"),
                );
                assert_eq!(
                    actual,
                    policy().check_buy(
                        &second,
                        &assignments(),
                        &known_types(),
                        &candidate("A", "100")
                    )
                );
                if first_value.checked_add(second_value).expect("합") >= 40 {
                    assert_block(&actual);
                } else {
                    let expected = 20_u32.checked_sub(first_value).expect("종목 여유").min(
                        40_u32
                            .checked_sub(first_value)
                            .and_then(|value| value.checked_sub(second_value))
                            .expect("섹터 여유"),
                    );
                    let limit = super::PolicyBuyLimit(money(&expected.to_string()));
                    assert_eq!(actual, PolicyBuyDecision::Reduce(limit));
                    assert_eq!(
                        limit.compare_krw_cost(money(&expected.to_string())),
                        PolicyCostFit::WithinLimit
                    );
                    assert_eq!(
                        limit.compare_krw_cost(money(
                            &expected.checked_add(1).expect("최소 단위").to_string()
                        )),
                        PolicyCostFit::AboveLimit
                    );
                }
                Ok(())
            })
            .expect("독립 정수 기준");
    }
    #[test]
    fn exact_room_errors_and_limit_boundaries_are_conservative() {
        let cap = InvestorProfile::default().weight_caps().instrument_cap();
        assert_eq!(
            super::room(
                money("100"),
                money("20"),
                cap,
                PolicyViolation::InvalidCandidate
            ),
            Err(PolicyViolation::InvalidCandidate)
        );
        assert_eq!(
            super::room(
                money("0.0000000000000000000000000001"),
                money("0"),
                cap,
                PolicyViolation::InvalidCandidate
            ),
            Err(PolicyViolation::Arithmetic(
                crate::MoneyError::PrecisionLoss
            ))
        );
        let tiny = valuation(
            vec![holding("A", Some("0"))],
            "0.0000000000000000000000000001",
        );
        assert_eq!(
            policy().check_buy(&tiny, &assignments(), &known_types(), &candidate("A", "1")),
            PolicyBuyDecision::Block(PolicyViolation::Arithmetic(
                crate::MoneyError::PrecisionLoss
            ))
        );
        let overflow_rows = vec![
            (
                AllocationInstrument::new(&instrument("A")),
                money("79228162514264337593543950335"),
            ),
            (AllocationInstrument::new(&instrument("B")), money("1")),
        ];
        assert_eq!(
            super::exposure_values(
                overflow_rows,
                &assignments(),
                &AllocationInstrument::new(&instrument("A")),
                &AllocationSector::Classified(sector())
            ),
            Err(PolicyViolation::Arithmetic(crate::MoneyError::Overflow))
        );
    }
    #[test]
    fn rational_reference_matches_variable_caps_and_native_cent_boundary() {
        TestRunner::new(Config::with_cases(4096))
            .run(
                &(1_u32..100_000, 1_u32..100, 0_u32..100, 0_u32..100),
                |(total, cap_percent, instrument_percent, sibling_percent)| {
                    let first = total
                        .checked_mul(instrument_percent)
                        .expect("정수 곱")
                        .div_euclid(200);
                    let sibling = total
                        .checked_mul(sibling_percent)
                        .expect("정수 곱")
                        .div_euclid(200);
                    let cash = total
                        .checked_sub(first)
                        .and_then(|value| value.checked_sub(sibling))
                        .expect("현금");
                    let sector_percent = cap_percent.saturating_add(10).min(100);
                    let configured = custom_policy(cap_percent, sector_percent);
                    let values = valuation(
                        vec![
                            holding("A", Some(&first.to_string())),
                            holding("B", Some(&sibling.to_string())),
                        ],
                        &cash.to_string(),
                    );
                    let actual = configured.check_buy(
                        &values,
                        &assignments(),
                        &known_types(),
                        &candidate("A", &total.to_string()),
                    );
                    let room = reference_room(total, cap_percent, sector_percent, first, sibling);
                    if room <= 0 {
                        assert_block(&actual);
                    } else {
                        let ceiling = super::PolicyBuyLimit(Money::new(
                            crate::Amount::new(rust_decimal::Decimal::new(room, 2)),
                            Currency::Krw,
                        ));
                        assert_eq!(actual, PolicyBuyDecision::Reduce(ceiling));
                        let next = Money::new(
                            crate::Amount::new(rust_decimal::Decimal::new(
                                room.checked_add(1).expect("최소 단위"),
                                2,
                            )),
                            Currency::Krw,
                        );
                        assert_eq!(ceiling.compare_krw_cost(next), PolicyCostFit::AboveLimit);
                    }
                    Ok(())
                },
            )
            .expect("독립 유리수 기준");
    }
    #[test]
    fn maximum_total_and_all_excluded_types_preserve_limits() {
        let values = valuation(
            vec![holding("A", Some("0"))],
            "79228162514264337593543950335",
        );
        assert_eq!(
            policy().check_buy(
                &values,
                &assignments(),
                &known_types(),
                &candidate("A", "79228162514264337593543950335")
            ),
            PolicyBuyDecision::Reduce(super::PolicyBuyLimit(money(
                "15845632502852867518708790067"
            )))
        );
        for security_type in [
            ExcludedSecurityType::Stock,
            ExcludedSecurityType::ForeignStock,
            ExcludedSecurityType::DepositaryReceipt,
            ExcludedSecurityType::InfrastructureFund,
            ExcludedSecurityType::Reit,
            ExcludedSecurityType::Etf,
            ExcludedSecurityType::ForeignEtf,
            ExcludedSecurityType::Etn,
            ExcludedSecurityType::StockWarrants,
        ] {
            let configured =
                Policy::new(InvestorProfile::default().with_exclusions(
                    Exclusions::try_new(vec![], vec![security_type]).expect("제외"),
                ));
            let item = PolicyCandidate::new(&instrument("A"), money("1")).expect("후보");
            let facts = PolicySecurityFacts::from_master_lookup_results(
                std::iter::once((AllocationInstrument::new(&instrument("A")), security_type))
                    .collect(),
            );
            assert_eq!(
                configured.check_buy(&values, &assignments(), &facts, &item),
                PolicyBuyDecision::Block(PolicyViolation::ExcludedType(security_type))
            );
        }
    }
    #[test]
    fn exact_current_caps_are_not_excess_and_sell_ignores_unresolved_rules() {
        let values = valuation(
            vec![holding("A", Some("20")), holding("B", Some("20"))],
            "60",
        );
        assert_eq!(
            policy().check_current(&values, &assignments(), &known_types()),
            Ok(vec![
                PolicyViolation::InstrumentWarning(AllocationInstrument::new(&instrument("A"))),
                PolicyViolation::InstrumentWarning(AllocationInstrument::new(&instrument("B")))
            ])
        );
        assert_eq!(
            policy().check_buy(
                &values,
                &assignments(),
                &known_types(),
                &candidate("A", "1")
            ),
            PolicyBuyDecision::Block(PolicyViolation::InstrumentCap(AllocationInstrument::new(
                &instrument("A")
            )))
        );
        assert_eq!(
            policy().check_buy(
                &values,
                &assignments(),
                &known_types(),
                &candidate("NEW", "1")
            ),
            PolicyBuyDecision::Block(PolicyViolation::SectorCap(sector()))
        );
        assert_eq!(Policy::check_sell(), crate::PolicySellDecision::Allowed);
        let cash_only = valuation(vec![], "100");
        assert_eq!(
            policy().check_buy(
                &cash_only,
                &assignments(),
                &known_types(),
                &candidate("NEW", "20")
            ),
            PolicyBuyDecision::Pass
        );
        assert_eq!(
            policy().check_current(&cash_only, &assignments(), &PolicySecurityFacts::default()),
            Ok(vec![])
        );
    }
    #[test]
    fn review_etf_and_unclassified_holdings_do_not_block_other_sectors() {
        let values = valuation(
            vec![
                holding("ETF", Some("10")),
                holding("A", Some("10")),
                holding("B", Some("10")),
            ],
            "70",
        );
        let mapping = SectorAssignments::try_new(vec![
            SectorAssignment::new(&instrument("ETF"), InstrumentClassification::EtfFund),
            SectorAssignment::new(&instrument("A"), InstrumentClassification::Sector(sector())),
            SectorAssignment::new(
                &instrument("B"),
                InstrumentClassification::Sector(SectorId::try_new("Energy").expect("섹터")),
            ),
            SectorAssignment::new(
                &instrument("C"),
                InstrumentClassification::Sector(SectorId::try_new("Health").expect("섹터")),
            ),
        ])
        .expect("분류");
        assert_eq!(
            policy().check_buy(&values, &mapping, &known_types(), &candidate("C", "10")),
            PolicyBuyDecision::Pass
        );
        assert_eq!(
            policy().check_buy(&values, &mapping, &known_types(), &candidate("ETF", "5")),
            PolicyBuyDecision::Pass
        );
    }
    fn text(value: &str) -> PolicyText {
        PolicyText::try_new(value).expect("원칙 문맥")
    }
    fn mixed_mapping() -> SectorAssignments {
        SectorAssignments::try_new(vec![
            SectorAssignment::new(&instrument("ETF"), InstrumentClassification::EtfFund),
            SectorAssignment::new(&instrument("A"), InstrumentClassification::Sector(sector())),
            SectorAssignment::new(
                &instrument("B"),
                InstrumentClassification::Sector(SectorId::try_new("Energy").expect("섹터")),
            ),
            SectorAssignment::new(
                &instrument("C"),
                InstrumentClassification::Sector(SectorId::try_new("Health").expect("섹터")),
            ),
        ])
        .expect("분류")
    }
    #[test]
    fn review_five_ordinary_portfolio_rows_and_unassigned_candidate() {
        let values = valuation(
            vec![
                holding("ETF", Some("10")),
                holding("A", Some("10")),
                holding("B", Some("10")),
            ],
            "70",
        );
        let mapping = mixed_mapping();
        for symbol in ["C", "A", "ETF"] {
            assert_eq!(
                policy().check_buy(&values, &mapping, &known_types(), &candidate(symbol, "5")),
                PolicyBuyDecision::Pass
            );
            let expected = if symbol == "C" { "20" } else { "10" };
            assert_eq!(
                policy().check_buy(&values, &mapping, &known_types(), &candidate(symbol, "30")),
                PolicyBuyDecision::Reduce(super::PolicyBuyLimit(money(expected)))
            );
        }
        let unassigned = valuation(
            vec![
                holding("ETF", Some("10")),
                holding("A", Some("10")),
                holding("B", Some("10")),
                holding("X", Some("10")),
            ],
            "60",
        );
        assert_eq!(
            policy().check_buy(&unassigned, &mapping, &known_types(), &candidate("C", "5")),
            PolicyBuyDecision::Pass
        );
        assert_eq!(
            policy().check_buy(&unassigned, &mapping, &known_types(), &candidate("X", "5")),
            PolicyBuyDecision::Block(PolicyViolation::CandidateSectorUnassigned)
        );
        let findings = policy()
            .check_current(&unassigned, &mapping, &known_types())
            .expect("현재 상태");
        assert!(
            findings.contains(&PolicyViolation::SectorUnassignedHoldings(
                crate::PolicyHoldingCount::new(1)
            ))
        );
        assert_eq!(findings.len(), 5);
        assert_eq!(crate::PolicyHoldingCount::new(1).as_usize(), 1);
    }
    #[test]
    fn concentration_is_warning_and_text_is_context_without_a_gate() {
        let values = valuation(vec![holding("A", Some("10"))], "90");
        for state in [
            PolicyConcentration::Warning,
            PolicyConcentration::Clear,
            PolicyConcentration::NotApplicable,
            PolicyConcentration::Unavailable,
        ] {
            let configured = policy()
                .with_concentration(state)
                .with_context(text("반도체 장기 보유"));
            assert_eq!(configured.context(), &[text("반도체 장기 보유")]);
            assert_eq!(configured.concentration(), state);
            assert_eq!(
                configured.check_buy(
                    &values,
                    &assignments(),
                    &known_types(),
                    &candidate("A", "1")
                ),
                PolicyBuyDecision::Pass
            );
            let findings = configured
                .check_current(&values, &assignments(), &known_types())
                .expect("현재 상태");
            assert_eq!(
                findings.contains(&PolicyViolation::ConcentrationWarning),
                state == PolicyConcentration::Warning
            );
            assert_eq!(
                findings.contains(&PolicyViolation::ConcentrationUnavailable),
                state == PolicyConcentration::Unavailable
            );
        }
        assert_eq!(policy().minimum_cash(), None);
        assert_eq!(policy().unresolved_principles(), []);
        let first = PolicyPrinciple::from_missing_contract(text("실행 계약 A 미정"));
        let second = PolicyPrinciple::from_missing_contract(text("실행 계약 B 미정"));
        let configured = policy()
            .with_unresolved_principle(first.clone())
            .with_unresolved_principle(second.clone());
        assert_eq!(first.description().as_str(), "실행 계약 A 미정");
        assert_eq!(configured.unresolved_principles(), &[first, second]);
        assert_eq!(
            configured.check_buy(
                &values,
                &assignments(),
                &known_types(),
                &candidate("A", "1")
            ),
            PolicyBuyDecision::Block(PolicyViolation::UnresolvedPrinciples)
        );
        assert_eq!(
            configured.check_current(&values, &assignments(), &known_types()),
            Err(PolicyViolation::UnresolvedPrinciples)
        );
    }
    #[test]
    fn minimum_cash_pass_reduce_block_and_current_boundary() {
        let minimum = crate::ProfileWeight::parse(&NumericInput::try_new("0.8").expect("입력"))
            .expect("비중");
        let configured = policy().with_minimum_cash(minimum);
        assert_eq!(configured.minimum_cash(), Some(minimum));
        let values = valuation(
            vec![holding("A", Some("10")), holding("B", Some("5"))],
            "85",
        );
        assert_eq!(
            configured.check_buy(
                &values,
                &assignments(),
                &known_types(),
                &candidate("A", "5")
            ),
            PolicyBuyDecision::Pass
        );
        assert_eq!(
            configured.check_buy(
                &values,
                &assignments(),
                &known_types(),
                &candidate("A", "6")
            ),
            PolicyBuyDecision::Reduce(super::PolicyBuyLimit(money("5")))
        );
        assert!(
            !configured
                .check_current(&values, &assignments(), &known_types())
                .expect("현재")
                .contains(&PolicyViolation::MinimumCash)
        );
        for cash in ["80", "79"] {
            let boundary = valuation(
                vec![
                    holding("A", Some("10")),
                    holding("B", Some(if cash == "80" { "10" } else { "11" })),
                ],
                cash,
            );
            assert_eq!(
                configured.check_buy(
                    &boundary,
                    &assignments(),
                    &known_types(),
                    &candidate("NEW", "1")
                ),
                PolicyBuyDecision::Block(PolicyViolation::MinimumCash)
            );
            assert_eq!(
                configured
                    .check_current(&boundary, &assignments(), &known_types())
                    .expect("현재")
                    .contains(&PolicyViolation::MinimumCash),
                cash == "79"
            );
        }
        assert_eq!(Policy::check_sell(), crate::PolicySellDecision::Allowed);
    }
    #[test]
    fn reported_total_mismatch_cannot_supply_an_order_budget() {
        let clean = valuation(vec![holding("A", Some("10"))], "90");
        assert_eq!(
            policy().check_buy(
                &clean,
                &assignments(),
                &known_types(),
                &candidate("A", "200")
            ),
            PolicyBuyDecision::Reduce(super::PolicyBuyLimit(money("10")))
        );
        let amounts = crate::CurrencyAmounts::new(money("910"), None).expect("요약");
        let reported = Portfolio::new(
            vec![holding("A", Some("10"))],
            BuyingPower::new(money("90"), time()),
            BuyingPower::new(Money::new(money("0").amount(), Currency::Usd), time()),
        )
        .expect("포트폴리오")
        .with_reported(crate::ReportedPortfolio::new(
            amounts,
            amounts,
            amounts,
            time(),
        ))
        .evaluate(None);
        assert_eq!(
            policy().check_buy(
                &reported,
                &assignments(),
                &known_types(),
                &candidate("A", "200")
            ),
            PolicyBuyDecision::Block(PolicyViolation::Allocation(
                crate::AllocationError::TotalMismatch
            ))
        );
        assert_eq!(
            policy().check_current(&reported, &assignments(), &known_types()),
            Err(PolicyViolation::Allocation(
                crate::AllocationError::TotalMismatch
            ))
        );
    }
    #[test]
    fn facts_cannot_be_overridden_by_candidate_fields_and_unknown_types_block() {
        let values = valuation(vec![holding("B", Some("1"))], "99");
        let configured = Policy::new(
            InvestorProfile::default().with_exclusions(
                Exclusions::try_new(vec![SectorId::try_new("Energy").expect("섹터")], vec![])
                    .expect("제외"),
            ),
        );
        let item = candidate("B", "1");
        assert_eq!(
            item.instrument(),
            &AllocationInstrument::new(&instrument("B"))
        );
        assert_eq!(item.requested_krw(), money("1"));
        assert_eq!(
            configured.check_buy(&values, &mixed_mapping(), &known_types(), &item),
            PolicyBuyDecision::Block(PolicyViolation::ExcludedSector(
                SectorId::try_new("Energy").expect("섹터")
            ))
        );
        assert_eq!(
            policy().check_buy(
                &values,
                &mixed_mapping(),
                &PolicySecurityFacts::default(),
                &item
            ),
            PolicyBuyDecision::Block(PolicyViolation::UnknownSecurityType)
        );
        assert_eq!(
            policy().check_current(&values, &mixed_mapping(), &PolicySecurityFacts::default()),
            Err(PolicyViolation::UnknownSecurityType)
        );
        for cost in [
            money("0"),
            money("-1"),
            Money::new(money("1").amount(), Currency::Usd),
        ] {
            assert_eq!(
                PolicyCandidate::new(&instrument("A"), cost),
                Err(PolicyViolation::InvalidCandidate)
            );
        }
        assert_eq!(policy().profile(), &InvestorProfile::default());
        let limit = super::PolicyBuyLimit(money("12345.6789"));
        assert_eq!(format!("{limit:?}"), "PolicyBuyLimit([비공개])");
        assert_eq!(
            format!("{:?}", PolicyBuyDecision::Reduce(limit)),
            "Reduce(PolicyBuyLimit([비공개]))"
        );
        assert_eq!(
            limit.compare_krw_cost(money("-1")),
            PolicyCostFit::InvalidCost
        );
        assert_eq!(
            limit.compare_krw_cost(Money::new(money("1").amount(), Currency::Usd)),
            PolicyCostFit::InvalidCost
        );
    }
    #[test]
    fn generated_simultaneous_missing_axes_never_pass() {
        TestRunner::new(Config::with_cases(4096))
            .run(&(1_u32..10000, 1_u8..16), |(amount, missing)| {
                let values = valuation(
                    vec![holding(
                        "A",
                        if missing & 1 == 1 { None } else { Some("1") },
                    )],
                    &if missing & 2 == 2 {
                        format!("-{amount}")
                    } else {
                        amount.to_string()
                    },
                );
                let mapping = if missing & 4 == 4 {
                    SectorAssignments::default()
                } else {
                    assignments()
                };
                let facts = if missing & 8 == 8 {
                    PolicySecurityFacts::default()
                } else {
                    known_types()
                };
                assert_block(&policy().check_buy(&values, &mapping, &facts, &candidate("A", "1")));
                Ok(())
            })
            .expect("4096회 복합 누락");
    }
    fn fractional_fx_portfolio() -> (
        PortfolioValuation,
        ResolvedOrderInstrument,
        SectorAssignments,
        PolicySecurityFacts,
    ) {
        let item = ResolvedOrderInstrument::from_master_lookup_result(
            Symbol::try_new("AUS").expect("심볼"),
            Market::Us,
        )
        .expect("종목");
        let usd = |value| {
            Money::parse(&NumericInput::try_new(value).expect("입력"), Currency::Usd).expect("달러")
        };
        let report = ReportedHolding::new(
            usd("0.125"),
            usd("0"),
            Ratio::parse(&NumericInput::try_new("0").expect("입력")).expect("비율"),
        )
        .expect("평가");
        let row = Holding::new(
            item.clone(),
            Quantity::parse(&NumericInput::try_new("1").expect("입력")).expect("수량"),
            AcquisitionPrice::try_new(usd("0")).expect("원가"),
            HoldingObservation::new(None, Some(report), time()),
        )
        .expect("보유");
        let end = Timestamp::from_parts(
            UnixSeconds::new(1060),
            Nanosecond::new(0),
            OffsetSeconds::new(0),
        )
        .expect("끝");
        let rate = crate::FxRate::parse(
            &NumericInput::try_new("1375.25").expect("입력"),
            crate::FxPair::UsdKrw,
            crate::FxRateKind::Mid,
            crate::TimeInterval::try_new(time(), end).expect("구간"),
            crate::FxSource::TossOpenApi,
        )
        .expect("환율");
        let values = Portfolio::new(
            vec![row],
            BuyingPower::new(money("807.465"), time()),
            BuyingPower::new(usd("0.015"), time()),
        )
        .expect("포트폴리오")
        .evaluate(Some(rate));
        let mapping = SectorAssignments::try_new(vec![SectorAssignment::new(
            &item,
            InstrumentClassification::Sector(sector()),
        )])
        .expect("분류");
        let facts = PolicySecurityFacts::from_master_lookup_results(
            std::iter::once((
                AllocationInstrument::new(&item),
                ExcludedSecurityType::ForeignStock,
            ))
            .collect(),
        );
        (values, item, mapping, facts)
    }
    #[test]
    fn fractional_dollars_and_fx_are_not_floored_into_more_headroom() {
        let (values, item, mapping, facts) = fractional_fx_portfolio();
        let candidate = PolicyCandidate::new(&item, money("28.5")).expect("후보");
        let limit = super::PolicyBuyLimit(money("28.09375"));
        assert_eq!(
            policy().check_buy(&values, &mapping, &facts, &candidate),
            PolicyBuyDecision::Reduce(limit)
        );
        assert_eq!(
            limit.compare_krw_cost(money("28.09375")),
            PolicyCostFit::WithinLimit
        );
        assert_eq!(
            limit.compare_krw_cost(money("28.09375000000000000000000001")),
            PolicyCostFit::AboveLimit
        );
        let minimum = crate::ProfileWeight::parse(&NumericInput::try_new("0.82").expect("입력"))
            .expect("비중");
        assert_eq!(
            policy()
                .with_minimum_cash(minimum)
                .check_buy(&values, &mapping, &facts, &candidate),
            PolicyBuyDecision::Reduce(super::PolicyBuyLimit(money("8.09375")))
        );
    }
    #[test]
    fn cash_floor_matches_independent_integer_room() {
        TestRunner::new(Config::with_cases(4096))
            .run(
                &(0_u32..20, 0_u32..20, 1_u32..101),
                |(first, second, minimum)| {
                    let cash = 100_u32
                        .checked_sub(first)
                        .and_then(|value| value.checked_sub(second))
                        .expect("현금");
                    let values = valuation(
                        vec![
                            holding("A", Some(&first.to_string())),
                            holding("B", Some(&second.to_string())),
                        ],
                        &cash.to_string(),
                    );
                    let floor = crate::ProfileWeight::parse_percent(
                        &NumericInput::try_new(&minimum.to_string()).expect("입력"),
                    )
                    .expect("현금 최소");
                    let actual = policy().with_minimum_cash(floor).check_buy(
                        &values,
                        &assignments(),
                        &known_types(),
                        &candidate("A", "100"),
                    );
                    if minimum >= cash {
                        assert_eq!(
                            actual,
                            PolicyBuyDecision::Block(PolicyViolation::MinimumCash)
                        );
                    } else {
                        let remaining = cash
                            .checked_sub(minimum)
                            .expect("현금 여유")
                            .min(20_u32.checked_sub(first).expect("종목"))
                            .min(
                                40_u32
                                    .checked_sub(first)
                                    .and_then(|value| value.checked_sub(second))
                                    .expect("섹터"),
                            );
                        assert_eq!(
                            actual,
                            PolicyBuyDecision::Reduce(super::PolicyBuyLimit(money(
                                &remaining.to_string()
                            )))
                        );
                    }
                    Ok(())
                },
            )
            .expect("현금 하한 독립 기준");
    }
    #[test]
    fn current_exclusions_and_cash_arithmetic_errors_remain_explicit() {
        let values = valuation(vec![holding("A", Some("10"))], "90");
        for exclusions in [
            Exclusions::try_new(vec![sector()], vec![]).expect("제외"),
            Exclusions::try_new(vec![], vec![ExcludedSecurityType::Stock]).expect("제외"),
        ] {
            let configured = Policy::new(InvestorProfile::default().with_exclusions(exclusions));
            assert_ne!(
                configured
                    .check_current(&values, &assignments(), &known_types())
                    .expect("현재"),
                vec![PolicyViolation::InstrumentWarning(
                    AllocationInstrument::new(&instrument("A"))
                )]
            );
        }
        let minimum = crate::ProfileWeight::parse(&NumericInput::try_new("0.1").expect("입력"))
            .expect("비중");
        let tiny = valuation(
            vec![holding("A", Some("0"))],
            "0.0000000000000000000000000005",
        );
        let error = PolicyViolation::Arithmetic(crate::MoneyError::PrecisionLoss);
        assert_eq!(
            policy().with_minimum_cash(minimum).check_buy(
                &tiny,
                &assignments(),
                &known_types(),
                &candidate("A", "1")
            ),
            PolicyBuyDecision::Block(error.clone())
        );
        assert_eq!(
            policy().with_minimum_cash(minimum).check_current(
                &tiny,
                &assignments(),
                &known_types()
            ),
            Err(error.clone())
        );
        assert_eq!(
            super::current_cash_finding(money("100"), Err(error.clone()), minimum),
            Err(error.clone())
        );
        assert_eq!(
            super::limit_from_values(
                money("100"),
                Err(error.clone()),
                InvestorProfile::default().weight_caps(),
                &AllocationInstrument::new(&instrument("A")),
                &AllocationSector::Classified(sector())
            ),
            Err(error)
        );
    }
}
