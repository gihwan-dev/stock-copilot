use crate::{ApproximateRatio, Money, MoneyError, Price, Ratio, RatioQuotient, TimeInterval};
use rust_decimal::Decimal;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EstimateEntryError {
    Instrument,
    Time,
    Currency,
}

#[derive(Clone, PartialEq, Eq)]
pub struct EstimateEntryPrice {
    estimate_id: crate::IssuedEstimateTaskId,
    submission: crate::SignalSubmission,
    observation: crate::PriceObservation,
}

impl std::fmt::Debug for EstimateEntryPrice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("EstimateEntryPrice([REDACTED])")
    }
}

impl EstimateEntryPrice {
    pub fn try_new<Run: Clone, Effort>(
        record: &crate::EstimateSubmissionRecord<Run, Effort>,
        observation: crate::PriceObservation,
    ) -> Result<Self, EstimateEntryError> {
        let submission = record.submission();
        let task = submission.task_key().task();
        match observation.check_task_instrument(task) {
            crate::CheckResult::Passed => {}
            crate::CheckResult::Failed | crate::CheckResult::Unknown => {
                return Err(EstimateEntryError::Instrument);
            }
        }
        if observation.observed_at() != submission.submitted_at() {
            return Err(EstimateEntryError::Time);
        }
        if observation.price().as_money().currency()
            != task.reference().price().as_money().currency()
        {
            return Err(EstimateEntryError::Currency);
        }
        Ok(Self {
            estimate_id: record.task_id().clone(),
            submission: submission.clone(),
            observation,
        })
    }

    #[must_use]
    pub const fn estimate_id(&self) -> &crate::CalibrationEstimateId {
        self.estimate_id.as_estimate_id()
    }

    #[must_use]
    pub const fn submission(&self) -> &crate::SignalSubmission {
        &self.submission
    }

    #[must_use]
    pub const fn observation(&self) -> &crate::PriceObservation {
        &self.observation
    }

    #[must_use]
    pub const fn submitted_at(&self) -> crate::Timestamp {
        self.submission.submitted_at()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EstimateReturnError {
    CurrencyMismatch,
    PeriodMismatch,
    NegativeCost,
    Arithmetic(MoneyError),
    OutcomeMismatch,
    UnresolvedOutcome,
    AmbiguousExit,
    EntryTimingPending,
    GapPricePending,
    CostMismatch,
    ExitUnavailable(crate::OutcomeUnavailable),
    InvalidBounds,
    Benchmark(crate::SignalReturnError),
    BenchmarkCalendar(crate::OutcomeUnavailable),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EstimateExitKind {
    Target,
    Stop,
    Maturity,
}

pub struct EstimateReturnFacts<'a> {
    calendars: &'a [crate::MarketCalendar],
    adjustment: &'a crate::OutcomeAdjustment,
    actions: &'a [crate::OutcomeCorporateAction],
    candles: &'a [crate::Candle],
    now: crate::Timestamp,
}
impl std::fmt::Debug for EstimateReturnFacts<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("EstimateReturnFacts([REDACTED])")
    }
}
impl<'a> EstimateReturnFacts<'a> {
    #[must_use]
    pub const fn new(
        calendars: &'a [crate::MarketCalendar],
        adjustment: &'a crate::OutcomeAdjustment,
        actions: &'a [crate::OutcomeCorporateAction],
        candles: &'a [crate::Candle],
        now: crate::Timestamp,
    ) -> Self {
        Self {
            calendars,
            adjustment,
            actions,
            candles,
            now,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EstimateExcess {
    value: RatioQuotient,
    alignment: crate::BenchmarkAlignment,
}
impl EstimateExcess {
    #[must_use]
    pub const fn value(self) -> RatioQuotient {
        self.value
    }
    #[must_use]
    pub const fn alignment(self) -> crate::BenchmarkAlignment {
        self.alignment
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct EstimateReturn {
    entry: EstimateEntryPrice,
    outcome: crate::EstimateOutcome,
    horizon: TimeInterval,
    kind: EstimateExitKind,
    exit_price: Price,
    exit_at: crate::VirtualExitMoment,
    net: RatioQuotient,
    excess: Result<EstimateExcess, EstimateReturnError>,
}
impl std::fmt::Debug for EstimateReturn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("EstimateReturn([REDACTED])")
    }
}
impl EstimateReturn {
    #[must_use]
    pub const fn entry(&self) -> &EstimateEntryPrice {
        &self.entry
    }
    #[must_use]
    pub const fn outcome(&self) -> &crate::EstimateOutcome {
        &self.outcome
    }
    #[must_use]
    pub const fn horizon(&self) -> TimeInterval {
        self.horizon
    }
    #[must_use]
    pub const fn exit_kind(&self) -> EstimateExitKind {
        self.kind
    }
    #[must_use]
    pub const fn exit_price(&self) -> Price {
        self.exit_price
    }
    #[must_use]
    pub const fn exit_at(&self) -> crate::VirtualExitMoment {
        self.exit_at
    }
    #[must_use]
    pub const fn net(&self) -> RatioQuotient {
        self.net
    }
    pub const fn excess(&self) -> Result<EstimateExcess, EstimateReturnError> {
        self.excess
    }
}

pub fn estimate_return(
    entry: &EstimateEntryPrice,
    outcome: &crate::EstimateOutcome,
    facts: &EstimateReturnFacts<'_>,
    cost: &crate::SignalReturnCost,
    benchmark: &crate::SignalBenchmark,
) -> Result<EstimateReturn, EstimateReturnError> {
    let task = entry.submission().task_key().task();
    let evidence = validated_outcome(entry, outcome, facts)?;
    let (signal, kind, fill) = resolved_exit(task, outcome, evidence, facts)?;
    check_timing(entry, kind, &fill, evidence, facts)?;
    adjustment_covers_exit(facts, &fill)?;
    let entry_price = adjusted_entry(entry, &signal, kind, &fill, facts)?;
    let cash = estimate_cash_flows(entry, entry_price, &fill, evidence, cost)?;
    let net = cash.net()?;
    let excess = estimate_excess(entry, net, &fill, facts, benchmark);
    Ok(EstimateReturn {
        entry: entry.clone(),
        outcome: outcome.clone(),
        horizon: cash.period,
        kind,
        exit_price: fill.price(),
        exit_at: fill.moment(),
        net,
        excess,
    })
}

fn validated_outcome<'a>(
    entry: &EstimateEntryPrice,
    outcome: &'a crate::EstimateOutcome,
    facts: &EstimateReturnFacts<'_>,
) -> Result<&'a crate::OutcomeEvidence, EstimateReturnError> {
    let task = entry.submission().task_key().task();
    if entry.submitted_at() == task.reference().observed_at()
        && entry.observation().price() != task.reference().price()
    {
        return Err(EstimateReturnError::OutcomeMismatch);
    }
    let adjustment =
        crate::OutcomeAdjustment::from_actions(task, facts.adjustment.basis(), facts.actions)
            .map_err(EstimateReturnError::ExitUnavailable)?;
    if &adjustment != facts.adjustment
        || crate::EstimateOutcome::evaluate_with_complete_data(
            task,
            facts.calendars,
            facts.adjustment,
            facts.candles,
            facts.now,
        ) != *outcome
    {
        return Err(EstimateReturnError::OutcomeMismatch);
    }
    outcome
        .evidence()
        .ok_or(EstimateReturnError::UnresolvedOutcome)
}

fn resolved_exit(
    task: &crate::EstimateTask,
    outcome: &crate::EstimateOutcome,
    evidence: &crate::OutcomeEvidence,
    facts: &EstimateReturnFacts<'_>,
) -> Result<
    (
        crate::VirtualExitSignal,
        EstimateExitKind,
        crate::VirtualExitFill,
    ),
    EstimateReturnError,
> {
    let signal = crate::VirtualExitSignal::for_estimate(task, evidence.interval().end())
        .map_err(|_error| EstimateReturnError::InvalidBounds)?;
    let exit = crate::virtual_exit(
        &signal,
        facts.calendars,
        facts.adjustment,
        facts.candles,
        facts.now,
    );
    let (kind, fill) = classify_exit(outcome.state(), exit)?;
    Ok((signal, kind, fill))
}

fn classify_exit(
    state: crate::OutcomeState,
    exit: crate::VirtualExit,
) -> Result<(EstimateExitKind, crate::VirtualExitFill), EstimateReturnError> {
    match (state, exit) {
        (crate::OutcomeState::Success, crate::VirtualExit::Target(value)) => {
            Ok((EstimateExitKind::Target, value.into_fill()))
        }
        (
            crate::OutcomeState::Failure(crate::OutcomeFailure::Stop),
            crate::VirtualExit::Stop(value),
        ) => Ok((EstimateExitKind::Stop, value.into_fill())),
        (
            crate::OutcomeState::Failure(crate::OutcomeFailure::Expired),
            crate::VirtualExit::Maturity(value),
        ) => Ok((EstimateExitKind::Maturity, value.into_fill())),
        (crate::OutcomeState::Failure(crate::OutcomeFailure::Ambiguous), _) => {
            Err(EstimateReturnError::AmbiguousExit)
        }
        (crate::OutcomeState::Ongoing | crate::OutcomeState::Unavailable(_), _) => {
            Err(EstimateReturnError::UnresolvedOutcome)
        }
        (_, crate::VirtualExit::Unavailable(reason)) => {
            Err(EstimateReturnError::ExitUnavailable(reason))
        }
        _ => Err(EstimateReturnError::OutcomeMismatch),
    }
}

fn check_timing(
    entry: &EstimateEntryPrice,
    kind: EstimateExitKind,
    fill: &crate::VirtualExitFill,
    evidence: &crate::OutcomeEvidence,
    facts: &EstimateReturnFacts<'_>,
) -> Result<(), EstimateReturnError> {
    let day = entry.observation().observed_at().offset_calendar_date();
    match (kind, fill.moment(), evidence.period()) {
        (EstimateExitKind::Maturity, crate::VirtualExitMoment::At(at), _)
            if entry.submitted_at() < at => {}
        (_, crate::VirtualExitMoment::TradingDay(exit_day), _) if day < exit_day => {}
        (_, crate::VirtualExitMoment::TradingDay(exit_day), _) if day == exit_day => {
            let open = regular_time(exit_day, facts.calendars, SessionBoundary::Open)
                .map_err(EstimateReturnError::ExitUnavailable)?;
            if entry.submitted_at() >= open {
                return Err(EstimateReturnError::EntryTimingPending);
            }
        }
        (_, crate::VirtualExitMoment::At(at), Some(crate::CandlePeriod::Daily))
            if entry.submitted_at() < at => {}
        (_, crate::VirtualExitMoment::At(at), Some(crate::CandlePeriod::Minute)) => {
            match entry.submitted_at().elapsed_until(at) {
                Ok(elapsed) if elapsed >= crate::TimeSpan::MINUTE => {}
                Ok(_) | Err(_) => return Err(EstimateReturnError::EntryTimingPending),
            }
        }
        _ => return Err(EstimateReturnError::EntryTimingPending),
    }
    Ok(())
}

fn adjustment_covers_exit(
    facts: &EstimateReturnFacts<'_>,
    fill: &crate::VirtualExitFill,
) -> Result<(), EstimateReturnError> {
    let at = match fill.moment() {
        crate::VirtualExitMoment::At(at) => at,
        crate::VirtualExitMoment::TradingDay(day) => {
            regular_close(day, facts.calendars).map_err(EstimateReturnError::ExitUnavailable)?
        }
    };
    if *facts.adjustment.basis().as_timestamp() < at {
        return Err(EstimateReturnError::ExitUnavailable(
            crate::OutcomeUnavailable::Adjustment,
        ));
    }
    Ok(())
}

fn regular_close(
    day: crate::CalendarDate,
    calendars: &[crate::MarketCalendar],
) -> Result<crate::Timestamp, crate::OutcomeUnavailable> {
    regular_time(day, calendars, SessionBoundary::Close)
}

#[derive(Clone, Copy)]
enum SessionBoundary {
    Open,
    Close,
}

fn regular_time(
    day: crate::CalendarDate,
    calendars: &[crate::MarketCalendar],
    boundary: SessionBoundary,
) -> Result<crate::Timestamp, crate::OutcomeUnavailable> {
    let times = calendars
        .iter()
        .flat_map(|calendar| {
            [
                calendar.previous_business_day(),
                calendar.today(),
                calendar.next_business_day(),
            ]
        })
        .filter(|row| row.date() == day)
        .flat_map(crate::MarketSession::sessions)
        .filter(|part| part.state() == crate::MarketState::RegularMarket)
        .map(|part| match boundary {
            SessionBoundary::Open => part.session().interval().start(),
            SessionBoundary::Close => part.session().interval().end(),
        })
        .collect::<std::collections::BTreeSet<_>>();
    if times.len() != 1 {
        return Err(crate::OutcomeUnavailable::Calendar);
    }
    times
        .into_iter()
        .next()
        .ok_or(crate::OutcomeUnavailable::Calendar)
}

fn adjusted_entry(
    entry: &EstimateEntryPrice,
    signal: &crate::VirtualExitSignal,
    kind: EstimateExitKind,
    fill: &crate::VirtualExitFill,
    facts: &EstimateReturnFacts<'_>,
) -> Result<Price, EstimateReturnError> {
    let task = signal.task();
    let submitted_day = entry.observation().observed_at().offset_calendar_date();
    let adjustment = crate::estimate_outcome::estimate_entry_adjustment(
        task,
        entry.submitted_at(),
        submitted_day,
        facts.adjustment.basis(),
        facts.actions,
    )
    .map_err(EstimateReturnError::ExitUnavailable)?;
    let price = crate::virtual_exit::adjusted_price(entry.observation().price(), &adjustment)
        .map_err(EstimateReturnError::ExitUnavailable)?;
    let (original_target, original_stop) = signal.estimate_bounds();
    let target = crate::virtual_exit::adjusted_price(original_target, facts.adjustment)
        .map_err(EstimateReturnError::ExitUnavailable)?;
    let stop = crate::virtual_exit::adjusted_price(original_stop, facts.adjustment)
        .map_err(EstimateReturnError::ExitUnavailable)?;
    if price.as_money().amount().as_decimal() >= target.as_money().amount().as_decimal()
        || price.as_money().amount().as_decimal() <= stop.as_money().amount().as_decimal()
    {
        return Err(EstimateReturnError::EntryTimingPending);
    }
    let boundary = match kind {
        EstimateExitKind::Target => Some(target),
        EstimateExitKind::Stop => Some(stop),
        EstimateExitKind::Maturity => None,
    };
    if let Some(expected) = boundary
        && fill.price() != expected
    {
        return Err(EstimateReturnError::GapPricePending);
    }
    Ok(price)
}

fn estimate_cash_flows(
    entry: &EstimateEntryPrice,
    price: Price,
    fill: &crate::VirtualExitFill,
    evidence: &crate::OutcomeEvidence,
    cost: &crate::SignalReturnCost,
) -> Result<EstimateCashFlows, EstimateReturnError> {
    let task = entry.submission().task_key().task();
    let source = cost.trade_cost();
    if source.period().task() != task
        || source.period().interval().end() != evidence.interval().end()
        || source.market() != task.reference().instrument().market()
    {
        return Err(EstimateReturnError::CostMismatch);
    }
    let buy = price
        .as_money()
        .checked_mul(&cost.buy_rate())
        .map_err(EstimateReturnError::Arithmetic)?;
    let sell = fill
        .price()
        .as_money()
        .checked_mul(&cost.sell_rate())
        .map_err(EstimateReturnError::Arithmetic)?;
    let horizon = TimeInterval::try_new(entry.submitted_at(), evidence.interval().end())
        .map_err(|_error| EstimateReturnError::EntryTimingPending)?;
    EstimateCashFlows::try_new(price, fill.price(), buy, sell, horizon)
}

fn estimate_excess(
    entry: &EstimateEntryPrice,
    net: RatioQuotient,
    fill: &crate::VirtualExitFill,
    facts: &EstimateReturnFacts<'_>,
    benchmark: &crate::SignalBenchmark,
) -> Result<EstimateExcess, EstimateReturnError> {
    let submitted_day = entry.observation().observed_at().offset_calendar_date();
    let exit_day = fill.exit_day();
    let currency = entry.observation().price().as_money().currency();
    for day in [submitted_day, exit_day] {
        let expected =
            regular_close(day, facts.calendars).map_err(EstimateReturnError::BenchmarkCalendar)?;
        if benchmark
            .close_on(day, currency)
            .map_err(EstimateReturnError::Benchmark)?
            != expected
        {
            return Err(EstimateReturnError::Benchmark(
                crate::SignalReturnError::BenchmarkMismatch,
            ));
        }
    }
    let alignment = if submitted_day == exit_day {
        crate::BenchmarkAlignment::DailyCloseApproximation
    } else {
        match (
            crate::virtual_exit::entry_close(entry.submitted_at(), facts.calendars),
            fill.exit_close(),
        ) {
            (crate::CheckResult::Passed, crate::CheckResult::Passed) => {
                crate::BenchmarkAlignment::Exact
            }
            (
                crate::CheckResult::Failed | crate::CheckResult::Unknown,
                crate::CheckResult::Passed
                | crate::CheckResult::Failed
                | crate::CheckResult::Unknown,
            )
            | (
                crate::CheckResult::Passed,
                crate::CheckResult::Failed | crate::CheckResult::Unknown,
            ) => crate::BenchmarkAlignment::DailyCloseApproximation,
        }
    };
    benchmark
        .between_dates(submitted_day, exit_day, currency)
        .map_err(EstimateReturnError::Benchmark)
        .and_then(|value| {
            subtract(
                net,
                RatioQuotient::Approximate(ApproximateRatio::new(value.number().as_decimal())),
            )
        })
        .map(|value| EstimateExcess { value, alignment })
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct EstimateCashFlows {
    entry: Price,
    exit: Price,
    buy_cost: Money,
    sell_cost: Money,
    period: TimeInterval,
}

impl std::fmt::Debug for EstimateCashFlows {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("EstimateCashFlows([REDACTED])")
    }
}

impl EstimateCashFlows {
    fn try_new(
        entry: Price,
        exit: Price,
        buy_cost: Money,
        sell_cost: Money,
        period: TimeInterval,
    ) -> Result<Self, EstimateReturnError> {
        let currency = entry.as_money().currency();
        if exit.as_money().currency() != currency
            || buy_cost.currency() != currency
            || sell_cost.currency() != currency
        {
            return Err(EstimateReturnError::CurrencyMismatch);
        }
        if buy_cost.amount().as_decimal() < Decimal::ZERO
            || sell_cost.amount().as_decimal() < Decimal::ZERO
        {
            return Err(EstimateReturnError::NegativeCost);
        }
        Ok(Self {
            entry,
            exit,
            buy_cost,
            sell_cost,
            period,
        })
    }

