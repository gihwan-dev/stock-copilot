use crate::{
    BuySignal, BuySignalRecord, CheckResult, CommissionRateFact, Currency, Market, MarketCalendar,
    Money, MoneyError, OrderAmountBasis, OrderExpectedAmount, OrderFxRates, OrderKrwEstimate,
    Policy, PolicyBuyDecision, PolicyBuyLimit, PolicyCandidate, PolicyCostFit, PolicySecurityFacts,
    PortfolioValuation, PriceObservation, Quantity, ResolvedOrderInstrument, SectorAssignments,
    SignalOperation, Timestamp, Total, WholeOrderQuantity,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoBuyQuantity {
    InsufficientBuyingPower,
    NoDollarsExchangeRequired,
    ZeroPolicyLimit,
    BelowMinimumUnit,
    RepeatedSignal,
    InvalidPriceOrFx,
    Indeterminate,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuyQuantityConstraint {
    TargetWeight,
    PolicyLimit,
    BuyingPower,
    Commission,
    ConservativeOperation,
}

#[derive(Clone, PartialEq, Eq)]
pub struct BuyQuantityPlan {
    signal: BuySignal,
    quantity: WholeOrderQuantity,
    price: PriceObservation,
    fx: OrderFxRates,
    amount: OrderExpectedAmount,
    fee: Money,
    constraint: BuyQuantityConstraint,
}
impl std::fmt::Debug for BuyQuantityPlan {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("BuyQuantityPlan(..)")
    }
}
impl BuyQuantityPlan {
    #[must_use]
    pub fn instrument(&self) -> &ResolvedOrderInstrument {
        self.signal.facts().task().reference().instrument()
    }
    #[must_use]
    pub const fn quantity(&self) -> WholeOrderQuantity {
        self.quantity
    }
    #[must_use]
    pub const fn price(&self) -> &PriceObservation {
        &self.price
    }
    #[must_use]
    pub const fn fx(&self) -> OrderFxRates {
        self.fx
    }
    #[must_use]
    pub const fn amount(&self) -> OrderExpectedAmount {
        self.amount
    }
    #[must_use]
    pub const fn fee(&self) -> Money {
        self.fee
    }
    #[must_use]
    pub const fn constraint(&self) -> BuyQuantityConstraint {
        self.constraint
    }
    #[must_use]
    pub const fn limit(&self) -> PolicyBuyLimit {
        self.signal.limit()
    }
    #[must_use]
    pub const fn signal(&self) -> &BuySignal {
        &self.signal
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuyQuantityDecision {
    Plan(Box<BuyQuantityPlan>),
    None(NoBuyQuantity),
}
impl BuyQuantityDecision {
    #[must_use]
    pub fn into_plan(self) -> Option<BuyQuantityPlan> {
        match self {
            Self::Plan(plan) => Some(*plan),
            Self::None(_) => None,
        }
    }
}
pub struct BuyQuantityFacts<'facts> {
    valuation: &'facts PortfolioValuation,
    policy: &'facts Policy,
    assignments: &'facts SectorAssignments,
    securities: &'facts PolicySecurityFacts,
    price: &'facts PriceObservation,
    fx: OrderFxRates,
    commission: &'facts CommissionRateFact,
}
impl std::fmt::Debug for BuyQuantityFacts<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("BuyQuantityFacts(..)")
    }
}
impl<'facts> BuyQuantityFacts<'facts> {
    #[must_use]
    pub const fn new(
        valuation: &'facts PortfolioValuation,
        policy: &'facts Policy,
        classification: (&'facts SectorAssignments, &'facts PolicySecurityFacts),
        price: &'facts PriceObservation,
        costs: (OrderFxRates, &'facts CommissionRateFact),
    ) -> Self {
        Self {
            valuation,
            policy,
            assignments: classification.0,
            securities: classification.1,
            price,
            fx: costs.0,
            commission: costs.1,
        }
    }
}
#[must_use]
pub fn calculate_buy_quantity(
    signal: &BuySignal,
    facts: &BuyQuantityFacts<'_>,
    now: Timestamp,
    calendars: &[MarketCalendar],
    history: &[BuySignalRecord],
) -> BuyQuantityDecision {
    match calculate(signal, facts, now, calendars, history) {
        Ok(plan) => BuyQuantityDecision::Plan(Box::new(plan)),
        Err(reason) => BuyQuantityDecision::None(reason),
    }
}
fn calculate(
    signal: &BuySignal,
    facts: &BuyQuantityFacts<'_>,
    now: Timestamp,
    calendars: &[MarketCalendar],
    history: &[BuySignalRecord],
) -> Result<BuyQuantityPlan, NoBuyQuantity> {
    crate::buy_signal_history::check_buy_history(signal, now, history)?;
    crate::SignalTiming::new(now, calendars)
        .check(
            signal.facts().task(),
            signal.facts().submission().submitted_at(),
        )
        .or(Err(NoBuyQuantity::Indeterminate))?;
    let (available, total) =
        prepare_balances(facts, signal.facts().task().reference().instrument(), now)?;
    let (quantity, constraint) = size_quantity(signal, facts, now, available, total)?;
    finish_plan(signal, facts, now, quantity, constraint)
}
fn finish_plan(
    signal: &BuySignal,
    facts: &BuyQuantityFacts<'_>,
    now: Timestamp,
    quantity: Quantity,
    constraint: BuyQuantityConstraint,
) -> Result<BuyQuantityPlan, NoBuyQuantity> {
    let whole = WholeOrderQuantity::try_new(quantity).or(Err(NoBuyQuantity::BelowMinimumUnit))?;
    let amount = order_amount(facts, quantity, now)?;
    finish_amount(signal, facts, now, (whole, constraint), amount)
}
fn finish_amount(
    signal: &BuySignal,
    facts: &BuyQuantityFacts<'_>,
    now: Timestamp,
    sized: (WholeOrderQuantity, BuyQuantityConstraint),
    amount: OrderExpectedAmount,
) -> Result<BuyQuantityPlan, NoBuyQuantity> {
    let instrument = signal.facts().task().reference().instrument();
    let candidate = krw(amount).and_then(|cost| {
        PolicyCandidate::new(instrument, cost).or(Err(NoBuyQuantity::Indeterminate))
    })?;
    if facts.policy.check_buy(
        facts.valuation,
        facts.assignments,
        facts.securities,
        &candidate,
    ) != PolicyBuyDecision::Pass
    {
        return Err(NoBuyQuantity::Indeterminate);
    }
    crate::trade_cost::buy_quantity_fee(facts.commission, &instrument.market(), amount.money(), now)
        .or(Err(NoBuyQuantity::Indeterminate))
        .map(|fee| BuyQuantityPlan {
            signal: signal.clone(),
            quantity: sized.0,
            price: facts.price.clone(),
            fx: facts.fx,
            amount,
            fee,
            constraint: sized.1,
        })
}
fn compare_money(cost: Money, limit: Money) -> CheckResult {
    if cost.currency() == limit.currency()
        && cost.amount().as_decimal() <= limit.amount().as_decimal()
    {
        CheckResult::Passed
    } else {
        CheckResult::Failed
    }
}
fn native_fits(
    facts: &BuyQuantityFacts<'_>,
    trial: Quantity,
    limit: Money,
) -> Result<CheckResult, NoBuyQuantity> {
    facts
        .price
        .price()
        .checked_mul(&trial)
        .map(|cost| compare_money(cost, limit))
        .or(Err(NoBuyQuantity::Indeterminate))
}
fn order_amount(
    facts: &BuyQuantityFacts<'_>,
    trial: Quantity,
    now: Timestamp,
) -> Result<OrderExpectedAmount, NoBuyQuantity> {
    let amount = facts
        .price
        .price()
        .checked_mul(&trial)
        .or(Err(NoBuyQuantity::Indeterminate))
        .map(|cost| {
            OrderExpectedAmount::from_order(cost, OrderAmountBasis::Estimated, facts.fx, now)
        })?;
    krw(amount).map(|_cost| amount)
}
fn krw(amount: OrderExpectedAmount) -> Result<Money, NoBuyQuantity> {
    match amount.krw_estimate() {
        OrderKrwEstimate::Native(value) => Ok(value),
        OrderKrwEstimate::Converted(value) => Ok(value.into_parts().0),
        OrderKrwEstimate::Unavailable(_) => Err(NoBuyQuantity::InvalidPriceOrFx),
    }
}
fn within_amount<F>(
    facts: &BuyQuantityFacts<'_>,
    trial: Quantity,
    now: Timestamp,
    check: F,
) -> Result<CheckResult, NoBuyQuantity>
where
    F: FnOnce(OrderExpectedAmount) -> Result<CheckResult, NoBuyQuantity>,
{
    let cost = facts
        .price
        .price()
        .checked_mul(&trial)
        .or(Err(NoBuyQuantity::Indeterminate))?;
    let amount = OrderExpectedAmount::from_order(cost, OrderAmountBasis::Estimated, facts.fx, now);
    if amount.krw_estimate()
        == OrderKrwEstimate::Unavailable(crate::OrderFxUnavailable::Conversion(
            crate::FxConversionError::Arithmetic(MoneyError::Overflow),
        ))
    {
        return Ok(CheckResult::Failed);
    }
    check(amount)
}
fn prepare_balances(
    facts: &BuyQuantityFacts<'_>,
    instrument: &ResolvedOrderInstrument,
    now: Timestamp,
) -> Result<(Money, Money), NoBuyQuantity> {
    let currency = if instrument.market() == Market::Kr {
        Currency::Krw
    } else {
        Currency::Usd
    };
    if facts.price.check(instrument, now) != CheckResult::Passed
        || facts.price.price().as_money().currency() != currency
    {
        return Err(NoBuyQuantity::InvalidPriceOrFx);
    }
    let power = facts.valuation.buying_power(currency);
    if power.observed_at() > now {
        return Err(NoBuyQuantity::Indeterminate);
    }
    let available = power.money().or(Err(NoBuyQuantity::Indeterminate))?;
    if available.amount().as_decimal().is_zero() {
        return Err(if currency == Currency::Usd {
            NoBuyQuantity::NoDollarsExchangeRequired
        } else {
            NoBuyQuantity::InsufficientBuyingPower
        });
    }
    let total = match facts.valuation.assets() {
        Total::Complete(value) => value.value().clone().into_parts().0,
        Total::Incomplete(_) => return Err(NoBuyQuantity::Indeterminate),
    };
    Ok((available, total))
}
fn targets(total: Money, appetite: crate::RiskAppetite) -> Result<(Money, Money), NoBuyQuantity> {
    let normal = crate::buy_quantity_arithmetic::floor_target(
        total,
        appetite,
        SignalOperation::EligibleBeforeE91,
    )?;
    crate::buy_quantity_arithmetic::floor_target(total, appetite, SignalOperation::Conservative)
        .map(|conservative| (normal, conservative))
}
fn target_quantity(
    facts: &BuyQuantityFacts<'_>,
    now: Timestamp,
    target: Money,
) -> Result<Quantity, NoBuyQuantity> {
    let maximum = crate::quantity::maximum_buy_quantity();
    crate::buy_quantity_arithmetic::maximum_quantity(maximum, |trial| {
        within_amount(facts, trial, now, |cost| {
            krw(cost).map(|value| compare_money(value, target))
        })
    })
}
fn reduced(
    initial: (Quantity, BuyQuantityConstraint),
    quantity: Quantity,
    constraint: BuyQuantityConstraint,
) -> (Quantity, BuyQuantityConstraint) {
    if quantity < initial.0 {
        (quantity, constraint)
    } else {
        initial
    }
}
fn positive(
    sized: (Quantity, BuyQuantityConstraint),
    reason: NoBuyQuantity,
) -> Result<(Quantity, BuyQuantityConstraint), NoBuyQuantity> {
    if sized.0.as_decimal().is_zero() {
        Err(reason)
    } else {
        Ok(sized)
    }
}
fn size_quantity(
    signal: &BuySignal,
    facts: &BuyQuantityFacts<'_>,
    now: Timestamp,
    available: Money,
    total: Money,
) -> Result<(Quantity, BuyQuantityConstraint), NoBuyQuantity> {
    let (target, conservative) = targets(total, facts.policy.profile().risk_appetite())?;
    let sized = target_quantity(facts, now, target)
        .and_then(|quantity| {
            positive(
                (quantity, BuyQuantityConstraint::TargetWeight),
                NoBuyQuantity::BelowMinimumUnit,
            )
        })
        .and_then(|sized| size_policy(signal, facts, now, sized))
        .and_then(|sized| size_cash(facts, now, available, sized))?;
    size_conservative(signal, facts, now, conservative, sized)
}
fn size_policy(
    signal: &BuySignal,
    facts: &BuyQuantityFacts<'_>,
    now: Timestamp,
    initial: (Quantity, BuyQuantityConstraint),
) -> Result<(Quantity, BuyQuantityConstraint), NoBuyQuantity> {
    let sized = crate::buy_quantity_arithmetic::maximum_quantity(initial.0, |trial| {
        within_amount(facts, trial, now, |cost| {
            krw(cost).map(|value| match signal.limit().compare_krw_cost(value) {
                PolicyCostFit::WithinLimit => CheckResult::Passed,
                PolicyCostFit::AboveLimit | PolicyCostFit::InvalidCost => CheckResult::Failed,
            })
        })
    })
    .map(|quantity| reduced(initial, quantity, BuyQuantityConstraint::PolicyLimit))?;
    positive(sized, NoBuyQuantity::ZeroPolicyLimit)
}
fn size_cash(
    facts: &BuyQuantityFacts<'_>,
    now: Timestamp,
    available: Money,
    initial: (Quantity, BuyQuantityConstraint),
) -> Result<(Quantity, BuyQuantityConstraint), NoBuyQuantity> {
    let sized = crate::buy_quantity_arithmetic::maximum_quantity(initial.0, |trial| {
        native_fits(facts, trial, available)
    })
    .map(|quantity| reduced(initial, quantity, BuyQuantityConstraint::BuyingPower))?;
    size_fee(facts, now, available, sized)
}
fn fee_fits(
    facts: &BuyQuantityFacts<'_>,
    now: Timestamp,
    available: Money,
    trial: Quantity,
) -> Result<CheckResult, NoBuyQuantity> {
    let cost = facts
        .price
        .price()
        .checked_mul(&trial)
        .or(Err(NoBuyQuantity::Indeterminate))?;
    let fee = crate::trade_cost::buy_quantity_fee(
        facts.commission,
        &facts.price.instrument().market(),
        cost,
        now,
    )
    .or(Err(NoBuyQuantity::Indeterminate))?;
    cost.checked_add(&fee)
        .map(|total_cost| compare_money(total_cost, available))
        .or(Err(NoBuyQuantity::Indeterminate))
}
fn size_fee(
    facts: &BuyQuantityFacts<'_>,
    now: Timestamp,
    available: Money,
    initial: (Quantity, BuyQuantityConstraint),
) -> Result<(Quantity, BuyQuantityConstraint), NoBuyQuantity> {
    let sized = crate::buy_quantity_arithmetic::maximum_quantity(initial.0, |trial| {
        fee_fits(facts, now, available, trial)
    })
    .map(|quantity| reduced(initial, quantity, BuyQuantityConstraint::Commission))?;
    positive(sized, NoBuyQuantity::InsufficientBuyingPower)
}
fn size_conservative(
    signal: &BuySignal,
    facts: &BuyQuantityFacts<'_>,
    now: Timestamp,
    conservative: Money,
    initial: (Quantity, BuyQuantityConstraint),
) -> Result<(Quantity, BuyQuantityConstraint), NoBuyQuantity> {
    if signal.facts().operation() == SignalOperation::Conservative {
        crate::buy_quantity_arithmetic::maximum_quantity(initial.0, |trial| {
            within_amount(facts, trial, now, |cost| {
                krw(cost).map(|value| compare_money(value, conservative))
            })
        })
        .map(|quantity| {
            reduced(
                initial,
                quantity,
                BuyQuantityConstraint::ConservativeOperation,
            )
        })
    } else {
        Ok(initial)
    }
}

#[cfg(test)]
mod tests {
    use crate::Quantity;
    use crate::{
        AiProbability, AiProbabilityInput, AllocationInstrument, Amount, BuyQuantityConstraint,
        BuyQuantityDecision, BuyQuantityFacts, BuySignal, BuySignalRecord, BuyingPower,
        Calibration, CalibrationEstimate, CalibrationEstimateId, CalibrationFact, CalibrationKey,
        CalibrationModel, CalibrationOutcome, CollectedItem, CommissionRateFact,
        CommissionValidity, CostEligibleTask, CostRate, Currency, EstimateContext,
        EstimateCostPeriod, EstimateTask, Evidence, EvidenceRef, EvidenceUrl, ExcludedSecurityType,
        FxPair, FxRate, FxRateKind, FxSource, Horizon, HorizonSelection, HoursPermission,
        InstrumentClassification, InstrumentId, InvestorProfile, Margin, Market, MarketCalendar,
        MarketSession, ModelIdentityPart, Money, Nanosecond, NoBuyQuantity, NormalizedUrl,
        NumericInput, OffsetSeconds, OrderFxRates, Policy, PolicyBuyDecision, PolicyCandidate,
        PolicySecurityFacts, Portfolio, PortfolioValuation, Price, PriceObservation, ProfileWeight,
        Ratio, ResolvedOrderInstrument, RiskAppetite, SectorAssignment, SectorAssignments,
        SessionData, SessionInformation, SignalContext, SignalDecision, SignalEstimate,
        SignalSourceLedger, SignalSubmission, SignalTaskKey, SignalTiming, SpanSeconds,
        SubmittedCalibration, Symbol, TimeInterval, TimeSpan, Timestamp, TradeCost, TradeTaxFact,
        UntrustedText, Volatility, calculate_buy_quantity, calibrate_signal_submission, candle,
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
    fn later(at: Timestamp, seconds: u64) -> Timestamp {
        at.checked_add(
            TimeSpan::try_new(SpanSeconds::new(seconds), Nanosecond::new(0)).expect("기간"),
        )
        .expect("이후")
    }
    fn money(value: i64, currency: Currency) -> Money {
        Money::new(Amount::new(Decimal::from(value)), currency)
    }
    fn calendar(market: &Market) -> Vec<MarketCalendar> {
        let row = |day| {
            let start = at(day, market);
            let regular_market = Some(
                SessionData::try_new(
                    TimeInterval::try_new(start, later(start, 3600)).expect("장 구간"),
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
    fn rate(kind: FxRateKind, market: &Market) -> FxRate {
        FxRate::parse(
            &NumericInput::try_new("1000").expect("환율"),
            FxPair::UsdKrw,
            kind,
            TimeInterval::try_new(at(60, market), at(70, market)).expect("기간"),
            FxSource::TossOpenApi,
        )
        .expect("환율")
    }
    struct Fixture {
        signal: BuySignal,
        portfolio: PortfolioValuation,
        policy: Policy,
        assignments: SectorAssignments,
        securities: PolicySecurityFacts,
        price: PriceObservation,
        fx: OrderFxRates,
        fee: CommissionRateFact,
        calendars: Vec<MarketCalendar>,
        now: Timestamp,
    }
    impl Fixture {
        fn new(market: Market, krw: i64, usd: i64, price_value: i64, costs: (usize, i64)) -> Self {
            Self::new_for(market, (krw, usd), price_value, costs, "ABC")
        }
        fn new_for(
            market: Market,
            cash: (i64, i64),
            price_value: i64,
            costs: (usize, i64),
            symbol: &str,
        ) -> Self {
            let now = at(61, &market);
            let currency = if market == Market::Kr {
                Currency::Krw
            } else {
                Currency::Usd
            };
            let instrument = ResolvedOrderInstrument::from_master_lookup_result(
                Symbol::try_new(symbol).expect("코드"),
                market.clone(),
            )
            .expect("종목");
            let eligible = eligible_for(instrument.clone(), now, &market);
            let (estimate, item) = estimate_for(&eligible, now, &market, costs.0);
            let mid = rate(FxRateKind::Mid, &market);
            let portfolio = Portfolio::new(
                vec![],
                BuyingPower::new(money(cash.0, Currency::Krw), now),
                BuyingPower::new(money(cash.1, Currency::Usd), now),
            )
            .expect("포트")
            .evaluate(Some(mid));
            let policy = Policy::new(InvestorProfile::default());
            let assignments = SectorAssignments::try_new(vec![SectorAssignment::new(
                &instrument,
                InstrumentClassification::EtfFund,
            )])
            .expect("분류");
            let securities = securities_for(&instrument);
            let calendars = calendar(&market);
            let signal = raise(
                &estimate,
                &portfolio,
                (&policy, &assignments, &securities),
                item,
                &calendars,
            );
            let price = PriceObservation::from_quote_parts(
                instrument,
                Price::try_new(money(price_value, currency)).expect("가격"),
                now,
            )
            .expect("관측");
            let fx = OrderFxRates::from_broker_fx_result(rate(FxRateKind::Buy, &market), mid);
            let fee = CommissionRateFact::from_commissions_result(
                market,
                CostRate::try_new(Ratio::new(Decimal::new(costs.1, 4))).expect("요율"),
                CommissionValidity::try_new(None, None).expect("기간"),
            );
            Self {
                signal,
                portfolio,
                policy,
                assignments,
                securities,
                price,
                fx,
                fee,
                calendars,
                now,
            }
        }
        fn facts(&self) -> BuyQuantityFacts<'_> {
            BuyQuantityFacts::new(
                &self.portfolio,
                &self.policy,
                (&self.assignments, &self.securities),
                &self.price,
                (self.fx, &self.fee),
            )
        }
        fn calculate(&self, history: &[BuySignalRecord]) -> BuyQuantityDecision {
            calculate_buy_quantity(
                &self.signal,
                &self.facts(),
                self.now,
                &self.calendars,
                history,
            )
        }
    }
    fn securities_for(instrument: &ResolvedOrderInstrument) -> PolicySecurityFacts {
        PolicySecurityFacts::from_master_lookup_results(
            std::iter::once((
                AllocationInstrument::new(instrument),
                ExcludedSecurityType::Stock,
            ))
            .collect(),
        )
    }
    fn raise(
        estimate: &SignalEstimate,
        portfolio: &PortfolioValuation,
        policy: (&Policy, &SectorAssignments, &PolicySecurityFacts),
        item: CollectedItem,
        calendars: &[MarketCalendar],
    ) -> BuySignal {
        let items = [item];
        let ledger = SignalSourceLedger::new(&items, &[], &[]);
        let context = SignalContext::new(
            Some(portfolio),
            policy.0,
            (policy.1, policy.2),
            &ledger,
            SignalTiming::new(estimate.task().created_at(), calendars),
        );
        buy_from_decision(SignalDecision::assess(estimate, &context)).expect("매수 신호")
    }
    fn buy_from_decision(decision: SignalDecision) -> Option<BuySignal> {
        match decision {
            SignalDecision::Buy(value) => Some(value),
            SignalDecision::Sell(_) | SignalDecision::None(_) => None,
        }
    }
    #[test]
    fn absent_signal_does_not_create_fixture_buy_payload() {
        assert_eq!(
            buy_from_decision(SignalDecision::None(crate::NoSignalReason::ConditionsUnmet)),
            None
        );
    }
    fn eligible_for(
        instrument: ResolvedOrderInstrument,
        now: Timestamp,
        market: &Market,
    ) -> CostEligibleTask {
        let currency = if market == &Market::Kr {
            Currency::Krw
        } else {
            Currency::Usd
        };
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
            instrument,
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
        let period = EstimateCostPeriod::from_calendar_result(&task, later(at(66, market), 3600))
            .expect("만기");
        let zero = CostRate::try_new(Ratio::new(Decimal::ZERO)).expect("비용");
        let commission = CommissionRateFact::from_commissions_result(
            market.clone(),
            zero,
            CommissionValidity::try_new(None, None).expect("기간"),
        );
        let cost = TradeCost::from_facts(
            Some(commission),
            Some(TradeTaxFact::from_schedule_result(
                market.clone(),
                zero,
                zero,
            )),
            period,
        )
        .expect("비용");
        CostEligibleTask::assess(task, &cost).expect("적격")
    }
    fn estimate_for(
        eligible: &CostEligibleTask,
        now: Timestamp,
        market: &Market,
        samples: usize,
    ) -> (SignalEstimate, CollectedItem) {
        let part = ModelIdentityPart::try_new("d25").expect("모델");
        let key = CalibrationKey::new(
            CalibrationModel::new(part.clone(), part.clone(), part),
            Horizon::new(HorizonSelection::FewDays),
        );
        let item = CollectedItem::from_web(
            NormalizedUrl::try_new("https://example.com/d25").expect("주소"),
            UntrustedText::try_new("근거").expect("문구"),
            None,
        );
        let evidence = Evidence::try_new(vec![EvidenceRef::Web(
            EvidenceUrl::try_new(item.url().clone()).expect("주소"),
        )])
        .expect("근거");
        let ai =
            AiProbability::parse(&AiProbabilityInput::try_new("99").expect("입력")).expect("확률");
        let submission = SignalSubmission::create(
            SignalTaskKey::from_task(eligible),
            key.clone(),
            ai,
            Some(evidence),
            now,
        )
        .expect("제출");
        let submitted = submission_calibration(submission, key, ai, market, samples);
        (
            SignalEstimate::create(
                eligible,
                &submitted,
                Margin::for_appetite(RiskAppetite::Neutral),
            )
            .expect("추정"),
            item,
        )
    }
    fn submission_calibration(
        submission: SignalSubmission,
        key: CalibrationKey,
        ai: AiProbability,
        market: &Market,
        samples: usize,
    ) -> SubmittedCalibration {
        let records = (0..samples)
            .map(|index| {
                CalibrationFact::new(
                    CalibrationEstimate::new(
                        CalibrationEstimateId::try_new(&format!("e{index}")).expect("ID"),
                        InstrumentId::try_new(&format!("i{index}")).expect("종목"),
                        key.clone(),
                        TimeInterval::try_new(at(1, market), at(2, market)).expect("구간"),
                        ai,
                    ),
                    CalibrationOutcome::Hit,
                )
            })
            .collect::<Vec<_>>();
        calibrate_signal_submission(
            &Calibration::try_new(key, &records).expect("집계"),
            submission,
        )
    }
    #[test]
    fn plans_are_maximal_and_final_policy_passes_independent_integer_reference() {
        TestRunner::new(Config::with_cases(4096))
            .run(
                &(10_000_i64..1_000_000, 1_i64..1000, 0_usize..2),
                |(cash, price, conservative)| {
                    let fixture = Fixture::new(
                        Market::Kr,
                        cash,
                        0,
                        price,
                        (if conservative == 0 { 0 } else { 30 }, 0),
                    );
                    let target = cash
                        .checked_div(if conservative == 0 { 100 } else { 50 })
                        .expect("목표");
                    let expected = target.checked_div(price).expect("수량");
                    match fixture.calculate(&[]) {
                        BuyQuantityDecision::Plan(plan) => {
                            proptest::prop_assert!(expected > 0);
                            proptest::prop_assert_eq!(
                                plan.quantity().quantity().as_decimal(),
                                Decimal::from(expected)
                            );
                            proptest::prop_assert!(expected * price <= target);
                            proptest::prop_assert!((expected + 1) * price > target);
                            let candidate =
                                PolicyCandidate::new(plan.instrument(), plan.amount().money())
                                    .expect("후보");
                            proptest::prop_assert_eq!(
                                fixture.policy.check_buy(
                                    &fixture.portfolio,
                                    &fixture.assignments,
                                    &fixture.securities,
                                    &candidate
                                ),
                                PolicyBuyDecision::Pass
                            );
                            proptest::prop_assert_eq!(plan.limit(), fixture.signal.limit());
                            proptest::prop_assert_eq!(plan.price(), &fixture.price);
                            proptest::prop_assert_eq!(plan.fx(), fixture.fx);
                            proptest::prop_assert_eq!(plan.fee(), money(0, Currency::Krw));
                            proptest::prop_assert_eq!(plan.signal(), &fixture.signal);
                            proptest::prop_assert_eq!(
                                plan.instrument(),
                                fixture.price.instrument()
                            );
                        }
                        BuyQuantityDecision::None(reason) => {
                            proptest::prop_assert_eq!(expected, 0);
                            proptest::prop_assert_eq!(reason, NoBuyQuantity::BelowMinimumUnit);
                        }
                    }
                    Ok(())
                },
            )
            .expect("독립 최대 수량");
    }
    #[test]
    fn no_dollars_requires_exchange_even_with_large_krw_balance() {
        let fixture = Fixture::new(Market::Us, 100_000_000, 0, 1, (30, 0));
        assert_eq!(
            fixture.calculate(&[]),
            BuyQuantityDecision::None(NoBuyQuantity::NoDollarsExchangeRequired)
        );
        assert_eq!(fixture.calculate(&[]).into_plan(), None);
    }
    #[test]
    fn repeated_signal_is_blocked_without_an_order_and_history_order_is_irrelevant() {
        let fixture = Fixture::new(Market::Kr, 100_000, 0, 10, (30, 0));
        let record = BuySignalRecord::from_signal(&fixture.signal, fixture.now, &fixture.calendars)
            .expect("기록");
        assert_eq!(record.instrument(), fixture.price.instrument());
        assert_eq!(record.interval().start(), fixture.now);
        assert_eq!(record.raised_at(), fixture.now);
        assert_eq!(
            fixture.calculate(std::slice::from_ref(&record)),
            BuyQuantityDecision::None(NoBuyQuantity::RepeatedSignal)
        );
        assert_eq!(
            fixture.calculate(&[record.clone(), record.clone()]),
            fixture.calculate(&[record.clone(), record])
        );
    }
    #[test]
    fn commission_cash_and_policy_limit_reduce_quantity() {
        let mut fixture = Fixture::new(Market::Us, 100_000_000, 100, 1, (30, 100));
        let plan = fixture.calculate(&[]).into_plan().expect("계획");
        assert_eq!(
            plan.quantity().quantity().as_decimal(),
            Decimal::from(99_i32)
        );
        assert_eq!(plan.constraint(), BuyQuantityConstraint::Commission);
        assert_eq!(
            plan.fee(),
            Money::parse(&NumericInput::try_new("0.99").expect("입력"), Currency::Usd)
                .expect("수수료")
        );
        fixture.policy = Policy::new(InvestorProfile::default()).with_minimum_cash(
            ProfileWeight::parse_percent(&NumericInput::try_new("99.999").expect("입력"))
                .expect("하한"),
        );
        assert_eq!(
            fixture.calculate(&[]),
            BuyQuantityDecision::None(NoBuyQuantity::Indeterminate)
        );
    }
    fn reissue(fixture: &mut Fixture) {
        let item = CollectedItem::from_web(
            NormalizedUrl::try_new("https://example.com/d25").expect("주소"),
            UntrustedText::try_new("근거").expect("문구"),
            None,
        );
        fixture.signal = raise(
            fixture.signal.facts(),
            &fixture.portfolio,
            (&fixture.policy, &fixture.assignments, &fixture.securities),
            item,
            &fixture.calendars,
        );
    }
    #[test]
    fn policy_limit_and_zero_limit_are_binding_and_conservative_zero_is_no_order() {
        let mut fixture = Fixture::new(Market::Kr, 100_000, 0, 10, (30, 0));
        fixture.policy = fixture.policy.clone().with_minimum_cash(
            ProfileWeight::parse_percent(&NumericInput::try_new("99.99").expect("입력"))
                .expect("하한"),
        );
        reissue(&mut fixture);
        let plan = fixture.calculate(&[]).into_plan().expect("한 주");
        assert_eq!(plan.quantity().quantity().as_decimal(), Decimal::ONE);
        assert_eq!(plan.constraint(), BuyQuantityConstraint::PolicyLimit);
        fixture.price = PriceObservation::from_quote_parts(
            fixture.price.instrument().clone(),
            Price::try_new(money(11, Currency::Krw)).expect("단가"),
            fixture.now,
        )
        .expect("관측");
        assert_eq!(
            fixture.calculate(&[]),
            BuyQuantityDecision::None(NoBuyQuantity::ZeroPolicyLimit)
        );
        let small = Fixture::new(Market::Kr, 100, 0, 2, (0, 0));
        assert_eq!(
            small.calculate(&[]),
            BuyQuantityDecision::None(NoBuyQuantity::BelowMinimumUnit)
        );
    }
    #[test]
    fn cash_and_fee_can_each_prevent_the_minimum_whole_share() {
        let cash = Fixture::new(Market::Us, 100_000_000, 1, 2, (30, 0));
        assert_eq!(
            cash.calculate(&[]),
            BuyQuantityDecision::None(NoBuyQuantity::InsufficientBuyingPower)
        );
        let fee = Fixture::new(Market::Us, 100_000_000, 1, 1, (30, 1));
        assert_eq!(
            fee.calculate(&[]),
            BuyQuantityDecision::None(NoBuyQuantity::InsufficientBuyingPower)
        );
        let zero = Fixture::new(Market::Kr, 1, 0, 1, (30, 0));
        let empty = Portfolio::new(
            vec![],
            BuyingPower::new(money(0, Currency::Krw), zero.now),
            BuyingPower::new(money(0, Currency::Usd), zero.now),
        )
        .expect("포트")
        .evaluate(None);
        let facts = BuyQuantityFacts::new(
            &empty,
            &zero.policy,
            (&zero.assignments, &zero.securities),
            &zero.price,
            (zero.fx, &zero.fee),
        );
        assert_eq!(
            calculate_buy_quantity(&zero.signal, &facts, zero.now, &zero.calendars, &[]),
            BuyQuantityDecision::None(NoBuyQuantity::InsufficientBuyingPower)
        );
    }
    #[test]
    fn invalid_price_fx_calendar_and_fee_are_closed_failures() {
        let mut fixture = Fixture::new(Market::Us, 100_000_000, 100, 1, (30, 0));
        fixture.fx = OrderFxRates::missing();
        assert_eq!(
            fixture.calculate(&[]),
            BuyQuantityDecision::None(NoBuyQuantity::InvalidPriceOrFx)
        );
        fixture.fx = OrderFxRates::from_broker_fx_result(
            rate(FxRateKind::Buy, &Market::Us),
            rate(FxRateKind::Mid, &Market::Us),
        );
        fixture.now = later(fixture.now, 5);
        assert_eq!(
            fixture.calculate(&[]),
            BuyQuantityDecision::None(NoBuyQuantity::InvalidPriceOrFx)
        );
        fixture.now = at(61, &Market::Us);
        fixture.fee = CommissionRateFact::from_commissions_result(
            Market::Kr,
            CostRate::try_new(Ratio::new(Decimal::ZERO)).expect("요율"),
            CommissionValidity::try_new(None, None).expect("기간"),
        );
        assert_eq!(
            fixture.calculate(&[]),
            BuyQuantityDecision::None(NoBuyQuantity::Indeterminate)
        );
        fixture.calendars.clear();
        assert_eq!(
            fixture.calculate(&[]),
            BuyQuantityDecision::None(NoBuyQuantity::Indeterminate)
        );
        assert_eq!(
            BuySignalRecord::from_signal(&fixture.signal, fixture.now, &[]),
            Err(NoBuyQuantity::Indeterminate)
        );
    }
    #[test]
    fn history_identity_expiry_future_and_arbitrary_order_are_enforced() {
        let fixture = Fixture::new(Market::Kr, 100_000, 0, 1, (30, 0));
        let other = Fixture::new_for(Market::Kr, (100_000, 0), 1, (30, 0), "OTHER");
        let us = Fixture::new(Market::Us, 100_000, 100, 1, (30, 0));
        let first = BuySignalRecord::from_signal(&fixture.signal, fixture.now, &fixture.calendars)
            .expect("기록");
        let second = BuySignalRecord::from_signal(&other.signal, other.now, &other.calendars)
            .expect("다른 종목");
        let third =
            BuySignalRecord::from_signal(&us.signal, us.now, &us.calendars).expect("다른 시장");
        assert!(
            fixture
                .calculate(&[second.clone(), third.clone()])
                .into_plan()
                .is_some()
        );
        assert_eq!(
            crate::buy_signal_history::check_buy_history(
                &fixture.signal,
                first.interval().end(),
                std::slice::from_ref(&first)
            ),
            Ok(())
        );
        let future = BuySignalRecord::from_signal(
            &fixture.signal,
            later(fixture.now, 1),
            &fixture.calendars,
        )
        .expect("미래 기록");
        assert_eq!(
            fixture.calculate(&[future]),
            BuyQuantityDecision::None(NoBuyQuantity::Indeterminate)
        );
        assert_eq!(
            fixture.calculate(&vec![first.clone(); 65_537]),
            BuyQuantityDecision::None(NoBuyQuantity::Indeterminate)
        );
        TestRunner::new(Config::with_cases(4096))
            .run(&(0_usize..3), |rotation| {
                let mut records = vec![first.clone(), second.clone(), third.clone()];
                records.rotate_left(rotation);
                proptest::prop_assert_eq!(
                    fixture.calculate(&records),
                    BuyQuantityDecision::None(NoBuyQuantity::RepeatedSignal)
                );
                Ok(())
            })
            .expect("이력 순서");
        assert_eq!(format!("{first:?}"), "BuySignalRecord(..)");
    }
    #[test]
    fn us_cash_cap_is_independent_of_krw_when_target_is_nonbinding() {
        TestRunner::new(Config::with_cases(4096))
            .run(&(100_000_000_i64..200_000_000, 1_i64..100), |(krw, usd)| {
                let left = Fixture::new(Market::Us, krw, usd, 1, (30, 0));
                let right = Fixture::new(Market::Us, 100_000_000, usd, 1, (30, 0));
                let left_plan = left.calculate(&[]).into_plan().expect("달러 한도");
                let right_plan = right.calculate(&[]).into_plan().expect("달러 한도");
                proptest::prop_assert_eq!(left_plan.quantity(), right_plan.quantity());
                proptest::prop_assert_eq!(
                    left_plan.quantity().quantity().as_decimal(),
                    Decimal::from(usd)
                );
                proptest::prop_assert_eq!(
                    left_plan.constraint(),
                    BuyQuantityConstraint::BuyingPower
                );
                Ok(())
            })
            .expect("원화 잔액 독립");
    }
    #[test]
    fn future_cash_incomplete_assets_decimal_prices_and_fx_overflow_fail_closed() {
        let mut fixture = Fixture::new(Market::Us, 100_000_000, 100, 1, (30, 0));
        fixture.price = PriceObservation::from_quote_parts(
            fixture.price.instrument().clone(),
            Price::parse(
                &NumericInput::try_new("1.2345").expect("입력"),
                Currency::Usd,
            )
            .expect("단가"),
            fixture.now,
        )
        .expect("관측");
        let plan = fixture.calculate(&[]).into_plan().expect("소수 단가");
        assert_eq!(
            plan.quantity().quantity().as_decimal(),
            Decimal::from(81_i32)
        );
        assert_eq!(format!("{plan:?}"), "BuyQuantityPlan(..)");
        assert_eq!(format!("{:?}", fixture.facts()), "BuyQuantityFacts(..)");
        fixture.price = PriceObservation::from_quote_parts(
            fixture.price.instrument().clone(),
            Price::try_new(Money::new(Amount::new(Decimal::MAX), Currency::Usd)).expect("단가"),
            fixture.now,
        )
        .expect("관측");
        assert_eq!(
            fixture.calculate(&[]),
            BuyQuantityDecision::None(NoBuyQuantity::BelowMinimumUnit)
        );
        let future = Portfolio::new(
            vec![],
            BuyingPower::new(money(100_000, Currency::Krw), fixture.now),
            BuyingPower::new(money(100, Currency::Usd), later(fixture.now, 1)),
        )
        .expect("포트")
        .evaluate(Some(rate(FxRateKind::Mid, &Market::Us)));
        fixture.portfolio = future;
        assert_eq!(
            fixture.calculate(&[]),
            BuyQuantityDecision::None(NoBuyQuantity::Indeterminate)
        );
        fixture.portfolio = Portfolio::new(
            vec![],
            BuyingPower::new(money(100_000, Currency::Krw), fixture.now),
            BuyingPower::new(money(100, Currency::Usd), fixture.now),
        )
        .expect("포트")
        .evaluate(None);
        assert_eq!(
            fixture.calculate(&[]),
            BuyQuantityDecision::None(NoBuyQuantity::Indeterminate)
        );
    }
    fn rational_maximum(total: i64, usd: i64, price: i64, fee: i64) -> i64 {
        let mut result = 0_i64;
        for quantity in 1..=usd.checked_div(price).expect("정수 상한") {
            let cost = quantity.checked_mul(price).expect("비용");
            let cost_krw = cost.checked_mul(1000).expect("원화");
            let fee_cents = cost
                .checked_mul(fee)
                .and_then(|numerator| numerator.checked_add(99))
                .and_then(|numerator| numerator.checked_div(100))
                .expect("수수료 올림");
            if cost_krw <= total.checked_div(50).expect("목표 내림")
                && cost_krw.checked_mul(100_000).expect("비교")
                    <= total.checked_mul(5).expect("원칙 여유")
                && cost
                    .checked_mul(100)
                    .and_then(|numerator| numerator.checked_add(fee_cents))
                    .expect("총비용")
                    <= usd.checked_mul(100).expect("잔고")
            {
                result = quantity;
            }
        }
        result
    }
    #[test]
    fn all_constraints_match_rational_maximum_and_one_more_share_fails() {
        TestRunner::new(Config::with_cases(4096))
            .run(
                &(100_000_i64..1_000_000_000, 1_i64..100, 1_i64..5, 0_i64..100),
                |(krw, usd, price, fee)| {
                    let mut fixture = Fixture::new(Market::Us, krw, usd, price, (30, fee));
                    fixture.policy = fixture.policy.clone().with_minimum_cash(
                        ProfileWeight::parse_percent(
                            &NumericInput::try_new("99.995").expect("입력"),
                        )
                        .expect("하한"),
                    );
                    reissue(&mut fixture);
                    let total = krw
                        .checked_add(usd.checked_mul(1000).expect("달러 평가"))
                        .expect("총자산");
                    let expected = rational_maximum(total, usd, price, fee);
                    match fixture.calculate(&[]) {
                        BuyQuantityDecision::Plan(plan) => {
                            proptest::prop_assert!(expected > 0);
                            proptest::prop_assert_eq!(
                                plan.quantity().quantity().as_decimal(),
                                Decimal::from(expected)
                            );
                            let candidate = PolicyCandidate::new(
                                plan.instrument(),
                                money(
                                    expected
                                        .checked_mul(price)
                                        .and_then(|cost| cost.checked_mul(1000))
                                        .expect("원화 비용"),
                                    Currency::Krw,
                                ),
                            )
                            .expect("후보");
                            proptest::prop_assert_eq!(
                                fixture.policy.check_buy(
                                    &fixture.portfolio,
                                    &fixture.assignments,
                                    &fixture.securities,
                                    &candidate
                                ),
                                PolicyBuyDecision::Pass
                            );
                        }
                        BuyQuantityDecision::None(_) => {
                            proptest::prop_assert_eq!(expected, 0);
                        }
                    }
                    Ok(())
                },
            )
            .expect("모든 제약 최대값");
    }
    #[test]
    fn fee_validity_precision_and_wrong_currency_observations_are_rejected() {
        let mut fixture = Fixture::new(Market::Us, 100_000_000, 100, 1, (30, 1));
        fixture.fee = CommissionRateFact::from_commissions_result(
            Market::Us,
            CostRate::try_new(Ratio::new(Decimal::new(1, 4))).expect("요율"),
            CommissionValidity::try_new(Some(at(63, &Market::Us).offset_calendar_date()), None)
                .expect("기간"),
        );
        assert_eq!(
            fixture.calculate(&[]),
            BuyQuantityDecision::None(NoBuyQuantity::Indeterminate)
        );
        fixture.fee = CommissionRateFact::from_commissions_result(
            Market::Us,
            CostRate::try_new(Ratio::new(Decimal::new(1, 4))).expect("요율"),
            CommissionValidity::try_new(None, None).expect("기간"),
        );
        fixture.price = PriceObservation::from_quote_parts(
            fixture.price.instrument().clone(),
            Price::try_new(Money::new(Amount::new(Decimal::new(1, 28)), Currency::Usd))
                .expect("단가"),
            fixture.now,
        )
        .expect("관측");
        assert_eq!(
            fixture.calculate(&[]),
            BuyQuantityDecision::None(NoBuyQuantity::Indeterminate)
        );
        fixture.price = PriceObservation::from_quote_parts(
            fixture.price.instrument().clone(),
            Price::try_new(money(1, Currency::Krw)).expect("단가"),
            fixture.now,
        )
        .expect("관측");
        assert_eq!(
            fixture.calculate(&[]),
            BuyQuantityDecision::None(NoBuyQuantity::InvalidPriceOrFx)
        );
        let record = BuySignalRecord::from_signal(&fixture.signal, fixture.now, &fixture.calendars)
            .expect("기록");
        assert_eq!(
            BuySignalRecord::from_signal(
                &fixture.signal,
                record.interval().end(),
                &fixture.calendars
            ),
            Err(NoBuyQuantity::Indeterminate)
        );
    }
    #[test]
    fn sizing_helpers_propagate_invalid_amount_and_policy_facts() {
        let mut fixture = Fixture::new(Market::Us, 100_000, 100, 2, (30, 0));
        let one = Quantity::try_new(Decimal::ONE).expect("수량");
        let maximum = Quantity::try_new(Decimal::MAX).expect("상한");
        let initial = (one, BuyQuantityConstraint::TargetWeight);
        assert_eq!(
            super::finish_plan(
                &fixture.signal,
                &fixture.facts(),
                fixture.now,
                maximum,
                initial.1
            ),
            Err(NoBuyQuantity::Indeterminate)
        );
        let check = |_| Ok(crate::CheckResult::Passed);
        assert_eq!(
            super::within_amount(&fixture.facts(), one, fixture.now, check),
            Ok(crate::CheckResult::Passed)
        );
        assert_eq!(
            super::within_amount(&fixture.facts(), maximum, fixture.now, check),
            Err(NoBuyQuantity::Indeterminate)
        );
        assert_eq!(
            super::fee_fits(
                &fixture.facts(),
                fixture.now,
                money(100, Currency::Usd),
                maximum
            ),
            Err(NoBuyQuantity::Indeterminate)
        );
        fixture.fx = OrderFxRates::missing();
        assert_eq!(
            super::size_policy(&fixture.signal, &fixture.facts(), fixture.now, initial),
            Err(NoBuyQuantity::InvalidPriceOrFx)
        );
        assert_eq!(
            super::size_quantity(
                &fixture.signal,
                &fixture.facts(),
                fixture.now,
                money(100, Currency::Usd),
                money(-1, Currency::Krw)
            ),
            Err(NoBuyQuantity::Indeterminate)
        );
    }
    #[test]
    fn final_candidate_and_cash_helpers_reject_invalid_costs() {
        let mut fixture = Fixture::new(Market::Kr, 100_000, 0, 2, (30, 0));
        let whole =
            crate::WholeOrderQuantity::try_new(Quantity::try_new(Decimal::ONE).expect("수량"))
                .expect("정수");
        let amount = crate::OrderExpectedAmount::from_order(
            money(0, Currency::Krw),
            crate::OrderAmountBasis::Estimated,
            fixture.fx,
            fixture.now,
        );
        assert_eq!(
            super::finish_amount(
                &fixture.signal,
                &fixture.facts(),
                fixture.now,
                (whole, BuyQuantityConstraint::TargetWeight),
                amount
            ),
            Err(NoBuyQuantity::Indeterminate)
        );
        fixture.price = PriceObservation::from_quote_parts(
            fixture.price.instrument().clone(),
            Price::try_new(Money::new(Amount::new(Decimal::MAX), Currency::Krw)).expect("단가"),
            fixture.now,
        )
        .expect("관측");
        assert_eq!(
            super::size_cash(
                &fixture.facts(),
                fixture.now,
                Money::new(Amount::new(Decimal::MAX), Currency::Krw),
                (
                    Quantity::try_new(Decimal::from(2_i32)).expect("수량"),
                    BuyQuantityConstraint::TargetWeight
                )
            ),
            Err(NoBuyQuantity::Indeterminate)
        );
        fixture.portfolio = Portfolio::new(
            vec![],
            BuyingPower::new(money(-1, Currency::Krw), fixture.now),
            BuyingPower::new(money(0, Currency::Usd), fixture.now),
        )
        .expect("포트")
        .evaluate(None);
        assert_eq!(
            fixture.calculate(&[]),
            BuyQuantityDecision::None(NoBuyQuantity::Indeterminate)
        );
    }
}
