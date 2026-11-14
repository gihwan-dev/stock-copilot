use crate::{
    AllocationInstrument, CalibratedProbability, CollectedItem, CostEligibleTask, Disclosure,
    EstimateTask, Evidence, EvidenceRef, ExclusionDecision, InformationSource,
    InstrumentClassification, Margin, Money, NoThresholds, Policy, PolicyBuyDecision,
    PolicyBuyLimit, PolicyCandidate, PolicySecurityFacts, PortfolioValuation, Quantity,
    SectorAssignments, SignalCondition, ThresholdDisplay, ThresholdKind, Thresholds, ToolReference,
    Total,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignalOperation {
    Conservative,
    EligibleBeforeE91,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignalUnavailable {
    UnknownClassification,
    HoldingsQueryFailed,
    IncompleteHoldings,
    Thresholds(NoThresholds),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoSignalReason {
    ConditionsUnmet,
    UnheldLowProbability,
    Excluded,
    MissingSource,
    PolicyViolation,
    Unavailable(SignalUnavailable),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignalEstimate {
    task: EstimateTask,
    thresholds: Thresholds,
    calibration: CalibratedProbability,
    evidence: Option<Evidence>,
}
impl SignalEstimate {
    pub fn create(
        eligible: &CostEligibleTask,
        calibration: &CalibratedProbability,
        margin: Margin,
        evidence: Option<Evidence>,
    ) -> Result<Self, NoThresholds> {
        Ok(Self {
            task: eligible.task().clone(),
            thresholds: Thresholds::create(eligible, calibration, margin)?,
            calibration: calibration.clone(),
            evidence,
        })
    }
    #[must_use]
    pub const fn task(&self) -> &EstimateTask {
        &self.task
    }
    #[must_use]
    pub const fn thresholds(&self) -> &Thresholds {
        &self.thresholds
    }
    #[must_use]
    pub const fn calibration(&self) -> &CalibratedProbability {
        &self.calibration
    }
    #[must_use]
    pub const fn evidence(&self) -> Option<&Evidence> {
        self.evidence.as_ref()
    }
    #[must_use]
    pub const fn operation(&self) -> SignalOperation {
        match &self.calibration {
            CalibratedProbability::InsufficientSamples(_) => SignalOperation::Conservative,
            CalibratedProbability::EligibleBeforeE91(_) => SignalOperation::EligibleBeforeE91,
        }
    }
    pub fn display_probability(&self) -> Result<ThresholdDisplay, NoThresholds> {
        let probability = match &self.calibration {
            CalibratedProbability::InsufficientSamples(value) => value.evidence().probability(),
            CalibratedProbability::EligibleBeforeE91(value) => value.evidence().probability(),
        };
        crate::thresholds_arithmetic::ThresholdNumber::new(probability.as_decimal())
            .percent(ThresholdKind::Buy)
    }
}

#[derive(Debug)]
pub struct SignalSourceLedger<'facts> {
    items: &'facts [CollectedItem],
    disclosures: &'facts [Disclosure],
    tools: &'facts [ToolReference],
}
impl<'facts> SignalSourceLedger<'facts> {
    #[must_use]
    pub const fn new(
        items: &'facts [CollectedItem],
        disclosures: &'facts [Disclosure],
        tools: &'facts [ToolReference],
    ) -> Self {
        Self {
            items,
            disclosures,
            tools,
        }
    }
    fn check(&self, evidence: Option<&Evidence>) -> Result<(), NoSignalReason> {
        let verified = evidence.ok_or(NoSignalReason::MissingSource)?;
        let mut substantive = None;
        for reference in verified.as_items() {
            let present = match reference {
                EvidenceRef::Web(url) => self.items.iter().any(|item| {
                    item.source() == &InformationSource::Web && item.url() == url.as_url()
                }),
                EvidenceRef::News(source, url) => self.items.iter().any(|item| {
                    item.source() == &InformationSource::News(source.clone())
                        && item.url() == url.as_url()
                }),
                EvidenceRef::Disclosure(key, url) => self
                    .disclosures
                    .iter()
                    .any(|item| item.key() == key && item.collected().url() == url.as_url()),
                EvidenceRef::Tool(tool) => self.tools.contains(tool),
            };
            if !present {
                return Err(NoSignalReason::MissingSource);
            }
            match reference {
                EvidenceRef::Tool(_) => {}
                EvidenceRef::Web(_) | EvidenceRef::News(_, _) | EvidenceRef::Disclosure(_, _) => {
                    substantive = Some(());
                }
            }
        }
        substantive.ok_or(NoSignalReason::MissingSource)
    }
}

#[derive(Debug)]
pub struct SignalContext<'facts> {
    valuation: Option<&'facts PortfolioValuation>,
    policy: &'facts Policy,
    assignments: &'facts SectorAssignments,
    security: &'facts PolicySecurityFacts,
    sources: &'facts SignalSourceLedger<'facts>,
    requested_krw: Money,
}
impl<'facts> SignalContext<'facts> {
    #[must_use]
    pub const fn new(
        valuation: Option<&'facts PortfolioValuation>,
        policy: &'facts Policy,
        classification: (&'facts SectorAssignments, &'facts PolicySecurityFacts),
        sources: &'facts SignalSourceLedger<'facts>,
        requested_krw: Money,
    ) -> Self {
        Self {
            valuation,
            policy,
            assignments: classification.0,
            security: classification.1,
            sources,
            requested_krw,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignalBuyAdjustment {
    Unchanged,
    Reduce(PolicyBuyLimit),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuySignal {
    facts: SignalEstimate,
    adjustment: SignalBuyAdjustment,
}
impl BuySignal {
    #[must_use]
    pub const fn facts(&self) -> &SignalEstimate {
        &self.facts
    }
    #[must_use]
    pub const fn adjustment(&self) -> SignalBuyAdjustment {
        self.adjustment
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SellSignal {
    facts: SignalEstimate,
    held_quantity: Quantity,
}
impl SellSignal {
    #[must_use]
    pub const fn facts(&self) -> &SignalEstimate {
        &self.facts
    }
    #[must_use]
    pub const fn held_quantity(&self) -> Quantity {
        self.held_quantity
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RaisedSignal {
    Buy(BuySignal),
    Sell(SellSignal),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SignalDecision {
    Buy(BuySignal),
    Sell(SellSignal),
    None(NoSignalReason),
}
impl SignalDecision {
    #[must_use]
    pub fn assess(
        estimate: &SignalEstimate,
        calibration: &CalibratedProbability,
        context: &SignalContext<'_>,
    ) -> Self {
        match assess(estimate, calibration, context) {
            Ok(signal) => match signal {
                RaisedSignal::Buy(value) => Self::Buy(value),
                RaisedSignal::Sell(value) => Self::Sell(value),
            },
            Err(reason) => Self::None(reason),
        }
    }
    #[must_use]
    pub fn into_signal(self) -> Option<RaisedSignal> {
        match self {
            Self::Buy(value) => Some(RaisedSignal::Buy(value)),
            Self::Sell(value) => Some(RaisedSignal::Sell(value)),
            Self::None(_) => None,
        }
    }
}
fn exclusions(
    estimate: &SignalEstimate,
    context: &SignalContext<'_>,
) -> Result<(), NoSignalReason> {
    let instrument = AllocationInstrument::new(estimate.task.reference().instrument());
    let profile = context.policy.profile();
    let kind = context.security.security_type(&instrument);
    let classification = context.assignments.classification(&instrument);
    if kind.is_some_and(|value| {
        profile.exclusions().security_type_decision(value) == ExclusionDecision::Excluded
    }) || classification.is_some_and(|value| match value {
        InstrumentClassification::Sector(sector) => {
            profile.exclusions().sector_decision(sector) == ExclusionDecision::Excluded
        }
        InstrumentClassification::EtfFund | InstrumentClassification::Unclassified => false,
    }) {
        return Err(NoSignalReason::Excluded);
    }
    match (kind, classification) {
        (
            Some(_),
            Some(InstrumentClassification::Sector(_) | InstrumentClassification::EtfFund),
        ) => Ok(()),
        (None, _) | (Some(_), None | Some(InstrumentClassification::Unclassified)) => Err(
            NoSignalReason::Unavailable(SignalUnavailable::UnknownClassification),
        ),
    }
}
fn assess(
    estimate: &SignalEstimate,
    calibration: &CalibratedProbability,
    context: &SignalContext<'_>,
) -> Result<RaisedSignal, NoSignalReason> {
    exclusions(estimate, context)?;
    context.sources.check(estimate.evidence())?;
    let buy_result = estimate.thresholds.buy_condition(calibration);
    let sell_result = estimate.thresholds.sell_condition(calibration);
    let (buy, sell) = buy_result
        .and_then(|buy_value| sell_result.map(|sell_value| (buy_value, sell_value)))
        .map_err(|error| NoSignalReason::Unavailable(SignalUnavailable::Thresholds(error)))?;
    let valuation = context.valuation.ok_or(NoSignalReason::Unavailable(
        SignalUnavailable::HoldingsQueryFailed,
    ))?;
    if let Total::Incomplete(_) = valuation.valuation() {
        return Err(NoSignalReason::Unavailable(
            SignalUnavailable::IncompleteHoldings,
        ));
    }
    if buy == SignalCondition::Met {
        return buy_signal(estimate, context, valuation);
    }
    if sell == SignalCondition::Met {
        let holding = valuation
            .holdings()
            .iter()
            .find(|row| row.instrument() == estimate.task.reference().instrument())
            .ok_or(NoSignalReason::UnheldLowProbability)?;
        let quantity = holding.holding().quantity();
        if quantity.as_decimal() <= rust_decimal::Decimal::ZERO {
            return Err(NoSignalReason::UnheldLowProbability);
        }
        return Ok(RaisedSignal::Sell(SellSignal {
            facts: estimate.clone(),
            held_quantity: quantity,
        }));
    }
    Err(NoSignalReason::ConditionsUnmet)
}
fn buy_signal(
    estimate: &SignalEstimate,
    context: &SignalContext<'_>,
    valuation: &PortfolioValuation,
) -> Result<RaisedSignal, NoSignalReason> {
    let candidate = PolicyCandidate::new(
        estimate.task.reference().instrument(),
        context.requested_krw,
    )
    .map_err(|_error| NoSignalReason::PolicyViolation)?;
    let adjustment =
        match context
            .policy
            .check_buy(valuation, context.assignments, context.security, &candidate)
        {
            PolicyBuyDecision::Pass => SignalBuyAdjustment::Unchanged,
            PolicyBuyDecision::Reduce(limit) => SignalBuyAdjustment::Reduce(limit),
            PolicyBuyDecision::Block(_) => return Err(NoSignalReason::PolicyViolation),
        };
    Ok(RaisedSignal::Buy(BuySignal {
        facts: estimate.clone(),
        adjustment,
    }))
}

#[cfg(test)]
mod tests {
    use crate::{
        AcquisitionPrice, AiProbability, AiProbabilityInput, AllocationInstrument, Amount,
        BuyingPower, CalibratedProbability, Calibration, CalibrationEstimate,
        CalibrationEstimateId, CalibrationFact, CalibrationKey, CalibrationModel,
        CalibrationOutcome, CollectedItem, CommissionRateFact, CommissionValidity,
        CostEligibleTask, CostRate, Currency, DartReceiptNo, Disclosure, Evidence, EvidenceRef,
        EvidenceUrl, ExcludedSecurityType, Exclusions, Holding, HoldingObservation, Horizon,
        HorizonSelection, HoursPermission, InstrumentClassification, InstrumentId, InvestorProfile,
        Margin, Market, ModelIdentityPart, Money, NewsItem, NewsSource, NoSignalReason,
        NoThresholds, NormalizedUrl, NumericInput, Policy, PolicyCostFit, PolicyPrinciple,
        PolicySecurityFacts, PolicyText, Portfolio, PortfolioValuation, Quantity, RaisedSignal,
        Ratio, ReceiptNo, ReportedHolding, ResolvedOrderInstrument, RiskAppetite, SectorAssignment,
        SectorAssignments, SectorId, SignalBuyAdjustment, SignalCondition, SignalContext,
        SignalDecision, SignalEstimate, SignalOperation, SignalSourceLedger, SignalUnavailable,
        Symbol, TimeInterval, ToolReference, TotalCompleteness, TradeCost, TradeTaxFact,
        UntrustedText, WeightCaps, candle, thresholds,
    };
    use proptest::test_runner::{Config, TestRunner};
    use rust_decimal::Decimal;

    fn money(text: &str) -> Money {
        Money::parse(&NumericInput::try_new(text).expect("입력"), Currency::Krw).expect("금액")
    }
    fn probability(percent: i64, count: usize) -> CalibratedProbability {
        let part = ModelIdentityPart::try_new("d24").expect("모델");
        let key = CalibrationKey::new(
            CalibrationModel::new(part.clone(), part.clone(), part),
            Horizon::default(),
        );
        let predicted =
            AiProbability::parse(&AiProbabilityInput::try_new(&percent.to_string()).expect("입력"))
                .expect("확률");
        let facts = (0..count)
            .map(|index| {
                CalibrationFact::new(
                    CalibrationEstimate::new(
                        CalibrationEstimateId::try_new(&format!("e{index}")).expect("ID"),
                        InstrumentId::try_new(&format!("i{index}")).expect("종목"),
                        key.clone(),
                        TimeInterval::try_new(candle::tests::at(1), candle::tests::at(2))
                            .expect("구간"),
                        predicted,
                    ),
                    CalibrationOutcome::Hit,
                )
            })
            .collect::<Vec<_>>();
        Calibration::try_new(key.clone(), &facts)
            .expect("집계")
            .calibrate(&key, predicted)
    }
    fn eligible() -> CostEligibleTask {
        let zero = CostRate::try_new(Ratio::new(Decimal::ZERO)).expect("비용");
        let cost = TradeCost::from_facts(
            Some(CommissionRateFact::from_commissions_result(
                Market::Kr,
                zero,
                CommissionValidity::try_new(None, None).expect("기간"),
            )),
            Some(TradeTaxFact::from_schedule_result(Market::Kr, zero, zero)),
            thresholds::tests::cost_period(),
        )
        .expect("비용");
        CostEligibleTask::assess(thresholds::tests::task(), &cost).expect("적격")
    }
    fn margin(percent: i64) -> Margin {
        Margin::try_new(Ratio::new(Decimal::new(percent, 2))).expect("여유")
    }
    fn item() -> CollectedItem {
        CollectedItem::from_web(
            NormalizedUrl::try_new("https://example.com/a").expect("URL"),
            UntrustedText::try_new("근거").expect("제목"),
            None,
        )
    }
    fn evidence(mode: u8) -> Option<Evidence> {
        match mode {
            0 => None,
            1 => Some(
                Evidence::try_new(vec![EvidenceRef::Web(
                    EvidenceUrl::try_new(item().url().clone()).expect("URL"),
                )])
                .expect("근거"),
            ),
            2 => Some(
                Evidence::try_new(vec![EvidenceRef::Web(
                    EvidenceUrl::try_new(
                        NormalizedUrl::try_new("https://example.com/missing").expect("URL"),
                    )
                    .expect("URL"),
                )])
                .expect("근거"),
            ),
            _ => Some(
                Evidence::try_new(vec![EvidenceRef::Tool(
                    ToolReference::try_new("tool:quote/1").expect("도구"),
                )])
                .expect("근거"),
            ),
        }
    }
    fn instrument() -> ResolvedOrderInstrument {
        thresholds::tests::task().reference().instrument().clone()
    }
    fn valuation(quantity: Option<&str>, complete: TotalCompleteness) -> PortfolioValuation {
        let rows = quantity
            .map(|value| {
                Holding::new(
                    instrument(),
                    Quantity::parse(&NumericInput::try_new(value).expect("입력")).expect("수량"),
                    AcquisitionPrice::try_new(money("1")).expect("원가"),
                    HoldingObservation::new(
                        None,
                        (complete == TotalCompleteness::Complete).then(|| {
                            ReportedHolding::new(money("10"), money("0"), Ratio::new(Decimal::ZERO))
                                .expect("평가")
                        }),
                        candle::tests::at(61),
                    ),
                )
                .expect("보유")
            })
            .into_iter()
            .collect();
        Portfolio::new(
            rows,
            BuyingPower::new(money("100"), candle::tests::at(61)),
            BuyingPower::new(
                Money::new(Amount::new(Decimal::ZERO), Currency::Usd),
                candle::tests::at(61),
            ),
        )
        .expect("포트")
        .evaluate(None)
    }
    fn assignments() -> SectorAssignments {
        SectorAssignments::try_new(vec![SectorAssignment::new(
            &instrument(),
            InstrumentClassification::Sector(SectorId::try_new("Tech").expect("섹터")),
        )])
        .expect("분류")
    }
    fn facts() -> PolicySecurityFacts {
        PolicySecurityFacts::from_master_lookup_results(
            std::iter::once((
                AllocationInstrument::new(&instrument()),
                ExcludedSecurityType::Stock,
            ))
            .collect(),
        )
    }
    fn policy(exclusion: u8) -> Policy {
        let exclusions = Exclusions::try_new(
            if exclusion == 1 {
                vec![SectorId::try_new("Tech").expect("섹터")]
            } else {
                vec![]
            },
            if exclusion == 2 {
                vec![ExcludedSecurityType::Stock]
            } else {
                vec![]
            },
        )
        .expect("제외");
        Policy::new(
            InvestorProfile::try_new(
                Horizon::default(),
                HoursPermission::default(),
                RiskAppetite::Neutral,
                exclusions,
                WeightCaps::default(),
            )
            .expect("성향"),
        )
    }
    fn classify(decision: &SignalDecision) -> u8 {
        match decision {
            SignalDecision::Buy(_) => 1,
            SignalDecision::Sell(_) => 2,
            SignalDecision::None(_) => 0,
        }
    }

    #[test]
    fn independent_reference_covers_probability_threshold_holding_exclusion_and_sources() {
        let eligible = eligible();
        let base = eligible.task().base_probability().as_decimal();
        let rows = [item()];
        let ledger = SignalSourceLedger::new(&rows, &[], &[]);
        let assignments = assignments();
        let facts = facts();
        TestRunner::new(Config::with_cases(4096))
            .run(
                &(
                    1_i64..=99,
                    5_i64..=25,
                    0_u8..3,
                    0_u8..3,
                    0_u8..4,
                    proptest::bool::ANY,
                ),
                |(percent, buffer, holding, exclusion, source, ready)| {
                    let calibration = probability(percent, if ready { 30 } else { 0 });
                    let estimate = SignalEstimate::create(
                        &eligible,
                        &calibration,
                        margin(buffer),
                        evidence(source),
                    )
                    .expect("추정");
                    let portfolio = valuation(
                        match holding {
                            0 => None,
                            1 => Some("0"),
                            _ => Some("0.25"),
                        },
                        TotalCompleteness::Complete,
                    );
                    let policy = policy(exclusion);
                    let context = SignalContext::new(
                        Some(&portfolio),
                        &policy,
                        (&assignments, &facts),
                        &ledger,
                        money("1"),
                    );
                    let decision = SignalDecision::assess(&estimate, &calibration, &context);
                    let expected = reference(
                        (percent, buffer),
                        base,
                        estimate.operation(),
                        (holding, exclusion, source),
                    );
                    proptest::prop_assert_eq!(classify(&decision), expected);
                    if holding != 2 {
                        proptest::prop_assert_ne!(classify(&decision), 2);
                    }
                    if exclusion != 0 || source != 1 {
                        proptest::prop_assert_eq!(decision.into_signal(), None);
                    }
                    Ok(())
                },
            )
            .expect("독립 기준4096");
    }

    #[test]
    fn unavailable_inputs_never_fall_through_to_signals() {
        let eligible = eligible();
        let rows = [item()];
        let ledger = SignalSourceLedger::new(&rows, &[], &[]);
        let assignments = assignments();
        let facts = facts();
        let policy = policy(0);
        let incomplete = valuation(Some("1"), TotalCompleteness::Incomplete);
        let complete = valuation(Some("1"), TotalCompleteness::Complete);
        TestRunner::new(Config::with_cases(4096))
            .run(&(1_i64..=99), |percent| {
                let calibration = probability(percent, 0);
                let estimate =
                    SignalEstimate::create(&eligible, &calibration, margin(10), evidence(1))
                        .expect("추정");
                for snapshot in [None, Some(&incomplete)] {
                    let context = SignalContext::new(
                        snapshot,
                        &policy,
                        (&assignments, &facts),
                        &ledger,
                        money("1"),
                    );
                    proptest::prop_assert_eq!(
                        SignalDecision::assess(&estimate, &calibration, &context).into_signal(),
                        None
                    );
                }
                let context = SignalContext::new(
                    Some(&complete),
                    &policy,
                    (&assignments, &facts),
                    &ledger,
                    money("1"),
                );
                let different = probability(if percent == 99 { 1 } else { percent + 1 }, 0);
                proptest::prop_assert_eq!(
                    SignalDecision::assess(&estimate, &different, &context),
                    SignalDecision::None(NoSignalReason::Unavailable(
                        SignalUnavailable::Thresholds(NoThresholds::CalibrationMismatch)
                    ))
                );
                Ok(())
            })
            .expect("판정불가4096");
    }

    #[test]
    fn policy_blocks_buys_reduces_budgets_and_never_blocks_owned_sells() {
        let eligible = eligible();
        let rows = [item()];
        let ledger = SignalSourceLedger::new(&rows, &[], &[]);
        let assignments = assignments();
        let facts = facts();
        let portfolio = valuation(Some("0.25"), TotalCompleteness::Complete);
        let plain = policy(0);
        let blocked =
            plain
                .clone()
                .with_unresolved_principle(PolicyPrinciple::from_missing_contract(
                    PolicyText::try_new("미확정").expect("원칙"),
                ));
        for (percent, budget, policy, expected) in [
            (99, "1", &plain, 1),
            (99, "90", &plain, 1),
            (99, "1", &blocked, 0),
            (1, "1", &blocked, 2),
        ] {
            let calibration = probability(percent, 0);
            let estimate = SignalEstimate::create(&eligible, &calibration, margin(10), evidence(1))
                .expect("추정");
            let context = SignalContext::new(
                Some(&portfolio),
                policy,
                (&assignments, &facts),
                &ledger,
                money(budget),
            );
            let decision = SignalDecision::assess(&estimate, &calibration, &context);
            assert_eq!(classify(&decision), expected);
            match decision.clone().into_signal() {
                Some(RaisedSignal::Buy(signal)) => {
                    assert_eq!(signal.facts(), &estimate);
                    match signal.adjustment() {
                        SignalBuyAdjustment::Unchanged => assert_eq!(budget, "1"),
                        SignalBuyAdjustment::Reduce(limit) => assert_eq!(
                            limit.compare_krw_cost(money("90")),
                            PolicyCostFit::AboveLimit
                        ),
                    }
                }
                Some(RaisedSignal::Sell(signal)) => {
                    assert_eq!(signal.facts(), &estimate);
                    assert_eq!(signal.held_quantity().as_decimal(), Decimal::new(25, 2));
                }
                None => assert_eq!(
                    decision,
                    SignalDecision::None(NoSignalReason::PolicyViolation)
                ),
            }
        }
    }

    fn reference(
        values: (i64, i64),
        base: Decimal,
        mode: SignalOperation,
        axes: (u8, u8, u8),
    ) -> u8 {
        let (percent, buffer) = values;
        let (holding, exclusion, source) = axes;
        let (buy_margin, sell_margin) = match mode {
            SignalOperation::Conservative => (buffer.max(15), 15),
            SignalOperation::EligibleBeforeE91 => (buffer, buffer),
        };
        if exclusion != 0 || source != 1 {
            0
        } else if Decimal::new(percent, 2)
            >= base
                .checked_add(Decimal::new(buy_margin, 2))
                .expect("매수 기준")
        {
            1
        } else if Decimal::new(percent, 2)
            <= base
                .checked_sub(Decimal::new(sell_margin, 2))
                .expect("매도 기준")
                .max(Decimal::new(5, 2))
            && holding == 2
        {
            2
        } else {
            0
        }
    }

    #[test]
    fn threshold_conditions_are_disjoint_and_conservative_signals_keep_display_facts() {
        let eligible = eligible();
        TestRunner::new(Config::with_cases(4096))
            .run(&(1_i64..=99, 5_i64..=25), |(percent, buffer)| {
                let calibration = probability(percent, 0);
                let estimate =
                    SignalEstimate::create(&eligible, &calibration, margin(buffer), evidence(1))
                        .expect("추정");
                proptest::prop_assert!(
                    !(estimate
                        .thresholds()
                        .buy_condition(&calibration)
                        .expect("비교")
                        == SignalCondition::Met
                        && estimate
                            .thresholds()
                            .sell_condition(&calibration)
                            .expect("비교")
                            == SignalCondition::Met)
                );
                proptest::prop_assert_eq!(estimate.operation(), SignalOperation::Conservative);
                proptest::prop_assert_eq!(estimate.task(), eligible.task());
                proptest::prop_assert_eq!(estimate.calibration(), &calibration);
                proptest::prop_assert_eq!(
                    estimate
                        .display_probability()
                        .expect("표시")
                        .as_str()
                        .to_owned(),
                    format!("≈{percent}.0%")
                );
                Ok(())
            })
            .expect("배타성4096");
        let calibration = probability(99, 30);
        let estimate =
            SignalEstimate::create(&eligible, &calibration, margin(10), None).expect("추정");
        assert_eq!(estimate.operation(), SignalOperation::EligibleBeforeE91);
        assert_eq!(
            estimate.display_probability().expect("표시").as_str(),
            "≈99.0%"
        );
        assert!(estimate.task().target().to_string().starts_with('≈'));
        assert!(estimate.task().stop().to_string().starts_with('≈'));
        assert_eq!(
            estimate.task().horizon().as_ratio().as_decimal(),
            Decimal::from(40_i32)
        );
    }

    fn decide_sources(evidence: Evidence, ledger: &SignalSourceLedger<'_>) -> SignalDecision {
        let calibration = probability(99, 0);
        let estimate =
            SignalEstimate::create(&eligible(), &calibration, margin(10), Some(evidence))
                .expect("추정");
        let portfolio = valuation(None, TotalCompleteness::Complete);
        let assignments = assignments();
        let facts = facts();
        let policy = policy(0);
        let context = SignalContext::new(
            Some(&portfolio),
            &policy,
            (&assignments, &facts),
            ledger,
            money("1"),
        );
        SignalDecision::assess(&estimate, &calibration, &context)
    }

    #[test]
    fn source_ledger_matches_kinds_urls_receipts_and_tools_without_counting_tools_as_sources() {
        let web = item();
        let news = NewsItem::new(
            NewsSource::Hankyung,
            NormalizedUrl::try_new("https://example.com/news").expect("URL"),
            UntrustedText::try_new("뉴스").expect("제목"),
            None,
        );
        let disclosure = Disclosure::new(
            ReceiptNo::Dart(DartReceiptNo::try_new("20261114000001").expect("번호")),
            NormalizedUrl::try_new("https://example.com/disclosure").expect("URL"),
            UntrustedText::try_new("공시").expect("제목"),
            None,
        );
        let tool = ToolReference::try_new("tool:quote/1").expect("도구");
        let rows = [
            web.clone(),
            news.collected().clone(),
            disclosure.collected().clone(),
        ];
        let disclosures = [disclosure.clone()];
        let tools = [tool.clone()];
        let ledger = SignalSourceLedger::new(&rows, &disclosures, &tools);
        let web_ref = EvidenceRef::Web(EvidenceUrl::try_new(web.url().clone()).expect("URL"));
        let news_ref = EvidenceRef::News(
            NewsSource::Hankyung,
            EvidenceUrl::try_new(news.collected().url().clone()).expect("URL"),
        );
        let disclosure_ref = EvidenceRef::from_disclosure(&disclosure).expect("공시");
        for reference in [&web_ref, &news_ref, &disclosure_ref] {
            assert_eq!(
                classify(&decide_sources(
                    Evidence::try_new(vec![reference.clone(), EvidenceRef::Tool(tool.clone())])
                        .expect("근거"),
                    &ledger
                )),
                1
            );
        }
        assert_eq!(
            decide_sources(
                Evidence::try_new(vec![EvidenceRef::Tool(tool)]).expect("도구"),
                &ledger
            ),
            SignalDecision::None(NoSignalReason::MissingSource)
        );
        reject_sources(&ledger, &news, &disclosure, &web);
        let no_keys = SignalSourceLedger::new(&rows, &[], &tools);
        assert_eq!(
            decide_sources(
                Evidence::try_new(vec![disclosure_ref]).expect("공시"),
                &no_keys
            ),
            SignalDecision::None(NoSignalReason::MissingSource)
        );
    }

    fn reject_sources(
        ledger: &SignalSourceLedger<'_>,
        news: &NewsItem,
        disclosure: &Disclosure,
        web: &CollectedItem,
    ) {
        for reference in [
            EvidenceRef::Web(EvidenceUrl::try_new(news.collected().url().clone()).expect("URL")),
            EvidenceRef::News(
                NewsSource::Maekyung,
                EvidenceUrl::try_new(news.collected().url().clone()).expect("URL"),
            ),
            EvidenceRef::Disclosure(
                disclosure.key().clone(),
                EvidenceUrl::try_new(web.url().clone()).expect("URL"),
            ),
            EvidenceRef::Disclosure(
                Disclosure::new(
                    ReceiptNo::Dart(DartReceiptNo::try_new("20261114000002").expect("번호")),
                    disclosure.collected().url().clone(),
                    UntrustedText::try_new("다른 공시").expect("제목"),
                    None,
                )
                .key()
                .clone(),
                EvidenceUrl::try_new(disclosure.collected().url().clone()).expect("URL"),
            ),
            EvidenceRef::Tool(ToolReference::try_new("tool:missing").expect("도구")),
        ] {
            assert_eq!(
                decide_sources(Evidence::try_new(vec![reference]).expect("근거"), ledger),
                SignalDecision::None(NoSignalReason::MissingSource)
            );
        }
        let mixed = Evidence::try_new(vec![
            EvidenceRef::Web(EvidenceUrl::try_new(web.url().clone()).expect("URL")),
            EvidenceRef::Tool(ToolReference::try_new("tool:missing").expect("도구")),
        ])
        .expect("혼합 근거");
        assert_eq!(
            decide_sources(mixed, ledger),
            SignalDecision::None(NoSignalReason::MissingSource)
        );
    }

    #[test]
    fn closed_reasons_and_unknown_classification_precede_thresholds() {
        let eligible = eligible();
        let rows = [item()];
        let ledger = SignalSourceLedger::new(&rows, &[], &[]);
        let assignments = assignments();
        let facts = facts();
        let portfolio = valuation(None, TotalCompleteness::Complete);
        let plain = policy(0);
        for (percent, source, expected) in [
            (30, 1, NoSignalReason::ConditionsUnmet),
            (1, 1, NoSignalReason::UnheldLowProbability),
            (99, 0, NoSignalReason::MissingSource),
        ] {
            let calibration = probability(percent, 0);
            let estimate =
                SignalEstimate::create(&eligible, &calibration, margin(10), evidence(source))
                    .expect("추정");
            let context = SignalContext::new(
                Some(&portfolio),
                &plain,
                (&assignments, &facts),
                &ledger,
                money("1"),
            );
            assert_eq!(
                SignalDecision::assess(&estimate, &calibration, &context),
                SignalDecision::None(expected)
            );
        }
    }

    #[test]
    fn unknown_classification_and_invalid_budget_do_not_make_a_signal() {
        let eligible = eligible();
        let rows = [item()];
        let ledger = SignalSourceLedger::new(&rows, &[], &[]);
        let assignments = assignments();
        let facts = facts();
        let portfolio = valuation(None, TotalCompleteness::Complete);
        let plain = policy(0);
        let calibration = probability(99, 0);
        let estimate =
            SignalEstimate::create(&eligible, &calibration, margin(10), evidence(1)).expect("추정");
        let unknown = PolicySecurityFacts::default();
        let missing = SectorAssignments::default();
        let unclassified = SectorAssignments::try_new(vec![SectorAssignment::new(
            &instrument(),
            InstrumentClassification::Unclassified,
        )])
        .expect("미분류");
        for (mapping, types) in [
            (&assignments, &unknown),
            (&missing, &facts),
            (&unclassified, &facts),
        ] {
            let context = SignalContext::new(
                Some(&portfolio),
                &plain,
                (mapping, types),
                &ledger,
                money("1"),
            );
            assert_eq!(
                SignalDecision::assess(&estimate, &calibration, &context),
                SignalDecision::None(NoSignalReason::Unavailable(
                    SignalUnavailable::UnknownClassification
                ))
            );
        }
        for budget in ["0", "-1"] {
            let context = SignalContext::new(
                Some(&portfolio),
                &plain,
                (&assignments, &facts),
                &ledger,
                money(budget),
            );
            assert_eq!(
                SignalDecision::assess(&estimate, &calibration, &context),
                SignalDecision::None(NoSignalReason::PolicyViolation)
            );
        }
    }

    #[test]
    fn holding_identity_cannot_be_replaced_by_another_owned_instrument() {
        let other = ResolvedOrderInstrument::from_master_lookup_result(
            Symbol::try_new("OTHER").expect("종목"),
            Market::Kr,
        )
        .expect("시장");
        let holding = Holding::new(
            other,
            Quantity::parse(&NumericInput::try_new("1").expect("입력")).expect("수량"),
            AcquisitionPrice::try_new(money("1")).expect("원가"),
            HoldingObservation::new(
                None,
                Some(
                    ReportedHolding::new(money("10"), money("0"), Ratio::new(Decimal::ZERO))
                        .expect("평가"),
                ),
                candle::tests::at(61),
            ),
        )
        .expect("보유");
        let portfolio = Portfolio::new(
            vec![holding],
            BuyingPower::new(money("100"), candle::tests::at(61)),
            BuyingPower::new(
                Money::new(Amount::new(Decimal::ZERO), Currency::Usd),
                candle::tests::at(61),
            ),
        )
        .expect("포트")
        .evaluate(None);
        let rows = [item()];
        let ledger = SignalSourceLedger::new(&rows, &[], &[]);
        let assignments = assignments();
        let facts = facts();
        let policy = policy(0);
        let calibration = probability(1, 0);
        let estimate = SignalEstimate::create(&eligible(), &calibration, margin(10), evidence(1))
            .expect("추정");
        let context = SignalContext::new(
            Some(&portfolio),
            &policy,
            (&assignments, &facts),
            &ledger,
            money("1"),
        );
        assert_eq!(
            SignalDecision::assess(&estimate, &calibration, &context),
            SignalDecision::None(NoSignalReason::UnheldLowProbability)
        );
    }

    #[test]
    fn etf_classification_can_raise_a_signal_without_an_assigned_sector() {
        let eligible = eligible();
        let rows = [item()];
        let ledger = SignalSourceLedger::new(&rows, &[], &[]);
        let facts = facts();
        let portfolio = valuation(None, TotalCompleteness::Complete);
        let plain = policy(0);
        let calibration = probability(99, 0);
        let estimate =
            SignalEstimate::create(&eligible, &calibration, margin(10), evidence(1)).expect("추정");
        let etf = SectorAssignments::try_new(vec![SectorAssignment::new(
            &instrument(),
            InstrumentClassification::EtfFund,
        )])
        .expect("ETF");
        let context = SignalContext::new(
            Some(&portfolio),
            &plain,
            (&etf, &facts),
            &ledger,
            money("1"),
        );
        assert_eq!(
            classify(&SignalDecision::assess(&estimate, &calibration, &context)),
            1
        );
    }

    #[test]
    fn preparation_uses_d22_horizon_validation_before_any_signal() {
        let part = ModelIdentityPart::try_new("d24").expect("모델");
        let key = CalibrationKey::new(
            CalibrationModel::new(part.clone(), part.clone(), part),
            Horizon::new(HorizonSelection::FewDays),
        );
        let predicted =
            AiProbability::parse(&AiProbabilityInput::try_new("99").expect("입력")).expect("확률");
        let calibration = Calibration::try_new(key.clone(), &[])
            .expect("자료")
            .calibrate(&key, predicted);
        assert_eq!(
            SignalEstimate::create(&eligible(), &calibration, margin(10), evidence(1)),
            Err(NoThresholds::HorizonMismatch)
        );
    }
}
