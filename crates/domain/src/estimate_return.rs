use crate::{
    ApproximateRatio, Currency, Money, MoneyError, Price, Ratio, RatioQuotient, TimeInterval,
};
use rust_decimal::Decimal;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EstimateEntryError {
    Instrument,
    Time,
    Currency,
}

#[derive(Clone, PartialEq, Eq)]
pub struct EstimateEntryPrice {
    submission: crate::SignalSubmission,
    observation: crate::PriceObservation,
}

impl std::fmt::Debug for EstimateEntryPrice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("EstimateEntryPrice([REDACTED])")
    }
}

impl EstimateEntryPrice {
    pub fn try_new(
        submission: &crate::SignalSubmission,
        observation: crate::PriceObservation,
    ) -> Result<Self, EstimateEntryError> {
        let task = submission.task_key().task();
        if observation.check_task_instrument(task) != crate::CheckResult::Passed {
            return Err(EstimateEntryError::Instrument);
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
            submission: submission.clone(),
            observation,
        })
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
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct EstimateCashFlows {
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
    pub fn try_new(
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

    pub fn net(&self) -> Result<RatioQuotient, EstimateReturnError> {
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

    pub fn excess(
        &self,
        benchmark: &BenchmarkCashFlows,
    ) -> Result<RatioQuotient, EstimateReturnError> {
        if self.entry.as_money().currency() != benchmark.currency() {
            return Err(EstimateReturnError::CurrencyMismatch);
        }
        if self.period != benchmark.period {
            return Err(EstimateReturnError::PeriodMismatch);
        }
        subtract(self.net()?, benchmark.rate()?)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct BenchmarkCashFlows {
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
    pub fn new(
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

    fn submission_fixture() -> (crate::EstimateTask, crate::SignalSubmission) {
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
        (task, submission)
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

    #[test]
    fn d54_submission_price_preserves_submission_and_rejects_other_times() {
        let (task, submission) = submission_fixture();
        let time = submission.submitted_at();
        let reference = task.reference();
        let valid = observed(
            task.volatility().instrument().clone(),
            reference.instrument().clone(),
            price("112", Currency::Krw),
            time,
        );
        let result =
            crate::EstimateEntryPrice::try_new(&submission, valid.clone()).expect("제출 가격");
        assert_eq!(result.observation(), &valid);
        assert_eq!(result.submitted_at(), time);
        assert_eq!(result.submission(), &submission);
        assert_eq!(format!("{result:?}"), "EstimateEntryPrice([REDACTED])");
        let early = observed(
            task.volatility().instrument().clone(),
            reference.instrument().clone(),
            price("112", Currency::Krw),
            task.created_at(),
        );
        assert_eq!(
            crate::EstimateEntryPrice::try_new(&submission, early),
            Err(crate::EstimateEntryError::Time)
        );
    }

    #[test]
    fn d54_submission_price_rejects_other_instruments_and_currencies() {
        let (task, submission) = submission_fixture();
        let time = submission.submitted_at();
        let reference = task.reference();
        let wrong_id = observed(
            crate::InstrumentId::try_new("other").expect("종목"),
            reference.instrument().clone(),
            price("112", Currency::Krw),
            time,
        );
        assert_eq!(
            crate::EstimateEntryPrice::try_new(&submission, wrong_id),
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
            crate::EstimateEntryPrice::try_new(&submission, wrong_symbol),
            Err(crate::EstimateEntryError::Instrument)
        );
        let wrong_currency = observed(
            task.volatility().instrument().clone(),
            reference.instrument().clone(),
            price("112", Currency::Usd),
            time,
        );
        assert_eq!(
            crate::EstimateEntryPrice::try_new(&submission, wrong_currency),
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
}
