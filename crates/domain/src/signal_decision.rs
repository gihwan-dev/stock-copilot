use crate::{
    CalibratedProbability, CollectedItem, CostEligibleTask, Disclosure, EstimateTask, Evidence,
    EvidenceRef, InformationSource, Margin, NoThresholds, Policy, PolicyBuyLimit,
    PolicySecurityFacts, PolicyViolation, PortfolioValuation, Quantity, SectorAssignments,
    SignalCondition, SignalSubmission, SignalTiming, SubmittedCalibration, ThresholdDisplay,
    Thresholds, Timestamp, ToolReference,
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
    CashObservationFailed(crate::CashObservationFailures),
    NegativeCash(crate::Currency),
    Calendar,
    StaleHolding,
    SubmissionFromFuture,
    MissingTargetPrice,
    StaleTargetPrice,
    InvalidTargetPrice,
    MissingCalibration,
    StaleCalibration,
    CalibrationCombinationMismatch,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoSignalReason {
    ConditionsUnmet,
    UnheldLowProbability,
    Excluded,
    MissingSource,
    PolicyViolation,
    Expired,
    Unavailable(SignalUnavailable),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignalPreparationError {
    SubmissionMismatch,
    Thresholds(NoThresholds),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignalEstimate {
    task: EstimateTask,
    thresholds: Thresholds,
    submitted: SubmittedCalibration,
    buy: SignalCondition,
    sell: SignalCondition,
}
impl SignalEstimate {
    pub fn create(
        eligible: &CostEligibleTask,
        submitted: &SubmittedCalibration,
        margin: Margin,
    ) -> Result<Self, SignalPreparationError> {
        if submitted.submission().task_key().task() != eligible.task() {
            return Err(SignalPreparationError::SubmissionMismatch);
        }
        prepare(eligible, submitted, margin).map_err(SignalPreparationError::Thresholds)
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
        self.submitted.calibration()
    }
    #[must_use]
    pub const fn submission(&self) -> &SignalSubmission {
        self.submitted.submission()
    }
    #[must_use]
    pub const fn calibration_head(&self) -> &crate::CalibrationLedgerHead {
        self.submitted.through()
    }
    #[must_use]
    pub const fn evidence(&self) -> Option<&Evidence> {
        self.submission().evidence()
    }
    #[must_use]
    pub const fn operation(&self) -> SignalOperation {
        match self.calibration() {
            CalibratedProbability::InsufficientSamples(_) => SignalOperation::Conservative,
            CalibratedProbability::EligibleBeforeE91(_) => SignalOperation::EligibleBeforeE91,
        }
    }
    pub fn display_probability(&self) -> Result<ThresholdDisplay, NoThresholds> {
        let probability = match self.calibration() {
            CalibratedProbability::InsufficientSamples(value) => value.evidence().probability(),
            CalibratedProbability::EligibleBeforeE91(value) => value.evidence().probability(),
        };
        crate::thresholds_arithmetic::signal_probability_display(
            crate::thresholds_arithmetic::ThresholdNumber::new(probability.as_decimal()),
        )
    }
}
fn prepare(
    eligible: &CostEligibleTask,
    submitted: &SubmittedCalibration,
    margin: Margin,
) -> Result<SignalEstimate, NoThresholds> {
    let calibration = submitted.calibration();
    let thresholds = Thresholds::create(eligible, calibration, margin)?;
    let (buy, sell) = crate::thresholds::bound_signal_conditions(&thresholds);
    Ok(SignalEstimate {
        task: eligible.task().clone(),
        thresholds,
        submitted: submitted.clone(),
        buy,
        sell,
    })
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
pub struct SignalAssessmentFacts<'facts> {
    timing: SignalTiming<'facts>,
    target_price: Option<crate::PriceObservation>,
    calibration_head: Option<crate::CalibrationLedgerHead>,
}
impl<'facts> SignalAssessmentFacts<'facts> {
    #[must_use]
    pub const fn new(
        timing: SignalTiming<'facts>,
        target_price: Option<crate::PriceObservation>,
        calibration_head: Option<crate::CalibrationLedgerHead>,
    ) -> Self {
        Self {
            timing,
            target_price,
            calibration_head,
        }
    }
}
pub struct SignalContext<'facts> {
    valuation: Option<&'facts PortfolioValuation>,
    policy: &'facts Policy,
    assignments: &'facts SectorAssignments,
    security: &'facts PolicySecurityFacts,
    sources: &'facts SignalSourceLedger<'facts>,
    facts: SignalAssessmentFacts<'facts>,
}
impl<'facts> SignalContext<'facts> {
    #[must_use]
    pub const fn new(
        valuation: Option<&'facts PortfolioValuation>,
        policy: &'facts Policy,
        classification: (&'facts SectorAssignments, &'facts PolicySecurityFacts),
        sources: &'facts SignalSourceLedger<'facts>,
        facts: SignalAssessmentFacts<'facts>,
    ) -> Self {
        Self {
            valuation,
            policy,
            assignments: classification.0,
            security: classification.1,
            sources,
            facts,
        }
    }
}
impl std::fmt::Debug for SignalContext<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SignalContext(..)")
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuySignal {
    facts: SignalEstimate,
    limit: PolicyBuyLimit,
}
impl BuySignal {
    #[must_use]
    pub const fn facts(&self) -> &SignalEstimate {
        &self.facts
    }
    #[must_use]
    pub const fn limit(&self) -> PolicyBuyLimit {
        self.limit
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SellSignal {
    facts: SignalEstimate,
    held_quantity: Quantity,
    observed_at: Timestamp,
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
    #[must_use]
    pub const fn observed_at(&self) -> Timestamp {
        self.observed_at
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignalTransitionError {
    InstrumentMismatch,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SignalTransition {
    Raised(RaisedSignal),
    Unchanged,
    Updated(RaisedSignal),
    Cleared(crate::InstrumentId),
    Reversed(RaisedSignal),
}
impl SignalTransition {
    #[must_use]
    pub fn events(&self) -> Vec<crate::DomainEvent> {
        match self {
            Self::Raised(signal) => {
                vec![crate::DomainEvent::SignalRaised(signal_id(signal).clone())]
            }
            Self::Cleared(id) => vec![crate::DomainEvent::SignalCleared(id.clone())],
            Self::Reversed(signal) => vec![
                crate::DomainEvent::SignalCleared(signal_id(signal).clone()),
                crate::DomainEvent::SignalRaised(signal_id(signal).clone()),
            ],
            Self::Unchanged | Self::Updated(_) => Vec::new(),
        }
    }
}
fn signal_facts(signal: &RaisedSignal) -> &SignalEstimate {
    match signal {
        RaisedSignal::Buy(value) => value.facts(),
        RaisedSignal::Sell(value) => value.facts(),
    }
}
fn signal_id(signal: &RaisedSignal) -> &crate::InstrumentId {
    signal_facts(signal).task().volatility().instrument()
}
pub fn signal_transition(
    previous: Option<&RaisedSignal>,
    decision: &SignalDecision,
) -> Result<SignalTransition, SignalTransitionError> {
    let next = decision.clone().into_signal();
    match (previous, next) {
        (None, None) => Ok(SignalTransition::Unchanged),
        (None, Some(value)) => Ok(SignalTransition::Raised(value)),
        (Some(old), None) => Ok(SignalTransition::Cleared(signal_id(old).clone())),
        (Some(old), Some(value)) => {
            if signal_id(old) != signal_id(&value)
                || signal_facts(old).task().reference().instrument()
                    != signal_facts(&value).task().reference().instrument()
            {
                return Err(SignalTransitionError::InstrumentMismatch);
            }
            match (old, &value) {
                (RaisedSignal::Buy(_), RaisedSignal::Buy(_))
                | (RaisedSignal::Sell(_), RaisedSignal::Sell(_)) => {
                    if old == &value {
                        Ok(SignalTransition::Unchanged)
                    } else {
                        Ok(SignalTransition::Updated(value))
                    }
                }
                (RaisedSignal::Buy(_), RaisedSignal::Sell(_))
                | (RaisedSignal::Sell(_), RaisedSignal::Buy(_)) => {
                    Ok(SignalTransition::Reversed(value))
                }
            }
        }
    }
}
impl SignalDecision {
    #[must_use]
    pub fn assess(estimate: &SignalEstimate, context: &SignalContext<'_>) -> Self {
        match assess(estimate, context) {
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
fn assess(
    estimate: &SignalEstimate,
    context: &SignalContext<'_>,
) -> Result<RaisedSignal, NoSignalReason> {
    let head = context
        .facts
        .calibration_head
        .as_ref()
        .ok_or(NoSignalReason::Unavailable(
            SignalUnavailable::MissingCalibration,
        ))?;
    if head.key() != estimate.submitted.through().key() {
        return Err(NoSignalReason::Unavailable(
            SignalUnavailable::CalibrationCombinationMismatch,
        ));
    }
    if head.position() != estimate.submitted.through().position() {
        return Err(NoSignalReason::Unavailable(
            SignalUnavailable::StaleCalibration,
        ));
    }
    context.sources.check(estimate.evidence())?;
    context
        .facts
        .timing
        .check(estimate.task(), estimate.submission().submitted_at())?;
    if estimate.sell == SignalCondition::Met {
        return sell_signal(estimate, context);
    }
    let valuation = context.valuation.ok_or(NoSignalReason::Unavailable(
        SignalUnavailable::HoldingsQueryFailed,
    ))?;
    let limit = crate::policy::signal_buy_limit(
        context.policy,
        valuation,
        context.assignments,
        context.security,
        &crate::AllocationInstrument::new(estimate.task().reference().instrument()),
    )
    .map_err(|violation| policy_reason(&violation))?;
    if estimate.buy == SignalCondition::Met {
        check_target_price(estimate, context)?;
        return Ok(RaisedSignal::Buy(BuySignal {
            facts: estimate.clone(),
            limit,
        }));
    }
    Err(NoSignalReason::ConditionsUnmet)
}
fn check_target_price(
    estimate: &SignalEstimate,
    context: &SignalContext<'_>,
) -> Result<(), NoSignalReason> {
    let unavailable = |reason| NoSignalReason::Unavailable(reason);
    let price = context
        .facts
        .target_price
        .as_ref()
        .ok_or_else(|| unavailable(SignalUnavailable::MissingTargetPrice))?;
    let reference = estimate.task().reference();
    if price.instrument() != reference.instrument()
        || price.price().as_money().currency() != reference.price().as_money().currency()
        || price.instrument_id() != Some(estimate.task().volatility().instrument())
    {
        return Err(unavailable(SignalUnavailable::InvalidTargetPrice));
    }
    if price.observed_at() < reference.observed_at()
        || price.check(reference.instrument(), context.facts.timing.now())
            != crate::CheckResult::Passed
    {
        return Err(unavailable(SignalUnavailable::StaleTargetPrice));
    }
    Ok(())
}
fn sell_signal(
    estimate: &SignalEstimate,
    context: &SignalContext<'_>,
) -> Result<RaisedSignal, NoSignalReason> {
    let valuation = context.valuation.ok_or(NoSignalReason::Unavailable(
        SignalUnavailable::HoldingsQueryFailed,
    ))?;
    let holding = valuation
        .holdings()
        .iter()
        .find(|row| row.instrument() == estimate.task().reference().instrument())
        .ok_or(NoSignalReason::UnheldLowProbability)?
        .holding();
    let quantity = holding.quantity();
    if quantity.as_decimal() <= rust_decimal::Decimal::ZERO {
        return Err(NoSignalReason::UnheldLowProbability);
    }
    context
        .facts
        .timing
        .check_holding(estimate.task(), holding.observed_at())?;
    Ok(RaisedSignal::Sell(SellSignal {
        facts: estimate.clone(),
        held_quantity: quantity,
        observed_at: holding.observed_at(),
    }))
}
fn policy_reason(violation: &PolicyViolation) -> NoSignalReason {
    match violation {
        PolicyViolation::ExcludedSector(_) | PolicyViolation::ExcludedType(_) => {
            NoSignalReason::Excluded
        }
        PolicyViolation::CandidateSectorUnassigned | PolicyViolation::UnknownSecurityType => {
            NoSignalReason::Unavailable(SignalUnavailable::UnknownClassification)
        }
        PolicyViolation::Allocation(crate::AllocationError::CashObservationFailed(failures)) => {
            NoSignalReason::Unavailable(SignalUnavailable::CashObservationFailed(*failures))
        }
        PolicyViolation::Allocation(crate::AllocationError::NegativeCash(currency)) => {
            NoSignalReason::Unavailable(SignalUnavailable::NegativeCash(*currency))
        }
        PolicyViolation::Allocation(
            crate::AllocationError::Incomplete
            | crate::AllocationError::ZeroTotal
            | crate::AllocationError::NegativeValue
            | crate::AllocationError::TotalMismatch
            | crate::AllocationError::MissingFx
            | crate::AllocationError::Arithmetic(_)
            | crate::AllocationError::Conversion(_),
        ) => NoSignalReason::Unavailable(SignalUnavailable::IncompleteHoldings),
        PolicyViolation::InstrumentCap(_)
        | PolicyViolation::InstrumentWarning(_)
        | PolicyViolation::SectorCap(_)
        | PolicyViolation::SectorUnassignedHoldings(_)
        | PolicyViolation::MinimumCash
        | PolicyViolation::ConcentrationWarning
        | PolicyViolation::ConcentrationUnavailable
        | PolicyViolation::UnresolvedPrinciples
        | PolicyViolation::Arithmetic(_)
        | PolicyViolation::InvalidCandidate => NoSignalReason::PolicyViolation,
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        AcquisitionPrice, AiProbability, AiProbabilityInput, AllocationInstrument, Amount,
        BuyingPower, Calibration, CalibrationEstimate, CalibrationEstimateId, CalibrationFact,
        CalibrationKey, CalibrationModel, CalibrationOutcome, CollectedItem, CommissionRateFact,
        CommissionValidity, CostEligibleTask, CostRate, Currency, DartReceiptNo, Disclosure,
        EstimateContext, EstimateCostPeriod, EstimateTask, Evidence, EvidenceRef, EvidenceUrl,
        ExcludedSecurityType, Exclusions, Holding, HoldingObservation, Horizon, HorizonSelection,
        HoursPermission, InstrumentClassification, InstrumentId, InvestorProfile, Margin, Market,
        MarketCalendar, MarketSession, ModelIdentityPart, Money, Nanosecond, NewsItem, NewsSource,
        NoSignalReason, NormalizedUrl, NumericInput, OffsetSeconds, Policy, PolicyCostFit,
        PolicyPrinciple, PolicySecurityFacts, PolicyText, Portfolio, PortfolioValuation,
        PriceObservation, Quantity, RaisedSignal, Ratio, ReceiptNo, ReportedHolding,
        ResolvedOrderInstrument, RiskAppetite, SectorAssignment, SectorAssignments, SectorId,
        SessionData, SessionInformation, SignalContext, SignalDecision, SignalEstimate,
        SignalOperation, SignalPreparationError, SignalSourceLedger, SignalSubmission,
        SignalSubmissionError, SignalTaskKey, SignalTiming, SignalUnavailable, SpanSeconds,
        SubmittedCalibration, Symbol, TimeInterval, TimeSpan, Timestamp, ToolReference, TradeCost,
        TradeTaxFact, UntrustedText, WeightCaps, calibrate_signal_submission, candle, thresholds,
    };
    use proptest::test_runner::{Config, TestRunner};
    use rust_decimal::Decimal;

    fn money(text: &str) -> Money {
        Money::parse(&NumericInput::try_new(text).expect("입력"), Currency::Krw).expect("금액")
    }
    fn ai(percent: i64) -> AiProbability {
        AiProbability::parse(&AiProbabilityInput::try_new(&percent.to_string()).expect("입력"))
            .expect("확률")
    }
    fn at(day: i64) -> Timestamp {
        let point = candle::tests::at(day);
        Timestamp::from_parts(
            point.utc_seconds(),
            point.nanosecond(),
            OffsetSeconds::new(32_400),
        )
        .expect("현지 시각")
    }
    fn later(point: Timestamp, seconds: u64) -> Timestamp {
        point
            .checked_add(
                TimeSpan::try_new(SpanSeconds::new(seconds), Nanosecond::new(0)).expect("기간"),
            )
            .expect("이후")
    }
    fn row(day: i64) -> MarketSession {
        MarketSession::try_new(
            at(day).offset_calendar_date(),
            SessionInformation::Kr {
                pre_market: None,
                regular_market: Some(
                    SessionData::try_new(
                        TimeInterval::try_new(at(day), later(at(day), 3600)).expect("구간"),
                        None,
                    )
                    .expect("장"),
                ),
                after_market: None,
            },
        )
        .expect("시장")
    }
    fn calendars() -> Vec<MarketCalendar> {
        (61_i64..=66)
            .map(|day| {
                MarketCalendar::try_new(
                    row(day.checked_sub(1).expect("전일")),
                    row(day),
                    row(day.checked_add(1).expect("익일")),
                )
                .expect("캘린더")
            })
            .collect()
    }
    fn eligible() -> CostEligibleTask {
        let task = thresholds::tests::task_for(Horizon::new(HorizonSelection::FewDays));
        eligible_for(task)
    }
    fn eligible_for(task: EstimateTask) -> CostEligibleTask {
        let zero = CostRate::try_new(Ratio::new(Decimal::ZERO)).expect("비용");
        let period =
            EstimateCostPeriod::from_calendar_result(&task, later(at(66), 3600)).expect("만기");
        let cost = TradeCost::from_facts(
            Some(CommissionRateFact::from_commissions_result(
                Market::Kr,
                zero,
                CommissionValidity::try_new(None, None).expect("기간"),
            )),
            Some(TradeTaxFact::from_schedule_result(Market::Kr, zero, zero)),
            period,
        )
        .expect("비용");
        CostEligibleTask::assess(task, &cost).expect("적격")
    }
    fn key() -> CalibrationKey {
        let part = ModelIdentityPart::try_new("d24").expect("모델");
        CalibrationKey::new(
            CalibrationModel::new(part.clone(), part.clone(), part),
            Horizon::new(HorizonSelection::FewDays),
        )
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
    fn calibrated(submission: SignalSubmission, count: usize) -> SubmittedCalibration {
        let key = submission.calibration_key().clone();
        let facts = (0..count)
            .map(|index| {
                CalibrationFact::new(
                    CalibrationEstimate::new(
                        CalibrationEstimateId::try_new(&format!("e{index}")).expect("ID"),
                        InstrumentId::try_new(&format!("i{index}")).expect("종목"),
                        key.clone(),
                        TimeInterval::try_new(at(1), at(2)).expect("기간"),
                        ai(50),
                    ),
                    CalibrationOutcome::Hit,
                )
            })
            .collect::<Vec<_>>();
        let head =
            crate::CalibrationLedgerHead::new(key.clone(), crate::CalibrationPosition::new(0));
        let snapshot = crate::CalibrationSnapshot::try_new(
            Calibration::try_new(key, &facts).expect("집계"),
            head.clone(),
        )
        .expect("완전한 표");
        calibrate_signal_submission(Some(&snapshot), &head, submission).expect("보정")
    }
    fn submission(
        eligible: &CostEligibleTask,
        percent: i64,
        sources: Option<Evidence>,
    ) -> SignalSubmission {
        SignalSubmission::create(
            SignalTaskKey::from_task(eligible),
            key(),
            ai(percent),
            sources,
            eligible.task().created_at(),
        )
        .expect("제출")
    }
    fn estimate(
        eligible: &CostEligibleTask,
        percent: i64,
        count: usize,
        margin: i64,
        source: u8,
    ) -> SignalEstimate {
        SignalEstimate::create(
            eligible,
            &calibrated(submission(eligible, percent, evidence(source)), count),
            Margin::try_new(Ratio::new(Decimal::new(margin, 2))).expect("여유"),
        )
        .expect("신호 입력")
    }
    fn instrument(market: Market, symbol: &str) -> ResolvedOrderInstrument {
        ResolvedOrderInstrument::from_master_lookup_result(
            Symbol::try_new(symbol).expect("심볼"),
            market,
        )
        .expect("종목")
    }
    fn fresh_price(now: Timestamp) -> PriceObservation {
        PriceObservation::from_identified_quote_parts(
            InstrumentId::try_new("sample").expect("종목 ID"),
            instrument(Market::Kr, "ABC"),
            crate::Price::try_new(money("100")).expect("가격"),
            now,
        )
        .expect("시세")
    }
    fn latest_calibration() -> crate::CalibrationLedgerHead {
        crate::CalibrationLedgerHead::new(key(), crate::CalibrationPosition::new(0))
    }
    fn assessment_facts(
        now: Timestamp,
        calendars: &[MarketCalendar],
    ) -> crate::SignalAssessmentFacts<'_> {
        crate::SignalAssessmentFacts::new(
            SignalTiming::new(now, calendars),
            Some(fresh_price(now)),
            Some(latest_calibration()),
        )
    }
    fn holding(market: Market, symbol: &str, quantity: &str, observed_at: Timestamp) -> Holding {
        let currency = if market == Market::Kr {
            Currency::Krw
        } else {
            Currency::Usd
        };
        let value = |text| Money::new(money(text).amount(), currency);
        Holding::new(
            instrument(market, symbol),
            Quantity::parse(&NumericInput::try_new(quantity).expect("입력")).expect("수량"),
            AcquisitionPrice::try_new(value("1")).expect("원가"),
            HoldingObservation::new(
                None,
                Some(
                    ReportedHolding::new(value("10"), value("0"), Ratio::new(Decimal::ZERO))
                        .expect("평가"),
                ),
                observed_at,
            ),
        )
        .expect("보유")
    }
    fn portfolio(rows: Vec<Holding>) -> PortfolioValuation {
        Portfolio::new(
            rows,
            BuyingPower::new(money("100"), at(61)),
            BuyingPower::new(
                Money::new(Amount::new(Decimal::ZERO), Currency::Usd),
                at(61),
            ),
        )
        .expect("포트")
        .evaluate(None)
    }
    fn assignments() -> SectorAssignments {
        SectorAssignments::try_new(vec![SectorAssignment::new(
            &instrument(Market::Kr, "ABC"),
            InstrumentClassification::Sector(SectorId::try_new("Tech").expect("섹터")),
        )])
        .expect("분류")
    }
    fn facts() -> PolicySecurityFacts {
        PolicySecurityFacts::from_master_lookup_results(
            std::iter::once((
                AllocationInstrument::new(&instrument(Market::Kr, "ABC")),
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
            .expect("원칙"),
        )
    }
    fn classify(decision: &SignalDecision) -> u8 {
        match decision {
            SignalDecision::Buy(_) => 1,
            SignalDecision::Sell(_) => 2,
            SignalDecision::None(_) => 0,
        }
    }
    fn buy_payload(signal: RaisedSignal) -> Option<crate::BuySignal> {
        match signal {
            RaisedSignal::Buy(value) => Some(value),
            RaisedSignal::Sell(_) => None,
        }
    }
    fn sell_payload(signal: RaisedSignal) -> Option<crate::SellSignal> {
        match signal {
            RaisedSignal::Sell(value) => Some(value),
            RaisedSignal::Buy(_) => None,
        }
    }
    fn conservative(value: &crate::CalibratedProbability) -> bool {
        match value {
            crate::CalibratedProbability::InsufficientSamples(_) => true,
            crate::CalibratedProbability::EligibleBeforeE91(_) => false,
        }
    }
    fn is_buy(value: &SignalDecision) -> bool {
        match value {
            SignalDecision::Buy(_) => true,
            SignalDecision::Sell(_) | SignalDecision::None(_) => false,
        }
    }
    fn transition_kind(value: &crate::SignalTransition) -> u8 {
        match value {
            crate::SignalTransition::Raised(_) => 1,
            crate::SignalTransition::Unchanged => 2,
            crate::SignalTransition::Updated(_) => 3,
            crate::SignalTransition::Cleared(_) => 4,
            crate::SignalTransition::Reversed(_) => 5,
        }
    }
    fn expected_fit(policy: &Policy, amount: &str) -> PolicyCostFit {
        if (policy.minimum_cash().is_some() && amount != "1") || amount == "13" {
            PolicyCostFit::AboveLimit
        } else {
            PolicyCostFit::WithinLimit
        }
    }
    #[test]
    fn a27_signal_estimate_retains_its_calibration_ledger_head() {
        let issued = eligible();
        let record = submission(&issued, 99, evidence(1));
        let key = record.calibration_key().clone();
        let head =
            crate::CalibrationLedgerHead::new(key.clone(), crate::CalibrationPosition::new(17));
        let snapshot = crate::CalibrationSnapshot::try_new(
            Calibration::try_new(key, &[]).expect("보정"),
            head.clone(),
        )
        .expect("표");
        let prepared = calibrate_signal_submission(Some(&snapshot), &head, record).expect("보정");
        let signal =
            SignalEstimate::create(&issued, &prepared, Margin::default()).expect("신호 입력");
        assert_eq!(signal.calibration_head(), &head);
    }
    #[test]
    fn d51_calibration_snapshot_distinguishes_missing_empty_stale_and_wrong_key() {
        let eligible = eligible();
        let submitted = submission(&eligible, 63, evidence(1));
        let head = crate::CalibrationLedgerHead::new(key(), crate::CalibrationPosition::new(12));
        assert_eq!(*head.position().as_u64(), 12);
        let empty = Calibration::try_new(key(), &[]).expect("빈 표");
        let snapshot = crate::CalibrationSnapshot::try_new(empty, head.clone()).expect("완전한 표");
        assert_eq!(
            calibrate_signal_submission(None, &head, submitted.clone()),
            Err(crate::CalibrationError::MissingCalibration)
        );
        let valid = calibrate_signal_submission(Some(&snapshot), &head, submitted.clone())
            .expect("정상 빈 표");
        assert!(conservative(valid.calibration()));
        let ready = calibrated(submission(&eligible, 63, evidence(1)), 30);
        assert!(!conservative(ready.calibration()));
        let later = crate::CalibrationLedgerHead::new(key(), crate::CalibrationPosition::new(13));
        assert_eq!(
            calibrate_signal_submission(Some(&snapshot), &later, submitted.clone()),
            Err(crate::CalibrationError::StaleCalibration)
        );
        let wrong = crate::CalibrationLedgerHead::new(
            CalibrationKey::new(key().model().clone(), Horizon::new(HorizonSelection::Hours)),
            crate::CalibrationPosition::new(12),
        );
        assert_eq!(
            calibrate_signal_submission(Some(&snapshot), &wrong, submitted),
            Err(crate::CalibrationError::MismatchedCombination)
        );
        let other_key = wrong.key().clone();
        let other_table = Calibration::try_new(other_key, &[]).expect("다른 조합");
        assert_eq!(
            crate::CalibrationSnapshot::try_new(other_table.clone(), head.clone()),
            Err(crate::CalibrationError::MismatchedCombination)
        );
        let other_snapshot =
            crate::CalibrationSnapshot::try_new(other_table, wrong).expect("다른 표");
        assert_eq!(
            calibrate_signal_submission(
                Some(&other_snapshot),
                &head,
                submission(&eligible, 63, evidence(1))
            ),
            Err(crate::CalibrationError::MismatchedCombination)
        );
    }
    #[test]
    fn d51_submission_rejects_a_consistent_snapshot_for_another_model() {
        let eligible = eligible();
        let part = ModelIdentityPart::try_new("other").expect("다른 모델");
        let other = CalibrationKey::new(
            CalibrationModel::new(part.clone(), part.clone(), part),
            Horizon::new(HorizonSelection::FewDays),
        );
        let head =
            crate::CalibrationLedgerHead::new(other.clone(), crate::CalibrationPosition::new(0));
        let snapshot = crate::CalibrationSnapshot::try_new(
            Calibration::try_new(other, &[]).expect("다른 모델의 표"),
            head.clone(),
        )
        .expect("다른 모델의 완전한 표");
        assert_eq!(
            calibrate_signal_submission(
                Some(&snapshot),
                &head,
                submission(&eligible, 63, evidence(1))
            ),
            Err(crate::CalibrationError::MismatchedCombination)
        );
    }
    #[test]
    fn d51_calibration_positions_and_combinations_property() {
        let eligible = eligible();
        let submitted = submission(&eligible, 67, evidence(1));
        TestRunner::new(Config::with_cases(testkit::property_cases()))
            .run(
                &(0_u64..10_000, 0_u8..3, 0_u8..2),
                |(through, offset, mismatch)| {
                    let latest = match offset {
                        0 => through,
                        1 => through.saturating_add(1),
                        _ => through.saturating_sub(1),
                    };
                    let own = crate::CalibrationLedgerHead::new(
                        key(),
                        crate::CalibrationPosition::new(through),
                    );
                    let snapshot = crate::CalibrationSnapshot::try_new(
                        Calibration::try_new(key(), &[]).expect("표"),
                        own,
                    )
                    .expect("스냅숏");
                    let requested = if mismatch == 0 {
                        key()
                    } else {
                        CalibrationKey::new(
                            key().model().clone(),
                            Horizon::new(HorizonSelection::WeeksToMonths),
                        )
                    };
                    let head = crate::CalibrationLedgerHead::new(
                        requested,
                        crate::CalibrationPosition::new(latest),
                    );
                    let result =
                        calibrate_signal_submission(Some(&snapshot), &head, submitted.clone());
                    if mismatch != 0 {
                        proptest::prop_assert_eq!(
                            result,
                            Err(crate::CalibrationError::MismatchedCombination)
                        );
                    } else if through != latest {
                        proptest::prop_assert_eq!(
                            result,
                            Err(crate::CalibrationError::StaleCalibration)
                        );
                    } else {
                        proptest::prop_assert!(conservative(result.expect("일치").calibration()));
                    }
                    Ok(())
                },
            )
            .expect("조합과 위치");
    }
    fn decide_calibration_head(head: Option<crate::CalibrationLedgerHead>) -> SignalDecision {
        let eligible = eligible();
        let estimate = estimate(&eligible, 99, 0, 10, 1);
        let valuation = portfolio(vec![]);
        let policy = policy(0);
        let mapping = assignments();
        let types = facts();
        let items = [item()];
        let ledger = SignalSourceLedger::new(&items, &[], &[]);
        let calendar = calendars();
        let facts = crate::SignalAssessmentFacts::new(
            SignalTiming::new(at(61), &calendar),
            Some(fresh_price(at(61))),
            head,
        );
        let context = SignalContext::new(
            Some(&valuation),
            &policy,
            (&mapping, &types),
            &ledger,
            facts,
        );
        SignalDecision::assess(&estimate, &context)
    }
    #[test]
    fn d51_assessment_rechecks_ledger_head_after_preparation() {
        let latest = crate::CalibrationLedgerHead::new(key(), crate::CalibrationPosition::new(1));
        assert_eq!(
            decide_calibration_head(Some(latest)),
            SignalDecision::None(NoSignalReason::Unavailable(
                SignalUnavailable::StaleCalibration
            ))
        );
        assert_eq!(
            decide_calibration_head(None),
            SignalDecision::None(NoSignalReason::Unavailable(
                SignalUnavailable::MissingCalibration
            ))
        );
        let wrong = crate::CalibrationLedgerHead::new(
            CalibrationKey::new(key().model().clone(), Horizon::new(HorizonSelection::Hours)),
            crate::CalibrationPosition::new(0),
        );
        assert_eq!(
            decide_calibration_head(Some(wrong)),
            SignalDecision::None(NoSignalReason::Unavailable(
                SignalUnavailable::CalibrationCombinationMismatch
            ))
        );
    }
    fn decide_target_price(price: Option<PriceObservation>, now: Timestamp) -> SignalDecision {
        let eligible = eligible();
        let estimate = estimate(&eligible, 99, 0, 10, 1);
        let valuation = portfolio(vec![]);
        let policy = policy(0);
        let mapping = assignments();
        let types = facts();
        let items = [item()];
        let ledger = SignalSourceLedger::new(&items, &[], &[]);
        let calendar = calendars();
        let context = SignalContext::new(
            Some(&valuation),
            &policy,
            (&mapping, &types),
            &ledger,
            crate::SignalAssessmentFacts::new(
                SignalTiming::new(now, &calendar),
                price,
                Some(latest_calibration()),
            ),
        );
        SignalDecision::assess(&estimate, &context)
    }
    fn target_quote(
        instrument: ResolvedOrderInstrument,
        price: crate::Price,
        observed: Timestamp,
    ) -> PriceObservation {
        PriceObservation::from_identified_quote_parts(
            InstrumentId::try_new("sample").expect("종목 ID"),
            instrument,
            price,
            observed,
        )
        .expect("시세")
    }
    #[test]
    fn d51_buy_target_price_rejects_a_quote_without_an_instrument_id() {
        let task = eligible();
        let reference = task.task().reference();
        let now = at(61);
        let unidentified = PriceObservation::from_quote_parts(
            reference.instrument().clone(),
            reference.price(),
            now,
        )
        .expect("종목 ID 없는 시세");
        assert_eq!(
            decide_target_price(Some(unidentified), now),
            SignalDecision::None(NoSignalReason::Unavailable(
                SignalUnavailable::InvalidTargetPrice
            ))
        );
    }
    #[test]
    fn d51_buy_target_price_missing_stale_and_boundary() {
        let task = eligible();
        let reference = task.task().reference();
        let now = at(61);
        let valid = target_quote(reference.instrument().clone(), reference.price(), now);
        let before = now.checked_sub(TimeSpan::NANOSECOND).expect("기준 이전");
        let before_reference =
            target_quote(reference.instrument().clone(), reference.price(), before);
        let future = target_quote(
            reference.instrument().clone(),
            reference.price(),
            later(now, 1),
        );
        let just_before = later(now, 5)
            .checked_sub(TimeSpan::NANOSECOND)
            .expect("경계 직전");
        let missing = decide_target_price(None, now);
        assert!(!is_buy(&missing));
        assert_eq!(
            missing,
            SignalDecision::None(NoSignalReason::Unavailable(
                SignalUnavailable::MissingTargetPrice
            ))
        );
        assert_eq!(
            decide_target_price(Some(valid.clone()), later(now, 5)),
            SignalDecision::None(NoSignalReason::Unavailable(
                SignalUnavailable::StaleTargetPrice
            ))
        );
        assert_eq!(
            decide_target_price(Some(before_reference), now),
            SignalDecision::None(NoSignalReason::Unavailable(
                SignalUnavailable::StaleTargetPrice
            ))
        );
        assert_eq!(
            decide_target_price(Some(future), now),
            SignalDecision::None(NoSignalReason::Unavailable(
                SignalUnavailable::StaleTargetPrice
            ))
        );
        assert!(is_buy(&decide_target_price(
            Some(valid.clone()),
            just_before
        )));
        assert!(is_buy(&decide_target_price(Some(valid), now)));
    }
    #[test]
    fn d51_buy_target_price_rejects_wrong_currency_instrument_and_id() {
        let task = eligible();
        let reference = task.task().reference();
        let now = at(61);
        let wrong_currency = target_quote(
            reference.instrument().clone(),
            crate::Price::try_new(Money::new(Amount::new(Decimal::ONE), Currency::Usd))
                .expect("단가"),
            now,
        );
        let wrong_instrument =
            target_quote(instrument(Market::Kr, "OTHER"), reference.price(), now);
        let wrong_id = PriceObservation::from_identified_quote_parts(
            InstrumentId::try_new("other-id").expect("다른 ID"),
            reference.instrument().clone(),
            reference.price(),
            now,
        )
        .expect("다른 ID 시세");
        for price in [wrong_currency, wrong_instrument, wrong_id] {
            assert_eq!(
                decide_target_price(Some(price), now),
                SignalDecision::None(NoSignalReason::Unavailable(
                    SignalUnavailable::InvalidTargetPrice
                ))
            );
        }
    }
    #[test]
    fn d51_target_price_freshness_boundary_property() {
        let eligible = eligible();
        let estimate = estimate(&eligible, 99, 0, 10, 1);
        let valuation = portfolio(vec![]);
        let policy = policy(0);
        let mapping = assignments();
        let types = facts();
        let items = [item()];
        let ledger = SignalSourceLedger::new(&items, &[], &[]);
        let calendar = calendars();
        TestRunner::new(Config::with_cases(testkit::property_cases()))
            .run(&(0_u64..10), |seconds| {
                let now = later(at(61), seconds);
                let context = SignalContext::new(
                    Some(&valuation),
                    &policy,
                    (&mapping, &types),
                    &ledger,
                    crate::SignalAssessmentFacts::new(
                        SignalTiming::new(now, &calendar),
                        Some(fresh_price(at(61))),
                        Some(latest_calibration()),
                    ),
                );
                let result = SignalDecision::assess(&estimate, &context);
                if seconds < 5 {
                    proptest::prop_assert!(is_buy(&result));
                } else {
                    proptest::prop_assert_eq!(
                        result,
                        SignalDecision::None(NoSignalReason::Unavailable(
                            SignalUnavailable::StaleTargetPrice
                        ))
                    );
                }
                Ok(())
            })
            .expect("가격 시각 경계");
    }
    fn transition_decisions() -> (SignalDecision, SignalDecision, SignalDecision, InstrumentId) {
        let eligible = eligible();
        let valuation = portfolio(vec![holding(Market::Kr, "ABC", "1", at(61))]);
        let policy = policy(0);
        let mapping = assignments();
        let types = facts();
        let items = [item()];
        let ledger = SignalSourceLedger::new(&items, &[], &[]);
        let calendar = calendars();
        let context = SignalContext::new(
            Some(&valuation),
            &policy,
            (&mapping, &types),
            &ledger,
            assessment_facts(at(61), &calendar),
        );
        (
            SignalDecision::assess(&estimate(&eligible, 99, 0, 10, 1), &context),
            SignalDecision::assess(&estimate(&eligible, 98, 0, 10, 1), &context),
            SignalDecision::assess(&estimate(&eligible, 1, 0, 10, 1), &context),
            eligible.task().volatility().instrument().clone(),
        )
    }
    fn d51_eligible_with_instrument_id(id: &str) -> CostEligibleTask {
        let original = eligible();
        let task = original.task();
        let instrument_id = InstrumentId::try_new(id).expect("종목 ID");
        let values = (0_usize..61)
            .map(|index| if index % 2 == 0 { "100" } else { "102" })
            .collect::<Vec<_>>();
        let candles = candle::tests::series(&values)
            .into_iter()
            .map(|row| {
                crate::Candle::try_new(
                    instrument_id.clone(),
                    row.time(),
                    row.adjustment(),
                    row.values(),
                    crate::CandleObservation::new(row.collected_at(), row.status()),
                )
                .expect("종목의 봉")
            })
            .collect::<Vec<_>>();
        let reference = PriceObservation::from_identified_quote_parts(
            instrument_id,
            task.reference().instrument().clone(),
            task.reference().price(),
            task.reference().observed_at(),
        )
        .expect("종목 시세");
        let new_task = EstimateTask::create(
            EstimateContext::new(reference, task.created_at()),
            crate::Volatility::calculate(&candles).expect("변동성"),
            Horizon::new(HorizonSelection::FewDays),
            HoursPermission::default(),
            RiskAppetite::Neutral,
        )
        .expect("종목 과제");
        eligible_for(new_task)
    }
    fn d51_buy_with_instrument_id(id: &str) -> crate::BuySignal {
        let eligible = d51_eligible_with_instrument_id(id);
        let valuation = portfolio(vec![]);
        let policy = policy(0);
        let mapping = assignments();
        let types = facts();
        let items = [item()];
        let ledger = SignalSourceLedger::new(&items, &[], &[]);
        let calendar = calendars();
        let context = SignalContext::new(
            Some(&valuation),
            &policy,
            (&mapping, &types),
            &ledger,
            crate::SignalAssessmentFacts::new(
                SignalTiming::new(at(61), &calendar),
                Some(eligible.task().reference().clone()),
                Some(latest_calibration()),
            ),
        );
        let signal = SignalDecision::assess(&estimate(&eligible, 99, 0, 10, 1), &context)
            .into_signal()
            .expect("매수 발생");
        buy_payload(signal).expect("매수 신호")
    }
    #[test]
    fn d51_transition_rejects_same_symbol_with_a_different_instrument_id() {
        let previous = d51_buy_with_instrument_id("previous");
        let next = d51_buy_with_instrument_id("next");
        assert_eq!(
            previous.facts().task().reference().instrument(),
            next.facts().task().reference().instrument()
        );
        assert_ne!(
            previous.facts().task().volatility().instrument(),
            next.facts().task().volatility().instrument()
        );
        assert_eq!(
            crate::signal_transition(
                Some(&RaisedSignal::Buy(previous)),
                &SignalDecision::Buy(next)
            ),
            Err(crate::SignalTransitionError::InstrumentMismatch)
        );
    }
    #[test]
    fn d51_current_occurrence_rejects_same_symbol_with_a_different_instrument_id() {
        let previous = d51_buy_with_instrument_id("previous");
        let next = d51_buy_with_instrument_id("next");
        assert_eq!(
            previous.facts().task().reference().instrument(),
            next.facts().task().reference().instrument()
        );
        let occurrence = crate::BuySignalRecord::from_signal(
            crate::BuySignalOccurrenceId::try_new("occurrence").expect("발생 ID"),
            &previous,
            at(61),
            &calendars(),
        )
        .expect("발생 기록");
        assert_eq!(
            crate::CurrentBuySignal::try_new(next, occurrence),
            Err(crate::NoBuyQuantity::Indeterminate)
        );
    }

    #[test]
    fn a27_new_occurrence_retains_the_originating_task() {
        let signal = d51_buy_with_instrument_id("origin");
        let occurrence = crate::BuySignalRecord::from_signal(
            crate::BuySignalOccurrenceId::try_new("origin-occurrence").expect("발생 ID"),
            &signal,
            at(61),
            &calendars(),
        )
        .expect("발생 기록");
        assert_eq!(occurrence.originating_task().task(), signal.facts().task());
    }

    #[test]
    fn d51_history_blocks_same_symbol_with_a_different_instrument_id() {
        let previous = d51_buy_with_instrument_id("previous");
        let next = d51_buy_with_instrument_id("next");
        assert_ne!(
            previous.facts().task().volatility().instrument(),
            next.facts().task().volatility().instrument()
        );
        assert_eq!(
            previous.facts().task().reference().instrument(),
            next.facts().task().reference().instrument()
        );
        let record = crate::BuySignalRecord::from_signal(
            crate::BuySignalOccurrenceId::try_new("previous").expect("이전 ID"),
            &previous,
            at(61),
            &calendars(),
        )
        .expect("이전 발생");
        let occurrence = crate::BuySignalRecord::from_signal(
            crate::BuySignalOccurrenceId::try_new("current").expect("현재 ID"),
            &next,
            at(61),
            &calendars(),
        )
        .expect("현재 발생");
        let current = crate::CurrentBuySignal::try_new(next, occurrence).expect("현재 신호");
        assert_eq!(
            crate::buy_signal_history::check_buy_history(&current, at(61), &[record]),
            Err(crate::NoBuyQuantity::RepeatedSignal)
        );
    }
    #[test]
    fn d51_transition_table_emits_expected_events() {
        let (buy, _, sell, id) = transition_decisions();
        let none = SignalDecision::None(NoSignalReason::ConditionsUnmet);
        let buy_previous = buy.clone().into_signal().expect("매수");
        let sell_previous = sell.clone().into_signal().expect("매도");
        let raised = vec![crate::DomainEvent::SignalRaised(id.clone())];
        let cleared = vec![crate::DomainEvent::SignalCleared(id.clone())];
        let reversed = vec![
            crate::DomainEvent::SignalCleared(id.clone()),
            crate::DomainEvent::SignalRaised(id),
        ];
        for decision in [&buy, &sell] {
            let transition = crate::signal_transition(None, decision).expect("발생");
            assert_eq!(transition_kind(&transition), 1);
            assert_eq!(transition.events(), raised);
        }
        let no_transition = crate::signal_transition(None, &none).expect("없음 유지");
        assert_eq!(transition_kind(&no_transition), 2);
        for (previous, same, opposite) in
            [(&buy_previous, &buy, &sell), (&sell_previous, &sell, &buy)]
        {
            let same_transition = crate::signal_transition(Some(previous), same).expect("유지");
            assert_eq!(transition_kind(&same_transition), 2);
            assert_eq!(same_transition.events(), Vec::new());
            let cleared_transition = crate::signal_transition(Some(previous), &none).expect("해제");
            assert_eq!(transition_kind(&cleared_transition), 4);
            assert_eq!(cleared_transition.events(), cleared);
            let opposite_transition =
                crate::signal_transition(Some(previous), opposite).expect("변경");
            assert_eq!(transition_kind(&opposite_transition), 5);
            assert_eq!(opposite_transition.events(), reversed);
        }
    }
    #[test]
    fn d51_transition_updates_content_without_a_new_event() {
        let (buy, updated, _, _) = transition_decisions();
        let previous = buy.into_signal().expect("매수");
        let transition = crate::signal_transition(Some(&previous), &updated).expect("내용 갱신");
        assert_eq!(transition_kind(&transition), 3);
        assert_eq!(transition.events(), Vec::new());
    }
    #[test]
    fn d51_transition_rejects_a_different_instrument() {
        let (buy, _, _, _) = transition_decisions();
        let previous = buy.clone().into_signal().expect("매수");
        let original = eligible();
        let task = original.task();
        let other_reference = PriceObservation::from_identified_quote_parts(
            task.volatility().instrument().clone(),
            instrument(Market::Kr, "OTHER"),
            task.reference().price(),
            task.reference().observed_at(),
        )
        .expect("다른 종목");
        let other_task = EstimateTask::create(
            EstimateContext::new(other_reference, task.created_at()),
            task.volatility().clone(),
            Horizon::new(HorizonSelection::FewDays),
            HoursPermission::default(),
            RiskAppetite::Neutral,
        )
        .expect("다른 과제");
        let other_facts = estimate(&eligible_for(other_task), 99, 0, 10, 1);
        let signal = buy_payload(buy.into_signal().expect("매수")).expect("매수 신호");
        let other_decision = SignalDecision::Buy(super::BuySignal {
            facts: other_facts,
            limit: signal.limit,
        });
        assert_eq!(
            crate::signal_transition(Some(&previous), &other_decision),
            Err(crate::SignalTransitionError::InstrumentMismatch)
        );
    }
    #[test]
    fn d51_transition_property_preserves_same_direction_occurrence() {
        let eligible = eligible();
        let valuation = portfolio(vec![holding(Market::Kr, "ABC", "1", at(61))]);
        let policy = policy(0);
        let mapping = assignments();
        let types = facts();
        let items = [item()];
        let ledger = SignalSourceLedger::new(&items, &[], &[]);
        let calendar = calendars();
        let context = SignalContext::new(
            Some(&valuation),
            &policy,
            (&mapping, &types),
            &ledger,
            assessment_facts(at(61), &calendar),
        );
        let buy = SignalDecision::assess(&estimate(&eligible, 99, 0, 10, 1), &context);
        let sell = SignalDecision::assess(&estimate(&eligible, 1, 0, 10, 1), &context);
        let none = SignalDecision::None(NoSignalReason::ConditionsUnmet);
        let decisions = [none, buy, sell];
        TestRunner::new(Config::with_cases(testkit::property_cases()))
            .run(&(0_usize..3, 0_usize..3), |(before, after)| {
                let old = decisions[before].clone().into_signal();
                let transition =
                    crate::signal_transition(old.as_ref(), &decisions[after]).expect("전이");
                let count = transition.events().len();
                let expected = match (before, after) {
                    (0, 0) | (1, 1) | (2, 2) => 0,
                    (1, 2) | (2, 1) => 2,
                    _ => 1,
                };
                proptest::prop_assert_eq!(count, expected);
                Ok(())
            })
            .expect("전이 표");
    }
    #[test]
    fn signal_limit_is_always_present_and_cash_minimum_is_enforced() {
        let eligible = eligible();
        let estimate = estimate(&eligible, 99, 0, 10, 1);
        let portfolio = portfolio(vec![holding(Market::Kr, "ABC", "1", at(61))]);
        let rows = [item()];
        let ledger = SignalSourceLedger::new(&rows, &[], &[]);
        let mapping = assignments();
        let types = facts();
        let calendar = calendars();
        for policy in [
            policy(0),
            policy(0).with_minimum_cash(
                crate::ProfileWeight::parse_percent(&NumericInput::try_new("90").expect("입력"))
                    .expect("비중"),
            ),
        ] {
            let context = SignalContext::new(
                Some(&portfolio),
                &policy,
                (&mapping, &types),
                &ledger,
                assessment_facts(at(61), &calendar),
            );
            let decision = SignalDecision::assess(&estimate, &context);
            let raised = decision.into_signal().expect("신호");
            assert_eq!(sell_payload(raised.clone()), None);
            let signal = buy_payload(raised).expect("매수 한도");
            let limit = signal.limit();
            for amount in ["1", "11", "12", "13"] {
                let expected = expected_fit(&policy, amount);
                assert_eq!(limit.compare_krw_cost(money(amount)), expected);
            }
            assert_eq!(signal.facts(), &estimate);
        }
    }

    #[test]
    fn excluded_unclassified_unknown_and_incomplete_holdings_can_sell() {
        let eligible = eligible();
        let estimate = estimate(&eligible, 1, 0, 10, 1);
        let portfolio = portfolio(vec![
            holding(Market::Kr, "ABC", "0.25", at(61)),
            holding(Market::Us, "US", "1", at(61)),
        ]);
        assert_eq!(
            portfolio.valuation().completeness(),
            crate::TotalCompleteness::Incomplete
        );
        let rows = [item()];
        let ledger = SignalSourceLedger::new(&rows, &[], &[]);
        let mapping = assignments();
        let missing = SectorAssignments::default();
        let unclassified = SectorAssignments::try_new(vec![SectorAssignment::new(
            &instrument(Market::Kr, "ABC"),
            InstrumentClassification::Unclassified,
        )])
        .expect("분류");
        let types = facts();
        let unknown = PolicySecurityFacts::default();
        let calendar = calendars();
        for policy in [
            policy(0),
            policy(1),
            policy(2),
            policy(0).with_unresolved_principle(PolicyPrinciple::from_missing_contract(
                PolicyText::try_new("미확정").expect("원칙"),
            )),
        ] {
            for (candidate_mapping, candidate_types) in [
                (&mapping, &types),
                (&mapping, &unknown),
                (&missing, &unknown),
                (&unclassified, &types),
            ] {
                let context = SignalContext::new(
                    Some(&portfolio),
                    &policy,
                    (candidate_mapping, candidate_types),
                    &ledger,
                    assessment_facts(at(61), &calendar),
                );
                let raised = SignalDecision::assess(&estimate, &context)
                    .into_signal()
                    .expect("신호");
                assert_eq!(buy_payload(raised.clone()), None);
                let signal = sell_payload(raised).expect("손절 매도");
                assert_eq!(signal.held_quantity().as_decimal(), Decimal::new(25, 2));
                assert_eq!(signal.observed_at(), at(61));
                assert_eq!(signal.facts(), &estimate);
            }
        }
    }

    fn buy_block_reason(policy: &Policy) -> NoSignalReason {
        let exclusions = policy.profile().exclusions();
        if exclusions.sectors().is_empty() && exclusions.security_types().is_empty() {
            NoSignalReason::Unavailable(SignalUnavailable::IncompleteHoldings)
        } else {
            NoSignalReason::Excluded
        }
    }
    #[test]
    fn buy_policy_still_blocks_unknown_excluded_and_incomplete_inputs() {
        let eligible = eligible();
        let estimate = estimate(&eligible, 99, 0, 10, 1);
        let values = portfolio(vec![holding(Market::Kr, "ABC", "1", at(61))]);
        let incomplete = portfolio(vec![holding(Market::Us, "US", "1", at(61))]);
        let rows = [item()];
        let ledger = SignalSourceLedger::new(&rows, &[], &[]);
        let mapping = assignments();
        let missing = SectorAssignments::default();
        let types = facts();
        let unknown = PolicySecurityFacts::default();
        let calendar = calendars();
        let plain = policy(0);
        for policy in [policy(0), policy(1), policy(2)] {
            let snapshot = if policy.profile().exclusions().security_types().is_empty()
                && policy.profile().exclusions().sectors().is_empty()
            {
                &incomplete
            } else {
                &values
            };
            let context = SignalContext::new(
                Some(snapshot),
                &policy,
                (&mapping, &types),
                &ledger,
                assessment_facts(at(61), &calendar),
            );
            let expected = buy_block_reason(&policy);
            assert_eq!(
                SignalDecision::assess(&estimate, &context),
                SignalDecision::None(expected)
            );
        }
        for classification in [(&mapping, &unknown), (&missing, &types)] {
            let context = SignalContext::new(
                Some(&values),
                &plain,
                classification,
                &ledger,
                assessment_facts(at(61), &calendar),
            );
            assert_eq!(
                SignalDecision::assess(&estimate, &context).into_signal(),
                None
            );
        }
        let context = SignalContext::new(
            None,
            &plain,
            (&mapping, &types),
            &ledger,
            assessment_facts(at(61), &calendar),
        );
        assert_eq!(
            SignalDecision::assess(&estimate, &context),
            SignalDecision::None(NoSignalReason::Unavailable(
                SignalUnavailable::HoldingsQueryFailed
            ))
        );
        assert_eq!(format!("{context:?}"), "SignalContext(..)");
    }

    #[test]
    fn unrelated_market_symbol_absent_and_zero_holdings_never_sell() {
        let eligible = eligible();
        let estimate = estimate(&eligible, 1, 0, 10, 1);
        let rows = [item()];
        let ledger = SignalSourceLedger::new(&rows, &[], &[]);
        let mapping = assignments();
        let types = facts();
        let policy = policy(0);
        let calendar = calendars();
        for holdings in [
            vec![],
            vec![holding(Market::Kr, "ABC", "0", at(61))],
            vec![holding(Market::Us, "ABC", "1", at(61))],
            vec![holding(Market::Kr, "OTHER", "1", at(61))],
        ] {
            let values = portfolio(holdings);
            let context = SignalContext::new(
                Some(&values),
                &policy,
                (&mapping, &types),
                &ledger,
                assessment_facts(at(61), &calendar),
            );
            assert_eq!(
                SignalDecision::assess(&estimate, &context),
                SignalDecision::None(NoSignalReason::UnheldLowProbability)
            );
        }
        let context = SignalContext::new(
            None,
            &policy,
            (&mapping, &types),
            &ledger,
            assessment_facts(at(61), &calendar),
        );
        assert_eq!(
            SignalDecision::assess(&estimate, &context).into_signal(),
            None
        );
    }

    fn alternate_task() -> CostEligibleTask {
        let original = eligible();
        let task = original.task();
        let reference = PriceObservation::from_identified_quote_parts(
            task.volatility().instrument().clone(),
            instrument(Market::Kr, "OTHER"),
            task.reference().price(),
            task.reference().observed_at(),
        )
        .expect("관측");
        eligible_for(
            EstimateTask::create(
                EstimateContext::new(reference, task.created_at()),
                task.volatility().clone(),
                Horizon::new(HorizonSelection::FewDays),
                HoursPermission::default(),
                RiskAppetite::Neutral,
            )
            .expect("과제"),
        )
    }

    #[test]
    fn submission_binds_task_probability_model_evidence_and_time() {
        let eligible = eligible();
        let other = alternate_task();
        assert_ne!(
            SignalTaskKey::from_task(&eligible),
            SignalTaskKey::from_task(&other)
        );
        assert_eq!(
            SignalTaskKey::from_task(&eligible),
            SignalTaskKey::from_task(&eligible)
        );
        let submitted = submission(&other, 99, evidence(1));
        let bound = calibrated(submitted.clone(), 0);
        assert_eq!(bound.submission(), &submitted);
        assert_eq!(submitted.probability(), ai(99));
        assert_eq!(submitted.calibration_key(), &key());
        assert_eq!(submitted.submitted_at(), other.task().created_at());
        assert_eq!(
            SignalEstimate::create(&eligible, &bound, Margin::default()),
            Err(SignalPreparationError::SubmissionMismatch)
        );
        let before = eligible
            .task()
            .created_at()
            .checked_sub(TimeSpan::NANOSECOND)
            .expect("직전");
        assert_eq!(
            SignalSubmission::create(
                SignalTaskKey::from_task(&eligible),
                key(),
                ai(99),
                evidence(1),
                before
            ),
            Err(SignalSubmissionError::BeforeTask)
        );
        let bad_key = CalibrationKey::new(key().model().clone(), Horizon::default());
        let wrong_horizon = SignalSubmission::create(
            SignalTaskKey::from_task(&eligible),
            bad_key,
            ai(99),
            evidence(1),
            at(61),
        )
        .expect("제출");
        assert_eq!(
            SignalEstimate::create(
                &eligible,
                &calibrated_for_bad(wrong_horizon),
                Margin::default()
            ),
            Err(SignalPreparationError::Thresholds(
                crate::NoThresholds::HorizonMismatch
            ))
        );
    }
    fn calibrated_for_bad(submission: SignalSubmission) -> SubmittedCalibration {
        calibrated(submission, 0)
    }

    #[test]
    fn expiry_and_calendar_absence_are_common_signal_gates() {
        let eligible = eligible();
        let calendar = calendars();
        let endpoint = later(at(66), 3600);
        assert_eq!(SignalTiming::new(at(61), &calendar).now(), at(61));
        assert_eq!(
            format!("{:?}", SignalTiming::new(at(61), &calendar)),
            "SignalTiming(..)"
        );
        assert_eq!(
            crate::estimate_outcome::signal_task_interval(eligible.task(), &calendar)
                .expect("만기")
                .end(),
            endpoint
        );
        let rows = [item()];
        let ledger = SignalSourceLedger::new(&rows, &[], &[]);
        let mapping = assignments();
        let types = facts();
        let values = portfolio(vec![holding(Market::Kr, "ABC", "1", at(61))]);
        let policy = policy(0);
        for percent in [1, 99] {
            let estimate = estimate(&eligible, percent, 0, 10, 1);
            let context = SignalContext::new(
                Some(&values),
                &policy,
                (&mapping, &types),
                &ledger,
                assessment_facts(endpoint, &calendar),
            );
            assert_eq!(
                SignalDecision::assess(&estimate, &context),
                SignalDecision::None(NoSignalReason::Expired)
            );
            let missing = SignalContext::new(
                Some(&values),
                &policy,
                (&mapping, &types),
                &ledger,
                assessment_facts(at(61), &[]),
            );
            assert_eq!(
                SignalDecision::assess(&estimate, &missing),
                SignalDecision::None(NoSignalReason::Unavailable(SignalUnavailable::Calendar))
            );
        }
        let estimate = estimate(&eligible, 99, 0, 10, 1);
        let before = endpoint.checked_sub(TimeSpan::NANOSECOND).expect("직전");
        let context = SignalContext::new(
            Some(&values),
            &policy,
            (&mapping, &types),
            &ledger,
            assessment_facts(before, &calendar),
        );
        assert_eq!(classify(&SignalDecision::assess(&estimate, &context)), 1);
    }

    #[test]
    fn sell_holding_age_checks_task_creation_future_and_sixty_second_boundary() {
        let eligible = eligible();
        let calendar = calendars();
        let estimate = estimate(&eligible, 1, 0, 10, 1);
        let rows = [item()];
        let ledger = SignalSourceLedger::new(&rows, &[], &[]);
        let mapping = assignments();
        let types = facts();
        let policy = policy(0);
        let boundary = later(at(61), 60);
        let before_task = at(61).checked_sub(TimeSpan::NANOSECOND).expect("이전");
        for (observed, now, expected) in [
            (at(61), boundary, 2),
            (later(at(61), 1), later(at(61), 2), 2),
            (
                at(61),
                boundary.checked_add(TimeSpan::NANOSECOND).expect("직후"),
                0,
            ),
            (before_task, at(61), 0),
            (later(at(61), 1), at(61), 0),
        ] {
            let values = portfolio(vec![holding(Market::Kr, "ABC", "1", observed)]);
            let context = SignalContext::new(
                Some(&values),
                &policy,
                (&mapping, &types),
                &ledger,
                assessment_facts(now, &calendar),
            );
            let decision = SignalDecision::assess(&estimate, &context);
            assert_eq!(classify(&decision), expected);
            if let SignalDecision::Sell(signal) = decision {
                assert_eq!(signal.observed_at(), observed);
            }
        }
    }

    #[test]
    fn a_submission_from_the_future_cannot_raise_a_signal() {
        let eligible = eligible();
        let calendar = calendars();
        let rows = [item()];
        let ledger = SignalSourceLedger::new(&rows, &[], &[]);
        let mapping = assignments();
        let types = facts();
        let policy = policy(0);
        let future = SignalSubmission::create(
            SignalTaskKey::from_task(&eligible),
            key(),
            ai(99),
            evidence(1),
            later(at(61), 1),
        )
        .expect("제출");
        let future_input =
            SignalEstimate::create(&eligible, &calibrated(future, 0), Margin::default())
                .expect("입력");
        let values = portfolio(vec![]);
        let context = SignalContext::new(
            Some(&values),
            &policy,
            (&mapping, &types),
            &ledger,
            assessment_facts(at(61), &calendar),
        );
        assert_eq!(
            SignalDecision::assess(&future_input, &context),
            SignalDecision::None(NoSignalReason::Unavailable(
                SignalUnavailable::SubmissionFromFuture
            ))
        );
    }

    fn decide_sources(evidence: Evidence, ledger: &SignalSourceLedger<'_>) -> SignalDecision {
        let eligible = eligible();
        let input = SignalEstimate::create(
            &eligible,
            &calibrated(submission(&eligible, 99, Some(evidence)), 0),
            Margin::default(),
        )
        .expect("입력");
        let values = portfolio(vec![]);
        let mapping = assignments();
        let types = facts();
        let policy = policy(0);
        let calendar = calendars();
        let context = SignalContext::new(
            Some(&values),
            &policy,
            (&mapping, &types),
            ledger,
            assessment_facts(at(61), &calendar),
        );
        SignalDecision::assess(&input, &context)
    }

    fn reference(values: (i64, i64, usize), axes: (u8, u8, u8), base: Decimal) -> u8 {
        let (percent, margin, count) = values;
        let (held, excluded, source) = axes;
        let buy_margin = if count < 30 { margin.max(15) } else { margin };
        let sell_margin = if count < 30 { 15 } else { margin };
        if source != 1 {
            0
        } else if Decimal::new(percent, 2)
            <= base
                .checked_sub(Decimal::new(sell_margin, 2))
                .expect("매도")
                .max(Decimal::new(5, 2))
        {
            if held == 2 { 2 } else { 0 }
        } else {
            u8::from(
                excluded == 0
                    && Decimal::new(percent, 2)
                        >= base.checked_add(Decimal::new(buy_margin, 2)).expect("매수"),
            )
        }
    }

    fn assert_estimate_contract(
        estimate: &SignalEstimate,
        eligible: &CostEligibleTask,
        values: (i64, usize),
    ) {
        let (percent, count) = values;
        let operation = if count < 30 {
            SignalOperation::Conservative
        } else {
            SignalOperation::EligibleBeforeE91
        };
        assert_eq!(estimate.operation(), operation);
        assert_eq!(estimate.task(), eligible.task());
        assert_eq!(estimate.submission().probability(), ai(percent));
        assert_eq!(
            estimate.display_probability().expect("표시").as_str(),
            format!("≈{percent}.0%")
        );
        let calibration = estimate.calibration();
        assert!(
            !(estimate
                .thresholds()
                .buy_condition(calibration)
                .expect("비교")
                == crate::SignalCondition::Met
                && estimate
                    .thresholds()
                    .sell_condition(calibration)
                    .expect("비교")
                    == crate::SignalCondition::Met)
        );
    }
    #[test]
    fn independent_reference_covers_new_sell_priority_and_submission_readiness() {
        let eligible = eligible();
        let base = eligible.task().base_probability().as_decimal();
        let rows = [item()];
        let ledger = SignalSourceLedger::new(&rows, &[], &[]);
        let mapping = assignments();
        let types = facts();
        let calendar = calendars();
        TestRunner::new(Config::with_cases(testkit::property_cases()))
            .run(
                &(
                    1_i64..=99,
                    5_i64..=25,
                    0_u8..3,
                    0_u8..3,
                    0_u8..4,
                    0_usize..2,
                ),
                |(percent, margin, held, excluded, source, ready)| {
                    let count = ready * 30;
                    let estimate = estimate(&eligible, percent, count, margin, source);
                    let holdings = match held {
                        0 => vec![],
                        1 => vec![holding(Market::Kr, "ABC", "0", at(61))],
                        _ => vec![holding(Market::Kr, "ABC", "0.25", at(61))],
                    };
                    let portfolio = portfolio(holdings);
                    let policy = policy(excluded);
                    let context = SignalContext::new(
                        Some(&portfolio),
                        &policy,
                        (&mapping, &types),
                        &ledger,
                        assessment_facts(at(61), &calendar),
                    );
                    let decision = SignalDecision::assess(&estimate, &context);
                    proptest::prop_assert_eq!(
                        classify(&decision),
                        reference((percent, margin, count), (held, excluded, source), base)
                    );
                    assert_estimate_contract(&estimate, &eligible, (percent, count));
                    if source != 1 {
                        proptest::prop_assert_eq!(decision.into_signal(), None);
                    }
                    Ok(())
                },
            )
            .expect("독립 기준4096");
    }

    #[test]
    fn unavailable_and_missing_sources_never_produce_payloads() {
        let eligible = eligible();
        let rows = [item()];
        let ledger = SignalSourceLedger::new(&rows, &[], &[]);
        let mapping = assignments();
        let types = facts();
        let calendar = calendars();
        let values = portfolio(vec![holding(Market::Kr, "ABC", "1", at(61))]);
        let policy = policy(0);
        TestRunner::new(Config::with_cases(testkit::property_cases()))
            .run(&(1_i64..=99, 0_u8..3), |(percent, condition)| {
                let source = u8::from(condition != 0);
                let input = estimate(&eligible, percent, 0, 10, source);
                let now = if condition == 2 {
                    later(at(66), 3600)
                } else {
                    at(61)
                };
                let calendars = if condition == 1 {
                    &[][..]
                } else {
                    calendar.as_slice()
                };
                let context = SignalContext::new(
                    Some(&values),
                    &policy,
                    (&mapping, &types),
                    &ledger,
                    assessment_facts(now, calendars),
                );
                proptest::prop_assert_eq!(
                    SignalDecision::assess(&input, &context).into_signal(),
                    None
                );
                Ok(())
            })
            .expect("판정불가4096");
    }

    #[test]
    fn probability_display_uses_nearest_rounding_instead_of_threshold_ceiling() {
        for (value, expected) in [
            ("0.64149", "≈64.1%"),
            ("0.6415", "≈64.2%"),
            ("0.6405", "≈64.1%"),
            ("0", "≈0.0%"),
            ("1", "≈100.0%"),
        ] {
            let probability = crate::Probability::try_new(
                Ratio::parse(&NumericInput::try_new(value).expect("입력"))
                    .expect("비율")
                    .as_decimal(),
            )
            .expect("확률");
            assert_eq!(
                crate::thresholds_arithmetic::signal_probability_display(
                    crate::thresholds_arithmetic::ThresholdNumber::new(probability.as_decimal())
                )
                .expect("표시")
                .as_str(),
                expected
            );
        }
        assert_eq!(
            crate::thresholds_arithmetic::signal_probability_display(
                crate::thresholds_arithmetic::ThresholdNumber::new(Decimal::MAX)
            ),
            Err(crate::NoThresholds::NumericalDomain)
        );
    }

    #[test]
    fn zero_headroom_and_unresolved_principles_block_buys() {
        let eligible = eligible();
        let input = estimate(&eligible, 99, 0, 10, 1);
        let cap = crate::ProfileWeight::parse_percent(&NumericInput::try_new("5").expect("입력"))
            .expect("상한");
        let tight = Policy::new(InvestorProfile::default().with_weight_caps(
            WeightCaps::try_new(cap, cap, WeightCaps::default().sector_cap()).expect("관계"),
        ));
        let unresolved = policy(0).with_unresolved_principle(
            PolicyPrinciple::from_missing_contract(PolicyText::try_new("미확정").expect("원칙")),
        );
        let values = portfolio(vec![holding(Market::Kr, "ABC", "1", at(61))]);
        let rows = [item()];
        let ledger = SignalSourceLedger::new(&rows, &[], &[]);
        let mapping = assignments();
        let types = facts();
        let calendar = calendars();
        for policy in [tight, unresolved] {
            let context = SignalContext::new(
                Some(&values),
                &policy,
                (&mapping, &types),
                &ledger,
                assessment_facts(at(61), &calendar),
            );
            assert_eq!(
                SignalDecision::assess(&input, &context),
                SignalDecision::None(NoSignalReason::PolicyViolation)
            );
        }
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
    fn cash_network_failure() -> crate::CashObservationFailure {
        crate::CashObservationFailure::Unavailable {
            cause: crate::CashUnavailableCause::Network,
            retry: crate::CashRetryDelay::Known(
                TimeSpan::try_new(SpanSeconds::new(7), Nanosecond::new(0)).expect("재시도"),
            ),
        }
    }
    #[test]
    fn cash_failures_remain_distinct_through_allocation_policy_and_signal() {
        let network = cash_network_failure();
        let maintenance = crate::CashObservationFailure::Unavailable {
            cause: crate::CashUnavailableCause::Maintenance,
            retry: crate::CashRetryDelay::Unknown,
        };
        let fail = |currency, reason| crate::CashObservation::Failed {
            currency,
            reason,
            observed_at: at(61),
        };
        let krw = crate::CashObservation::Observed(BuyingPower::new(money("100"), at(61)));
        let usd = crate::CashObservation::Observed(BuyingPower::new(
            Money::new(money("0").amount(), Currency::Usd),
            at(61),
        ));
        let negative = crate::CashObservation::Observed(BuyingPower::new(
            Money::new(money("-1").amount(), Currency::Usd),
            at(61),
        ));
        let usd_error = crate::CashObservationFailures::Usd(maintenance);
        let krw_error = crate::CashObservationFailures::Krw(network);
        let both = crate::CashObservationFailures::Both {
            krw: network,
            usd: maintenance,
        };
        for (krw_cash, usd_cash, allocation_error, signal_error) in [
            (
                krw,
                fail(Currency::Usd, maintenance),
                crate::AllocationError::CashObservationFailed(usd_error),
                SignalUnavailable::CashObservationFailed(usd_error),
            ),
            (
                fail(Currency::Krw, network),
                usd,
                crate::AllocationError::CashObservationFailed(krw_error),
                SignalUnavailable::CashObservationFailed(krw_error),
            ),
            (
                fail(Currency::Krw, network),
                fail(Currency::Usd, maintenance),
                crate::AllocationError::CashObservationFailed(both),
                SignalUnavailable::CashObservationFailed(both),
            ),
            (
                krw,
                negative,
                crate::AllocationError::NegativeCash(Currency::Usd),
                SignalUnavailable::NegativeCash(Currency::Usd),
            ),
        ] {
            let value = crate::portfolio_from_cash_observations(
                vec![holding(Market::Kr, "ABC", "1", at(61))],
                krw_cash,
                usd_cash,
            )
            .expect("포트")
            .evaluate(None);
            assert_cash_consumer_results(&value, allocation_error, signal_error);
        }
    }
    fn assert_cash_consumer_results(
        value: &PortfolioValuation,
        allocation_error: crate::AllocationError,
        signal_error: SignalUnavailable,
    ) {
        let mapping = assignments();
        let types = facts();
        let policy = policy(0);
        let allocation = crate::Allocation::new(value, &mapping);
        assert_eq!(allocation.assets().instruments(), &Err(allocation_error));
        assert!(allocation.holdings().instruments().is_ok());
        assert_eq!(
            value.currency_total(Currency::Krw).completeness(),
            crate::TotalCompleteness::Complete
        );
        assert_eq!(
            value.assets().completeness(),
            crate::TotalCompleteness::Incomplete
        );
        assert_eq!(
            policy.check_current(value, &mapping, &types),
            Err(crate::PolicyViolation::Allocation(allocation_error))
        );
        let eligible = eligible();
        let estimate = estimate(&eligible, 99, 0, 10, 1);
        let candidate =
            crate::PolicyCandidate::new(estimate.task().reference().instrument(), money("1"))
                .expect("후보");
        assert_eq!(
            policy.check_buy(value, &mapping, &types, &candidate),
            crate::PolicyBuyDecision::Block(crate::PolicyViolation::Allocation(allocation_error))
        );
        let items = [item()];
        let ledger = SignalSourceLedger::new(&items, &[], &[]);
        let calendar = calendars();
        let context = SignalContext::new(
            Some(value),
            &policy,
            (&mapping, &types),
            &ledger,
            assessment_facts(at(61), &calendar),
        );
        assert_eq!(
            SignalDecision::assess(&estimate, &context),
            SignalDecision::None(NoSignalReason::Unavailable(signal_error))
        );
    }
    #[test]
    fn negative_krw_does_not_hide_failed_usd_lookup() {
        let reason = cash_network_failure();
        let failures = crate::CashObservationFailures::Usd(reason);
        let value = crate::portfolio_from_cash_observations(
            vec![holding(Market::Kr, "ABC", "1", at(61))],
            crate::CashObservation::Observed(BuyingPower::new(money("-1"), at(61))),
            crate::CashObservation::Failed {
                currency: Currency::Usd,
                reason,
                observed_at: at(61),
            },
        )
        .expect("포트")
        .evaluate(None);
        assert_cash_consumer_results(
            &value,
            crate::AllocationError::CashObservationFailed(failures),
            SignalUnavailable::CashObservationFailed(failures),
        );
    }
}