    fn net(&self) -> Result<RatioQuotient, EstimateReturnError> {
        let paid = self
            .entry
            .as_money()
            .checked_add(&self.buy_cost)
            .map_err(EstimateReturnError::Arithmetic)?;
        let received = self
            .exit
            .as_money()
            .checked_sub(&self.sell_cost)
            .map_err(EstimateReturnError::Arithmetic)?;
        let profit = received
            .checked_sub(&paid)
            .map_err(EstimateReturnError::Arithmetic)?;
        profit
            .checked_ratio(&paid)
            .map_err(EstimateReturnError::Arithmetic)
    }
}

fn subtract(
    left: RatioQuotient,
    right: RatioQuotient,
) -> Result<RatioQuotient, EstimateReturnError> {
    let (first, second) = (ratio_value(left), ratio_value(right));
    let difference = first
        .checked_sub(&second)
        .map_err(EstimateReturnError::Arithmetic)?;
    match (left, right) {
        (RatioQuotient::Exact(_), RatioQuotient::Exact(_)) => Ok(RatioQuotient::Exact(difference)),
        (RatioQuotient::Approximate(_), _) | (_, RatioQuotient::Approximate(_)) => Ok(
            RatioQuotient::Approximate(ApproximateRatio::new(difference.as_decimal())),
        ),
    }
}

fn ratio_value(quotient: RatioQuotient) -> Ratio {
    match quotient {
        RatioQuotient::Exact(value) => value,
        RatioQuotient::Approximate(value) => Ratio::new(value.as_decimal()),
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        CalibrationCapacity, CalibrationError, Currency, Money, Nanosecond, NumericInput,
        OffsetSeconds, Price, Ratio, TimeInterval, Timestamp, UnixSeconds,
    };

    fn amount(value: &str, currency: Currency) -> Money {
        Money::parse(&NumericInput::try_new(value).expect("금액 입력"), currency).expect("금액")
    }

    fn price(value: &str, currency: Currency) -> Price {
        Price::try_new(amount(value, currency)).expect("가격")
    }

    fn ratio(value: &str) -> Ratio {
        Ratio::parse(&NumericInput::try_new(value).expect("비율 입력")).expect("비율")
    }

    fn interval(start: i64, end: i64) -> TimeInterval {
        let at = |value| {
            Timestamp::from_parts(
                UnixSeconds::new(value),
                Nanosecond::new(0),
                OffsetSeconds::new(0),
            )
            .expect("시각")
        };
        TimeInterval::try_new(at(start), at(end)).expect("구간")
    }

    fn eligible_for_task(
        task: &crate::EstimateTask,
        maturity: Timestamp,
    ) -> crate::CostEligibleTask {
        let zero = crate::CostRate::try_new(ratio("0")).expect("0 비용률");
        let cost = crate::TradeCost::from_facts(
            Some(crate::CommissionRateFact::from_commissions_result(
                crate::Market::Kr,
                zero,
                crate::CommissionValidity::try_new(None, None).expect("유효"),
            )),
            Some(crate::TradeTaxFact::from_schedule_result(
                crate::Market::Kr,
                zero,
                zero,
            )),
            crate::EstimateCostPeriod::from_calendar_result(task, maturity).expect("만기"),
        )
        .expect("비용");
        crate::CostEligibleTask::assess(task.clone(), &cost).expect("과제")
    }

    fn record_for_submission(
        eligible: crate::CostEligibleTask,
        submission: crate::SignalSubmission,
        identity: &str,
    ) -> crate::EstimateSubmissionRecord<crate::InstrumentId, crate::HorizonSelection> {
        let id = crate::CalibrationEstimateId::try_new(identity).expect("추정 ID");
        let run = crate::InstrumentId::try_new("run-d54").expect("실행 ID");
        let ledger = crate::EstimateAssignments::default()
            .issue(id.clone(), run, eligible)
            .expect("과제 발급");
        let issued = ledger.lookup(&id).expect("발급 과제");
        let narrative = crate::EstimateNarrative {
            rationale: crate::UntrustedText::try_new("근거").expect("근거"),
            counterargument: crate::UntrustedText::try_new("반론").expect("반론"),
        };
        let metadata = crate::EstimateMetadata {
            effort: crate::HorizonSelection::FewDays,
            cli_version: crate::ModelIdentityPart::try_new("cli").expect("CLI"),
            prompt: crate::EstimatePrompt {
                id: crate::ScorecardPromptId::try_new("prompt").expect("프롬프트"),
                version: crate::ModelIdentityPart::try_new("version").expect("버전"),
                hash: crate::PromptHash::try_new(&"ab".repeat(32)).expect("해시"),
            },
            evidence_bundle: crate::EvidenceBundleId::try_new("bundle").expect("근거 ID"),
        };
        crate::EstimateSubmissionRecord::create(issued, submission, narrative, metadata)
            .expect("제출 기록")
    }

    #[test]
    fn d54_return_keeps_the_issued_estimate_identity() {
        use crate::signal_return_contract::tests::{cost, instant};
        let original = return_fixture("100");
        let submission = original.submission();
        let task = submission.task_key().task();
        let first = record_for_submission(
            eligible_for_task(task, instant(66, 4)),
            submission.clone(),
            "estimate-one",
        );
        let second = record_for_submission(
            eligible_for_task(task, instant(66, 4)),
            submission.clone(),
            "estimate-two",
        );
        let first_entry =
            crate::EstimateEntryPrice::try_new(&first, original.observation().clone())
                .expect("첫 제출가");
        let second_entry =
            crate::EstimateEntryPrice::try_new(&second, original.observation().clone())
                .expect("둘째 제출가");
        let first_return =
            calculate_return(&first_entry, &maturity_bars("100"), &cost("0", "0", "0"))
                .expect("첫 수익률");
        let second_return =
            calculate_return(&second_entry, &maturity_bars("100"), &cost("0", "0", "0"))
                .expect("둘째 수익률");
        assert_eq!(first_return.entry().estimate_id().as_str(), "estimate-one");
        assert_eq!(second_return.entry().estimate_id().as_str(), "estimate-two");
        assert_eq!(first_return.net(), second_return.net());
    }

    fn submission_fixture() -> (
        crate::EstimateTask,
        crate::EstimateSubmissionRecord<crate::InstrumentId, crate::HorizonSelection>,
    ) {
        let task = crate::thresholds::tests::task();
        let zero = crate::CostRate::try_new(ratio("0")).expect("0 비용률");
        let cost = crate::TradeCost::from_facts(
            Some(crate::CommissionRateFact::from_commissions_result(
                crate::Market::Kr,
                zero,
                crate::CommissionValidity::try_new(None, None).expect("유효"),
            )),
            Some(crate::TradeTaxFact::from_schedule_result(
                crate::Market::Kr,
                zero,
                zero,
            )),
            crate::thresholds::tests::cost_period(),
        )
        .expect("비용");
        let eligible = crate::CostEligibleTask::assess(task.clone(), &cost).expect("과제");
        let model_part = crate::ModelIdentityPart::try_new("model").expect("모델");
        let key = crate::CalibrationKey::new(
            crate::CalibrationModel::new(model_part.clone(), model_part.clone(), model_part),
            crate::Horizon::default(),
        );
        let time = task
            .created_at()
            .checked_add(
                crate::TimeSpan::try_new(crate::SpanSeconds::new(10), Nanosecond::new(0))
                    .expect("기간"),
            )
            .expect("제출 시각");
        let submission = crate::SignalSubmission::create(
            crate::SignalTaskKey::from_task(&eligible),
            key,
            crate::AiProbability::parse(&crate::AiProbabilityInput::try_new("60").expect("입력"))
                .expect("확률"),
            None,
            time,
        )
        .expect("추정 제출");
        (
            task,
            record_for_submission(eligible, submission, "fixture-id"),
        )
    }

    fn observed(
        instrument: crate::InstrumentId,
        resolved: crate::ResolvedOrderInstrument,
        value: Price,
        at: Timestamp,
    ) -> crate::PriceObservation {
        crate::PriceObservation::from_identified_quote_parts(instrument, resolved, value, at)
            .expect("관측")
    }

    fn return_fixture_at(value: &str, time: Timestamp) -> crate::EstimateEntryPrice {
        return_fixture_for_task(&crate::signal_return_contract::tests::task(), value, time)
    }

    fn return_fixture_for_task(
        task: &crate::EstimateTask,
        value: &str,
        time: Timestamp,
    ) -> crate::EstimateEntryPrice {
        use crate::signal_return_contract::tests::instant;
        let eligible = eligible_for_task(task, instant(66, 4));
        let key = crate::signal_return_contract::tests::key();
        let submission = crate::SignalSubmission::create(
            crate::SignalTaskKey::from_task(&eligible),
            key,
            crate::AiProbability::parse(&crate::AiProbabilityInput::try_new("60").expect("입력"))
                .expect("확률"),
            None,
            time,
        )
        .expect("제출");
        let observation = observed(
            task.volatility().instrument().clone(),
            task.reference().instrument().clone(),
            price(value, Currency::Krw),
            time,
        );
        let record = record_for_submission(eligible, submission, "estimate-default");
        crate::EstimateEntryPrice::try_new(&record, observation).expect("진입")
    }

    fn return_fixture(value: &str) -> crate::EstimateEntryPrice {
        return_fixture_at(value, crate::signal_return_contract::tests::instant(61, 1))
    }

    fn calculate_return(
        entry: &crate::EstimateEntryPrice,
        bars: &[crate::Candle],
        cost: &crate::SignalReturnCost,
    ) -> Result<crate::EstimateReturn, crate::EstimateReturnError> {
        use crate::signal_return_contract::tests::{adjustment, benchmark, calendars, instant};
        let task = entry.submission().task_key().task();
        let calendar = calendars();
        let adjustment = adjustment();
        let now = instant(66, 4);
        let outcome = crate::EstimateOutcome::evaluate_with_complete_data(
            task,
            &calendar,
            &adjustment,
            bars,
            now,
        );
        let facts = crate::EstimateReturnFacts::new(&calendar, &adjustment, &[], bars, now);
        crate::estimate_return(entry, &outcome, &facts, cost, &benchmark())
    }

    fn maturity_bars(close: &str) -> Vec<crate::Candle> {
        use crate::signal_return_contract::tests::{candle, entry_bars};
        let mut bars = entry_bars();
        bars.extend((62_i64..66).map(|day| candle(day, 0, crate::CandlePeriod::Daily, ["100"; 4])));
        let closing = price(close, Currency::Krw).as_money().amount().as_decimal();
        let values = if closing < rust_decimal::Decimal::ONE_HUNDRED {
            ["100", "100", close, close]
        } else {
            ["100", close, "100", close]
        };
        bars.push(candle(66, 0, crate::CandlePeriod::Daily, values));
        bars
    }

    #[test]
    fn d54_resolved_return_monotonicity_and_cost_properties() {
        use crate::signal_return_contract::tests::{
            adjustment, benchmark, calendars, cost, instant,
        };
        use proptest::test_runner::{Config, TestRunner};
        use proptest::{prop_assert, prop_assert_eq};
        let entry = return_fixture("100");
        let task = entry.submission().task_key().task();
        let calendar = calendars();
        let adjustment = adjustment();
        let benchmark = benchmark();
        let free = cost("0", "0", "0");
        let charged = cost("0.001", "0", "0.002");
        let mut runner = TestRunner::new(Config::with_cases(testkit::property_cases()));
        runner
            .run(&(9900_u32..10001, 0_u32..=100), |(first, increment)| {
                let second = first.checked_add(increment).expect("범위");
                let decimal = |number: u32| {
                    format!(
                        "{}.{:02}",
                        number.checked_div(100).expect("원"),
                        number.checked_rem(100).expect("소수")
                    )
                };
                let first_bars = maturity_bars(&decimal(first));
                let second_bars = maturity_bars(&decimal(second));
                let value = |bars: &[crate::Candle], fee: &crate::SignalReturnCost| {
                    let now = instant(66, 4);
                    let outcome = crate::EstimateOutcome::evaluate_with_complete_data(
                        task,
                        &calendar,
                        &adjustment,
                        bars,
                        now,
                    );
                    let facts =
                        crate::EstimateReturnFacts::new(&calendar, &adjustment, &[], bars, now);
                    crate::estimate_return(&entry, &outcome, &facts, fee, &benchmark)
                        .expect("만기 수익률")
                };
                let lower = value(&first_bars, &free);
                let higher = value(&second_bars, &free);
                let with_cost = value(&second_bars, &charged);
                prop_assert!(super::ratio_value(lower.net()) <= super::ratio_value(higher.net()));
                prop_assert!(
                    super::ratio_value(with_cost.net()) <= super::ratio_value(higher.net())
                );
                prop_assert_eq!(
                    super::ratio_value(higher.net())
                        .as_decimal()
                        .cmp(&rust_decimal::Decimal::ZERO),
                    second.cmp(&10_000)
                );
                prop_assert_eq!(
                    super::ratio_value(higher.net()),
                    super::ratio_value(higher.excess().expect("초과 수익").value())
                );
                Ok(())
            })
            .expect("수익률 속성");
    }

    #[test]
    fn d54_resolved_outcome_uses_submission_price_and_goal_stop_or_maturity() {
        use crate::signal_return_contract::tests::{candle, cost, entry_bars};
        use crate::{CandlePeriod, EstimateExitKind};
        let entry = return_fixture("100");
        let task = entry.submission().task_key().task();
        let zero_cost = cost("0", "0", "0");
        for (bar, kind, factor) in [
            (
                ["100", "200", "100", "100"],
                EstimateExitKind::Target,
                task.target().as_decimal(),
            ),
            (
                ["100", "100", "1", "100"],
                EstimateExitKind::Stop,
                -task.stop().as_decimal(),
            ),
        ] {
            let mut bars = entry_bars();
            bars.push(candle(62, 0, CandlePeriod::Daily, bar));
            let result = calculate_return(&entry, &bars, &zero_cost).expect("판정된 수익률");
            assert_eq!(result.exit_kind(), kind);
            let expected = task
                .reference()
                .price()
                .as_money()
                .checked_mul(&Ratio::new(
                    rust_decimal::Decimal::ONE
                        .checked_add(factor)
                        .expect("경계"),
                ))
                .expect("가격");
            assert_eq!(result.exit_price().as_money(), expected);
        }
    }

    #[test]
    fn d54_maturity_uses_submission_price_and_proportional_costs() {
        use crate::signal_return_contract::tests::{candle, cost, entry_bars};
        use crate::{CandlePeriod, EstimateExitKind, RatioQuotient};
        let entry = return_fixture("100");
        let zero_cost = cost("0", "0", "0");
        let mut bars = entry_bars();
        bars.extend((62_i64..=66).map(|day| candle(day, 0, CandlePeriod::Daily, ["100"; 4])));
        let result = calculate_return(&entry, &bars, &zero_cost).expect("만기 수익률");
        assert_eq!(result.exit_kind(), EstimateExitKind::Maturity);
        assert_eq!(result.exit_price(), price("100", Currency::Krw));
        assert_eq!(
            result.outcome().state(),
            crate::OutcomeState::Failure(crate::OutcomeFailure::Expired)
        );
        assert_eq!(result.net(), RatioQuotient::Exact(ratio("0")));
        assert_eq!(
            result.excess().expect("벤치마크").alignment(),
            crate::BenchmarkAlignment::DailyCloseApproximation
        );
        let different_entry = return_fixture("99");
        let different =
            calculate_return(&different_entry, &bars, &zero_cost).expect("제출 가격 사용");
        assert_eq!(
            different.net(),
            RatioQuotient::Approximate(crate::ApproximateRatio::new(
                ratio("0.0101010101010101010101010101").as_decimal(),
            ))
        );
        let with_cost =
            calculate_return(&entry, &bars, &cost("0.001", "0", "0.002")).expect("비용 수익률");
        assert_eq!(
            super::ratio_value(with_cost.net()),
            ratio("-0.003996003996003996003996004")
        );
        assert_eq!(with_cost.entry(), &entry);
        assert_eq!(with_cost.horizon().start(), entry.submitted_at());
    }

    #[test]
    fn d54_return_refuses_outcome_from_different_candles() {
        use crate::signal_return_contract::tests::{
            adjustment, benchmark, calendars, candle, cost, entry_bars, instant,
        };
        let entry = return_fixture("100");
        let task = entry.submission().task_key().task();
        let calendar = calendars();
        let adjustment = adjustment();
        let mut target = entry_bars();
        target.push(candle(
            62,
            0,
            crate::CandlePeriod::Daily,
            ["100", "200", "100", "100"],
        ));
        let outcome = crate::EstimateOutcome::evaluate_with_complete_data(
            task,
            &calendar,
            &adjustment,
            &target,
            instant(66, 4),
        );
        let mut stop = entry_bars();
        stop.push(candle(
            62,
            0,
            crate::CandlePeriod::Daily,
            ["100", "100", "1", "100"],
        ));
        let facts =
            crate::EstimateReturnFacts::new(&calendar, &adjustment, &[], &stop, instant(66, 4));
        assert_eq!(
            crate::estimate_return(&entry, &outcome, &facts, &cost("0", "0", "0"), &benchmark()),
            Err(crate::EstimateReturnError::OutcomeMismatch)
        );
    }

    #[test]
    fn d54_ambiguous_failure_has_a_distinct_exit_error() {
        use crate::signal_return_contract::tests::{
            adjustment, benchmark, calendars, candle, cost, entry_bars, instant,
        };
        let entry = return_fixture("100");
        let task = entry.submission().task_key().task();
        let calendar = calendars();
        let adjustment = adjustment();
        let mut bars = entry_bars();
        bars.push(candle(
            62,
            0,
            crate::CandlePeriod::Daily,
            ["100", "200", "1", "100"],
        ));
        bars.push(candle(
            62,
            1,
            crate::CandlePeriod::Minute,
            ["100", "200", "1", "100"],
        ));
        let now = instant(66, 4);
        let outcome = crate::EstimateOutcome::evaluate_with_complete_data(
            task,
            &calendar,
            &adjustment,
            &bars,
            now,
        );
        assert_eq!(
            outcome.state(),
            crate::OutcomeState::Failure(crate::OutcomeFailure::Ambiguous)
        );
        let facts = crate::EstimateReturnFacts::new(&calendar, &adjustment, &[], &bars, now);
        assert_eq!(
            crate::estimate_return(&entry, &outcome, &facts, &cost("0", "0", "0"), &benchmark()),
            Err(crate::EstimateReturnError::AmbiguousExit)
        );
    }

    #[test]
    fn d54_inconsistent_simultaneous_prices_and_bad_action_are_rejected() {
        use crate::signal_return_contract::tests::{
            adjustment, benchmark, calendars, cost, instant,
        };
        let invalid_entry = return_fixture_at("99", instant(61, 0));
        let bars = maturity_bars("100");
        assert_eq!(
            calculate_return(&invalid_entry, &bars, &cost("0", "0", "0")),
            Err(crate::EstimateReturnError::OutcomeMismatch)
        );
        let entry = return_fixture("100");
        let task = entry.submission().task_key().task();
        let calendar = calendars();
        let adjustment = adjustment();
        let outcome = crate::EstimateOutcome::evaluate_with_complete_data(
            task,
            &calendar,
            &adjustment,
            &bars,
            instant(66, 4),
        );
        let action = split_action(63);
        let wrong = [crate::OutcomeCorporateAction::new(
            crate::InstrumentId::try_new("other").expect("다른 종목"),
            action.action().clone(),
        )];
        let facts =
            crate::EstimateReturnFacts::new(&calendar, &adjustment, &wrong, &bars, instant(66, 4));
        assert_eq!(
            crate::estimate_return(&entry, &outcome, &facts, &cost("0", "0", "0"), &benchmark()),
            Err(crate::EstimateReturnError::ExitUnavailable(
                crate::OutcomeUnavailable::Adjustment
            ))
        );
    }

    #[test]
    fn d54_unresolved_outcome_and_incompatible_exit_are_not_returns() {
        use crate::signal_return_contract::tests::{
            adjustment, benchmark, calendars, cost, entry_bars, instant, task,
        };
        let entry = return_fixture("100");
        let calendar = calendars();
        let adjustment = adjustment();
        let missing_now = instant(66, 4);
        let missing = crate::EstimateOutcome::evaluate_with_complete_data(
            &task(),
            &calendar,
            &adjustment,
            &[],
            missing_now,
        );
        let missing_facts =
            crate::EstimateReturnFacts::new(&calendar, &adjustment, &[], &[], missing_now);
        assert_eq!(
            crate::estimate_return(
                &entry,
                &missing,
                &missing_facts,
                &cost("0", "0", "0"),
                &benchmark()
            ),
            Err(crate::EstimateReturnError::UnresolvedOutcome)
        );
        let bars = entry_bars();
        let ongoing_now = instant(62, 0);
        let ongoing = crate::EstimateOutcome::evaluate_with_complete_data(
            &task(),
            &calendar,
            &adjustment,
            &bars,
            ongoing_now,
        );
        let ongoing_facts =
            crate::EstimateReturnFacts::new(&calendar, &adjustment, &[], &bars, ongoing_now);
        assert_eq!(
            crate::estimate_return(
                &entry,
                &ongoing,
                &ongoing_facts,
                &cost("0", "0", "0"),
                &benchmark()
            ),
            Err(crate::EstimateReturnError::UnresolvedOutcome)
        );
        let exit = crate::virtual_exit::tests::virtual_exit_fixture_fill(
            &task(),
            price("100", Currency::Krw),
            instant(66, 4),
        );
        assert_eq!(
            super::classify_exit(crate::OutcomeState::Success, exit),
            Err(crate::EstimateReturnError::OutcomeMismatch)
        );
    }

    #[test]
    fn d54_minute_exit_and_entry_calendar_evidence() {
        use crate::signal_return_contract::tests::{candle, cost, entry_bars, instant};
        let mut bars = entry_bars();
        bars[1] = candle(
            61,
            2,
            crate::CandlePeriod::Minute,
            ["100", "200", "100", "100"],
        );
        let result = calculate_return(&return_fixture("100"), &bars, &cost("0", "0", "0"))
            .expect("다음 분봉 목표");
        assert_eq!(
            result.exit_at(),
            crate::VirtualExitMoment::At(instant(61, 2))
        );
        let after_calendar = return_fixture_at("100", instant(70, 1));
        assert_eq!(
            calculate_return(&after_calendar, &bars, &cost("0", "0", "0")),
            Err(crate::EstimateReturnError::EntryTimingPending)
        );
        let time = instant(61, 1);
        let wrong_offset = Timestamp::from_parts(
            time.utc_seconds(),
            time.nanosecond(),
            OffsetSeconds::new(-43_200),
        )
        .expect("다른 날짜 표기");
        let original = return_fixture("100");
        let submission = original.submission();
        let differently_notated = crate::SignalSubmission::create(
            submission.task_key().clone(),
            submission.calibration_key().clone(),
            submission.probability(),
            None,
            wrong_offset,
        )
        .expect("같은 순간의 제출");
        let record = record_for_submission(
            eligible_for_task(submission.task_key().task(), instant(66, 4)),
            differently_notated,
            "offset-id",
        );
        let entry = crate::EstimateEntryPrice::try_new(&record, original.observation().clone())
            .expect("같은 순간의 관측");
        let shifted = calculate_return(&entry, &bars, &cost("0", "0", "0")).expect("같은 결과");
        assert_eq!(shifted.net(), result.net());
        assert_eq!(shifted.exit_at(), result.exit_at());
        assert_eq!(shifted.excess(), result.excess());
    }

    #[test]
    fn d54_cost_precision_loss_is_not_a_return() {
        use crate::signal_return_contract::tests::cost;
        let precise_entry = return_fixture("99.00000000000000000000000001");
        assert_eq!(
            calculate_return(
                &precise_entry,
                &maturity_bars("100"),
                &cost("0.001", "0", "0")
            ),
            Err(crate::EstimateReturnError::Arithmetic(
                crate::MoneyError::PrecisionLoss
            ))
        );
        let precise_exit = maturity_bars("100.00000000000000000000000001");
        assert_eq!(
            calculate_return(
                &return_fixture("100"),
                &precise_exit,
                &cost("0", "0", "0.001")
            ),
            Err(crate::EstimateReturnError::Arithmetic(
                crate::MoneyError::PrecisionLoss
            ))
        );
    }

    fn task_with_reference(value: &str) -> crate::EstimateTask {
        let original = crate::signal_return_contract::tests::task();
        task_with_reference_at(value, original.reference().observed_at())
    }

    fn task_with_reference_at(value: &str, at: Timestamp) -> crate::EstimateTask {
        let original = crate::signal_return_contract::tests::task();
        let reference = observed(
            original.volatility().instrument().clone(),
            original.reference().instrument().clone(),
            price(value, Currency::Krw),
            at,
        );
        crate::EstimateTask::create(
            crate::EstimateContext::new(reference, at),
            original.volatility().clone(),
            crate::Horizon::new(crate::HorizonSelection::FewDays),
            crate::HoursPermission::default(),
            crate::RiskAppetite::Neutral,
        )
        .expect("큰 기준 가격 과제")
    }

    #[test]
    fn d54_preopen_submission_uses_next_regular_session() {
        use crate::signal_return_contract::tests::{benchmark, calendars, instant};
        let at = instant(61, -1);
        let task = task_with_reference_at("100", at);
        let entry = return_fixture_for_task(&task, "100", at);
        let calendar = calendars();
        let adjustment = crate::OutcomeAdjustment::from_actions(
            &task,
            crate::AdjustmentBasis::new(instant(66, 4)),
            &[],
        )
        .expect("기준");
        let bars = maturity_bars("100");
        let outcome = crate::EstimateOutcome::evaluate_with_complete_data(
            &task,
            &calendar,
            &adjustment,
            &bars,
            instant(66, 4),
        );
        assert_eq!(
            outcome.state(),
            crate::OutcomeState::Failure(crate::OutcomeFailure::Expired)
        );
        let facts =
            crate::EstimateReturnFacts::new(&calendar, &adjustment, &[], &bars, instant(66, 4));
        let cost = zero_cost_for(&task, crate::Market::Kr, instant(66, 4));
        let result = crate::estimate_return(&entry, &outcome, &facts, &cost, &benchmark())
            .expect("장 시작 전 제출의 수익률");
        assert_eq!(result.net(), crate::RatioQuotient::Exact(ratio("0")));
        assert_eq!(
            result.excess().expect("초과 수익").alignment(),
            crate::BenchmarkAlignment::DailyCloseApproximation
        );
    }

    #[test]
    fn d54_preopen_submission_precedes_a_same_day_daily_exit() {
        use crate::signal_return_contract::tests::{candle, cost, entry_bars, instant};
        let mut bars = entry_bars();
        bars.push(candle(
            62,
            0,
            crate::CandlePeriod::Daily,
            ["100", "200", "100", "100"],
        ));
        let before_open = return_fixture_at("100", instant(62, -1));
        let result =
            calculate_return(&before_open, &bars, &cost("0", "0", "0")).expect("개장 전 제출");
        assert_eq!(result.exit_kind(), crate::EstimateExitKind::Target);
        assert_eq!(
            result.exit_at(),
            crate::VirtualExitMoment::TradingDay(instant(62, 0).offset_calendar_date())
        );
        let during_market = return_fixture_at("100", instant(62, 1));
        assert_eq!(
            calculate_return(&during_market, &bars, &cost("0", "0", "0")),
            Err(crate::EstimateReturnError::EntryTimingPending)
        );
        let mut gap = entry_bars();
        gap.push(candle(
            62,
            0,
            crate::CandlePeriod::Daily,
            ["200", "210", "190", "200"],
        ));
        assert_eq!(
            calculate_return(&before_open, &gap, &cost("0", "0", "0")),
            Err(crate::EstimateReturnError::GapPricePending)
        );
    }

    #[test]
    fn d54_gap_prices_wait_for_the_estimate_policy() {
        use crate::signal_return_contract::tests::{candle, cost, entry_bars, instant};
        for entry_at in [instant(61, 1), instant(62, -1)] {
            let entry = return_fixture_at("100", entry_at);
            for values in [["200", "210", "190", "200"], ["50", "60", "40", "50"]] {
                let mut bars = entry_bars();
                bars.push(candle(62, 0, crate::CandlePeriod::Daily, values));
                assert_eq!(
                    calculate_return(&entry, &bars, &cost("0", "0", "0")),
                    Err(crate::EstimateReturnError::GapPricePending)
                );
            }
        }
        for values in [["200", "210", "190", "200"], ["50", "60", "40", "50"]] {
            let entry = return_fixture_at("100", instant(61, 1));
            let mut bars = entry_bars();
            bars[2] = candle(61, 3, crate::CandlePeriod::Minute, values);
            assert_eq!(
                calculate_return(&entry, &bars, &cost("0", "0", "0")),
                Err(crate::EstimateReturnError::GapPricePending)
            );
        }
        let half_minute = crate::TimeSpan::try_new(crate::SpanSeconds::new(30), Nanosecond::new(0))
            .expect("30초");
        let submitted = instant(61, 1).checked_add(half_minute).expect("제출 시각");
        let mut bars = entry_bars();
        bars[2] = candle(
            61,
            3,
            crate::CandlePeriod::Minute,
            ["200", "210", "190", "200"],
        );
        assert_eq!(
            calculate_return(
                &return_fixture_at("100", submitted),
                &bars,
                &cost("0", "0", "0")
            ),
            Err(crate::EstimateReturnError::EntryTimingPending)
        );
    }

    #[test]
    fn d54_after_close_submission_uses_next_trading_day() {
        use crate::signal_return_contract::tests::{benchmark, instant, row};
        use crate::signal_return_scenarios::tests::bar;
        let at = instant(61, 5);
        let task = task_with_reference_at("100", at);
        let entry = return_fixture_for_task(&task, "100", at);
        let calendars = (61_i64..=67)
            .map(|day| {
                crate::MarketCalendar::try_new(
                    row(day.checked_sub(1).expect("이전")),
                    row(day),
                    row(day.checked_add(1).expect("다음")),
                )
                .expect("달력")
            })
            .collect::<Vec<_>>();
        let now = instant(67, 4);
        let basis = crate::AdjustmentBasis::new(now);
        let adjustment = crate::OutcomeAdjustment::from_actions(&task, basis, &[]).expect("기준");
        let mut bars = (1_i64..=4)
            .map(|minute| bar(62, minute, crate::CandlePeriod::Minute, "100", basis))
            .collect::<Vec<_>>();
        bars.extend((63_i64..=67).map(|day| bar(day, 0, crate::CandlePeriod::Daily, "100", basis)));
        let outcome = crate::EstimateOutcome::evaluate_with_complete_data(
            &task,
            &calendars,
            &adjustment,
            &bars,
            now,
        );
        assert_eq!(
            outcome.state(),
            crate::OutcomeState::Failure(crate::OutcomeFailure::Expired)
        );
        assert_eq!(outcome.evidence().expect("판정 근거").interval().end(), now);
        let facts = crate::EstimateReturnFacts::new(&calendars, &adjustment, &[], &bars, now);
        let cost = zero_cost_for(&task, crate::Market::Kr, now);
        let result = crate::estimate_return(&entry, &outcome, &facts, &cost, &benchmark())
            .expect("장 종료 후 제출의 수익률");
        assert_eq!(result.net(), crate::RatioQuotient::Exact(ratio("0")));
        assert_eq!(
            result.excess().expect("초과 수익").alignment(),
            crate::BenchmarkAlignment::DailyCloseApproximation
        );
    }

    fn large_return(value: &str, buy_rate: &str) -> Option<crate::EstimateReturnError> {
        use crate::signal_return_contract::tests::{benchmark, calendars, candle, instant};
        let task = task_with_reference(value);
        let entry = return_fixture_for_task(&task, value, instant(61, 1));
        let calendar = calendars();
        let adjustment = crate::OutcomeAdjustment::from_actions(
            &task,
            crate::AdjustmentBasis::new(instant(66, 4)),
            &[],
        )
        .expect("기준");
        let mut bars = (1_i64..=4)
            .map(|minute| candle(61, minute, crate::CandlePeriod::Minute, [value; 4]))
            .collect::<Vec<_>>();
        bars.extend(
            (62_i64..=66).map(|day| candle(day, 0, crate::CandlePeriod::Daily, [value; 4])),
        );
        let outcome = crate::EstimateOutcome::evaluate_with_complete_data(
            &task,
            &calendar,
            &adjustment,
            &bars,
            instant(66, 4),
        );
        let zero = crate::CostRate::try_new(ratio("0")).expect("0 비용");
        let buy = crate::CostRate::try_new(ratio(buy_rate)).expect("매수 비용");
        let cost = crate::signal_return_cost(
            Some(crate::CommissionRateFact::from_commissions_result(
                crate::Market::Kr,
                zero,
                crate::CommissionValidity::try_new(None, None).expect("유효"),
            )),
            Some(crate::TradeTaxFact::from_schedule_result(
                crate::Market::Kr,
                buy,
                zero,
            )),
            crate::EstimateCostPeriod::from_calendar_result(&task, instant(66, 4)).expect("기간"),
        )
        .expect("비용");
        let facts =
            crate::EstimateReturnFacts::new(&calendar, &adjustment, &[], &bars, instant(66, 4));
        crate::estimate_return(&entry, &outcome, &facts, &cost, &benchmark()).err()
    }

    #[test]
    fn d54_large_prices_do_not_hide_invalid_bounds_or_cost_overflow() {
        assert_eq!(
            large_return("79228162514264337593543950335", "0"),
            Some(crate::EstimateReturnError::InvalidBounds)
        );
        assert_eq!(
            large_return("50000000000000000000000000000", "0.7"),
            Some(crate::EstimateReturnError::Arithmetic(
                crate::MoneyError::Overflow
            ))
        );
    }

    #[test]
    fn d54_judged_outcome_with_zero_volume_cannot_be_a_virtual_exit() {
        use crate::signal_return_contract::tests::{candle, cost, entry_bars};
        let mut bars = entry_bars();
        let bar = &bars[0];
        let values = bar.values();
        let zero =
            crate::Quantity::parse(&NumericInput::try_new("0").expect("입력")).expect("수량");
        bars[0] = crate::Candle::try_new(
            bar.instrument().clone(),
            bar.time(),
            bar.adjustment(),
            crate::Ohlcv::try_new(
                values.open(),
                values.high(),
                values.low(),
                values.close(),
                zero,
            )
            .expect("봉"),
            crate::CandleObservation::new(bar.collected_at(), bar.status()),
        )
        .expect("거래량 없음");
        bars.push(candle(
            62,
            0,
            crate::CandlePeriod::Daily,
            ["100", "200", "100", "100"],
        ));
        assert_eq!(
            calculate_return(&return_fixture("100"), &bars, &cost("0", "0", "0")),
            Err(crate::EstimateReturnError::ExitUnavailable(
                crate::OutcomeUnavailable::MissingCandles
            ))
        );
    }

    #[test]
    fn d54_calendar_and_benchmark_failures_remain_explicit() {
        use crate::signal_return_contract::tests::{adjustment, benchmark, instant, task};
        let source = task();
        let fill = crate::virtual_exit::tests::virtual_exit_fixture_fill(
            &source,
            price("100", Currency::Krw),
            instant(66, 4),
        )
        .into_fill()
        .expect("청산");
        let adjustment = adjustment();
        let facts = crate::EstimateReturnFacts::new(&[], &adjustment, &[], &[], instant(66, 4));
        assert_eq!(format!("{facts:?}"), "EstimateReturnFacts([REDACTED])");
        assert_eq!(
            super::regular_close(instant(61, 0).offset_calendar_date(), &[]),
            Err(crate::OutcomeUnavailable::Calendar)
        );
        assert_eq!(
            super::estimate_excess(
                &return_fixture("100"),
                crate::RatioQuotient::Exact(ratio("0")),
                &fill,
                &facts,
                &benchmark()
            ),
            Err(crate::EstimateReturnError::BenchmarkCalendar(
                crate::OutcomeUnavailable::Calendar
            ))
        );
        assert_eq!(
            benchmark().close_on(instant(61, 0).offset_calendar_date(), Currency::Usd),
            Err(crate::SignalReturnError::BenchmarkMismatch)
        );
        assert_eq!(
            crate::VirtualExitSignal::for_estimate(&source, source.reference().observed_at()),
            Err(crate::VirtualExitSignalError::InvalidEntry)
        );
    }

    fn premarket_row(day: i64) -> crate::MarketSession {
        use crate::signal_return_contract::tests::instant;
        crate::MarketSession::try_new(
            instant(day, 0).offset_calendar_date(),
            crate::SessionInformation::Kr {
                pre_market: Some(
                    crate::SessionData::try_new(
                        TimeInterval::try_new(instant(day, 0), instant(day, 4)).expect("시간외"),
                        None,
                    )
                    .expect("시간외"),
                ),
                regular_market: None,
                after_market: None,
            },
        )
        .expect("거래일")
    }

    #[test]
    fn d54_daily_exit_requires_its_calendar_and_ordered_horizon() {
        use crate::signal_return_contract::tests::{
            adjustment, benchmark, calendars, candle, entry_bars, instant, task,
        };
        let source = task();
        let calendar = calendars();
        let adjustment = adjustment();
        let mut bars = entry_bars();
        bars.push(candle(
            62,
            0,
            crate::CandlePeriod::Daily,
            ["100", "200", "100", "100"],
        ));
        let signal = crate::VirtualExitSignal::for_estimate(&source, instant(66, 4)).expect("경계");
        let exit = crate::virtual_exit(&signal, &calendar, &adjustment, &bars, instant(66, 4));
        let fill = exit.into_fill().expect("청산");
        let missing = crate::EstimateReturnFacts::new(&[], &adjustment, &[], &bars, instant(66, 4));
        assert_eq!(
            super::adjustment_covers_exit(&missing, &fill),
            Err(crate::EstimateReturnError::ExitUnavailable(
                crate::OutcomeUnavailable::Calendar
            ))
        );
        let premarket = [crate::MarketCalendar::try_new(
            premarket_row(60),
            premarket_row(61),
            premarket_row(62),
        )
        .expect("달력")];
        let no_close =
            crate::EstimateReturnFacts::new(&premarket, &adjustment, &[], &bars, instant(66, 4));
        assert_eq!(
            super::estimate_excess(
                &return_fixture("100"),
                crate::RatioQuotient::Exact(ratio("0")),
                &fill,
                &no_close,
                &benchmark()
            ),
            Err(crate::EstimateReturnError::BenchmarkCalendar(
                crate::OutcomeUnavailable::Calendar
            ))
        );
    }

    #[test]
    fn d54_same_day_exit_requires_the_regular_open() {
        use crate::signal_return_contract::tests::{
            adjustment, calendars, candle, entry_bars, instant, task,
        };
        let source = task();
        let calendar = calendars();
        let adjustment = adjustment();
        let mut bars = entry_bars();
        bars.push(candle(
            62,
            0,
            crate::CandlePeriod::Daily,
            ["100", "200", "100", "100"],
        ));
        let outcome = crate::EstimateOutcome::evaluate_with_complete_data(
            &source,
            &calendar,
            &adjustment,
            &bars,
            instant(66, 4),
        );
        let signal = crate::VirtualExitSignal::for_estimate(&source, instant(66, 4)).expect("경계");
        let fill = crate::virtual_exit(&signal, &calendar, &adjustment, &bars, instant(66, 4))
            .into_fill()
            .expect("청산");
        let missing = crate::EstimateReturnFacts::new(&[], &adjustment, &[], &bars, instant(66, 4));
        let entry = return_fixture_at("100", instant(62, -1));
        assert_eq!(
            super::check_timing(
                &entry,
                crate::EstimateExitKind::Target,
                &fill,
                outcome.evidence().expect("판정"),
                &missing
            ),
            Err(crate::EstimateReturnError::ExitUnavailable(
                crate::OutcomeUnavailable::Calendar
            ))
        );
    }

    #[test]
    fn d54_cash_flow_horizon_must_have_positive_length() {
        use crate::signal_return_contract::tests::{adjustment, calendars, cost, instant, task};
        let source = task();
        let adjustment = adjustment();
        let bars = maturity_bars("100");
        let outcome = crate::EstimateOutcome::evaluate_with_complete_data(
            &source,
            &calendars(),
            &adjustment,
            &bars,
            instant(66, 4),
        );
        let fill = crate::virtual_exit::tests::virtual_exit_fixture_fill(
            &source,
            price("100", Currency::Krw),
            instant(66, 4),
        )
        .into_fill()
        .expect("청산");
        let at_end = return_fixture_at("100", instant(66, 4));
        let evidence = outcome.evidence().expect("판정 근거");
        assert_eq!(
            super::estimate_cash_flows(
                &at_end,
                price("100", Currency::Krw),
                &fill,
                evidence,
                &cost("0", "0", "0")
            ),
            Err(crate::EstimateReturnError::EntryTimingPending)
        );
    }

    fn estimate_signal_with_bounds(target: &str, stop: &str) -> crate::VirtualExitSignal {
        let task = crate::signal_return_contract::tests::task();
        crate::VirtualExitSignal::from_entry(
            task.clone(),
            task.reference().clone(),
            price(target, Currency::Krw),
            price(stop, Currency::Krw),
            crate::signal_return_contract::tests::instant(66, 4),
        )
        .expect("경계")
    }

    #[test]
    fn d54_unrepresentable_adjustment_is_reported_for_each_price() {
        use crate::signal_return_contract::tests::{calendars, instant, task};
        let source = task();
        let calendar = calendars();
        let actions = [split_factor(63, 3)];
        let adjustment = crate::OutcomeAdjustment::from_actions(
            &source,
            crate::AdjustmentBasis::new(instant(66, 4)),
            &actions,
        )
        .expect("분할");
        let facts =
            crate::EstimateReturnFacts::new(&calendar, &adjustment, &actions, &[], instant(66, 4));
        let fill = crate::virtual_exit::tests::virtual_exit_fixture_fill(
            &source,
            price("100", Currency::Krw),
            instant(66, 4),
        )
        .into_fill()
        .expect("청산");
        let error = Err(crate::EstimateReturnError::ExitUnavailable(
            crate::OutcomeUnavailable::Adjustment,
        ));
        assert_eq!(
            super::adjusted_entry(
                &return_fixture("100"),
                &estimate_signal_with_bounds("110", "90"),
                crate::EstimateExitKind::Maturity,
                &fill,
                &facts
            ),
            error
        );
        assert_eq!(
            super::adjusted_entry(
                &return_fixture("99"),
                &estimate_signal_with_bounds("110", "90"),
                crate::EstimateExitKind::Maturity,
                &fill,
                &facts
            ),
            error
        );
        assert_eq!(
            super::adjusted_entry(
                &return_fixture("99"),
                &estimate_signal_with_bounds("120", "98"),
                crate::EstimateExitKind::Maturity,
                &fill,
                &facts
            ),
            error
        );
    }

    #[test]
    fn d54_entry_adjustment_requires_basis_after_submission() {
        use crate::signal_return_contract::tests::{calendars, instant, task};
        let source = task();
        let calendar = calendars();
        let fill = crate::virtual_exit::tests::virtual_exit_fixture_fill(
            &source,
            price("100", Currency::Krw),
            instant(66, 4),
        )
        .into_fill()
        .expect("청산");
        let early = crate::OutcomeAdjustment::from_actions(
            &source,
            crate::AdjustmentBasis::new(instant(61, 4)),
            &[],
        )
        .expect("이른 기준");
        let early_facts =
            crate::EstimateReturnFacts::new(&calendar, &early, &[], &[], instant(66, 4));
        assert_eq!(
            super::adjusted_entry(
                &return_fixture_at("100", instant(62, 1)),
                &estimate_signal_with_bounds("110", "90"),
                crate::EstimateExitKind::Maturity,
                &fill,
                &early_facts
            ),
            Err(crate::EstimateReturnError::ExitUnavailable(
                crate::OutcomeUnavailable::Adjustment
            ))
        );
        let missing = crate::EstimateReturnFacts::new(&[], &early, &[], &[], instant(66, 4));
        assert_eq!(
            super::adjusted_entry(
                &return_fixture_at("100", instant(62, 1)),
                &estimate_signal_with_bounds("110", "90"),
                crate::EstimateExitKind::Maturity,
                &fill,
                &missing,
            ),
            Err(crate::EstimateReturnError::ExitUnavailable(
                crate::OutcomeUnavailable::Adjustment
            ))
        );
    }

    #[test]
    fn d54_entry_outside_recorded_bounds_is_pending() {
        let entry = return_fixture("200");
        assert_eq!(
            calculate_return(
                &entry,
                &maturity_bars("100"),
                &crate::signal_return_contract::tests::cost("0", "0", "0")
            ),
            Err(crate::EstimateReturnError::EntryTimingPending)
        );
    }

    #[test]
    fn d54_return_rejects_earlier_or_partially_observed_exit_and_gap() {
        use crate::signal_return_contract::tests::{candle, cost, entry_bars, instant};
        let mut minute = entry_bars();
        minute[0] = candle(
            61,
            1,
            crate::CandlePeriod::Minute,
            ["100", "200", "100", "100"],
        );
        assert_eq!(
            calculate_return(
                &return_fixture_at("100", instant(61, 1)),
                &minute,
                &cost("0", "0", "0")
            ),
            Err(crate::EstimateReturnError::EntryTimingPending)
        );
        assert_eq!(
            calculate_return(
                &return_fixture_at("100", instant(61, 2)),
                &minute,
                &cost("0", "0", "0"),
            ),
            Err(crate::EstimateReturnError::EntryTimingPending)
        );
        let mut bars = entry_bars();
        bars.push(candle(
            62,
            0,
            crate::CandlePeriod::Daily,
            ["100", "200", "100", "100"],
        ));
        assert_eq!(
            calculate_return(
                &return_fixture_at("100", instant(63, 1)),
                &bars,
                &cost("0", "0", "0")
            ),
            Err(crate::EstimateReturnError::EntryTimingPending)
        );
        let mut gap = entry_bars();
        gap.push(candle(
            62,
            0,
            crate::CandlePeriod::Daily,
            ["200", "210", "190", "200"],
        ));
        assert_eq!(
            calculate_return(&return_fixture("100"), &gap, &cost("0", "0", "0")),
            Err(crate::EstimateReturnError::GapPricePending)
        );
    }

    fn split_factor(day: i64, new_shares: u64) -> crate::OutcomeCorporateAction {
        use crate::signal_return_contract::tests::{instant, task};
        let action = crate::CorporateAction::try_new(
            crate::CorporateActionId::new(crate::InstrumentId::try_new("split-1").expect("ID")),
            instant(day, 0).offset_calendar_date(),
            crate::CorporateActionKind::Split(crate::ShareRatio::new(
                crate::ShareCount::try_new(1).expect("구주"),
                crate::ShareCount::try_new(new_shares).expect("신주"),
            )),
        )
        .expect("분할");
        crate::OutcomeCorporateAction::new(task().volatility().instrument().clone(), action)
    }

    fn split_action(day: i64) -> crate::OutcomeCorporateAction {
        split_factor(day, 2)
    }

    #[test]
    fn d54_split_adjusts_entry_once_and_rejects_mismatched_actions() {
        use crate::signal_return_contract::tests::{benchmark, calendars, candle, cost, instant};
        use crate::{
            AdjustmentBasis, CandlePeriod, EstimateOutcome, EstimateReturnError,
            EstimateReturnFacts, OutcomeAdjustment, RatioQuotient,
        };
        let calendar = calendars();
        let task = crate::signal_return_contract::tests::task();
        let actions = [split_action(63)];
        let adjustment =
            OutcomeAdjustment::from_actions(&task, AdjustmentBasis::new(instant(66, 4)), &actions)
                .expect("분할 기준");
        let mut bars = (1_i64..=4)
            .map(|minute| candle(61, minute, CandlePeriod::Minute, ["50"; 4]))
            .collect::<Vec<_>>();
        bars.extend((62_i64..=66).map(|day| candle(day, 0, CandlePeriod::Daily, ["50"; 4])));
        let outcome = EstimateOutcome::evaluate_with_complete_data(
            &task,
            &calendar,
            &adjustment,
            &bars,
            instant(66, 4),
        );
        let facts =
            EstimateReturnFacts::new(&calendar, &adjustment, &actions, &bars, instant(66, 4));
        let before = return_fixture_at("100", instant(61, 1));
        let after = return_fixture_at("50", instant(64, 1));
        for entry in [&before, &after] {
            let result =
                crate::estimate_return(entry, &outcome, &facts, &cost("0", "0", "0"), &benchmark())
                    .expect("분할 수익률");
            assert_eq!(result.exit_price(), price("50", Currency::Krw));
            assert_eq!(result.net(), RatioQuotient::Exact(ratio("0")));
        }
        let same_day = return_fixture_at("50", instant(63, 1));
        let same_day_return = crate::estimate_return(
            &same_day,
            &outcome,
            &facts,
            &cost("0", "0", "0"),
            &benchmark(),
        )
        .expect("행사일 시작에 분할이 적용된 제출가");
        assert_eq!(same_day_return.net(), RatioQuotient::Exact(ratio("0")));
        let missing = EstimateReturnFacts::new(&calendar, &adjustment, &[], &bars, instant(66, 4));
        assert_eq!(
            crate::estimate_return(
                &before,
                &outcome,
                &missing,
                &cost("0", "0", "0"),
                &benchmark()
            ),
            Err(EstimateReturnError::OutcomeMismatch)
        );
    }

    #[test]
    fn d54_benchmark_alignment_uses_submission_instead_of_task_reference() {
        use crate::signal_return_contract::tests::{candle, cost, entry_bars, instant};
        let mut bars = entry_bars();
        bars.extend(
            (62_i64..=66).map(|day| candle(day, 0, crate::CandlePeriod::Daily, ["100"; 4])),
        );
        let entry = return_fixture_at("100", instant(62, 4));
        let result = calculate_return(&entry, &bars, &cost("0", "0", "0")).expect("수익률");
        let excess = result.excess().expect("초과 수익률");
        assert_eq!(excess.alignment(), crate::BenchmarkAlignment::Exact);
        assert_eq!(super::ratio_value(excess.value()), ratio("0"));
        assert_eq!(format!("{result:?}"), "EstimateReturn([REDACTED])");
    }

    fn altered_close_row(day: i64) -> crate::MarketSession {
        use crate::signal_return_contract::tests::instant;
        crate::MarketSession::try_new(
            instant(day, 0).offset_calendar_date(),
            crate::SessionInformation::Kr {
                pre_market: None,
                regular_market: Some(
                    crate::SessionData::try_new(
                        TimeInterval::try_new(instant(day, 0), instant(day, 5)).expect("거래 시간"),
                        None,
                    )
                    .expect("거래 시간"),
                ),
                after_market: None,
            },
        )
        .expect("거래일")
    }

    fn benchmark_with_different_close() -> crate::SignalBenchmark {
        use crate::signal_return_contract::tests::instant;
        use crate::signal_return_scenarios::tests::{bar, snapshot};
        let days = (61_i64..=67).collect::<Vec<_>>();
        let basis = crate::AdjustmentBasis::new(instant(67, 5));
        let candles = days
            .iter()
            .map(|day| bar(*day, 0, crate::CandlePeriod::Daily, "100", basis))
            .collect::<Vec<_>>();
        let indices =
            crate::index_benchmark::tests::index_candles(crate::BenchmarkKind::Kospi, &candles);
        let snapshots = days.iter().map(|day| snapshot(*day)).collect::<Vec<_>>();
        let calendars = days
            .iter()
            .map(|day| {
                crate::MarketCalendar::try_new(
                    altered_close_row(day.checked_sub(1).expect("이전")),
                    altered_close_row(*day),
                    altered_close_row(day.checked_add(1).expect("다음")),
                )
                .expect("달력")
            })
            .collect::<Vec<_>>();
        crate::SignalBenchmark::from_index_candles(&indices, &snapshots, &calendars)
            .expect("다른 종료 시각")
    }

    fn benchmark_with_single_altered_close(altered: i64) -> crate::SignalBenchmark {
        use crate::signal_return_contract::tests::{instant, row};
        use crate::signal_return_scenarios::tests::{bar, snapshot};
        let days = (61_i64..=67).collect::<Vec<_>>();
        let basis = crate::AdjustmentBasis::new(instant(67, 5));
        let candles = days
            .iter()
            .map(|day| bar(*day, 0, crate::CandlePeriod::Daily, "100", basis))
            .collect::<Vec<_>>();
        let indices =
            crate::index_benchmark::tests::index_candles(crate::BenchmarkKind::Kospi, &candles);
        let snapshots = days.iter().map(|day| snapshot(*day)).collect::<Vec<_>>();
        let session = |day| {
            if day == altered {
                altered_close_row(day)
            } else {
                row(day)
            }
        };
        let calendars = days
            .iter()
            .map(|day| {
                crate::MarketCalendar::try_new(
                    session(day.checked_sub(1).expect("이전")),
                    session(*day),
                    session(day.checked_add(1).expect("다음")),
                )
                .expect("달력")
            })
            .collect::<Vec<_>>();
        crate::SignalBenchmark::from_index_candles(&indices, &snapshots, &calendars)
            .expect("한 거래일의 종료 시각이 다른 벤치마크")
    }

    #[test]
    fn d54_benchmark_rejects_each_mismatched_endpoint() {
        use crate::signal_return_contract::tests::{adjustment, calendars, cost, instant};
        let entry = return_fixture("100");
        let task = entry.submission().task_key().task();
        let calendar = calendars();
        let adjustment = adjustment();
        let bars = maturity_bars("100");
        let now = instant(66, 4);
        let outcome = crate::EstimateOutcome::evaluate_with_complete_data(
            task,
            &calendar,
            &adjustment,
            &bars,
            now,
        );
        let facts = crate::EstimateReturnFacts::new(&calendar, &adjustment, &[], &bars, now);
        for altered in [61_i64, 66] {
            let result = crate::estimate_return(
                &entry,
                &outcome,
                &facts,
                &cost("0", "0", "0"),
                &benchmark_with_single_altered_close(altered),
            )
            .expect("순수익률");
            assert_eq!(result.exit_kind(), crate::EstimateExitKind::Maturity);
            assert_eq!(
                result.excess(),
                Err(crate::EstimateReturnError::Benchmark(
                    crate::SignalReturnError::BenchmarkMismatch
                )),
                "불일치한 거래일: {altered}"
            );
        }
    }

    fn benchmark_with_matching_gain(target: &str) -> crate::SignalBenchmark {
        use crate::signal_return_contract::tests::instant;
        use crate::signal_return_scenarios::tests::{bar, calendar, snapshot};
        let days = (61_i64..=67).collect::<Vec<_>>();
        let basis = crate::AdjustmentBasis::new(instant(67, 4));
        let candles = days
            .iter()
            .map(|day| {
                bar(
                    *day,
                    0,
                    crate::CandlePeriod::Daily,
                    if *day == 61 { "100" } else { target },
                    basis,
                )
            })
            .collect::<Vec<_>>();
        let indices =
            crate::index_benchmark::tests::index_candles(crate::BenchmarkKind::Kospi, &candles);
        let snapshots = days.iter().map(|day| snapshot(*day)).collect::<Vec<_>>();
        let calendars = days.iter().map(|day| calendar(*day)).collect::<Vec<_>>();
        crate::SignalBenchmark::from_index_candles(&indices, &snapshots, &calendars)
            .expect("같은 수익률의 지수")
    }

    #[test]
    fn d54_nonzero_equal_benchmark_has_zero_excess() {
        use crate::signal_return_contract::tests::{
            adjustment, calendars, candle, cost, entry_bars, instant,
        };
        let entry = return_fixture("100");
        let task = entry.submission().task_key().task();
        let factor = Ratio::new(
            rust_decimal::Decimal::ONE
                .checked_add(task.target().as_decimal())
                .expect("목표"),
        );
        let target = task
            .reference()
            .price()
            .as_money()
            .checked_mul(&factor)
            .expect("목표 가격")
            .amount()
            .as_decimal()
            .to_string();
        let benchmark = benchmark_with_matching_gain(&target);
        let calendar = calendars();
        let adjustment = adjustment();
        let mut bars = entry_bars();
        bars.push(candle(
            62,
            0,
            crate::CandlePeriod::Daily,
            ["100", "200", "100", "100"],
        ));
        let outcome = crate::EstimateOutcome::evaluate_with_complete_data(
            task,
            &calendar,
            &adjustment,
            &bars,
            instant(66, 4),
        );
        let facts =
            crate::EstimateReturnFacts::new(&calendar, &adjustment, &[], &bars, instant(66, 4));
        let result =
            crate::estimate_return(&entry, &outcome, &facts, &cost("0", "0", "0"), &benchmark)
                .expect("추정 수익률");
        assert!(super::ratio_value(result.net()) > ratio("0"));
        assert_eq!(
            super::ratio_value(result.excess().expect("초과 수익").value()),
            ratio("0")
        );
    }

    #[test]
    fn d54_benchmark_rejects_same_dates_with_different_close_times() {
        use crate::signal_return_contract::tests::{
            adjustment, calendars, candle, cost, entry_bars, instant,
        };
        let entry = return_fixture("100");
        let task = entry.submission().task_key().task();
        let calendar = calendars();
        let adjustment = adjustment();
        let mut bars = entry_bars();
        bars.extend(
            (62_i64..=66).map(|day| candle(day, 0, crate::CandlePeriod::Daily, ["100"; 4])),
        );
        let outcome = crate::EstimateOutcome::evaluate_with_complete_data(
            task,
            &calendar,
            &adjustment,
            &bars,
            instant(66, 4),
        );
        let facts =
            crate::EstimateReturnFacts::new(&calendar, &adjustment, &[], &bars, instant(66, 4));
        let result = crate::estimate_return(
            &entry,
            &outcome,
            &facts,
            &cost("0", "0", "0"),
            &benchmark_with_different_close(),
        )
        .expect("순수익률");
        assert_eq!(
            result.excess(),
            Err(crate::EstimateReturnError::Benchmark(
                crate::SignalReturnError::BenchmarkMismatch
            ))
        );
    }

    fn unrelated_cost() -> crate::SignalReturnCost {
        use crate::signal_return_contract::tests::instant;
        let zero = crate::CostRate::try_new(ratio("0")).expect("0 비용");
        crate::signal_return_cost(
            Some(crate::CommissionRateFact::from_commissions_result(
                crate::Market::Kr,
                zero,
                crate::CommissionValidity::try_new(None, None).expect("유효"),
            )),
            Some(crate::TradeTaxFact::from_schedule_result(
                crate::Market::Kr,
                zero,
                zero,
            )),
            crate::EstimateCostPeriod::from_calendar_result(
                &crate::thresholds::tests::task(),
                instant(66, 4),
            )
            .expect("다른 과제"),
        )
        .expect("비용")
    }

    fn zero_cost_for(
        task: &crate::EstimateTask,
        market: crate::Market,
        maturity: Timestamp,
    ) -> crate::SignalReturnCost {
        let zero = crate::CostRate::try_new(ratio("0")).expect("0 비용");
        crate::signal_return_cost(
            Some(crate::CommissionRateFact::from_commissions_result(
                market.clone(),
                zero,
                crate::CommissionValidity::try_new(None, None).expect("유효"),
            )),
            Some(crate::TradeTaxFact::from_schedule_result(
                market, zero, zero,
            )),
            crate::EstimateCostPeriod::from_calendar_result(task, maturity).expect("기간"),
        )
        .expect("비용")
    }

    #[test]
    fn d54_cost_requires_the_same_market_and_maturity() {
        use crate::signal_return_contract::tests::instant;
        let entry = return_fixture("100");
        let task = entry.submission().task_key().task();
        let bars = maturity_bars("100");
        let wrong_market = zero_cost_for(task, crate::Market::Us, instant(66, 4));
        let wrong_maturity = zero_cost_for(task, crate::Market::Kr, instant(67, 4));
        for cost in [&wrong_market, &wrong_maturity] {
            assert_eq!(
                calculate_return(&entry, &bars, cost),
                Err(crate::EstimateReturnError::CostMismatch)
            );
        }
    }

    #[test]
    fn d54_unrelated_cost_and_missing_benchmark_are_separate_errors() {
        use crate::signal_return_contract::tests::{
            adjustment, calendars, candle, entry_bars, instant,
        };
        let entry = return_fixture("100");
        let task = entry.submission().task_key().task();
        let calendar = calendars();
        let adjustment = adjustment();
        let mut bars = entry_bars();
        bars.extend(
            (62_i64..=66).map(|day| candle(day, 0, crate::CandlePeriod::Daily, ["100"; 4])),
        );
        let outcome = crate::EstimateOutcome::evaluate_with_complete_data(
            task,
            &calendar,
            &adjustment,
            &bars,
            instant(66, 4),
        );
        let facts =
            crate::EstimateReturnFacts::new(&calendar, &adjustment, &[], &bars, instant(66, 4));
        let unrelated = unrelated_cost();
        let shifted = crate::signal_return_scenarios::tests::benchmark(
            1,
            "100",
            crate::AdjustmentBasis::new(instant(68, 4)),
        );
        assert_eq!(
            crate::estimate_return(&entry, &outcome, &facts, &unrelated, &shifted),
            Err(crate::EstimateReturnError::CostMismatch)
        );
        let result = crate::estimate_return(
            &entry,
            &outcome,
            &facts,
            &crate::signal_return_contract::tests::cost("0", "0", "0"),
            &shifted,
        )
        .expect("순수익률");
        assert_eq!(result.net(), crate::RatioQuotient::Exact(ratio("0")));
        assert_eq!(
            result.excess(),
            Err(crate::EstimateReturnError::Benchmark(
                crate::SignalReturnError::BenchmarkMismatch
            ))
        );
    }

    #[test]
    fn d54_adjustment_must_cover_exit_evidence() {
        use crate::signal_return_contract::tests::{
            benchmark, calendars, candle, cost, entry_bars, instant,
        };
        let entry = return_fixture("100");
        let task = entry.submission().task_key().task();
        let calendar = calendars();
        let basis = crate::AdjustmentBasis::new(instant(61, 4));
        let adjustment =
            crate::OutcomeAdjustment::from_actions(task, basis, &[]).expect("이른 수정 기준");
        let mut bars = entry_bars();
        bars.push(candle(
            62,
            0,
            crate::CandlePeriod::Daily,
            ["100", "200", "100", "100"],
        ));
        let early = bars
            .iter()
            .map(|bar| {
                crate::Candle::try_new(
                    bar.instrument().clone(),
                    bar.time(),
                    crate::PriceAdjustment::Adjusted(basis),
                    bar.values(),
                    crate::CandleObservation::new(bar.collected_at(), bar.status()),
                )
                .expect("같은 기준 봉")
            })
            .collect::<Vec<_>>();
        let outcome = crate::EstimateOutcome::evaluate_with_complete_data(
            task,
            &calendar,
            &adjustment,
            &early,
            instant(66, 4),
        );
        let facts =
            crate::EstimateReturnFacts::new(&calendar, &adjustment, &[], &early, instant(66, 4));
        assert_eq!(
            crate::estimate_return(&entry, &outcome, &facts, &cost("0", "0", "0"), &benchmark()),
            Err(crate::EstimateReturnError::ExitUnavailable(
                crate::OutcomeUnavailable::Adjustment
            ))
        );
    }

    #[test]
    fn d54_submission_price_preserves_submission_and_rejects_other_times() {
        let (task, record) = submission_fixture();
        let submission = record.submission();
        let time = submission.submitted_at();
        let reference = task.reference();
        let valid = observed(
            task.volatility().instrument().clone(),
            reference.instrument().clone(),
            price("112", Currency::Krw),
            time,
        );
        let result = crate::EstimateEntryPrice::try_new(&record, valid.clone()).expect("제출 가격");
        assert_eq!(result.observation(), &valid);
        assert_eq!(result.submitted_at(), time);
        assert_eq!(result.submission(), submission);
        assert_eq!(format!("{result:?}"), "EstimateEntryPrice([REDACTED])");
        let early = observed(
            task.volatility().instrument().clone(),
            reference.instrument().clone(),
            price("112", Currency::Krw),
            task.created_at(),
        );
        assert_eq!(
            crate::EstimateEntryPrice::try_new(&record, early),
            Err(crate::EstimateEntryError::Time)
        );
        let later = observed(
            task.volatility().instrument().clone(),
            reference.instrument().clone(),
            price("112", Currency::Krw),
            time.checked_add(
                crate::TimeSpan::try_new(crate::SpanSeconds::new(1), Nanosecond::new(0))
                    .expect("1초"),
            )
            .expect("나중 시각"),
        );
        assert_eq!(
            crate::EstimateEntryPrice::try_new(&record, later),
            Err(crate::EstimateEntryError::Time)
        );
    }

    #[test]
    fn d54_submission_price_rejects_other_instruments_and_currencies() {
        let (task, record) = submission_fixture();
        let submission = record.submission();
        let time = submission.submitted_at();
        let reference = task.reference();
        let wrong_id = observed(
            crate::InstrumentId::try_new("other").expect("종목"),
            reference.instrument().clone(),
            price("112", Currency::Krw),
            time,
        );
        assert_eq!(
            crate::EstimateEntryPrice::try_new(&record, wrong_id),
            Err(crate::EstimateEntryError::Instrument)
        );
        let wrong_symbol = observed(
            task.volatility().instrument().clone(),
            crate::ResolvedOrderInstrument::from_master_lookup_result(
                crate::Symbol::try_new("XYZ").expect("코드"),
                crate::Market::Kr,
            )
            .expect("시장"),
            price("112", Currency::Krw),
            time,
        );
        assert_eq!(
            crate::EstimateEntryPrice::try_new(&record, wrong_symbol),
            Err(crate::EstimateEntryError::Instrument)
        );
        let wrong_currency = observed(
            task.volatility().instrument().clone(),
            reference.instrument().clone(),
            price("112", Currency::Usd),
            time,
        );
        assert_eq!(
            crate::EstimateEntryPrice::try_new(&record, wrong_currency),
            Err(crate::EstimateEntryError::Currency)
        );
    }

    #[test]
    fn d54_capacity_reports_full_without_changing_existing_evidence() {
        let model = crate::CalibrationModel::new(
            crate::ModelIdentityPart::try_new("provider").expect("제공자"),
            crate::ModelIdentityPart::try_new("model").expect("모델"),
            crate::ModelIdentityPart::try_new("version").expect("버전"),
        );
        let key = crate::CalibrationKey::new(
            model,
            crate::Horizon::new(crate::HorizonSelection::FewDays),
        );
        let fact = |id: u32| {
            crate::CalibrationFact::new(
                crate::CalibrationEstimate::new(
                    crate::CalibrationEstimateId::try_new(&format!("fact-{id}")).expect("식별자"),
                    crate::InstrumentId::try_new("sample").expect("종목"),
                    key.clone(),
                    interval(1, 5),
                    crate::AiProbability::parse(
                        &crate::AiProbabilityInput::try_new("60").expect("입력"),
                    )
                    .expect("확률"),
                ),
                crate::CalibrationOutcome::Hit,
            )
        };
        let facts = (0_u32..65_536).map(fact).collect::<Vec<_>>();
        let almost = crate::Calibration::try_new(key.clone(), &facts[..65_535]).expect("상한 직전");
        assert_eq!(almost.capacity(), CalibrationCapacity::Available);
        let full = almost.with_fact(fact(65_535)).expect("마지막 사실");
        assert_eq!(full.capacity(), CalibrationCapacity::Exhausted);
        assert_eq!(full.facts().len(), 65_536);
        assert_eq!(
            full.with_fact(fact(65_536)),
            Err(CalibrationError::TooManyFacts)
        );
        assert_eq!(
            full.facts(),
            crate::Calibration::try_new(key.clone(), &facts)
                .expect("복원")
                .facts()
        );
        assert_eq!(almost.facts().len(), 65_535);
        let empty = crate::Calibration::try_new(key, &[]).expect("빈 조합");
        assert_eq!(empty.capacity(), CalibrationCapacity::Available);
    }

    #[test]
    fn d54_daily_open_boundary_hit_requires_strictly_earlier_submission() {
        use crate::signal_return_contract::tests::{candle, cost, entry_bars, instant};
        let preopen = return_fixture_at("100", instant(62, -1));
        let task = preopen.submission().task_key().task();
        let factor = Ratio::new(
            rust_decimal::Decimal::ONE
                .checked_add(task.target().as_decimal())
                .expect("목표"),
        );
        let target_money = task
            .reference()
            .price()
            .as_money()
            .checked_mul(&factor)
            .expect("목표 가격");
        let target_text = target_money.amount().as_decimal().to_string();
        let mut bars = entry_bars();
        bars.push(candle(
            62,
            0,
            crate::CandlePeriod::Daily,
            [&target_text, &target_text, &target_text, &target_text],
        ));
        let zero_cost = cost("0", "0", "0");
        let allowed = calculate_return(&preopen, &bars, &zero_cost).expect("개장 전 제출");
        assert_eq!(allowed.exit_kind(), crate::EstimateExitKind::Target);
        assert_eq!(
            allowed.exit_at(),
            crate::VirtualExitMoment::At(instant(62, 0))
        );
        assert_eq!(allowed.exit_price().as_money(), target_money);
        for submitted in [instant(62, 0), instant(62, 1)] {
            assert_eq!(
                calculate_return(&return_fixture_at("100", submitted), &bars, &zero_cost),
                Err(crate::EstimateReturnError::EntryTimingPending)
            );
        }
    }

    #[test]
    fn d54_check_timing_rejects_maturity_boundary_and_after_maturity() {
        use crate::signal_return_contract::tests::{adjustment, calendars, cost, instant, task};
        let source = task();
        let calendar = calendars();
        let adjustment = adjustment();
        let now = instant(66, 4);
        let bars = maturity_bars("100");
        let outcome = crate::EstimateOutcome::evaluate_with_complete_data(
            &source,
            &calendar,
            &adjustment,
            &bars,
            now,
        );
        let evidence = outcome.evidence().expect("판정 근거");
        let facts = crate::EstimateReturnFacts::new(&calendar, &adjustment, &[], &bars, now);
        let fill = crate::virtual_exit::tests::virtual_exit_fixture_fill(
            &source,
            price("100", Currency::Krw),
            now,
        )
        .into_fill()
        .expect("청산");
        let at_maturity = return_fixture_at("100", now);
        let after_maturity = return_fixture_at("100", instant(67, 1));
        for entry in [&at_maturity, &after_maturity] {
            assert_eq!(
                super::check_timing(
                    entry,
                    crate::EstimateExitKind::Maturity,
                    &fill,
                    evidence,
                    &facts,
                ),
                Err(crate::EstimateReturnError::EntryTimingPending)
            );
        }
        assert_eq!(
            calculate_return(&after_maturity, &bars, &cost("0", "0", "0")),
            Err(crate::EstimateReturnError::EntryTimingPending)
        );
    }

    #[test]
    fn d54_check_timing_rejects_later_trading_day_without_calendar_lookup() {
        use crate::signal_return_contract::tests::{
            adjustment, calendars, candle, entry_bars, instant, task,
        };
        let source = task();
        let calendar = calendars();
        let adjustment = adjustment();
        let now = instant(66, 4);
        let mut daily_bars = entry_bars();
        daily_bars.push(candle(
            62,
            0,
            crate::CandlePeriod::Daily,
            ["100", "200", "100", "100"],
        ));
        let daily_outcome = crate::EstimateOutcome::evaluate_with_complete_data(
            &source,
            &calendar,
            &adjustment,
            &daily_bars,
            now,
        );
        let signal = crate::VirtualExitSignal::for_estimate(&source, now).expect("경계");
        let daily_fill = crate::virtual_exit(&signal, &calendar, &adjustment, &daily_bars, now)
            .into_fill()
            .expect("일봉 청산");
        let missing_calendar =
            crate::EstimateReturnFacts::new(&[], &adjustment, &[], &daily_bars, now);
        let later_day = return_fixture_at("100", instant(63, 1));
        assert_eq!(
            super::check_timing(
                &later_day,
                crate::EstimateExitKind::Target,
                &daily_fill,
                daily_outcome.evidence().expect("일봉 판정"),
                &missing_calendar,
            ),
            Err(crate::EstimateReturnError::EntryTimingPending)
        );
    }
}

#[cfg(test)]
mod arithmetic_tests {
    use super::EstimateCashFlows;
    use crate::{
        Currency, EstimateReturnError, Money, Nanosecond, NumericInput, OffsetSeconds, Price,
        Ratio, RatioQuotient, TimeInterval, Timestamp, UnixSeconds,
    };
    use proptest::test_runner::Config as ProptestConfig;
    use proptest::{prop_assert_eq, prop_assert_ne, proptest};

