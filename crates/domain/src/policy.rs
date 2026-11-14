use crate::{
    Allocation, AllocationError, AllocationInstrument, AllocationSector, AssetAllocation,
    AssetInstrument, Currency, ExcludedSecurityType, ExclusionDecision, InstrumentClassification,
    InvestorProfile, Money, PortfolioValuation, ProfileWeight, ResolvedOrderInstrument,
    SectorAssignments, SectorId, Total,
};
use std::{cmp::Ordering, collections::BTreeMap};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyPrinciple {
    FreeText,
    MinimumCash,
    ConcentrationWarning,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicyViolation {
    InstrumentCap(AllocationInstrument),
    InstrumentWarning(AllocationInstrument),
    SectorCap(SectorId),
    ExcludedSector(SectorId),
    ExcludedType(ExcludedSecurityType),
    UnknownClassification,
    UnknownSecurityType,
    UnresolvedPrinciple(PolicyPrinciple),
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PolicyBuyLimit(Money);
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
    security_type: Option<ExcludedSecurityType>,
    requested_krw: Money,
}
impl PolicyCandidate {
    pub fn new(
        instrument: &ResolvedOrderInstrument,
        security_type: Option<ExcludedSecurityType>,
        requested_krw: Money,
    ) -> Result<Self, PolicyViolation> {
        if requested_krw.currency() != Currency::Krw
            || requested_krw.amount().as_decimal() <= rust_decimal::Decimal::ZERO
        {
            return Err(PolicyViolation::InvalidCandidate);
        }
        Ok(Self {
            instrument: AllocationInstrument::new(instrument),
            security_type,
            requested_krw,
        })
    }
    #[must_use]
    pub const fn instrument(&self) -> &AllocationInstrument {
        &self.instrument
    }
    #[must_use]
    pub const fn security_type(&self) -> Option<ExcludedSecurityType> {
        self.security_type
    }
    #[must_use]
    pub const fn requested_krw(&self) -> Money {
        self.requested_krw
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Policy {
    profile: InvestorProfile,
    unresolved: Option<PolicyPrinciple>,
}
impl Policy {
    #[must_use]
    pub const fn new(profile: InvestorProfile) -> Self {
        Self {
            profile,
            unresolved: None,
        }
    }
    #[must_use]
    pub fn with_unresolved_principle(mut self, principle: PolicyPrinciple) -> Self {
        self.unresolved = Some(principle);
        self
    }
    #[must_use]
    pub const fn profile(&self) -> &InvestorProfile {
        &self.profile
    }
    #[must_use]
    pub const fn unresolved_principle(&self) -> Option<PolicyPrinciple> {
        self.unresolved
    }
    #[must_use]
    pub fn check_sell() -> PolicySellDecision {
        PolicySellDecision::Allowed
    }

    pub fn check_current(
        &self,
        valuation: &PortfolioValuation,
        assignments: &SectorAssignments,
        security_types: &BTreeMap<AllocationInstrument, ExcludedSecurityType>,
    ) -> Result<Vec<PolicyViolation>, PolicyViolation> {
        self.require_resolved()?;
        let allocation = Allocation::new(valuation, assignments);
        let (instruments, sectors) = asset_axes(&allocation)?;
        let caps = self.profile.weight_caps();
        let mut violations = Vec::new();
        for row in instruments.entries() {
            let AssetInstrument::Holding(instrument) = row.key() else {
                continue;
            };
            let security_type = security_types
                .get(instrument)
                .copied()
                .ok_or(PolicyViolation::UnknownSecurityType)?;
            record_exclusion(
                self.exclusion_check(instrument, Some(security_type), assignments),
                &mut violations,
            )?;
            if row.weight().cmp_profile_weight(&caps.instrument_cap()) == Ordering::Greater {
                violations.push(PolicyViolation::InstrumentCap(instrument.clone()));
            } else if row.weight().cmp_profile_weight(&caps.instrument_warning()) != Ordering::Less
            {
                violations.push(PolicyViolation::InstrumentWarning(instrument.clone()));
            }
        }
        for row in sectors.entries() {
            if let AllocationSector::Classified(sector) = row.key()
                && row.weight().cmp_profile_weight(&caps.sector_cap()) == Ordering::Greater
            {
                violations.push(PolicyViolation::SectorCap(sector.clone()));
            }
        }
        Ok(violations)
    }
    #[must_use]
    pub fn check_buy(
        &self,
        valuation: &PortfolioValuation,
        assignments: &SectorAssignments,
        candidate: &PolicyCandidate,
    ) -> PolicyBuyDecision {
        match self.buy_limit(valuation, assignments, candidate) {
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
        match self.unresolved {
            Some(principle) => Err(PolicyViolation::UnresolvedPrinciple(principle)),
            None => Ok(()),
        }
    }
    fn exclusion_check(
        &self,
        instrument: &AllocationInstrument,
        security_type: Option<ExcludedSecurityType>,
        assignments: &SectorAssignments,
    ) -> Result<SectorId, PolicyViolation> {
        let resolved_type = security_type.ok_or(PolicyViolation::UnknownSecurityType)?;
        if self
            .profile
            .exclusions()
            .security_type_decision(resolved_type)
            == ExclusionDecision::Excluded
        {
            return Err(PolicyViolation::ExcludedType(resolved_type));
        }
        let Some(InstrumentClassification::Sector(sector)) = assignments.classification(instrument)
        else {
            return Err(PolicyViolation::UnknownClassification);
        };
        if self.profile.exclusions().sector_decision(sector) == ExclusionDecision::Excluded {
            return Err(PolicyViolation::ExcludedSector(sector.clone()));
        }
        Ok(sector.clone())
    }
    fn buy_limit(
        &self,
        valuation: &PortfolioValuation,
        assignments: &SectorAssignments,
        candidate: &PolicyCandidate,
    ) -> Result<PolicyBuyLimit, PolicyViolation> {
        self.require_resolved()?;
        let (total, rates, _) = match valuation.assets() {
            Total::Complete(value) => value.value().clone().into_parts(),
            Total::Incomplete(value) => value.subtotal().clone().into_parts(),
        };
        let sector =
            self.exclusion_check(&candidate.instrument, candidate.security_type, assignments)?;
        let values = crate::allocation::policy_krw_holding_values(valuation, &rates)
            .map_err(PolicyViolation::Allocation)
            .and_then(|rows| exposure_values(rows, assignments, &candidate.instrument, &sector))?;
        let allocation = Allocation::new(valuation, assignments);
        let (instruments, sectors) = asset_axes(&allocation)?;
        require_classified_sectors(&sectors)?;
        let caps = self.profile.weight_caps();
        reject_at_cap(
            &instruments,
            &AssetInstrument::Holding(candidate.instrument.clone()),
            caps.instrument_cap(),
            PolicyViolation::InstrumentCap(candidate.instrument.clone()),
        )?;
        reject_at_cap(
            &sectors,
            &AllocationSector::Classified(sector.clone()),
            caps.sector_cap(),
            PolicyViolation::SectorCap(sector.clone()),
        )?;
        limit_from_values(total, values, caps, &candidate.instrument, &sector)
    }
}
fn limit_from_values(
    total: Money,
    values: (Money, Money),
    caps: crate::WeightCaps,
    instrument: &AllocationInstrument,
    sector: &SectorId,
) -> Result<PolicyBuyLimit, PolicyViolation> {
    let instrument_room = room(
        total,
        values.0,
        caps.instrument_cap(),
        PolicyViolation::InstrumentCap(instrument.clone()),
    )?;
    room(
        total,
        values.1,
        caps.sector_cap(),
        PolicyViolation::SectorCap(sector.clone()),
    )
    .map(|sector_room| minimum_room(instrument_room, sector_room))
}
fn minimum_room(instrument: Money, sector: Money) -> PolicyBuyLimit {
    PolicyBuyLimit(
        if instrument.amount().as_decimal() <= sector.amount().as_decimal() {
            instrument
        } else {
            sector
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
fn record_exclusion(
    result: Result<SectorId, PolicyViolation>,
    violations: &mut Vec<PolicyViolation>,
) -> Result<(), PolicyViolation> {
    match result {
        Ok(_) => Ok(()),
        Err(
            violation @ (PolicyViolation::ExcludedSector(_) | PolicyViolation::ExcludedType(_)),
        ) => {
            violations.push(violation);
            Ok(())
        }
        Err(
            violation @ (PolicyViolation::InstrumentCap(_)
            | PolicyViolation::InstrumentWarning(_)
            | PolicyViolation::SectorCap(_)
            | PolicyViolation::UnknownClassification
            | PolicyViolation::UnknownSecurityType
            | PolicyViolation::UnresolvedPrinciple(_)
            | PolicyViolation::Allocation(_)
            | PolicyViolation::Arithmetic(_)
            | PolicyViolation::InvalidCandidate),
        ) => Err(violation),
    }
}
fn require_classified_sectors(
    sectors: &AssetAllocation<AllocationSector>,
) -> Result<(), PolicyViolation> {
    for row in sectors.entries() {
        match row.key() {
            AllocationSector::Classified(_) | AllocationSector::Cash => {}
            AllocationSector::EtfFund | AllocationSector::Unclassified => {
                return Err(PolicyViolation::UnknownClassification);
            }
        }
    }
    Ok(())
}
fn exposure_values(
    rows: Vec<(AllocationInstrument, Money)>,
    assignments: &SectorAssignments,
    candidate: &AllocationInstrument,
    sector: &SectorId,
) -> Result<(Money, Money), PolicyViolation> {
    let mut instrument_value = crate::holding::zero(Currency::Krw);
    let mut sector_value = crate::holding::zero(Currency::Krw);
    for (instrument, value) in rows {
        if &instrument == candidate {
            instrument_value = value;
        }
        if assignments.sector(&instrument) == AllocationSector::Classified(sector.clone()) {
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
    fn assert_block(decision: &PolicyBuyDecision) {
        assert_eq!(
            std::mem::discriminant(decision),
            std::mem::discriminant(&PolicyBuyDecision::Block(PolicyViolation::InvalidCandidate))
        );
    }

    use crate::{
        AcquisitionPrice, AllocationInstrument, BuyingPower, Currency, ExcludedSecurityType,
        Exclusions, Holding, HoldingObservation, InstrumentClassification, InvestorProfile, Market,
        Money, Nanosecond, NumericInput, OffsetSeconds, Policy, PolicyBuyDecision, PolicyCandidate,
        PolicyCostFit, PolicyPrinciple, PolicyViolation, Portfolio, PortfolioValuation, Quantity,
        Ratio, ReportedHolding, ResolvedOrderInstrument, SectorAssignment, SectorAssignments,
        SectorId, Symbol, Timestamp, UnixSeconds,
    };
    use proptest::test_runner::{Config, TestRunner};
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
        PolicyCandidate::new(
            &instrument(symbol),
            Some(ExcludedSecurityType::Stock),
            money(amount),
        )
        .expect("후보")
    }
    fn policy() -> Policy {
        Policy::new(InvestorProfile::default())
    }
    fn known_types() -> BTreeMap<AllocationInstrument, ExcludedSecurityType> {
        ["A", "B"]
            .map(|value| {
                (
                    AllocationInstrument::new(&instrument(value)),
                    ExcludedSecurityType::Stock,
                )
            })
            .into_iter()
            .collect()
    }
    #[test]
    fn buy_pass_reduce_and_block_at_both_caps() {
        let initial = valuation(
            vec![holding("A", Some("10")), holding("B", Some("25"))],
            "65",
        );
        assert_eq!(
            policy().check_buy(&initial, &assignments(), &candidate("A", "5")),
            PolicyBuyDecision::Pass
        );
        let limit = super::PolicyBuyLimit(money("5"));
        assert_eq!(
            policy().check_buy(&initial, &assignments(), &candidate("A", "10")),
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
            policy().check_buy(&at_instrument, &assignments(), &candidate("A", "1")),
            PolicyBuyDecision::Block(PolicyViolation::InstrumentCap(AllocationInstrument::new(
                &instrument("A")
            )))
        );
        let at_sector = valuation(
            vec![holding("A", Some("19")), holding("B", Some("21"))],
            "60",
        );
        assert_eq!(
            policy().check_buy(&at_sector, &assignments(), &candidate("A", "1")),
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
                let actual = policy().check_buy(&first, &assignments(), &candidate("A", "100"));
                assert_eq!(
                    actual,
                    policy().check_buy(&second, &assignments(), &candidate("A", "100"))
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
    fn unknown_inputs_never_pass_and_exclusions_use_profile_decisions() {
        let complete = valuation(vec![holding("A", Some("1"))], "99");
        for values in [
            valuation(vec![holding("A", None)], "99"),
            valuation(vec![holding("A", Some("1"))], "-1"),
        ] {
            assert_block(&policy().check_buy(&values, &assignments(), &candidate("A", "1")));
            policy()
                .check_current(&values, &assignments(), &known_types())
                .expect_err("판정 불가");
        }
        for classification in [
            InstrumentClassification::Unclassified,
            InstrumentClassification::EtfFund,
        ] {
            let mapping = SectorAssignments::try_new(vec![SectorAssignment::new(
                &instrument("A"),
                classification,
            )])
            .expect("분류");
            assert_block(&policy().check_buy(&complete, &mapping, &candidate("A", "1")));
        }
        assert_block(&policy().check_buy(
            &complete,
            &SectorAssignments::default(),
            &candidate("A", "1"),
        ));
        let unknown = PolicyCandidate::new(&instrument("A"), None, money("1")).expect("후보");
        assert_block(&policy().check_buy(&complete, &assignments(), &unknown));
    }
    #[test]
    fn exclusions_and_unresolved_principles_block() {
        let complete = valuation(vec![holding("A", Some("1"))], "99");
        for exclusions in [
            Exclusions::try_new(vec![sector()], vec![]).expect("제외"),
            Exclusions::try_new(vec![], vec![ExcludedSecurityType::Stock]).expect("제외"),
        ] {
            let configured = Policy::new(InvestorProfile::default().with_exclusions(exclusions));
            assert_block(&configured.check_buy(&complete, &assignments(), &candidate("A", "1")));
            assert_ne!(
                configured
                    .check_current(&complete, &assignments(), &known_types())
                    .expect("완전"),
                vec![]
            );
        }
        for principle in [
            PolicyPrinciple::FreeText,
            PolicyPrinciple::MinimumCash,
            PolicyPrinciple::ConcentrationWarning,
        ] {
            let configured = policy().with_unresolved_principle(principle);
            assert_block(&configured.check_buy(&complete, &assignments(), &candidate("A", "1")));
            configured
                .check_current(&complete, &assignments(), &known_types())
                .expect_err("판정 불가");
        }
    }
    #[test]
    fn public_boundaries_accessors_and_unknown_current_metadata() {
        let instance = policy();
        assert_eq!(instance.profile(), &InvestorProfile::default());
        assert_eq!(instance.unresolved_principle(), None);
        assert_eq!(
            instance
                .with_unresolved_principle(PolicyPrinciple::FreeText)
                .unresolved_principle(),
            Some(PolicyPrinciple::FreeText)
        );
        let item = candidate("NEW", "1");
        assert_eq!(
            item.instrument(),
            &AllocationInstrument::new(&instrument("NEW"))
        );
        assert_eq!(item.security_type(), Some(ExcludedSecurityType::Stock));
        assert_eq!(item.requested_krw(), money("1"));
        for cost in [
            money("0"),
            money("-1"),
            Money::new(money("1").amount(), Currency::Usd),
        ] {
            assert_eq!(
                PolicyCandidate::new(&instrument("A"), Some(ExcludedSecurityType::Stock), cost),
                Err(PolicyViolation::InvalidCandidate)
            );
        }
    }
    #[test]
    fn missing_current_metadata_and_cost_fit_boundaries() {
        let item = candidate("NEW", "1");
        let values = valuation(vec![holding("A", Some("1"))], "99");
        assert_eq!(
            policy().check_current(&values, &assignments(), &BTreeMap::new()),
            Err(PolicyViolation::UnknownSecurityType)
        );
        assert_eq!(
            policy().check_current(&values, &SectorAssignments::default(), &known_types()),
            Err(PolicyViolation::UnknownClassification)
        );
        assert_eq!(
            policy().check_current(&values, &assignments(), &known_types()),
            Ok(vec![])
        );
        assert_eq!(
            policy().check_buy(&values, &assignments(), &item),
            PolicyBuyDecision::Pass
        );
        let limit = super::PolicyBuyLimit(money("19"));
        assert_eq!(
            policy().check_buy(&values, &assignments(), &candidate("A", "100")),
            PolicyBuyDecision::Reduce(limit)
        );
        assert_eq!(
            limit.compare_krw_cost(money("-1")),
            PolicyCostFit::InvalidCost
        );
        assert_eq!(
            limit.compare_krw_cost(Money::new(money("1").amount(), Currency::Usd)),
            PolicyCostFit::InvalidCost
        );
        assert_eq!(
            limit.compare_krw_cost(money("0")),
            PolicyCostFit::WithinLimit
        );
        let empty = valuation(vec![], "0");
        assert_block(&policy().check_buy(&empty, &assignments(), &item));
    }
    #[test]
    fn generated_missing_information_and_negative_cash_never_pass() {
        TestRunner::new(Config::with_cases(512))
            .run(&(1_u32..10000, 0_u8..4), |(amount, missing)| {
                let values = valuation(
                    vec![holding("A", if missing == 0 { None } else { Some("1") })],
                    &if missing == 1 {
                        format!("-{amount}")
                    } else {
                        amount.to_string()
                    },
                );
                let mapping = if missing == 2 {
                    SectorAssignments::default()
                } else {
                    assignments()
                };
                let item = PolicyCandidate::new(
                    &instrument("A"),
                    if missing == 3 {
                        None
                    } else {
                        Some(ExcludedSecurityType::Stock)
                    },
                    money("1"),
                )
                .expect("후보");
                assert_block(&policy().check_buy(&values, &mapping, &item));
                Ok(())
            })
            .expect("판정 불가 속성");
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
            policy().check_buy(&tiny, &assignments(), &candidate("A", "1")),
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
                &sector()
            ),
            Err(PolicyViolation::Arithmetic(crate::MoneyError::Overflow))
        );
    }
    #[test]
    fn unknown_other_holding_and_separate_sector_cannot_hide_exposure() {
        let values = valuation(vec![holding("A", Some("1")), holding("B", Some("1"))], "98");
        let missing = SectorAssignments::try_new(vec![SectorAssignment::new(
            &instrument("A"),
            InstrumentClassification::Sector(sector()),
        )])
        .expect("분류");
        assert_eq!(
            policy().check_buy(&values, &missing, &candidate("A", "1")),
            PolicyBuyDecision::Block(PolicyViolation::UnknownClassification)
        );
        let split = SectorAssignments::try_new(vec![
            SectorAssignment::new(&instrument("A"), InstrumentClassification::Sector(sector())),
            SectorAssignment::new(
                &instrument("B"),
                InstrumentClassification::Sector(SectorId::try_new("Energy").expect("섹터")),
            ),
        ])
        .expect("분류");
        assert_eq!(
            policy().check_buy(&values, &split, &candidate("A", "1")),
            PolicyBuyDecision::Pass
        );
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
            let item = PolicyCandidate::new(&instrument("A"), Some(security_type), money("1"))
                .expect("후보");
            assert_eq!(
                configured.check_buy(&values, &assignments(), &item),
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
            policy().check_buy(&values, &assignments(), &candidate("A", "1")),
            PolicyBuyDecision::Block(PolicyViolation::InstrumentCap(AllocationInstrument::new(
                &instrument("A")
            )))
        );
        assert_eq!(
            policy().check_buy(&values, &assignments(), &candidate("NEW", "1")),
            PolicyBuyDecision::Block(PolicyViolation::SectorCap(sector()))
        );
        assert_eq!(Policy::check_sell(), crate::PolicySellDecision::Allowed);
        let cash_only = valuation(vec![], "100");
        assert_eq!(
            policy().check_buy(&cash_only, &assignments(), &candidate("NEW", "20")),
            PolicyBuyDecision::Pass
        );
        assert_eq!(
            policy().check_current(&cash_only, &assignments(), &BTreeMap::new()),
            Ok(vec![])
        );
    }
}
