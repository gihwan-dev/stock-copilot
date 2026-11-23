use crate::{
    AccountId, CheckResult, Holding, InstrumentId, MarketCalendar, NoSignalReason,
    PortfolioValuation, Quantity, ResolvedOrderInstrument, SellSignal, SellableQuantityFact,
    SignalTiming, Timestamp, ValuedHolding, WholeOrderQuantity,
};

#[must_use]
pub fn sell_signal_matches_reassessed(stored: &SellSignal, reassessed: &SellSignal) -> CheckResult {
    if reassessed.facts() == stored.facts()
        && reassessed.held_quantity() == stored.held_quantity()
        && reassessed.observed_at() >= stored.observed_at()
    {
        CheckResult::Passed
    } else {
        CheckResult::Failed
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoSellDraftQuantity {
    NotHeld,
    StaleHolding,
    HoldingChanged,
    SellableQuantityUnknown,
    ZeroSellableQuantity,
    StaleSellableQuantity,
    DifferentFact,
    FractionalQuantityUnsupported,
    SignalUnavailable(NoSignalReason),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SellQuantityConstraint {
    FullHolding,
    SellableQuantityCap,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SellQuantityAdjustment {
    Default,
    Reduced,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SellDraftQuantityAdjustmentError {
    BelowMinimumUnit,
    InvalidQuantityUnit,
    ExceedsAllowedQuantity,
}

pub struct SellDraftQuantityFacts<'facts> {
    account: &'facts AccountId,
    valuation: &'facts PortfolioValuation,
    sellable: Option<&'facts SellableQuantityFact>,
}

impl std::fmt::Debug for SellDraftQuantityFacts<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SellDraftQuantityFacts(..)")
    }
}

impl<'facts> SellDraftQuantityFacts<'facts> {
    #[must_use]
    pub const fn new(
        account: &'facts AccountId,
        valuation: &'facts PortfolioValuation,
        sellable: Option<&'facts SellableQuantityFact>,
    ) -> Self {
        Self {
            account,
            valuation,
            sellable,
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct SellDraftQuantityPlan {
    account: AccountId,
    signal: SellSignal,
    held_quantity: WholeOrderQuantity,
    sellable_quantity: WholeOrderQuantity,
    default_quantity: WholeOrderQuantity,
    quantity: WholeOrderQuantity,
    locked_quantity: Quantity,
    constraint: SellQuantityConstraint,
    adjustment: SellQuantityAdjustment,
    holding_observed_at: Timestamp,
    sellable_observed_at: Timestamp,
    min_sellable_observed_at: Timestamp,
}

impl std::fmt::Debug for SellDraftQuantityPlan {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SellDraftQuantityPlan(..)")
    }
}

impl SellDraftQuantityPlan {
    #[must_use]
    pub const fn account_id(&self) -> &AccountId {
        &self.account
    }

    #[must_use]
    pub const fn signal(&self) -> &SellSignal {
        &self.signal
    }

    #[must_use]
    pub fn instrument(&self) -> &ResolvedOrderInstrument {
        self.signal.facts().task().reference().instrument()
    }

    #[must_use]
    pub fn instrument_id(&self) -> &InstrumentId {
        self.signal.facts().task().volatility().instrument()
    }

    #[must_use]
    pub const fn held_quantity(&self) -> WholeOrderQuantity {
        self.held_quantity
    }

    #[must_use]
    pub const fn sellable_quantity(&self) -> WholeOrderQuantity {
        self.sellable_quantity
    }

    #[must_use]
    pub const fn default_quantity(&self) -> WholeOrderQuantity {
        self.default_quantity
    }

    #[must_use]
    pub const fn quantity(&self) -> WholeOrderQuantity {
        self.quantity
    }

    #[must_use]
    pub const fn locked_quantity(&self) -> Quantity {
        self.locked_quantity
    }

    #[must_use]
    pub const fn constraint(&self) -> SellQuantityConstraint {
        self.constraint
    }

    #[must_use]
    pub const fn adjustment(&self) -> SellQuantityAdjustment {
        self.adjustment
    }

    #[must_use]
    pub const fn holding_observed_at(&self) -> Timestamp {
        self.holding_observed_at
    }

    #[must_use]
    pub const fn sellable_observed_at(&self) -> Timestamp {
        self.sellable_observed_at
    }

    pub fn reduce_quantity(
        &self,
        requested: Quantity,
    ) -> Result<Self, SellDraftQuantityAdjustmentError> {
        if requested.as_decimal().normalize().scale() != 0 {
            return Err(SellDraftQuantityAdjustmentError::InvalidQuantityUnit);
        }
        let whole = WholeOrderQuantity::try_new(requested)
            .map_err(|_error| SellDraftQuantityAdjustmentError::BelowMinimumUnit)?;
        if whole.quantity() > self.default_quantity.quantity() {
            return Err(SellDraftQuantityAdjustmentError::ExceedsAllowedQuantity);
        }
        let adjustment = if whole.quantity() < self.default_quantity.quantity() {
            SellQuantityAdjustment::Reduced
        } else {
            SellQuantityAdjustment::Default
        };
        Ok(Self {
            account: self.account.clone(),
            signal: self.signal.clone(),
            held_quantity: self.held_quantity,
            sellable_quantity: self.sellable_quantity,
            default_quantity: self.default_quantity,
            quantity: whole,
            locked_quantity: self.locked_quantity,
            constraint: self.constraint,
            adjustment,
            holding_observed_at: self.holding_observed_at,
            sellable_observed_at: self.sellable_observed_at,
            min_sellable_observed_at: self.min_sellable_observed_at,
        })
    }

    pub fn check_facts_at(
        &self,
        now: Timestamp,
        calendars: &[MarketCalendar],
    ) -> Result<(), NoSellDraftQuantity> {
        let task = self.signal.facts().task();
        let timing = SignalTiming::new(now, calendars);
        timing
            .check(task, self.signal.facts().submission().submitted_at())
            .map_err(NoSellDraftQuantity::SignalUnavailable)?;
        if timing
            .check_holding(task, self.holding_observed_at)
            .is_err()
        {
            return Err(NoSellDraftQuantity::StaleHolding);
        }
        check_sellable_freshness(
            self.min_sellable_observed_at,
            self.sellable_observed_at,
            now,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SellDraftQuantityDecision {
    Plan(Box<SellDraftQuantityPlan>),
    None(NoSellDraftQuantity),
}

impl SellDraftQuantityDecision {
    #[must_use]
    pub fn into_plan(self) -> Option<SellDraftQuantityPlan> {
        match self {
            Self::Plan(plan) => Some(*plan),
            Self::None(_) => None,
        }
    }
}

#[must_use]
pub fn calculate_sell_draft_quantity(
    signal: &SellSignal,
    facts: &SellDraftQuantityFacts<'_>,
    now: Timestamp,
    calendars: &[MarketCalendar],
) -> SellDraftQuantityDecision {
    match calculate(signal, signal.observed_at(), facts, now, calendars) {
        Ok(plan) => SellDraftQuantityDecision::Plan(Box::new(plan)),
        Err(reason) => SellDraftQuantityDecision::None(reason),
    }
}

#[must_use]
pub fn calculate_sell_draft_quantity_with_history(
    live: &SellSignal,
    stored: &crate::HistoricalSignal,
    facts: &SellDraftQuantityFacts<'_>,
    now: Timestamp,
    calendars: &[MarketCalendar],
) -> SellDraftQuantityDecision {
    let Some(stored_sell) = stored.sell_signal() else {
        return SellDraftQuantityDecision::None(NoSellDraftQuantity::DifferentFact);
    };
    if stored_sell.facts() != live.facts() {
        return SellDraftQuantityDecision::None(NoSellDraftQuantity::DifferentFact);
    }
    if stored_sell.held_quantity() != live.held_quantity() {
        return SellDraftQuantityDecision::None(NoSellDraftQuantity::HoldingChanged);
    }
    if live.observed_at() < stored_sell.observed_at() {
        return SellDraftQuantityDecision::None(NoSellDraftQuantity::StaleHolding);
    }
    match calculate(live, stored_sell.observed_at(), facts, now, calendars) {
        Ok(plan) => SellDraftQuantityDecision::Plan(Box::new(plan)),
        Err(reason) => SellDraftQuantityDecision::None(reason),
    }
}

fn calculate(
    signal: &SellSignal,
    min_sellable_observed_at: Timestamp,
    facts: &SellDraftQuantityFacts<'_>,
    now: Timestamp,
    calendars: &[MarketCalendar],
) -> Result<SellDraftQuantityPlan, NoSellDraftQuantity> {
    let task = signal.facts().task();
    let timing = SignalTiming::new(now, calendars);
    timing
        .check(task, signal.facts().submission().submitted_at())
        .map_err(NoSellDraftQuantity::SignalUnavailable)?;
    let holding = verified_holding(signal, facts.valuation, timing)?;
    let sellable = verified_sellable(signal, min_sellable_observed_at, facts, now)?;
    let held_quantity = WholeOrderQuantity::try_new(holding.quantity())
        .map_err(|_error| NoSellDraftQuantity::FractionalQuantityUnsupported)?;
    let sellable_quantity = WholeOrderQuantity::try_new(sellable.quantity())
        .map_err(|_error| NoSellDraftQuantity::FractionalQuantityUnsupported)?;
    let (default_quantity, constraint) = if sellable_quantity.quantity() < held_quantity.quantity()
    {
        (
            sellable_quantity,
            SellQuantityConstraint::SellableQuantityCap,
        )
    } else {
        (held_quantity, SellQuantityConstraint::FullHolding)
    };
    let locked_quantity = crate::quantity::whole_excess(held_quantity, sellable_quantity);
    Ok(SellDraftQuantityPlan {
        account: facts.account.clone(),
        signal: signal.clone(),
        held_quantity,
        sellable_quantity,
        default_quantity,
        quantity: default_quantity,
        locked_quantity,
        constraint,
        adjustment: SellQuantityAdjustment::Default,
        holding_observed_at: holding.observed_at(),
        sellable_observed_at: sellable.observed_at(),
        min_sellable_observed_at,
    })
}

fn verified_holding<'facts>(
    signal: &SellSignal,
    valuation: &'facts PortfolioValuation,
    timing: SignalTiming<'_>,
) -> Result<&'facts Holding, NoSellDraftQuantity> {
    let task = signal.facts().task();
    let instrument = task.reference().instrument();
    let Some(holding) = valuation
        .holdings()
        .iter()
        .map(ValuedHolding::holding)
        .find(|item| item.instrument() == instrument)
    else {
        return Err(NoSellDraftQuantity::NotHeld);
    };
    if holding.quantity().as_decimal().is_zero() {
        return Err(NoSellDraftQuantity::NotHeld);
    }
    if timing.check_holding(task, holding.observed_at()).is_err()
        || holding.observed_at() < signal.observed_at()
    {
        return Err(NoSellDraftQuantity::StaleHolding);
    }
    if holding.quantity() != signal.held_quantity() {
        return Err(NoSellDraftQuantity::HoldingChanged);
    }
    Ok(holding)
}

fn verified_sellable<'facts>(
    signal: &SellSignal,
    min_observed_at: Timestamp,
    facts: &'facts SellDraftQuantityFacts<'_>,
    now: Timestamp,
) -> Result<&'facts SellableQuantityFact, NoSellDraftQuantity> {
    let Some(sellable) = facts.sellable else {
        return Err(NoSellDraftQuantity::SellableQuantityUnknown);
    };
    let instrument = signal.facts().task().reference().instrument();
    if sellable.account_id() != facts.account || sellable.instrument() != instrument {
        return Err(NoSellDraftQuantity::DifferentFact);
    }
    check_sellable_freshness(min_observed_at, sellable.observed_at(), now)?;
    if sellable.quantity().as_decimal().is_zero() {
        return Err(NoSellDraftQuantity::ZeroSellableQuantity);
    }
    Ok(sellable)
}

fn check_sellable_freshness(
    min_observed_at: Timestamp,
    observed_at: Timestamp,
    now: Timestamp,
) -> Result<(), NoSellDraftQuantity> {
    let elapsed = observed_at
        .elapsed_until(now)
        .or(Err(NoSellDraftQuantity::StaleSellableQuantity))?;
    if observed_at < min_observed_at || elapsed > crate::domain_time::SELLABLE_QUANTITY_MAX_AGE {
        return Err(NoSellDraftQuantity::StaleSellableQuantity);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        NoSellDraftQuantity, SellDraftQuantityAdjustmentError, SellDraftQuantityDecision,
        SellDraftQuantityFacts, SellDraftQuantityPlan, SellQuantityAdjustment,
        SellQuantityConstraint, calculate_sell_draft_quantity,
        calculate_sell_draft_quantity_with_history, sell_signal_matches_reassessed,
    };
    use crate::{
        AccountId, AcquisitionPrice, AiProbability, AiProbabilityInput, AllocationInstrument,
        Amount, BuyingPower, Calibration, CalibrationKey, CalibrationLedgerHead, CalibrationModel,
        CalibrationPosition, CalibrationSnapshot, CheckResult, CollectedItem, CommissionRateFact,
        CommissionValidity, CostEligibleTask, CostRate, Currency, EstimateContext,
        EstimateCostPeriod, EstimateTask, Evidence, EvidenceRef, EvidenceUrl, ExcludedSecurityType,
        FxPair, FxRate, FxRateKind, FxSource, Holding, HoldingObservation, Horizon,
        HorizonSelection, HoursPermission, InstrumentClassification, InvestorProfile, Margin,
        Market, MarketCalendar, MarketSession, ModelIdentityPart, Money, Nanosecond,
        NoSignalReason, NormalizedUrl, NumericInput, OffsetSeconds, Policy, PolicySecurityFacts,
        Portfolio, PortfolioValuation, Price, PriceObservation, Quantity, Ratio,
        ResolvedOrderInstrument, RiskAppetite, SectorAssignment, SectorAssignments, SellSignal,
        SellableQuantityFact, SessionData, SessionInformation, SignalAssessmentFacts,
        SignalContext, SignalDecision, SignalEstimate, SignalSourceLedger, SignalSubmission,
        SignalTaskKey, SignalTiming, SignalUnavailable, SpanSeconds, Symbol, TimeInterval,
        TimeSpan, Timestamp, TradeCost, TradeTaxFact, UntrustedText, Volatility,
        WholeOrderQuantity, calibrate_signal_submission, candle,
    };
    use proptest::test_runner::{Config, TestRunner};
    use rust_decimal::Decimal;

    fn at(day: i64, market: &Market) -> Timestamp {
        let point = candle::tests::at(day);
        Timestamp::from_parts(
            point.utc_seconds(),
            point.nanosecond(),
            OffsetSeconds::new(if market == &Market::Kr {
                32_400
            } else {
                -18_000
            }),
        )
        .expect("시각")
    }

    fn later(point: Timestamp, seconds: u64, nanos: u32) -> Timestamp {
        point
            .checked_add(
                TimeSpan::try_new(SpanSeconds::new(seconds), Nanosecond::new(nanos)).expect("기간"),
            )
            .expect("이후")
    }

    fn earlier(point: Timestamp, seconds: u64, nanos: u32) -> Timestamp {
        point
            .checked_sub(
                TimeSpan::try_new(SpanSeconds::new(seconds), Nanosecond::new(nanos)).expect("기간"),
            )
            .expect("이전")
    }

    fn qty(text: &str) -> Quantity {
        Quantity::parse(&NumericInput::try_new(text).expect("입력")).expect("수량")
    }

    fn whole(text: &str) -> WholeOrderQuantity {
        WholeOrderQuantity::try_new(qty(text)).expect("정수 수량")
    }

    fn money(value: i64, currency: Currency) -> Money {
        Money::new(Amount::new(Decimal::from(value)), currency)
    }

    fn currency_for(market: &Market) -> Currency {
        if market == &Market::Kr {
            Currency::Krw
        } else {
            Currency::Usd
        }
    }

    fn calendars_for(market: &Market) -> Vec<MarketCalendar> {
        let row = |day| {
            let start = at(day, market);
            let regular_market = Some(
                SessionData::try_new(
                    TimeInterval::try_new(start, later(start, 3600, 0)).expect("장 구간"),
                    None,
                )
                .expect("장"),
            );
            let information = if market == &Market::Kr {
                SessionInformation::Kr {
                    pre_market: None,
                    regular_market,
                    after_market: None,
                }
            } else {
                SessionInformation::Us {
                    day_market: None,
                    pre_market: None,
                    regular_market,
                    after_market: None,
                }
            };
            MarketSession::try_new(start.offset_calendar_date(), information).expect("세션")
        };
        (61_i64..=75)
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

    fn instrument_for(symbol: &str, market: &Market) -> ResolvedOrderInstrument {
        ResolvedOrderInstrument::from_master_lookup_result(
            Symbol::try_new(symbol).expect("심볼"),
            market.clone(),
        )
        .expect("종목")
    }

    fn valuation_with(
        market: &Market,
        holdings: Vec<Holding>,
        now: Timestamp,
    ) -> PortfolioValuation {
        let mid = FxRate::parse(
            &NumericInput::try_new("1300").expect("환율"),
            FxPair::UsdKrw,
            FxRateKind::Mid,
            TimeInterval::try_new(at(60, market), at(70, market)).expect("구간"),
            FxSource::TossOpenApi,
        )
        .expect("환율");
        Portfolio::new(
            holdings,
            BuyingPower::new(money(10_000, Currency::Krw), now),
            BuyingPower::new(money(1_000, Currency::Usd), now),
        )
        .expect("포트")
        .evaluate(Some(mid))
    }

    fn holding_for(
        instrument: &ResolvedOrderInstrument,
        quantity: Quantity,
        observed_at: Timestamp,
    ) -> Holding {
        let currency = currency_for(&instrument.market());
        Holding::new(
            instrument.clone(),
            quantity,
            AcquisitionPrice::try_new(money(100, currency)).expect("매입가"),
            HoldingObservation::new(
                Some(Price::try_new(money(100, currency)).expect("현재가")),
                None,
                observed_at,
            ),
        )
        .expect("보유")
    }

    struct Fixture {
        account: AccountId,
        market: Market,
        instrument: ResolvedOrderInstrument,
        signal: SellSignal,
        valuation: PortfolioValuation,
        calendars: Vec<MarketCalendar>,
        now: Timestamp,
    }

    impl Fixture {
        fn new(market: Market, held: Quantity) -> Self {
            let now = at(61, &market);
            let symbol = if market == Market::Kr {
                "005930"
            } else {
                "AAPL"
            };
            let instrument = instrument_for(symbol, &market);
            let calendars = calendars_for(&market);
            let holding = holding_for(&instrument, held, now);
            let valuation = valuation_with(&market, vec![holding], now);
            let signal = build_sell_signal(&instrument, &valuation, now, &calendars);
            Self {
                account: AccountId::new(4321),
                market,
                instrument,
                signal,
                valuation,
                calendars,
                now,
            }
        }

        fn sellable(&self, quantity: Quantity, observed_at: Timestamp) -> SellableQuantityFact {
            SellableQuantityFact::from_lookup_result(
                self.account.clone(),
                self.instrument.clone(),
                quantity,
                observed_at,
            )
            .expect("매도가능수량")
        }

        fn decide(
            &self,
            sellable: Option<&SellableQuantityFact>,
            now: Timestamp,
        ) -> SellDraftQuantityDecision {
            let facts = SellDraftQuantityFacts::new(&self.account, &self.valuation, sellable);
            calculate_sell_draft_quantity(&self.signal, &facts, now, &self.calendars)
        }
    }

    fn eligible_task(
        instrument: &ResolvedOrderInstrument,
        now: Timestamp,
        calendars: &[MarketCalendar],
    ) -> CostEligibleTask {
        let market = instrument.market();
        let currency = currency_for(&market);
        let series = (0_i32..61_i32)
            .map(|index| {
                if index.wrapping_rem(2) == 0_i32 {
                    "100"
                } else {
                    "102"
                }
            })
            .collect::<Vec<_>>();
        let volatility = Volatility::calculate(&candle::tests::series(&series)).expect("변동성");
        let reference = PriceObservation::from_identified_quote_parts(
            volatility.instrument().clone(),
            instrument.clone(),
            Price::try_new(money(100, currency)).expect("가격"),
            now,
        )
        .expect("기준");
        let task = EstimateTask::create(
            EstimateContext::new(reference, now),
            volatility,
            Horizon::new(HorizonSelection::FewDays),
            HoursPermission::default(),
            RiskAppetite::Neutral,
        )
        .expect("과제");
        let end = crate::estimate_outcome::signal_task_interval(&task, calendars)
            .expect("만기")
            .end();
        let zero = CostRate::try_new(Ratio::new(Decimal::ZERO)).expect("비용");
        let cost = TradeCost::from_facts(
            Some(CommissionRateFact::from_commissions_result(
                market.clone(),
                zero,
                CommissionValidity::try_new(None, None).expect("기간"),
            )),
            Some(TradeTaxFact::from_schedule_result(market, zero, zero)),
            EstimateCostPeriod::from_calendar_result(&task, end).expect("기간"),
        )
        .expect("비용");
        CostEligibleTask::assess(task, &cost).expect("적격")
    }

    fn sell_estimate(
        eligible: &CostEligibleTask,
        now: Timestamp,
    ) -> (SignalEstimate, CollectedItem, CalibrationLedgerHead) {
        sell_estimate_with_probability(eligible, now, "1")
    }

    fn sell_estimate_with_probability(
        eligible: &CostEligibleTask,
        now: Timestamp,
        probability: &str,
    ) -> (SignalEstimate, CollectedItem, CalibrationLedgerHead) {
        let part = ModelIdentityPart::try_new("d57").expect("모델");
        let key = CalibrationKey::new(
            CalibrationModel::new(part.clone(), part.clone(), part),
            Horizon::new(HorizonSelection::FewDays),
        );
        let item = CollectedItem::from_web(
            NormalizedUrl::try_new("https://example.com/d57").expect("URL"),
            UntrustedText::try_new("근거").expect("제목"),
            None,
        );
        let evidence = Some(
            Evidence::try_new(vec![EvidenceRef::Web(
                EvidenceUrl::try_new(item.url().clone()).expect("URL"),
            )])
            .expect("근거"),
        );
        let submission = SignalSubmission::create(
            SignalTaskKey::from_task(eligible),
            key.clone(),
            AiProbability::parse(&AiProbabilityInput::try_new(probability).expect("입력"))
                .expect("확률"),
            evidence,
            now,
        )
        .expect("제출");
        let head = CalibrationLedgerHead::new(key.clone(), CalibrationPosition::new(0));
        let snapshot =
            CalibrationSnapshot::try_new(Calibration::try_new(key, &[]).expect("표"), head.clone())
                .expect("스냅숏");
        let calibrated =
            calibrate_signal_submission(Some(&snapshot), &head, submission).expect("보정");
        let estimate = SignalEstimate::create(
            eligible,
            &calibrated,
            Margin::for_appetite(RiskAppetite::Neutral),
        )
        .expect("추정");
        (estimate, item, head)
    }

    fn build_sell_signal(
        instrument: &ResolvedOrderInstrument,
        valuation: &PortfolioValuation,
        now: Timestamp,
        calendars: &[MarketCalendar],
    ) -> SellSignal {
        build_sell_signal_at(instrument, valuation, (now, now), "1", calendars)
    }

    fn build_sell_signal_at(
        instrument: &ResolvedOrderInstrument,
        valuation: &PortfolioValuation,
        times: (Timestamp, Timestamp),
        probability: &str,
        calendars: &[MarketCalendar],
    ) -> SellSignal {
        let (task_at, assess_at) = times;
        let eligible = eligible_task(instrument, task_at, calendars);
        let (estimate, item, head) =
            sell_estimate_with_probability(&eligible, task_at, probability);
        let items = [item];
        let ledger = SignalSourceLedger::new(&items, &[], &[]);
        let policy = Policy::new(InvestorProfile::default());
        let assignments = SectorAssignments::try_new(vec![SectorAssignment::new(
            instrument,
            InstrumentClassification::EtfFund,
        )])
        .expect("분류");
        let security = PolicySecurityFacts::from_master_lookup_results(
            std::iter::once((
                AllocationInstrument::new(instrument),
                ExcludedSecurityType::Stock,
            ))
            .collect(),
        );
        let context = SignalContext::new(
            Some(valuation),
            &policy,
            (&assignments, &security),
            &ledger,
            SignalAssessmentFacts::new(
                SignalTiming::new(assess_at, calendars),
                Some(estimate.task().reference().clone()),
                Some(head),
            ),
        );
        sell_from_decision(SignalDecision::assess(&estimate, &context)).expect("매도 신호 필요")
    }

    fn sell_from_decision(decision: SignalDecision) -> Option<SellSignal> {
        match decision {
            SignalDecision::Sell(value) => Some(value),
            SignalDecision::Buy(_) | SignalDecision::None(_) => None,
        }
    }

    #[test]
    fn absent_signal_does_not_create_fixture_sell_payload() {
        assert_eq!(
            sell_from_decision(SignalDecision::None(NoSignalReason::ConditionsUnmet)),
            None
        );
    }

    #[test]
    fn sell_draft_plan_accessors_and_redacted_debug() {
        let fixture = Fixture::new(Market::Kr, qty("17"));
        let sellable_time = later(fixture.now, 5, 0);
        let calc_time = later(fixture.now, 10, 0);
        let sellable = fixture.sellable(qty("11"), sellable_time);
        let facts =
            SellDraftQuantityFacts::new(&fixture.account, &fixture.valuation, Some(&sellable));
        assert_eq!(format!("{facts:?}"), "SellDraftQuantityFacts(..)");
        let decision =
            calculate_sell_draft_quantity(&fixture.signal, &facts, calc_time, &fixture.calendars);
        let plan = decision.into_plan().expect("매도 계획");
        assert_eq!(format!("{plan:?}"), "SellDraftQuantityPlan(..)");
        assert_eq!(plan.account_id(), &fixture.account);
        assert_eq!(plan.signal(), &fixture.signal);
        assert_eq!(plan.instrument(), &fixture.instrument);
        assert_eq!(
            plan.instrument_id(),
            fixture.signal.facts().task().volatility().instrument()
        );
        assert_eq!(plan.held_quantity(), whole("17"));
        assert_eq!(plan.sellable_quantity(), whole("11"));
        assert_eq!(plan.default_quantity(), whole("11"));
        assert_eq!(plan.quantity(), whole("11"));
        assert_eq!(plan.locked_quantity(), qty("6"));
        assert_eq!(
            plan.constraint(),
            SellQuantityConstraint::SellableQuantityCap
        );
        assert_eq!(plan.adjustment(), SellQuantityAdjustment::Default);
        assert_eq!(plan.holding_observed_at(), fixture.now);
        assert_eq!(plan.sellable_observed_at(), sellable_time);
    }

    fn assert_example_case(
        market: Market,
        quantities: (&str, &str),
        expected: (&str, &str, SellQuantityConstraint),
    ) {
        let fixture = Fixture::new(market, qty(quantities.0));
        let fact = fixture.sellable(qty(quantities.1), fixture.now);
        let built = fixture
            .decide(Some(&fact), fixture.now)
            .into_plan()
            .expect("계획");
        assert_eq!(built.default_quantity(), whole(expected.0));
        assert_eq!(built.quantity(), whole(expected.0));
        assert_eq!(built.quantity().quantity().as_decimal().scale(), 0);
        assert_eq!(built.locked_quantity(), qty(expected.1));
        assert_eq!(built.constraint(), expected.2);
        assert_eq!(built.adjustment(), SellQuantityAdjustment::Default);
    }

    #[test]
    fn sell_draft_examples_cover_kr_us_cap_and_max_quantity() {
        assert_example_case(
            Market::Kr,
            ("17", "17"),
            ("17", "0", SellQuantityConstraint::FullHolding),
        );
        assert_example_case(
            Market::Kr,
            ("7", "19"),
            ("7", "0", SellQuantityConstraint::FullHolding),
        );
        assert_example_case(
            Market::Kr,
            ("10.00", "6.000"),
            ("6", "4", SellQuantityConstraint::SellableQuantityCap),
        );
        assert_example_case(
            Market::Us,
            ("12", "12"),
            ("12", "0", SellQuantityConstraint::FullHolding),
        );
        assert_example_case(
            Market::Us,
            ("12", "5"),
            ("5", "7", SellQuantityConstraint::SellableQuantityCap),
        );
        assert_example_case(
            Market::Us,
            ("1", "1"),
            ("1", "0", SellQuantityConstraint::FullHolding),
        );
        assert_example_case(
            Market::Us,
            ("79228162514264337593543950335", "1"),
            (
                "1",
                "79228162514264337593543950334",
                SellQuantityConstraint::SellableQuantityCap,
            ),
        );
    }

    #[test]
    fn sell_draft_rejects_unknown_zero_and_mismatched_sellable() {
        let fixture = Fixture::new(Market::Kr, qty("17"));
        let unknown = fixture.decide(None, fixture.now);
        assert_eq!(
            unknown,
            SellDraftQuantityDecision::None(NoSellDraftQuantity::SellableQuantityUnknown)
        );
        assert_eq!(unknown.into_plan(), None);

        let zero_sellable = fixture.sellable(qty("0"), fixture.now);
        assert_eq!(
            fixture.decide(Some(&zero_sellable), fixture.now),
            SellDraftQuantityDecision::None(NoSellDraftQuantity::ZeroSellableQuantity)
        );

        let wrong_account = SellableQuantityFact::from_lookup_result(
            AccountId::new(9999),
            fixture.instrument.clone(),
            qty("10"),
            fixture.now,
        )
        .expect("타 계좌");
        assert_eq!(
            fixture.decide(Some(&wrong_account), fixture.now),
            SellDraftQuantityDecision::None(NoSellDraftQuantity::DifferentFact)
        );

        let wrong_symbol = SellableQuantityFact::from_lookup_result(
            fixture.account.clone(),
            instrument_for("000660", &Market::Kr),
            qty("10"),
            fixture.now,
        )
        .expect("타 심볼");
        assert_eq!(
            fixture.decide(Some(&wrong_symbol), fixture.now),
            SellDraftQuantityDecision::None(NoSellDraftQuantity::DifferentFact)
        );

        let wrong_market = SellableQuantityFact::from_lookup_result(
            fixture.account.clone(),
            instrument_for("005930", &Market::Us),
            qty("10"),
            fixture.now,
        )
        .expect("타 시장");
        assert_eq!(
            fixture.decide(Some(&wrong_market), fixture.now),
            SellDraftQuantityDecision::None(NoSellDraftQuantity::DifferentFact)
        );
    }

    #[test]
    fn sell_draft_enforces_sellable_60s_freshness_and_future_rejection() {
        let fixture = Fixture::new(Market::Kr, qty("17"));
        let fresh = fixture.sellable(qty("10"), fixture.now);
        assert_eq!(
            fixture
                .decide(Some(&fresh), later(fixture.now, 60, 0))
                .into_plan()
                .expect("정확히 60초 허용")
                .quantity(),
            whole("10")
        );
        let fresher_holding =
            holding_for(&fixture.instrument, qty("17"), later(fixture.now, 30, 0));
        let fresher_val = valuation_with(
            &fixture.market,
            vec![fresher_holding],
            later(fixture.now, 30, 0),
        );
        let stale_facts = SellDraftQuantityFacts::new(&fixture.account, &fresher_val, Some(&fresh));
        assert_eq!(
            calculate_sell_draft_quantity(
                &fixture.signal,
                &stale_facts,
                later(fixture.now, 60, 1),
                &fixture.calendars,
            ),
            SellDraftQuantityDecision::None(NoSellDraftQuantity::StaleSellableQuantity)
        );

        let future_sellable = fixture.sellable(qty("10"), later(fixture.now, 0, 1));
        assert_eq!(
            fixture.decide(Some(&future_sellable), fixture.now),
            SellDraftQuantityDecision::None(NoSellDraftQuantity::StaleSellableQuantity)
        );

        let pre_task_sellable = fixture.sellable(qty("10"), earlier(fixture.now, 1, 0));
        assert_eq!(
            fixture.decide(Some(&pre_task_sellable), fixture.now),
            SellDraftQuantityDecision::None(NoSellDraftQuantity::StaleSellableQuantity)
        );
    }

    #[test]
    fn sell_draft_rejects_missing_zero_and_changed_holdings() {
        let fixture = Fixture::new(Market::Kr, qty("17"));
        let sellable = fixture.sellable(qty("17"), fixture.now);

        let empty_val = valuation_with(&fixture.market, vec![], fixture.now);
        let empty_facts =
            SellDraftQuantityFacts::new(&fixture.account, &empty_val, Some(&sellable));
        assert_eq!(
            calculate_sell_draft_quantity(
                &fixture.signal,
                &empty_facts,
                fixture.now,
                &fixture.calendars
            ),
            SellDraftQuantityDecision::None(NoSellDraftQuantity::NotHeld)
        );

        let zero_holding = holding_for(&fixture.instrument, qty("0"), fixture.now);
        let zero_val = valuation_with(&fixture.market, vec![zero_holding], fixture.now);
        let zero_facts = SellDraftQuantityFacts::new(&fixture.account, &zero_val, Some(&sellable));
        assert_eq!(
            calculate_sell_draft_quantity(
                &fixture.signal,
                &zero_facts,
                fixture.now,
                &fixture.calendars
            ),
            SellDraftQuantityDecision::None(NoSellDraftQuantity::NotHeld)
        );

        let changed_holding = holding_for(&fixture.instrument, qty("16"), fixture.now);
        let changed_val = valuation_with(&fixture.market, vec![changed_holding], fixture.now);
        let changed_facts =
            SellDraftQuantityFacts::new(&fixture.account, &changed_val, Some(&sellable));
        assert_eq!(
            calculate_sell_draft_quantity(
                &fixture.signal,
                &changed_facts,
                fixture.now,
                &fixture.calendars
            ),
            SellDraftQuantityDecision::None(NoSellDraftQuantity::HoldingChanged)
        );
    }

    #[test]
    fn sell_draft_rejects_stale_future_and_pre_signal_holdings() {
        let fixture = Fixture::new(Market::Kr, qty("17"));
        let sellable = fixture.sellable(qty("17"), fixture.now);

        let stale_holding_now = later(fixture.now, 60, 1);
        let fresh_sellable = fixture.sellable(qty("17"), stale_holding_now);
        assert_eq!(
            fixture.decide(Some(&fresh_sellable), stale_holding_now),
            SellDraftQuantityDecision::None(NoSellDraftQuantity::StaleHolding)
        );

        let future_holding = holding_for(&fixture.instrument, qty("17"), later(fixture.now, 1, 0));
        let future_val = valuation_with(&fixture.market, vec![future_holding], fixture.now);
        let future_facts =
            SellDraftQuantityFacts::new(&fixture.account, &future_val, Some(&sellable));
        assert_eq!(
            calculate_sell_draft_quantity(
                &fixture.signal,
                &future_facts,
                fixture.now,
                &fixture.calendars
            ),
            SellDraftQuantityDecision::None(NoSellDraftQuantity::StaleHolding)
        );

        let signal_at = later(fixture.now, 10, 0);
        let signal_holding = holding_for(&fixture.instrument, qty("17"), signal_at);
        let signal_val = valuation_with(&fixture.market, vec![signal_holding], signal_at);
        let later_signal = build_sell_signal_at(
            &fixture.instrument,
            &signal_val,
            (fixture.now, signal_at),
            "1",
            &fixture.calendars,
        );
        let older_holding = holding_for(&fixture.instrument, qty("17"), later(fixture.now, 5, 0));
        let older_val = valuation_with(&fixture.market, vec![older_holding], signal_at);
        let fresh_sellable_after = fixture.sellable(qty("17"), signal_at);
        let older_facts =
            SellDraftQuantityFacts::new(&fixture.account, &older_val, Some(&fresh_sellable_after));
        assert_eq!(
            calculate_sell_draft_quantity(
                &later_signal,
                &older_facts,
                signal_at,
                &fixture.calendars
            ),
            SellDraftQuantityDecision::None(NoSellDraftQuantity::StaleHolding)
        );

        let pre_signal_sellable = fixture.sellable(qty("17"), later(fixture.now, 5, 0));
        let pre_signal_facts =
            SellDraftQuantityFacts::new(&fixture.account, &signal_val, Some(&pre_signal_sellable));
        assert_eq!(
            calculate_sell_draft_quantity(
                &later_signal,
                &pre_signal_facts,
                signal_at,
                &fixture.calendars
            ),
            SellDraftQuantityDecision::None(NoSellDraftQuantity::StaleSellableQuantity)
        );
    }

    fn sell_signal_for(
        fixture: &Fixture,
        held: &str,
        observed_at: Timestamp,
        probability: &str,
    ) -> SellSignal {
        let valuation = valuation_with(
            &fixture.market,
            vec![holding_for(&fixture.instrument, qty(held), observed_at)],
            observed_at,
        );
        build_sell_signal_at(
            &fixture.instrument,
            &valuation,
            (fixture.now, observed_at),
            probability,
            &fixture.calendars,
        )
    }

    #[test]
    fn sell_signal_matches_reassessed_accepts_fresher_observation_and_rejects_changes() {
        let fixture = Fixture::new(Market::Kr, qty("17"));
        let signal_at = later(fixture.now, 10, 0);
        let newer_at = later(fixture.now, 25, 0);
        let older_at = later(fixture.now, 5, 0);
        let stored = sell_signal_for(&fixture, "17", signal_at, "1");
        assert_eq!(
            sell_signal_matches_reassessed(&stored, &stored),
            CheckResult::Passed
        );
        let fresher = sell_signal_for(&fixture, "17", newer_at, "1");
        assert_eq!(
            sell_signal_matches_reassessed(&stored, &fresher),
            CheckResult::Passed
        );
        let regressed = sell_signal_for(&fixture, "17", older_at, "1");
        assert_eq!(
            sell_signal_matches_reassessed(&stored, &regressed),
            CheckResult::Failed
        );
        let qty_changed = sell_signal_for(&fixture, "16", newer_at, "1");
        assert_eq!(
            sell_signal_matches_reassessed(&stored, &qty_changed),
            CheckResult::Failed
        );
        let diff_facts = sell_signal_for(&fixture, "17", newer_at, "2");
        assert_eq!(
            sell_signal_matches_reassessed(&stored, &diff_facts),
            CheckResult::Failed
        );
    }

    #[test]
    fn sell_draft_rejects_signal_timing_and_fractional_quantities() {
        let fixture = Fixture::new(Market::Kr, qty("17"));
        let sellable = fixture.sellable(qty("17"), fixture.now);
        let expired = crate::estimate_outcome::signal_task_interval(
            fixture.signal.facts().task(),
            &fixture.calendars,
        )
        .expect("만기")
        .end();
        assert_eq!(
            fixture.decide(Some(&sellable), expired),
            SellDraftQuantityDecision::None(NoSellDraftQuantity::SignalUnavailable(
                NoSignalReason::Expired
            ))
        );
        assert_eq!(
            fixture.decide(Some(&sellable), earlier(fixture.now, 1, 0)),
            SellDraftQuantityDecision::None(NoSellDraftQuantity::SignalUnavailable(
                NoSignalReason::Unavailable(SignalUnavailable::SubmissionFromFuture)
            ))
        );
        let facts =
            SellDraftQuantityFacts::new(&fixture.account, &fixture.valuation, Some(&sellable));
        assert_eq!(
            calculate_sell_draft_quantity(&fixture.signal, &facts, fixture.now, &[]),
            SellDraftQuantityDecision::None(NoSellDraftQuantity::SignalUnavailable(
                NoSignalReason::Unavailable(SignalUnavailable::Calendar)
            ))
        );

        let kr_frac = Fixture::new(Market::Kr, qty("10.5"));
        let kr_frac_sellable = kr_frac.sellable(qty("10"), kr_frac.now);
        assert_eq!(
            kr_frac.decide(Some(&kr_frac_sellable), kr_frac.now),
            SellDraftQuantityDecision::None(NoSellDraftQuantity::FractionalQuantityUnsupported)
        );

        let us_frac_held = Fixture::new(Market::Us, qty("10.5"));
        let us_whole_sellable = us_frac_held.sellable(qty("10"), us_frac_held.now);
        assert_eq!(
            us_frac_held.decide(Some(&us_whole_sellable), us_frac_held.now),
            SellDraftQuantityDecision::None(NoSellDraftQuantity::FractionalQuantityUnsupported)
        );

        let us_whole_held = Fixture::new(Market::Us, qty("10"));
        for frac in ["5.5", "0.5", "12.5"] {
            let frac_sellable = us_whole_held.sellable(qty(frac), us_whole_held.now);
            assert_eq!(
                us_whole_held.decide(Some(&frac_sellable), us_whole_held.now),
                SellDraftQuantityDecision::None(NoSellDraftQuantity::FractionalQuantityUnsupported)
            );
        }
    }

    #[test]
    fn check_facts_at_revalidates_signal_holding_and_sellable_freshness() {
        let fixture = Fixture::new(Market::Kr, qty("17"));
        let sellable_at = later(fixture.now, 10, 0);
        let sellable = fixture.sellable(qty("11"), sellable_at);
        let plan = fixture
            .decide(Some(&sellable), sellable_at)
            .into_plan()
            .expect("계획");
        assert_eq!(
            plan.check_facts_at(later(fixture.now, 60, 0), &fixture.calendars),
            Ok(())
        );
        assert_eq!(
            plan.check_facts_at(later(fixture.now, 60, 1), &fixture.calendars),
            Err(NoSellDraftQuantity::StaleHolding)
        );
        assert_eq!(
            plan.check_facts_at(earlier(fixture.now, 1, 0), &fixture.calendars),
            Err(NoSellDraftQuantity::SignalUnavailable(
                NoSignalReason::Unavailable(SignalUnavailable::SubmissionFromFuture)
            ))
        );
        let expired = crate::estimate_outcome::signal_task_interval(
            fixture.signal.facts().task(),
            &fixture.calendars,
        )
        .expect("만기")
        .end();
        assert_eq!(
            plan.check_facts_at(expired, &fixture.calendars),
            Err(NoSellDraftQuantity::SignalUnavailable(
                NoSignalReason::Expired
            ))
        );

        let holding_at = later(fixture.now, 20, 0);
        let fresher_holding = holding_for(&fixture.instrument, qty("17"), holding_at);
        let fresher_val = valuation_with(&fixture.market, vec![fresher_holding], holding_at);
        let older_sellable = fixture.sellable(qty("11"), fixture.now);
        let facts =
            SellDraftQuantityFacts::new(&fixture.account, &fresher_val, Some(&older_sellable));
        let staggered =
            calculate_sell_draft_quantity(&fixture.signal, &facts, holding_at, &fixture.calendars)
                .into_plan()
                .expect("시차 계획");
        assert_eq!(
            staggered.check_facts_at(later(fixture.now, 60, 0), &fixture.calendars),
            Ok(())
        );
        assert_eq!(
            staggered.check_facts_at(later(fixture.now, 60, 1), &fixture.calendars),
            Err(NoSellDraftQuantity::StaleSellableQuantity)
        );
    }

    #[test]
    fn reduce_quantity_enforces_kr_and_us_bounds() {
        for market in [Market::Kr, Market::Us] {
            let fixture = Fixture::new(market, qty("17"));
            let sellable = fixture.sellable(qty("11"), fixture.now);
            let plan = fixture
                .decide(Some(&sellable), fixture.now)
                .into_plan()
                .expect("계획");

            let unchanged = plan.reduce_quantity(qty("11.00")).expect("기본 수량 유지");
            assert_eq!(unchanged.quantity(), whole("11"));
            assert_eq!(unchanged.default_quantity(), whole("11"));
            assert_eq!(unchanged.adjustment(), SellQuantityAdjustment::Default);
            assert_eq!(unchanged.locked_quantity(), qty("6"));
            assert_eq!(
                unchanged.constraint(),
                SellQuantityConstraint::SellableQuantityCap
            );

            let reduced = plan.reduce_quantity(qty("7")).expect("축소");
            assert_eq!(reduced.quantity(), whole("7"));
            assert_eq!(reduced.default_quantity(), whole("11"));
            assert_eq!(reduced.held_quantity(), whole("17"));
            assert_eq!(reduced.sellable_quantity(), whole("11"));
            assert_eq!(reduced.locked_quantity(), qty("6"));
            assert_eq!(
                reduced.constraint(),
                SellQuantityConstraint::SellableQuantityCap
            );
            assert_eq!(reduced.adjustment(), SellQuantityAdjustment::Reduced);

            let kept = reduced.reduce_quantity(qty("7")).expect("축소 유지");
            assert_eq!(kept.adjustment(), SellQuantityAdjustment::Reduced);
            let min = reduced.reduce_quantity(qty("1")).expect("최소 1주");
            assert_eq!(min.quantity(), whole("1"));
            let raised = min
                .reduce_quantity(qty("8"))
                .expect("기본 수량 이하 재증가");
            assert_eq!(raised.quantity(), whole("8"));
            assert_eq!(raised.default_quantity(), whole("11"));
            assert_eq!(raised.adjustment(), SellQuantityAdjustment::Reduced);
            let restored = raised.reduce_quantity(qty("11")).expect("기본 수량 복원");
            assert_eq!(restored.quantity(), whole("11"));
            assert_eq!(restored.adjustment(), SellQuantityAdjustment::Default);
            assert_eq!(
                reduced.reduce_quantity(qty("12")),
                Err(SellDraftQuantityAdjustmentError::ExceedsAllowedQuantity)
            );
            assert_eq!(
                plan.reduce_quantity(qty("12")),
                Err(SellDraftQuantityAdjustmentError::ExceedsAllowedQuantity)
            );
            assert_eq!(
                plan.reduce_quantity(qty("17")),
                Err(SellDraftQuantityAdjustmentError::ExceedsAllowedQuantity)
            );
        }
    }

    #[test]
    fn with_history_allows_parallel_sellable_after_stored_signal_and_rejects_mismatches() {
        let fixture = Fixture::new(Market::Kr, qty("17"));
        let stored_at = later(fixture.now, 10, 0);
        let live_at = later(fixture.now, 30, 0);
        let parallel_sellable_at = later(fixture.now, 25, 0);
        let stored_signal = sell_signal_for(&fixture, "17", stored_at, "1");
        let stored_history =
            crate::HistoricalSignal::from_signal(&crate::RaisedSignal::Sell(stored_signal));
        let live_signal = sell_signal_for(&fixture, "17", live_at, "1");
        let live_holding = holding_for(&fixture.instrument, qty("17"), live_at);
        let live_val = valuation_with(&fixture.market, vec![live_holding], live_at);

        let parallel_sellable = fixture.sellable(qty("11"), parallel_sellable_at);
        let ok_facts =
            SellDraftQuantityFacts::new(&fixture.account, &live_val, Some(&parallel_sellable));
        let plan = calculate_sell_draft_quantity_with_history(
            &live_signal,
            &stored_history,
            &ok_facts,
            live_at,
            &fixture.calendars,
        )
        .into_plan()
        .expect("병렬 조회 허용");
        assert_eq!(plan.quantity(), whole("11"));
        assert_eq!(
            plan.check_facts_at(later(parallel_sellable_at, 60, 0), &fixture.calendars),
            Ok(())
        );
        assert_eq!(
            plan.check_facts_at(later(parallel_sellable_at, 60, 1), &fixture.calendars),
            Err(NoSellDraftQuantity::StaleSellableQuantity)
        );

        let pre_stored_sellable = fixture.sellable(qty("11"), later(fixture.now, 9, 0));
        let stale_facts =
            SellDraftQuantityFacts::new(&fixture.account, &live_val, Some(&pre_stored_sellable));
        assert_eq!(
            calculate_sell_draft_quantity_with_history(
                &live_signal,
                &stored_history,
                &stale_facts,
                live_at,
                &fixture.calendars,
            ),
            SellDraftQuantityDecision::None(NoSellDraftQuantity::StaleSellableQuantity)
        );
    }

    fn fixture_buy_signal(fixture: &Fixture) -> crate::RaisedSignal {
        let eligible = eligible_task(&fixture.instrument, fixture.now, &fixture.calendars);
        let (buy_estimate, buy_item, buy_head) =
            sell_estimate_with_probability(&eligible, fixture.now, "99");
        let items = [buy_item];
        let ledger = SignalSourceLedger::new(&items, &[], &[]);
        let policy = Policy::new(InvestorProfile::default());
        let assignments = SectorAssignments::try_new(vec![SectorAssignment::new(
            &fixture.instrument,
            InstrumentClassification::EtfFund,
        )])
        .expect("분류");
        let security = PolicySecurityFacts::from_master_lookup_results(
            std::iter::once((
                AllocationInstrument::new(&fixture.instrument),
                ExcludedSecurityType::Stock,
            ))
            .collect(),
        );
        let buy_context = SignalContext::new(
            Some(&fixture.valuation),
            &policy,
            (&assignments, &security),
            &ledger,
            SignalAssessmentFacts::new(
                SignalTiming::new(fixture.now, &fixture.calendars),
                Some(buy_estimate.task().reference().clone()),
                Some(buy_head),
            ),
        );
        SignalDecision::assess(&buy_estimate, &buy_context)
            .into_signal()
            .expect("매수 신호")
    }

    #[test]
    fn with_history_rejects_mismatched_and_regressed_stored_signals() {
        let fixture = Fixture::new(Market::Kr, qty("17"));
        let stored_at = later(fixture.now, 10, 0);
        let live_at = later(fixture.now, 30, 0);
        let stored_signal = sell_signal_for(&fixture, "17", stored_at, "1");
        let stored_history =
            crate::HistoricalSignal::from_signal(&crate::RaisedSignal::Sell(stored_signal));
        let live_signal = sell_signal_for(&fixture, "17", live_at, "1");
        let live_holding = holding_for(&fixture.instrument, qty("17"), live_at);
        let live_val = valuation_with(&fixture.market, vec![live_holding], live_at);
        let parallel_sellable = fixture.sellable(qty("11"), later(fixture.now, 25, 0));
        let ok_facts =
            SellDraftQuantityFacts::new(&fixture.account, &live_val, Some(&parallel_sellable));
        let buy_history = crate::HistoricalSignal::from_signal(&fixture_buy_signal(&fixture));
        assert_eq!(
            calculate_sell_draft_quantity_with_history(
                &live_signal,
                &buy_history,
                &ok_facts,
                live_at,
                &fixture.calendars,
            ),
            SellDraftQuantityDecision::None(NoSellDraftQuantity::DifferentFact)
        );
        let diff_facts_live = sell_signal_for(&fixture, "17", live_at, "2");
        assert_eq!(
            calculate_sell_draft_quantity_with_history(
                &diff_facts_live,
                &stored_history,
                &ok_facts,
                live_at,
                &fixture.calendars,
            ),
            SellDraftQuantityDecision::None(NoSellDraftQuantity::DifferentFact)
        );
        let diff_qty_live = sell_signal_for(&fixture, "15", live_at, "1");
        assert_eq!(
            calculate_sell_draft_quantity_with_history(
                &diff_qty_live,
                &stored_history,
                &ok_facts,
                live_at,
                &fixture.calendars,
            ),
            SellDraftQuantityDecision::None(NoSellDraftQuantity::HoldingChanged)
        );
        let regressed_live = sell_signal_for(&fixture, "17", later(fixture.now, 5, 0), "1");
        assert_eq!(
            calculate_sell_draft_quantity_with_history(
                &regressed_live,
                &stored_history,
                &ok_facts,
                live_at,
                &fixture.calendars,
            ),
            SellDraftQuantityDecision::None(NoSellDraftQuantity::StaleHolding)
        );
    }

    #[test]
    fn reduce_quantity_rejects_zero_fractions_and_handles_single_share() {
        let single = Fixture::new(Market::Kr, qty("1"));
        let single_sellable = single.sellable(qty("1"), single.now);
        let plan = single
            .decide(Some(&single_sellable), single.now)
            .into_plan()
            .expect("1주 계획");
        assert_eq!(
            plan.reduce_quantity(qty("1"))
                .expect("1주 유지")
                .adjustment(),
            SellQuantityAdjustment::Default
        );
        for zero in ["0", "0.00"] {
            assert_eq!(
                plan.reduce_quantity(qty(zero)),
                Err(SellDraftQuantityAdjustmentError::BelowMinimumUnit)
            );
        }
        assert_eq!(
            plan.reduce_quantity(qty("2")),
            Err(SellDraftQuantityAdjustmentError::ExceedsAllowedQuantity)
        );
        for fraction in ["0.5", "1.5", "10.999999", "0.000001"] {
            assert_eq!(
                plan.reduce_quantity(qty(fraction)),
                Err(SellDraftQuantityAdjustmentError::InvalidQuantityUnit)
            );
        }
    }

    fn assert_reduction_property(
        plan: &SellDraftQuantityPlan,
        expected_default: u64,
        requests: (u64, u64),
        frac_part: u32,
    ) {
        let (first_raw, second_raw) = requests;
        let first_input = Quantity::try_new(Decimal::from(first_raw)).expect("요청");
        let first_outcome = plan.reduce_quantity(first_input);
        if first_raw == 0 {
            assert_eq!(
                first_outcome,
                Err(SellDraftQuantityAdjustmentError::BelowMinimumUnit)
            );
        } else if first_raw > expected_default {
            assert_eq!(
                first_outcome,
                Err(SellDraftQuantityAdjustmentError::ExceedsAllowedQuantity)
            );
        } else {
            let reduced = first_outcome.expect("축소 허용");
            assert_eq!(
                reduced.quantity().quantity().as_decimal(),
                Decimal::from(first_raw)
            );
            assert_eq!(
                reduced.default_quantity().quantity().as_decimal(),
                Decimal::from(expected_default)
            );
            let expected_adj = if first_raw < expected_default {
                SellQuantityAdjustment::Reduced
            } else {
                SellQuantityAdjustment::Default
            };
            assert_eq!(reduced.adjustment(), expected_adj);
            assert_second_reduction(&reduced, expected_default, second_raw);
        }
        let fractional_decimal = Decimal::from_i128_with_scale(i128::from(frac_part), 6_u32);
        let fractional = Quantity::try_new(fractional_decimal).expect("소수");
        assert_eq!(
            plan.reduce_quantity(fractional),
            Err(SellDraftQuantityAdjustmentError::InvalidQuantityUnit)
        );
    }

    fn assert_second_reduction(
        reduced: &SellDraftQuantityPlan,
        expected_default: u64,
        second_raw: u64,
    ) {
        let second_input = Quantity::try_new(Decimal::from(second_raw)).expect("재요청");
        let second_outcome = reduced.reduce_quantity(second_input);
        if second_raw == 0 {
            assert_eq!(
                second_outcome,
                Err(SellDraftQuantityAdjustmentError::BelowMinimumUnit)
            );
        } else if second_raw > expected_default {
            assert_eq!(
                second_outcome,
                Err(SellDraftQuantityAdjustmentError::ExceedsAllowedQuantity)
            );
        } else {
            let adjusted = second_outcome.expect("기본 수량 이하 재조정 허용");
            assert_eq!(
                adjusted.quantity().quantity().as_decimal(),
                Decimal::from(second_raw)
            );
            assert_eq!(
                adjusted.default_quantity().quantity().as_decimal(),
                Decimal::from(expected_default)
            );
            let expected_adj = if second_raw < expected_default {
                SellQuantityAdjustment::Reduced
            } else {
                SellQuantityAdjustment::Default
            };
            assert_eq!(adjusted.adjustment(), expected_adj);
        }
    }

    struct PropertyBase {
        account: AccountId,
        market: Market,
        instrument: ResolvedOrderInstrument,
        calendars: Vec<MarketCalendar>,
        now: Timestamp,
        estimate: SignalEstimate,
        item: CollectedItem,
        head: CalibrationLedgerHead,
        policy: Policy,
        assignments: SectorAssignments,
        security: PolicySecurityFacts,
    }

    impl PropertyBase {
        fn new(market: Market) -> Self {
            let now = at(61, &market);
            let symbol = if market == Market::Kr {
                "005930"
            } else {
                "AAPL"
            };
            let instrument = instrument_for(symbol, &market);
            let calendars = calendars_for(&market);
            let eligible = eligible_task(&instrument, now, &calendars);
            let (estimate, item, head) = sell_estimate(&eligible, now);
            let policy = Policy::new(InvestorProfile::default());
            let assignments = SectorAssignments::try_new(vec![SectorAssignment::new(
                &instrument,
                InstrumentClassification::EtfFund,
            )])
            .expect("분류");
            let security = PolicySecurityFacts::from_master_lookup_results(
                std::iter::once((
                    AllocationInstrument::new(&instrument),
                    ExcludedSecurityType::Stock,
                ))
                .collect(),
            );
            Self {
                account: AccountId::new(4321),
                market,
                instrument,
                calendars,
                now,
                estimate,
                item,
                head,
                policy,
                assignments,
                security,
            }
        }

        fn sell_signal(&self, valuation: &PortfolioValuation) -> SellSignal {
            let items = [self.item.clone()];
            let ledger = SignalSourceLedger::new(&items, &[], &[]);
            let context = SignalContext::new(
                Some(valuation),
                &self.policy,
                (&self.assignments, &self.security),
                &ledger,
                SignalAssessmentFacts::new(
                    SignalTiming::new(self.now, &self.calendars),
                    Some(self.estimate.task().reference().clone()),
                    Some(self.head.clone()),
                ),
            );
            sell_from_decision(SignalDecision::assess(&self.estimate, &context))
                .expect("매도 신호 필요")
        }
    }

    fn edge_aware_quantity() -> proptest::strategy::BoxedStrategy<u64> {
        use proptest::strategy::Strategy;
        proptest::prop_oneof![
            1 => proptest::strategy::Just(0_u64),
            1 => proptest::strategy::Just(1_u64),
            2 => 1_u64..=10_u64,
            6 => 0_u64..=100_000_u64,
        ]
        .boxed()
    }

    fn check_property_case(
        base: &PropertyBase,
        quantities: (u64, u64),
        requests: (u64, u64),
        frac_part: u32,
    ) {
        let (held_raw, sellable_raw) = quantities;
        let held = Quantity::try_new(Decimal::from(held_raw)).expect("보유");
        let holding = holding_for(&base.instrument, held, base.now);
        let valuation = valuation_with(&base.market, vec![holding], base.now);
        let signal = base.sell_signal(&valuation);
        let sellable_fact = SellableQuantityFact::from_lookup_result(
            base.account.clone(),
            base.instrument.clone(),
            Quantity::try_new(Decimal::from(sellable_raw)).expect("매도가능"),
            base.now,
        )
        .expect("사실");
        let facts = SellDraftQuantityFacts::new(&base.account, &valuation, Some(&sellable_fact));
        let decision = calculate_sell_draft_quantity(&signal, &facts, base.now, &base.calendars);
        if sellable_raw == 0 {
            assert_eq!(
                decision,
                SellDraftQuantityDecision::None(NoSellDraftQuantity::ZeroSellableQuantity)
            );
            return;
        }
        let plan = decision.into_plan().expect("매도 계획");
        let expected_default = held_raw.min(sellable_raw);
        let expected_locked = held_raw.saturating_sub(sellable_raw);
        let expected_constraint = if sellable_raw < held_raw {
            SellQuantityConstraint::SellableQuantityCap
        } else {
            SellQuantityConstraint::FullHolding
        };
        assert_eq!(
            plan.default_quantity().quantity().as_decimal(),
            Decimal::from(expected_default)
        );
        assert_eq!(plan.quantity(), plan.default_quantity());
        assert!(plan.quantity().quantity().as_decimal() <= Decimal::from(held_raw));
        assert!(plan.quantity().quantity().as_decimal() <= Decimal::from(sellable_raw));
        assert_eq!(
            plan.locked_quantity().as_decimal(),
            Decimal::from(expected_locked)
        );
        assert_eq!(plan.constraint(), expected_constraint);
        assert_eq!(plan.adjustment(), SellQuantityAdjustment::Default);
        assert_reduction_property(&plan, expected_default, requests, frac_part);
    }

    #[test]
    fn property_default_quantity_and_reduction_match_independent_reference() {
        let kr_base = PropertyBase::new(Market::Kr);
        let us_base = PropertyBase::new(Market::Us);
        let strategy = (
            0_u8..=1_u8,
            proptest::prop_oneof![
                1 => proptest::strategy::Just(1_u64),
                2 => 1_u64..=10_u64,
                7 => 1_u64..=100_000_u64,
            ],
            edge_aware_quantity(),
            edge_aware_quantity(),
            edge_aware_quantity(),
            1_u32..=999_999_u32,
        );
        TestRunner::new(Config::with_cases(testkit::property_cases()))
            .run(
                &strategy,
                |(market_flag, held_raw, sellable_raw, first_raw, second_raw, frac_part)| {
                    let base = if market_flag == 0 { &kr_base } else { &us_base };
                    check_property_case(
                        base,
                        (held_raw, sellable_raw),
                        (first_raw, second_raw),
                        frac_part,
                    );
                    Ok(())
                },
            )
            .expect("속성 대조");
    }
}
