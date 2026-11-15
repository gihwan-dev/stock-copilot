use crate::performance_arithmetic::Number;
use crate::{
    BootstrapInterval, BootstrapSeed, CalibrationKey, Currency, EstimateTask, Market,
    PerformanceValue, Ratio, SignalBenchmark, TimeInterval, Timestamp, TradeCost, VirtualExit,
};
use rust_decimal::Decimal;

#[derive(Clone, PartialEq, Eq)]
pub struct SignalReturnCost {
    cost: TradeCost,
    buy: Ratio,
    sell: Ratio,
}
impl std::fmt::Debug for SignalReturnCost {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SignalReturnCost")
    }
}
impl SignalReturnCost {
    const fn new(cost: TradeCost, buy: Ratio, sell: Ratio) -> Self {
        Self { cost, buy, sell }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignalReturnError {
    Cost,
    ExitUnavailable,
    BenchmarkMismatch,
    Arithmetic,
    ConflictingReturn,
    Scorecard(crate::ScorecardError),
    TooManyInputs,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BenchmarkAlignment {
    Exact,
    DailyCloseApproximation,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SignalExcess {
    value: PerformanceValue,
    alignment: BenchmarkAlignment,
}
impl SignalExcess {
    #[must_use]
    pub const fn value(self) -> PerformanceValue {
        self.value
    }
    #[must_use]
    pub const fn alignment(self) -> BenchmarkAlignment {
        self.alignment
    }
}
impl std::fmt::Display for SignalExcess {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.value.fmt(f)
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignalReturnBasis {
    NetOfBuyAndSellCosts,
    GrossSellAvoidance,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignalReturn {
    instrument: crate::InstrumentId,
    task_id: crate::ScorecardTaskId,
    estimate_id: Option<crate::CalibrationEstimateId>,
    direction: crate::ScorecardSignalDirection,
    origin: Option<crate::ScorecardSignalRecord>,
    interval: TimeInterval,
    exit_at: crate::VirtualExitMoment,
    entry_day: crate::CalendarDate,
    cluster_day: crate::CalendarDate,
    exit_day: crate::CalendarDate,
    currency: Currency,
    net: PerformanceValue,
    excess: Result<SignalExcess, SignalReturnError>,
}
impl SignalReturn {
    #[must_use]
    pub const fn task_id(&self) -> &crate::ScorecardTaskId {
        &self.task_id
    }
    #[must_use]
    pub const fn estimate_id(&self) -> Option<&crate::CalibrationEstimateId> {
        self.estimate_id.as_ref()
    }
    #[must_use]
    pub const fn direction(&self) -> crate::ScorecardSignalDirection {
        self.direction
    }
    #[must_use]
    pub const fn instrument(&self) -> &crate::InstrumentId {
        &self.instrument
    }
    #[must_use]
    pub fn entry_at(&self) -> Timestamp {
        self.interval.start()
    }
    #[must_use]
    pub fn maturity(&self) -> Timestamp {
        self.interval.end()
    }
    #[must_use]
    pub const fn exit_at(&self) -> crate::VirtualExitMoment {
        self.exit_at
    }
    #[must_use]
    pub const fn cluster_day(&self) -> crate::CalendarDate {
        self.cluster_day
    }
    #[must_use]
    pub const fn net(&self) -> PerformanceValue {
        self.net
    }
    #[must_use]
    pub const fn basis(&self) -> SignalReturnBasis {
        match self.direction {
            crate::ScorecardSignalDirection::Buy => SignalReturnBasis::NetOfBuyAndSellCosts,
            crate::ScorecardSignalDirection::SellAvoidance => SignalReturnBasis::GrossSellAvoidance,
        }
    }
    pub const fn excess(&self) -> Result<SignalExcess, SignalReturnError> {
        self.excess
    }
    #[must_use]
    pub const fn currency(&self) -> Currency {
        self.currency
    }
    #[must_use]
    pub const fn entry_day(&self) -> crate::CalendarDate {
        self.entry_day
    }
    #[must_use]
    pub const fn exit_day(&self) -> crate::CalendarDate {
        self.exit_day
    }
}
pub fn signal_return(
    task: &EstimateTask,
    exit: VirtualExit,
    cost: &SignalReturnCost,
    benchmark: &SignalBenchmark,
) -> Result<SignalReturn, SignalReturnError> {
    let fill = exit.into_fill().ok_or(SignalReturnError::ExitUnavailable)?;
    let direction = fill.identity().map_or(
        crate::ScorecardSignalDirection::Buy,
        crate::ScorecardSignalRecord::direction,
    );
    if fill.task() != task {
        return Err(SignalReturnError::ConflictingReturn);
    }
    if direction == crate::ScorecardSignalDirection::Buy
        && (cost.cost.period().task() != task
            || cost.cost.period().interval().end() != fill.interval().end())
    {
        return Err(SignalReturnError::Cost);
    }
    let entry = fill.entry();
    let currency = entry.as_money().currency();
    let expected = match currency {
        Currency::Krw => Market::Kr,
        Currency::Usd => Market::Us,
    };
    if direction == crate::ScorecardSignalDirection::Buy && cost.cost.market() != expected {
        return Err(SignalReturnError::Cost);
    }
    let asset = asset_return(entry, fill.price(), cost, direction)?;
    let net = directed(asset, direction);
    let entry_day = fill.entry_day();
    let exit_day = fill.exit_day();
    let comparison = benchmark
        .between_dates(entry_day, exit_day, currency)
        .and_then(|benchmark_value| {
            asset
                .number()
                .sub(benchmark_value.number())
                .map(PerformanceValue::from_number)
                .map(|value| directed(value, direction))
                .map_err(arithmetic)
        });
    let alignment = if fill.benchmark_ready() == crate::CheckResult::Passed && entry_day != exit_day
    {
        BenchmarkAlignment::Exact
    } else {
        BenchmarkAlignment::DailyCloseApproximation
    };
    let excess = comparison.map(|value| SignalExcess { value, alignment });
    Ok(SignalReturn {
        instrument: task.volatility().instrument().clone(),
        task_id: crate::ScorecardTaskId::from_task(task),
        estimate_id: fill.identity().map(|record| record.estimate_id().clone()),
        origin: fill.identity().cloned(),
        direction,
        interval: fill.interval(),
        exit_at: fill.moment(),
        entry_day,
        cluster_day: entry_day,
        exit_day,
        currency,
        net,
        excess,
    })
}
fn asset_return(
    entry: crate::Price,
    exit: crate::Price,
    cost: &SignalReturnCost,
    direction: crate::ScorecardSignalDirection,
) -> Result<PerformanceValue, SignalReturnError> {
    match direction {
        crate::ScorecardSignalDirection::Buy => net_return(entry, exit, cost),
        crate::ScorecardSignalDirection::SellAvoidance => {
            Number::new(exit.as_money().amount().as_decimal())
                .div(Number::new(entry.as_money().amount().as_decimal()))
                .and_then(|value| value.sub(Number::new(Decimal::ONE)))
                .map(PerformanceValue::from_number)
                .map_err(arithmetic)
        }
    }
}
fn directed(
    value: PerformanceValue,
    direction: crate::ScorecardSignalDirection,
) -> PerformanceValue {
    match direction {
        crate::ScorecardSignalDirection::Buy => value,
        crate::ScorecardSignalDirection::SellAvoidance => {
            PerformanceValue::from_number(value.number().neg())
        }
    }
}
#[derive(Clone, Copy)]
enum CostSide {
    Buy,
    Sell,
}
fn cost_factor(rate: Ratio, side: CostSide) -> Result<Number, crate::PerformanceError> {
    let unit = Number::new(Decimal::ONE);
    let value = Number::new(rate.as_decimal());
    match side {
        CostSide::Buy => unit.add(value),
        CostSide::Sell => unit.sub(value),
    }
}
fn net_return(
    entry: crate::Price,
    exit: crate::Price,
    cost: &SignalReturnCost,
) -> Result<PerformanceValue, SignalReturnError> {
    cost_factor(cost.buy, CostSide::Buy)
        .and_then(|paid| cost_factor(cost.sell, CostSide::Sell).map(|retained| (paid, retained)))
        .and_then(|(paid, retained)| {
            let after_buy = Number::new(exit.as_money().amount().as_decimal())
                .div(Number::new(entry.as_money().amount().as_decimal()))
                .and_then(|growth| growth.mul(retained))
                .and_then(|after_sell| after_sell.div(paid))?;
            after_buy.sub(Number::new(Decimal::ONE))
        })
        .map(PerformanceValue::from_number)
        .map_err(arithmetic)
}

fn arithmetic<Error>(_error: Error) -> SignalReturnError {
    SignalReturnError::Arithmetic
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignalReturnCount(usize);
impl SignalReturnCount {
    #[must_use]
    pub const fn as_usize(&self) -> usize {
        self.0
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScorecardEdge {
    cards: crate::Scorecards,
    confidence: BootstrapInterval,
    unavailable: SignalReturnCount,
    approximate: SignalReturnCount,
}
impl ScorecardEdge {
    #[must_use]
    pub const fn scorecards(&self) -> &crate::Scorecards {
        &self.cards
    }
    #[must_use]
    pub const fn confidence(&self) -> &BootstrapInterval {
        &self.confidence
    }
    #[must_use]
    pub const fn approximate_samples(&self) -> &SignalReturnCount {
        &self.approximate
    }
    #[must_use]
    pub const fn unavailable(&self) -> &SignalReturnCount {
        &self.unavailable
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScorecardReturnObservation {
    original: crate::ScorecardObservation,
    value: SignalReturn,
}
impl ScorecardReturnObservation {
    pub fn try_new(
        original: crate::ScorecardObservation,
        value: SignalReturn,
    ) -> Result<Self, SignalReturnError> {
        let (id, _, outcome) = crate::scorecard_input::scorecard_return_identity(&original);
        let evidence = outcome
            .evidence()
            .ok_or(SignalReturnError::ExitUnavailable)?;
        match outcome.state() {
            crate::OutcomeState::Ongoing | crate::OutcomeState::Unavailable(_) => {
                return Err(SignalReturnError::ExitUnavailable);
            }
            crate::OutcomeState::Success | crate::OutcomeState::Failure(_) => {}
        }
        if value.estimate_id.as_ref() != Some(id)
            || crate::scorecard_input::scorecard_task_id(&original) != Some(&value.task_id)
            || value.origin.as_ref().is_some_and(|signal| {
                crate::scorecard_input::scorecard_matches_signal(&original, signal)
                    != crate::CheckResult::Passed
            })
        {
            return Err(SignalReturnError::ConflictingReturn);
        }
        if evidence.instrument() != value.instrument()
            || value.interval.start() < evidence.interval().start()
            || value.interval.end() != evidence.interval().end()
        {
            return Err(SignalReturnError::ConflictingReturn);
        }
        Ok(Self { original, value })
    }
    #[must_use]
    pub const fn value(&self) -> &SignalReturn {
        &self.value
    }
}
struct ReturnIndex<'a>(
    std::collections::BTreeMap<crate::CalibrationEstimateId, &'a ScorecardReturnObservation>,
);
impl<'a> ReturnIndex<'a> {
    fn new(
        values: &'a [ScorecardReturnObservation],
        key: &CalibrationKey,
    ) -> Result<Self, SignalReturnError> {
        if values.len() > 65536 {
            return Err(SignalReturnError::TooManyInputs);
        }
        let mut index = std::collections::BTreeMap::new();
        for value in values {
            let (id, combination, _) =
                crate::scorecard_input::scorecard_return_identity(&value.original);
            if combination != key {
                continue;
            }
            if let Some(previous) = index.insert(id.clone(), value)
                && previous != value
            {
                return Err(SignalReturnError::ConflictingReturn);
            }
        }
        Ok(Self(index))
    }
}
pub fn scorecard_edge(
    key: &CalibrationKey,
    input: &[crate::ScorecardObservation],
    returns: &[ScorecardReturnObservation],
    now: Timestamp,
    seed: BootstrapSeed,
) -> Result<ScorecardEdge, SignalReturnError> {
    let (cards, selected, completed) =
        crate::scorecard_input::scorecard_edge_facts(input, key, now)
            .map_err(SignalReturnError::Scorecard)?;
    let index = ReturnIndex::new(returns, key)?;
    let mut samples = Vec::new();
    let mut unavailable = SignalReturnCount(0);
    let mut approximate = SignalReturnCount(0);
    for fact in &completed {
        match index
            .0
            .get(crate::scorecard_input::scorecard_return_identity(fact).0)
        {
            None => unavailable.0 = unavailable.0.wrapping_add(1),
            Some(value) => {
                if &value.original != fact {
                    return Err(SignalReturnError::ConflictingReturn);
                }
                if value.value.excess.is_err() {
                    unavailable.0 = unavailable.0.wrapping_add(1);
                }
            }
        }
    }
    for fact in &selected {
        if let Some(first) = index
            .0
            .get(crate::scorecard_input::scorecard_return_identity(fact).0)
            && let Ok(value) = first.value.excess
        {
            if value.alignment == BenchmarkAlignment::DailyCloseApproximation {
                approximate.0 = approximate.0.wrapping_add(1);
            }
            samples.push((first.value.cluster_day, value.value));
        }
    }
    let confidence = if unavailable.0 == 0 {
        crate::bootstrap::interval(&samples, seed).map_err(arithmetic)?
    } else {
        BootstrapInterval::Withheld(
            crate::WithheldReason::IncomparableSamples,
            crate::WithheldCount::new(unavailable.0),
        )
    };
    Ok(ScorecardEdge {
        cards,
        confidence,
        unavailable,
        approximate,
    })
}
pub(crate) fn cost_rates(
    cost: TradeCost,
    commission: crate::CostRate,
    buy_tax: crate::CostRate,
    sell_tax: crate::CostRate,
) -> Result<SignalReturnCost, crate::TradeCostError> {
    commission
        .ratio()
        .checked_add(&buy_tax.ratio())
        .and_then(|buy| {
            commission
                .ratio()
                .checked_add(&sell_tax.ratio())
                .map(|sell| (buy, sell))
        })
        .map_err(crate::TradeCostError::Arithmetic)
        .and_then(validate_rates)
        .map(|(buy, sell)| SignalReturnCost::new(cost, buy, sell))
}
fn validate_rates(rates: (Ratio, Ratio)) -> Result<(Ratio, Ratio), crate::TradeCostError> {
    if !(Decimal::ZERO..=Decimal::ONE).contains(&rates.0.as_decimal())
        || !(Decimal::ZERO..=Decimal::ONE).contains(&rates.1.as_decimal())
    {
        return Err(crate::TradeCostError::InvalidRate);
    }
    Ok(rates)
}

#[cfg(test)]
pub(crate) mod boundary_tests {
    use super::{
        ScorecardReturnObservation, SignalReturnCost, SignalReturnError, scorecard_edge,
        signal_return,
    };
    use crate::candle::tests::price;
    use crate::signal_return_contract::tests::{benchmark, instant, key, task};
    use crate::{
        BootstrapSeed, Currency, EstimateContext, EstimateTask, Horizon, HorizonSelection,
        HoursPermission, Market, PerformanceValue, PriceObservation, ResolvedOrderInstrument,
        RiskAppetite, Symbol,
    };
    pub(crate) fn identify_fixture(
        original: crate::ScorecardObservation,
        mut value: super::SignalReturn,
    ) -> (crate::ScorecardObservation, super::SignalReturn) {
        value.estimate_id = Some(
            crate::scorecard_input::scorecard_return_identity(&original)
                .0
                .clone(),
        );
        let record = crate::scorecard_input::identified_scorecard(original, value.task_id.clone());
        (record, value)
    }
    fn source(currency: Currency, amount: &str) -> EstimateTask {
        let original = task();
        let market = match currency {
            Currency::Krw => Market::Kr,
            Currency::Usd => Market::Us,
        };
        let offset = match currency {
            Currency::Krw => 32400_i32,
            Currency::Usd => -18000_i32,
        };
        let at = crate::Timestamp::from_parts(
            instant(61, 0).utc_seconds(),
            crate::Nanosecond::new(0),
            crate::OffsetSeconds::new(offset),
        )
        .expect("현지 시각");
        let reference = PriceObservation::from_identified_quote_parts(
            original.volatility().instrument().clone(),
            ResolvedOrderInstrument::from_master_lookup_result(
                Symbol::try_new("ABC").expect("코드"),
                market,
            )
            .expect("시장"),
            price(amount, currency),
            at,
        )
        .expect("가격");
        EstimateTask::create(
            EstimateContext::new(reference, at),
            original.volatility().clone(),
            Horizon::new(HorizonSelection::FewDays),
            HoursPermission::default(),
            RiskAppetite::Neutral,
        )
        .expect("과제")
    }
    fn cost(source: &EstimateTask, market: Market, end: crate::Timestamp) -> SignalReturnCost {
        let zero =
            crate::CostRate::try_new(crate::Ratio::new(rust_decimal::Decimal::ZERO)).expect("요율");
        crate::signal_return_cost(
            Some(crate::CommissionRateFact::from_commissions_result(
                market.clone(),
                zero,
                crate::CommissionValidity::try_new(None, None).expect("기간"),
            )),
            Some(crate::TradeTaxFact::from_schedule_result(
                market, zero, zero,
            )),
            crate::EstimateCostPeriod::from_calendar_result(source, end).expect("기간"),
        )
        .expect("비용")
    }
    #[test]
    fn d29_return_rejects_mismatched_cost_and_overflow_but_keeps_us_native_currency() {
        let source = source(Currency::Usd, "100");
        let fill = crate::virtual_exit::tests::virtual_exit_fixture_fill(
            &source,
            price("110", Currency::Usd),
            instant(66, 4),
        );
        let us_cost = cost(&source, Market::Us, instant(66, 4));
        let value =
            signal_return(&source, fill.clone(), &us_cost, &benchmark()).expect("달러 수익률");
        assert_eq!(value.currency(), Currency::Usd);
        assert_eq!(value.net().to_string(), "0.100000");
        assert_eq!(
            signal_return(&task(), fill.clone(), &us_cost, &benchmark()),
            Err(SignalReturnError::ConflictingReturn)
        );
        assert_eq!(
            signal_return(
                &source,
                fill.clone(),
                &cost(&source, Market::Us, instant(65, 4)),
                &benchmark()
            ),
            Err(SignalReturnError::Cost)
        );
        assert_eq!(
            signal_return(
                &source,
                fill,
                &cost(&source, Market::Kr, instant(66, 4)),
                &benchmark()
            ),
            Err(SignalReturnError::Cost)
        );
        let tiny = super::boundary_tests::source(Currency::Krw, "0.0000000000000000000000000001");
        let extreme = crate::virtual_exit::tests::virtual_exit_fixture_fill(
            &tiny,
            price("79228162514264337593543950335", Currency::Krw),
            instant(66, 4),
        );
        assert_eq!(
            signal_return(
                &tiny,
                extreme,
                &cost(&tiny, Market::Kr, instant(66, 4)),
                &benchmark()
            ),
            Err(SignalReturnError::Arithmetic)
        );
    }
    fn changed_prompt(original: &crate::ScorecardObservation) -> crate::ScorecardObservation {
        let (id, combination, outcome) =
            crate::scorecard_input::scorecard_return_identity(original);
        crate::ScorecardObservation::try_new(
            id.clone(),
            combination.clone(),
            crate::ScorecardPromptId::try_new("other-prompt").expect("프롬프트"),
            crate::AiProbability::parse(&crate::AiProbabilityInput::try_new("60").expect("확률"))
                .expect("확률"),
            outcome.clone(),
        )
        .expect("원본")
    }
    #[test]
    fn d29_conflicting_return_origin_and_input_cap_are_not_silently_accepted() {
        let (record, bound) = crate::signal_return_scenarios::tests::sample(0, "100");
        let mut conflict = bound.clone();
        conflict.value.net = PerformanceValue::from_number(
            crate::performance_arithmetic::Number::new(rust_decimal::Decimal::ONE),
        );
        assert_eq!(
            scorecard_edge(
                &key(),
                std::slice::from_ref(&record),
                &[bound.clone(), conflict],
                instant(300, 4),
                BootstrapSeed::new(0)
            ),
            Err(SignalReturnError::ConflictingReturn)
        );
        assert_eq!(
            scorecard_edge(
                &key(),
                &[changed_prompt(&record)],
                std::slice::from_ref(&bound),
                instant(300, 4),
                BootstrapSeed::new(0)
            ),
            Err(SignalReturnError::ConflictingReturn)
        );
        assert_eq!(
            scorecard_edge(
                &key(),
                std::slice::from_ref(&record),
                &vec![bound.clone(); 65537],
                instant(300, 4),
                BootstrapSeed::new(0)
            ),
            Err(SignalReturnError::TooManyInputs)
        );
        let mut wrong = bound.value.clone();
        wrong.instrument = crate::InstrumentId::try_new("other").expect("종목");
        assert_eq!(
            ScorecardReturnObservation::try_new(record, wrong),
            Err(SignalReturnError::ConflictingReturn)
        );
        let missing =
            crate::signal_return_contract::tests::observation(crate::EstimateOutcome::evaluate(
                &task(),
                &[],
                &crate::signal_return_contract::tests::adjustment(),
                &[],
                instant(66, 4),
            ));
        assert_eq!(
            ScorecardReturnObservation::try_new(missing, bound.value),
            Err(SignalReturnError::ExitUnavailable)
        );
    }
    #[test]
    fn review_binding_rejects_modified_start_maturity_and_task_identity() {
        let (record, bound) = crate::scorecard_signal_review::tests::sample(
            0,
            crate::ScorecardSignalDirection::Buy,
            "0",
            "100",
            "100",
        );
        let mut earlier = bound.value.clone();
        earlier.interval =
            crate::TimeInterval::try_new(instant(60, 4), earlier.maturity()).expect("시작 변조");
        assert_eq!(
            ScorecardReturnObservation::try_new(record.clone(), earlier),
            Err(SignalReturnError::ConflictingReturn)
        );
        let mut later = bound.value.clone();
        later.interval =
            crate::TimeInterval::try_new(later.entry_at(), instant(68, 4)).expect("만기 변조");
        assert_eq!(
            ScorecardReturnObservation::try_new(record.clone(), later),
            Err(SignalReturnError::ConflictingReturn)
        );
        let mut other = bound.value;
        other.task_id =
            crate::ScorecardTaskId::from_task(&crate::signal_return_scenarios::tests::task(7));
        assert_eq!(
            ScorecardReturnObservation::try_new(record, other),
            Err(SignalReturnError::ConflictingReturn)
        );
    }
    #[test]
    fn review_unavailable_excess_within_bound_record_withholds_interval() {
        let (record, mut bound) = crate::scorecard_signal_review::tests::sample(
            0,
            crate::ScorecardSignalDirection::Buy,
            "0",
            "100",
            "100",
        );
        assert_eq!(
            bound.value.basis(),
            super::SignalReturnBasis::NetOfBuyAndSellCosts
        );
        bound.value.excess = Err(SignalReturnError::BenchmarkMismatch);
        let edge = scorecard_edge(
            &key(),
            &[record],
            &[bound],
            instant(300, 4),
            BootstrapSeed::new(0),
        )
        .expect("보류");
        assert_eq!(
            edge.confidence(),
            &crate::BootstrapInterval::Withheld(
                crate::WithheldReason::IncomparableSamples,
                crate::WithheldCount::new(1)
            )
        );
        assert_eq!(crate::WithheldCount::new(1).as_usize(), 1);
    }
    fn other_key() -> crate::CalibrationKey {
        let other_model = crate::CalibrationModel::new(
            crate::ModelIdentityPart::try_new("other").expect("제공자"),
            crate::ModelIdentityPart::try_new("model").expect("모델"),
            crate::ModelIdentityPart::try_new("version").expect("버전"),
        );
        crate::CalibrationKey::new(other_model, Horizon::new(HorizonSelection::FewDays))
    }
    #[test]
    fn d29_other_combination_is_ignored_and_bootstrap_overflow_is_reported() {
        let pairs = (0_i64..30)
            .map(|index| {
                crate::signal_return_scenarios::tests::sample(
                    index.checked_mul(7).expect("간격"),
                    "100",
                )
            })
            .collect::<Vec<_>>();
        let records = pairs
            .iter()
            .map(|(record, _)| record.clone())
            .collect::<Vec<_>>();
        let mut values = pairs
            .into_iter()
            .map(|(_, bound)| bound)
            .collect::<Vec<_>>();
        for value in &mut values {
            value.value.excess = Ok(super::SignalExcess {
                value: PerformanceValue::from_number(crate::performance_arithmetic::Number::new(
                    rust_decimal::Decimal::MAX,
                )),
                alignment: super::BenchmarkAlignment::Exact,
            });
        }
        assert_eq!(
            scorecard_edge(
                &key(),
                &records,
                &values,
                instant(300, 4),
                BootstrapSeed::new(0)
            ),
            Err(SignalReturnError::Arithmetic)
        );
        let other_key = other_key();
        let (id, _, outcome) = crate::scorecard_input::scorecard_return_identity(&records[0]);
        let other = crate::ScorecardObservation::try_new(
            id.clone(),
            other_key,
            crate::ScorecardPromptId::try_new("prompt").expect("프롬프트"),
            crate::AiProbability::parse(&crate::AiProbabilityInput::try_new("60").expect("입력"))
                .expect("확률"),
            outcome.clone(),
        )
        .expect("관측");
        let identified_other =
            crate::scorecard_input::identified_scorecard(other, values[0].value.task_id.clone());
        let value = ScorecardReturnObservation::try_new(identified_other, values[0].value.clone())
            .expect("결합");
        let edge = scorecard_edge(
            &key(),
            &records,
            &[value],
            instant(300, 4),
            BootstrapSeed::new(0),
        )
        .expect("다른 조합 제외");
        assert_eq!(edge.unavailable().as_usize(), 30);
    }
}