    #[derive(Clone, Copy, PartialEq, Eq)]
    struct BenchmarkCashFlows {
        entry: Price,
        exit: Price,
        period: TimeInterval,
    }

    impl std::fmt::Debug for BenchmarkCashFlows {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("BenchmarkCashFlows([REDACTED])")
        }
    }

    impl BenchmarkCashFlows {
        fn new(
            entry: Price,
            exit: Price,
            period: TimeInterval,
        ) -> Result<Self, EstimateReturnError> {
            if entry.as_money().currency() != exit.as_money().currency() {
                return Err(EstimateReturnError::CurrencyMismatch);
            }
            Ok(Self {
                entry,
                exit,
                period,
            })
        }

        fn currency(&self) -> Currency {
            self.entry.as_money().currency()
        }

        fn rate(&self) -> Result<RatioQuotient, EstimateReturnError> {
            self.exit
                .as_money()
                .checked_sub(&self.entry.as_money())
                .and_then(|change| change.checked_ratio(&self.entry.as_money()))
                .map_err(EstimateReturnError::Arithmetic)
        }
    }

    struct CashFixture(EstimateCashFlows);

    impl std::fmt::Debug for CashFixture {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            std::fmt::Debug::fmt(&self.0, f)
        }
    }

    impl CashFixture {
        fn net(&self) -> Result<RatioQuotient, EstimateReturnError> {
            self.0.net()
        }
        fn excess(
            &self,
            benchmark: &BenchmarkCashFlows,
        ) -> Result<RatioQuotient, EstimateReturnError> {
            if self.0.entry.as_money().currency() != benchmark.currency() {
                return Err(EstimateReturnError::CurrencyMismatch);
            }
            if self.0.period != benchmark.period {
                return Err(EstimateReturnError::PeriodMismatch);
            }
            super::subtract(self.net()?, benchmark.rate()?)
        }
    }

    fn amount(value: &str, currency: Currency) -> Money {
        Money::parse(&NumericInput::try_new(value).expect("금액 입력"), currency).expect("금액")
    }

    fn price(value: &str, currency: Currency) -> Price {
        Price::try_new(amount(value, currency)).expect("가격")
    }

    fn ratio(value: &str) -> Ratio {
        Ratio::parse(&NumericInput::try_new(value).expect("비율 입력")).expect("비율")
    }

    fn interval(start: i64, end: i64) -> TimeInterval {
        let at = |value| {
            Timestamp::from_parts(
                UnixSeconds::new(value),
                Nanosecond::new(0),
                OffsetSeconds::new(0),
            )
            .expect("시각")
        };
        TimeInterval::try_new(at(start), at(end)).expect("구간")
    }

    fn cash(entry: &str, exit: &str, buy: &str, sell: &str) -> CashFixture {
        CashFixture(
            EstimateCashFlows::try_new(
                price(entry, Currency::Krw),
                price(exit, Currency::Krw),
                amount(buy, Currency::Krw),
                amount(sell, Currency::Krw),
                interval(1, 5),
            )
            .expect("현금 흐름"),
        )
    }

    fn benchmark(entry: &str, exit: &str) -> BenchmarkCashFlows {
        BenchmarkCashFlows::new(
            price(entry, Currency::Krw),
            price(exit, Currency::Krw),
            interval(1, 5),
        )
        .expect("벤치마크")
    }

    fn cmp(quotient: RatioQuotient, against: Ratio) -> std::cmp::Ordering {
        match quotient {
            RatioQuotient::Exact(value) => value.cmp(&against),
            RatioQuotient::Approximate(value) => value.cmp_ratio(&against),
        }
    }

    fn expect_approximate(quotient: RatioQuotient) {
        let approximate =
            RatioQuotient::Approximate(crate::ApproximateRatio::new(ratio("0").as_decimal()));
        assert_eq!(
            std::mem::discriminant(&quotient),
            std::mem::discriminant(&approximate),
            "근삿값이어야 함"
        );
    }

    #[test]
    fn d54_return_examples_preserve_sign_cost_and_excess() {
        for (exit, buy, sell, expected) in [
            ("120", "0", "0", "0.2"),
            ("80", "0", "0", "-0.2"),
            ("115", "2", "3", "0.0980392156862745098039215686"),
            ("100", "0", "0", "0"),
        ] {
            let actual = cash("100", exit, buy, sell).net().expect("수익률");
            assert_eq!(cmp(actual, ratio(expected)), std::cmp::Ordering::Equal);
        }
        let equal = cash("100", "120", "0", "0")
            .excess(&benchmark("250", "300"))
            .expect("같은 수익률");
        assert_eq!(cmp(equal, ratio("0")), std::cmp::Ordering::Equal);
        let excess = cash("100", "120", "0", "0")
            .excess(&benchmark("250", "275"))
            .expect("초과 수익률");
        assert_eq!(excess, RatioQuotient::Exact(ratio("0.1")));
    }

    #[test]
    fn d54_cash_flows_reject_mixed_currency_negative_cost_and_incomparable_periods() {
        let base = price("100", Currency::Krw);
        let end = price("120", Currency::Krw);
        let zero = amount("0", Currency::Krw);
        let span = interval(1, 5);
        assert_eq!(
            EstimateCashFlows::try_new(base, price("120", Currency::Usd), zero, zero, span),
            Err(EstimateReturnError::CurrencyMismatch)
        );
        assert_eq!(
            EstimateCashFlows::try_new(base, end, amount("0", Currency::Usd), zero, span),
            Err(EstimateReturnError::CurrencyMismatch)
        );
        assert_eq!(
            EstimateCashFlows::try_new(base, end, zero, amount("1", Currency::Usd), span),
            Err(EstimateReturnError::CurrencyMismatch)
        );
        assert_eq!(
            EstimateCashFlows::try_new(base, end, amount("-1", Currency::Krw), zero, span),
            Err(EstimateReturnError::NegativeCost)
        );
        assert_eq!(
            EstimateCashFlows::try_new(base, end, zero, amount("-1", Currency::Krw), span),
            Err(EstimateReturnError::NegativeCost)
        );
        assert_eq!(
            cash("100", "120", "0", "121")
                .net()
                .expect("수취액보다 큰 비용"),
            RatioQuotient::Exact(ratio("-1.01"))
        );
        assert_eq!(
            BenchmarkCashFlows::new(base, price("100", Currency::Usd), span),
            Err(EstimateReturnError::CurrencyMismatch)
        );
        let wrong_currency = BenchmarkCashFlows::new(
            price("100", Currency::Usd),
            price("110", Currency::Usd),
            span,
        )
        .expect("달러 비교");
        assert_eq!(
            cash("100", "120", "0", "0").excess(&wrong_currency),
            Err(EstimateReturnError::CurrencyMismatch)
        );
        let wrong_period = BenchmarkCashFlows::new(base, end, interval(1, 6)).expect("다른 구간");
        assert_eq!(
            cash("100", "120", "0", "0").excess(&wrong_period),
            Err(EstimateReturnError::PeriodMismatch)
        );
        let wrong_start = BenchmarkCashFlows::new(base, end, interval(2, 5)).expect("다른 시작");
        assert_eq!(
            cash("100", "120", "0", "0").excess(&wrong_start),
            Err(EstimateReturnError::PeriodMismatch)
        );
    }

    #[test]
    fn d54_nonterminating_division_is_marked_approximate_and_direction_is_retained() {
        let positive = cash("3", "4", "0", "0").net().expect("양수");
        expect_approximate(positive);
        assert_eq!(cmp(positive, ratio("0")), std::cmp::Ordering::Greater);
        let negative = cash("3", "2", "0", "0").net().expect("음수");
        expect_approximate(negative);
        assert_eq!(cmp(negative, ratio("0")), std::cmp::Ordering::Less);
        let positive_two_thirds = cash("3", "5", "0", "0").net().expect("양수 반올림");
        assert_eq!(
            cmp(positive_two_thirds, ratio("0.6666666666666666666666666667")),
            std::cmp::Ordering::Equal
        );
        let negative_two_thirds = cash("3", "1", "0", "0").net().expect("음수 반올림");
        assert_eq!(
            cmp(
                negative_two_thirds,
                ratio("-0.6666666666666666666666666667")
            ),
            std::cmp::Ordering::Equal
        );
        let equal_approximate = cash("3", "4", "0", "0")
            .excess(&benchmark("6", "8"))
            .expect("동일 근사 수익률");
        expect_approximate(equal_approximate);
        assert_eq!(
            cmp(equal_approximate, ratio("0")),
            std::cmp::Ordering::Equal
        );
        let approximate_minus_exact = cash("3", "4", "0", "0")
            .excess(&benchmark("100", "100"))
            .expect("근사에서 정확 차감");
        expect_approximate(approximate_minus_exact);
        assert_eq!(
            cmp(
                approximate_minus_exact,
                ratio("0.3333333333333333333333333333")
            ),
            std::cmp::Ordering::Equal
        );
        let exact_minus_approximate = cash("100", "100", "0", "0")
            .excess(&benchmark("3", "4"))
            .expect("정확에서 근사 차감");
        expect_approximate(exact_minus_approximate);
        assert_eq!(
            cmp(
                exact_minus_approximate,
                ratio("-0.3333333333333333333333333333")
            ),
            std::cmp::Ordering::Equal
        );
    }

    #[test]
    fn d54_overflow_is_not_turned_into_a_return() {
        let maximum = "79228162514264337593543950335";
        assert_eq!(
            cash(maximum, "1", "1", "0").net(),
            Err(EstimateReturnError::Arithmetic(crate::MoneyError::Overflow))
        );
        assert_eq!(
            cash("1", maximum, "0", "0").excess(&benchmark("1", "2")),
            Ok(RatioQuotient::Exact(ratio("79228162514264337593543950333")))
        );
    }

    #[test]
    fn d54_cash_flow_errors_propagate_at_each_arithmetic_boundary() {
        let maximum = "79228162514264337593543950335";
        let tiny = "0.0000000000000000000000000001";
        assert_eq!(
            cash("1", tiny, "0", maximum).net(),
            Err(EstimateReturnError::Arithmetic(
                crate::MoneyError::PrecisionLoss
            ))
        );
        assert_eq!(
            cash(maximum, "1", "0", maximum).net(),
            Err(EstimateReturnError::Arithmetic(crate::MoneyError::Overflow))
        );
        assert_eq!(
            cash(maximum, "1", "1", "0").excess(&benchmark("1", "2")),
            Err(EstimateReturnError::Arithmetic(crate::MoneyError::Overflow))
        );
        assert_eq!(
            cash("1", "2", "0", "0").excess(&benchmark(tiny, maximum)),
            Err(EstimateReturnError::Arithmetic(
                crate::MoneyError::PrecisionLoss
            ))
        );
        assert_eq!(
            cash("1", maximum, "0", "0").excess(&benchmark("2", "1")),
            Err(EstimateReturnError::Arithmetic(
                crate::MoneyError::PrecisionLoss
            ))
        );
    }

    #[test]
    fn d54_cash_flow_debug_hides_prices_and_costs() {
        assert_eq!(
            format!("{:?}", cash("123", "234", "5", "6")),
            "EstimateCashFlows([REDACTED])"
        );
        assert_eq!(
            format!("{:?}", benchmark("345", "456")),
            "BenchmarkCashFlows([REDACTED])"
        );
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(testkit::property_cases()))]
        #[test]
        fn d54_exit_is_monotone_and_cost_never_increases_return(
            first in 100_u32..10_000,
            delta in 0_u32..10_000,
            cost in 0_u32..100,
        ) {
            let last = first.checked_add(delta).expect("종가 범위");
            let expensive = cash("10000", &last.to_string(), &cost.to_string(), "0").net().expect("비용 수익률");
            let sell_cost = cash("10000", &last.to_string(), "0", &cost.to_string()).net().expect("매도 비용 수익률");
            let free = cash("10000", &last.to_string(), "0", "0").net().expect("무비용 수익률");
            let earlier = cash("10000", &first.to_string(), "0", "0").net().expect("낮은 종가 수익률");
            let equal = cash("10000", &last.to_string(), "0", "0").excess(&benchmark("10000", &last.to_string())).expect("동일 수익률");
            prop_assert_eq!(std::mem::discriminant(&free), std::mem::discriminant(&RatioQuotient::Exact(ratio("0"))), "무비용 수익률은 유한 십진수");
            let free_value = super::ratio_value(free);
            prop_assert_ne!(cmp(expensive, free_value), std::cmp::Ordering::Greater);
            prop_assert_ne!(cmp(sell_cost, free_value), std::cmp::Ordering::Greater);
            prop_assert_ne!(cmp(earlier, free_value), std::cmp::Ordering::Greater);
            prop_assert_eq!(cmp(equal, ratio("0")), std::cmp::Ordering::Equal);
        }
    }
}
