use crate::thresholds_arithmetic::{ThresholdNumber, ThresholdValues};
use crate::{
    CalibratedProbability, CalibrationEvidence, EstimateTask, Ratio, RiskAppetite, TradeCost,
};
use rust_decimal::Decimal;
use std::cmp::Ordering;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoThresholds {
    InvalidMargin,
    ExcessiveCost,
    NumericalDomain,
    MarketMismatch,
    HorizonMismatch,
    CalibrationMismatch,
    CostPeriodMismatch,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Margin(Ratio);
impl Margin {
    pub fn try_new(value: Ratio) -> Result<Self, NoThresholds> {
        if !(Decimal::new(5, 2)..=Decimal::new(25, 2)).contains(&value.as_decimal()) {
            return Err(NoThresholds::InvalidMargin);
        }
        Ok(Self(value))
    }
    #[must_use]
    pub fn for_appetite(appetite: RiskAppetite) -> Self {
        Self(Ratio::new(match appetite {
            RiskAppetite::Conservative => Decimal::new(20, 2),
            RiskAppetite::Neutral => Decimal::new(15, 2),
            RiskAppetite::Aggressive => Decimal::new(10, 2),
        }))
    }
    fn for_calibration(self, calibration: &CalibratedProbability) -> SideMargins {
        match calibration {
            CalibratedProbability::InsufficientSamples(_) => SideMargins {
                buy: Self(Ratio::new(self.0.as_decimal().max(Decimal::new(15, 2)))),
                sell: Self(Ratio::new(Decimal::new(15, 2))),
            },
            CalibratedProbability::EligibleBeforeE91(_) => SideMargins {
                buy: self,
                sell: self,
            },
        }
    }
    pub(crate) fn ratio(self) -> Ratio {
        self.0
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SideMargins {
    pub(crate) buy: Margin,
    pub(crate) sell: Margin,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignalCondition {
    Met,
    Unmet,
}
#[derive(Clone, PartialEq, Eq)]
pub struct CostEligibleTask {
    task: EstimateTask,
    delta: ThresholdNumber,
}
impl CostEligibleTask {
    pub fn assess(task: EstimateTask, cost: &TradeCost) -> Result<Self, NoThresholds> {
        if task.reference().instrument().market() != cost.market() {
            return Err(NoThresholds::MarketMismatch);
        }
        if cost.period().task() != &task {
            return Err(NoThresholds::CostPeriodMismatch);
        }
        let delta = crate::thresholds_arithmetic::cost_delta(
            ThresholdNumber::new(task.target().as_decimal()),
            ThresholdNumber::new(task.stop().as_decimal()),
            ThresholdNumber::new(cost.rate().as_decimal()),
        )?;
        Ok(Self { task, delta })
    }
    #[must_use]
    pub fn task(&self) -> &EstimateTask {
        &self.task
    }
}
impl std::fmt::Debug for CostEligibleTask {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("CostEligibleTask(..)")
    }
}
impl Default for Margin {
    fn default() -> Self {
        Self::for_appetite(RiskAppetite::Neutral)
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThresholdKind {
    Breakeven,
    Buy,
    Sell,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThresholdDisplay(String);
impl ThresholdDisplay {
    pub(crate) fn new(value: String) -> Self {
        Self(value)
    }
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
#[derive(Clone, PartialEq, Eq)]
pub struct Thresholds {
    values: ThresholdValues,
    source: CalibratedProbability,
}
impl std::fmt::Debug for Thresholds {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Thresholds(..)")
    }
}
fn evidence(calibration: &CalibratedProbability) -> &CalibrationEvidence {
    match calibration {
        CalibratedProbability::InsufficientSamples(value) => value.evidence(),
        CalibratedProbability::EligibleBeforeE91(value) => value.evidence(),
    }
}
impl Thresholds {
    pub fn create(
        eligible: &CostEligibleTask,
        calibration: &CalibratedProbability,
        margin: Margin,
    ) -> Result<Self, NoThresholds> {
        let task = eligible.task();
        if task.horizon()
            != crate::TradingDayHorizon::from_estimate_policy(
                evidence(calibration).key().horizon().selection(),
            )
        {
            return Err(NoThresholds::HorizonMismatch);
        }
        crate::thresholds_arithmetic::calculate_values(
            ThresholdNumber::new(task.base_probability().as_decimal()),
            eligible.delta,
            margin.for_calibration(calibration),
        )
        .map(|values| Self {
            values,
            source: calibration.clone(),
        })
    }
    pub fn buy_condition(
        &self,
        calibration: &CalibratedProbability,
    ) -> Result<SignalCondition, NoThresholds> {
        self.compare(calibration, ThresholdKind::Buy)
            .map(buy_condition_from_order)
    }
    pub fn sell_condition(
        &self,
        calibration: &CalibratedProbability,
    ) -> Result<SignalCondition, NoThresholds> {
        self.compare(calibration, ThresholdKind::Sell)
            .map(sell_condition_from_order)
    }
    pub(crate) fn compare(
        &self,
        calibration: &CalibratedProbability,
        kind: ThresholdKind,
    ) -> Result<Ordering, NoThresholds> {
        let input = evidence(calibration);
        if calibration != &self.source {
            return Err(NoThresholds::CalibrationMismatch);
        }
        Ok(ThresholdNumber::new(input.probability().as_decimal()).cmp(&self.value(kind)))
    }
    pub fn display(&self, kind: ThresholdKind) -> Result<ThresholdDisplay, NoThresholds> {
        self.value(kind).percent(kind)
    }
    fn value(&self, kind: ThresholdKind) -> ThresholdNumber {
        match kind {
            ThresholdKind::Buy => self.values.buy,
            ThresholdKind::Sell => self.values.sell,
            ThresholdKind::Breakeven => self.values.breakeven,
        }
    }
}

fn buy_condition_from_order(ordering: Ordering) -> SignalCondition {
    match ordering {
        Ordering::Less => SignalCondition::Unmet,
        Ordering::Equal | Ordering::Greater => SignalCondition::Met,
    }
}
fn sell_condition_from_order(ordering: Ordering) -> SignalCondition {
    match ordering {
        Ordering::Greater => SignalCondition::Unmet,
        Ordering::Equal | Ordering::Less => SignalCondition::Met,
    }
}
pub(crate) fn bound_signal_conditions(
    thresholds: &Thresholds,
) -> (SignalCondition, SignalCondition) {
    let probability = ThresholdNumber::new(evidence(&thresholds.source).probability().as_decimal());
    (
        buy_condition_from_order(probability.cmp(&thresholds.value(ThresholdKind::Buy))),
        sell_condition_from_order(probability.cmp(&thresholds.value(ThresholdKind::Sell))),
    )
}

#[cfg(test)]
pub(crate) mod tests {
    use crate::thresholds_arithmetic::ThresholdNumber;
    use crate::{Margin, NoThresholds, NumericInput, Ratio};
    use proptest::test_runner::{Config, TestRunner};
    use rust_decimal::Decimal;

    fn calculate(
        base: ThresholdNumber,
        target: ThresholdNumber,
        stop: ThresholdNumber,
        cost: ThresholdNumber,
        margin: Margin,
    ) -> Result<crate::thresholds_arithmetic::ThresholdValues, NoThresholds> {
        let delta = crate::thresholds_arithmetic::cost_delta(target, stop, cost)?;
        crate::thresholds_arithmetic::calculate_values(
            base,
            delta,
            super::SideMargins {
                buy: margin,
                sell: margin,
            },
        )
    }
    pub(crate) fn cost_period() -> crate::EstimateCostPeriod {
        crate::EstimateCostPeriod::from_calendar_result(&task(), crate::candle::tests::at(121))
            .expect("캘린더 만기")
    }
    fn number(value: i64, scale: u32) -> ThresholdNumber {
        ThresholdNumber::new(Decimal::new(value, scale))
    }
    fn margin(value: &str) -> Margin {
        Margin::try_new(Ratio::parse(&NumericInput::try_new(value).expect("입력")).expect("비율"))
            .expect("여유분")
    }

    fn calibrated(count: usize) -> crate::CalibratedProbability {
        calibrated_at(count, 50)
    }
    fn calibrated_at(count: usize, percent: i64) -> crate::CalibratedProbability {
        use crate::{
            AiProbability, AiProbabilityInput, Calibration, CalibrationEstimate,
            CalibrationEstimateId, CalibrationFact, CalibrationKey, CalibrationModel,
            CalibrationOutcome, Horizon, InstrumentId, ModelIdentityPart, TimeInterval,
        };
        let part = ModelIdentityPart::try_new("test").expect("모델");
        let key = CalibrationKey::new(
            CalibrationModel::new(part.clone(), part.clone(), part),
            Horizon::default(),
        );
        let base =
            AiProbability::parse(&AiProbabilityInput::try_new(&percent.to_string()).expect("입력"))
                .expect("확률");
        let facts = (0..count)
            .map(|index| {
                CalibrationFact::new(
                    CalibrationEstimate::new(
                        CalibrationEstimateId::try_new(&format!("e{index}")).expect("ID"),
                        InstrumentId::try_new(&format!("index{index}")).expect("종목"),
                        key.clone(),
                        TimeInterval::try_new(
                            crate::candle::tests::at(1),
                            crate::candle::tests::at(2),
                        )
                        .expect("기간"),
                        base,
                    ),
                    CalibrationOutcome::Hit,
                )
            })
            .collect::<Vec<_>>();
        Calibration::try_new(key.clone(), &facts)
            .expect("자료")
            .calibrate(&key, base)
    }

    #[test]
    fn conservative_margin_overrides_both_larger_and_smaller_settings() {
        let insufficient = calibrated(29);
        let eligible = calibrated(30);
        TestRunner::new(Config::with_cases(4096))
            .run(&(5_i64..=25), |setting| {
                let configured = margin(&Decimal::new(setting, 2).to_string());
                proptest::prop_assert_eq!(
                    configured.for_calibration(&insufficient).buy,
                    margin(&Decimal::new(setting.max(15), 2).to_string())
                );
                proptest::prop_assert_eq!(configured.for_calibration(&eligible).buy, configured);
                Ok(())
            })
            .expect("29/30 경계와 설정 강제");
        assert_eq!(
            margin("0.25").for_calibration(&calibrated(0)).sell,
            margin("0.15")
        );
        for value in ["0.049", "0.251"] {
            assert_eq!(
                Margin::try_new(
                    Ratio::parse(&NumericInput::try_new(value).expect("입력")).expect("비율")
                ),
                Err(NoThresholds::InvalidMargin)
            );
        }
    }

    #[test]
    fn rational_reference_checks_thresholds_and_exact_cost_boundary() {
        TestRunner::new(Config::with_cases(4096))
            .run(
                &(
                    1_i64..=400,
                    1_i64..=400,
                    0_i64..=400,
                    5_i64..=25,
                    0_i64..=100,
                ),
                |(target, stop, cost, buffer, base)| {
                    let result = calculate(
                        number(base, 2),
                        number(target, 3),
                        number(stop, 3),
                        number(cost, 5),
                        margin(&Decimal::new(buffer, 2).to_string()),
                    );
                    if cost * 20 > (target + stop) * 100 {
                        proptest::prop_assert_eq!(result, Err(NoThresholds::ExcessiveCost));
                    } else {
                        let values = result.expect("5%base 이하");
                        proptest::prop_assert!(values.buy >= values.breakeven);
                        proptest::prop_assert!(values.breakeven >= number(base, 2));
                        proptest::prop_assert!(values.sell >= number(5, 2));
                        let actual = values.breakeven.as_decimal();
                        let denominator = i128::from((target + stop) * 100);
                        let numerator = i128::from(base * (target + stop) + cost);
                        let units = 10_i128.pow(actual.scale());
                        let residual = (actual.mantissa() * denominator - numerator * units).abs();
                        proptest::prop_assert!(residual * 2 <= denominator);
                        proptest::prop_assert_eq!(
                            values.sell.as_decimal(),
                            Decimal::new(base - buffer, 2).max(Decimal::new(5, 2))
                        );
                    }
                    Ok(())
                },
            )
            .expect("독립 정수 유리수 기준");
    }

    #[test]
    fn exact_five_point_boundary_and_sell_floor() {
        let boundary = calculate(
            number(30, 2),
            number(2, 2),
            number(3, 2),
            number(25, 4),
            margin("0.10"),
        )
        .expect("정확히 5%p 허용");
        assert_eq!(boundary.breakeven, number(35, 2));
        assert_eq!(
            calculate(
                number(30, 2),
                number(2, 2),
                number(3, 2),
                number(2_500_001, 9),
                margin("0.10")
            ),
            Err(NoThresholds::ExcessiveCost)
        );
        assert_eq!(
            calculate(
                number(20, 2),
                number(2, 2),
                number(3, 2),
                number(0, 0),
                margin("0.15")
            )
            .expect("경계")
            .sell,
            number(5, 2)
        );
    }

    #[test]
    fn documented_examples_use_unrounded_formula_inputs() {
        for (target, stop, base, expected) in [
            ("0.01", "0.005", "0.2726583497377840129425013015", None),
            (
                "0.0447213595499957939281834734",
                "0.0223606797749978969640917367",
                "0.2752596578945449105591702896",
                Some(("31.3", "41.3", "17.5")),
            ),
            (
                "0.1264911064067351732799557418",
                "0.0632455532033675866399778709",
                "0.2806863606500262171700824076",
                Some(("29.4", "39.4", "18.1")),
            ),
            (
                "0.2190890230020664453827879131",
                "0.1095445115010332226913939565",
                "0.2858154623201750689564720914",
                Some(("29.3", "39.3", "18.6")),
            ),
        ] {
            let parse = |text| {
                ThresholdNumber::new(
                    Ratio::parse(&NumericInput::try_new(text).expect("입력"))
                        .expect("비율")
                        .as_decimal(),
                )
            };
            let result = calculate(
                parse(base),
                parse(target),
                parse(stop),
                number(25, 4),
                margin("0.10"),
            );
            match expected {
                None => assert_eq!(result, Err(NoThresholds::ExcessiveCost)),
                Some((be, buy, sell)) => {
                    let values = result.expect("과제");
                    assert_eq!(
                        crate::thresholds_arithmetic::tests::document_rounding(values.breakeven)
                            .as_str(),
                        format!("≈{be}%")
                    );
                    assert_eq!(
                        crate::thresholds_arithmetic::tests::document_rounding(values.buy).as_str(),
                        format!("≈{buy}%")
                    );
                    assert_eq!(
                        crate::thresholds_arithmetic::tests::document_rounding(values.sell)
                            .as_str(),
                        format!("≈{sell}%")
                    );
                }
            }
        }
    }

    pub(crate) fn task() -> crate::EstimateTask {
        task_for(crate::Horizon::default())
    }
    pub(crate) fn task_for(horizon: crate::Horizon) -> crate::EstimateTask {
        use crate::{
            Currency, EstimateContext, EstimateTask, HoursPermission, Market, PriceObservation,
            ResolvedOrderInstrument, RiskAppetite, Symbol, Volatility,
        };
        let texts = (0_i32..61_i32)
            .map(|index| if index % 2_i32 == 0_i32 { "100" } else { "102" })
            .collect::<Vec<_>>();
        let volatility =
            Volatility::calculate(&crate::candle::tests::series(&texts)).expect("변동성");
        let raw_at = crate::candle::tests::at(61);
        let at = crate::Timestamp::from_parts(
            raw_at.utc_seconds(),
            raw_at.nanosecond(),
            crate::OffsetSeconds::new(32_400),
        )
        .expect("현지 시각");
        let instrument = ResolvedOrderInstrument::from_master_lookup_result(
            Symbol::try_new("ABC").expect("코드"),
            Market::Kr,
        )
        .expect("시장");
        let reference = PriceObservation::from_identified_quote_parts(
            volatility.instrument().clone(),
            instrument,
            crate::candle::tests::price("100", Currency::Krw),
            at,
        )
        .expect("시세");
        EstimateTask::create(
            EstimateContext::new(reference, at),
            volatility,
            horizon,
            HoursPermission::default(),
            RiskAppetite::Neutral,
        )
        .expect("과제")
    }

    #[test]
    fn sealed_task_cost_and_calibration_feed_comparisons_and_display() {
        use crate::{
            CommissionRateFact, CostRate, Market, RiskAppetite, ThresholdKind, Thresholds,
            TradeCost, TradeTaxFact,
        };
        let zero = CostRate::try_new(Ratio::new(Decimal::ZERO)).expect("0");
        let cost = TradeCost::from_facts(
            Some(CommissionRateFact::from_commissions_result(
                Market::Kr,
                zero,
                crate::CommissionValidity::try_new(None, None).expect("적용 기간"),
            )),
            Some(TradeTaxFact::from_schedule_result(Market::Kr, zero, zero)),
            cost_period(),
        )
        .expect("비용");
        let cal = calibrated(29);
        let thresholds = Thresholds::create(
            &crate::CostEligibleTask::assess(task(), &cost).expect("비용 적격"),
            &cal,
            margin("0.10"),
        )
        .expect("임계값");
        for kind in [
            ThresholdKind::Buy,
            ThresholdKind::Sell,
            ThresholdKind::Breakeven,
        ] {
            assert_eq!(
                thresholds.compare(&cal, kind).expect("동일 모델 기간"),
                std::cmp::Ordering::Greater
            );
            assert!(
                thresholds
                    .display(kind)
                    .expect("표시")
                    .as_str()
                    .starts_with('≈')
            );
        }
        assert_eq!(Margin::default(), margin("0.15"));
        for (risk, text) in [
            (RiskAppetite::Conservative, "0.20"),
            (RiskAppetite::Neutral, "0.15"),
            (RiskAppetite::Aggressive, "0.10"),
        ] {
            assert_eq!(Margin::for_appetite(risk), margin(text));
        }
    }

    #[test]
    fn numerical_failures_and_sub_ulp_cost_excess_remain_closed() {
        let parse = |value| {
            ThresholdNumber::new(
                Ratio::parse(&NumericInput::try_new(value).expect("입력"))
                    .expect("비율")
                    .as_decimal(),
            )
        };
        assert_eq!(
            calculate(
                number(30, 2),
                number(1, 1),
                number(1, 1),
                parse("0.0100000000000000000000000001"),
                margin("0.10")
            ),
            Err(NoThresholds::ExcessiveCost)
        );
        assert_eq!(
            calculate(
                number(30, 2),
                number(0, 0),
                number(0, 0),
                number(0, 0),
                margin("0.10")
            ),
            Err(NoThresholds::NumericalDomain)
        );
        assert_eq!(
            calculate(
                number(30, 2),
                ThresholdNumber::new(Decimal::MAX),
                ThresholdNumber::new(Decimal::MAX),
                number(0, 0),
                margin("0.10")
            ),
            Err(NoThresholds::NumericalDomain)
        );
        assert_eq!(
            ThresholdNumber::new(Decimal::MAX).percent(crate::ThresholdKind::Buy),
            Err(NoThresholds::NumericalDomain)
        );
        assert_eq!(format!("{:?}", number(12345, 5)), "ThresholdNumber(..)");
    }

    fn free_cost(market: crate::Market) -> crate::TradeCost {
        let zero = crate::CostRate::try_new(Ratio::new(Decimal::ZERO)).expect("요율");
        crate::TradeCost::from_facts(
            Some(crate::CommissionRateFact::from_commissions_result(
                market.clone(),
                zero,
                crate::CommissionValidity::try_new(None, None).expect("적용 기간"),
            )),
            Some(crate::TradeTaxFact::from_schedule_result(
                market, zero, zero,
            )),
            cost_period(),
        )
        .expect("비용 사실")
    }

    #[test]
    fn mismatched_market_horizon_and_calibration_are_rejected() {
        use crate::{
            AiProbability, AiProbabilityInput, Calibration, CalibrationKey, Horizon,
            HorizonSelection, Market, ThresholdKind, Thresholds,
        };
        let source = calibrated(30);
        let thresholds = Thresholds::create(
            &crate::CostEligibleTask::assess(task(), &free_cost(Market::Kr)).expect("비용 적격"),
            &source,
            margin("0.10"),
        )
        .expect("임계값");
        assert_eq!(
            thresholds.compare(&calibrated(29), ThresholdKind::Buy),
            Err(NoThresholds::CalibrationMismatch)
        );
        assert_eq!(
            crate::CostEligibleTask::assess(task(), &free_cost(Market::Us)),
            Err(NoThresholds::MarketMismatch)
        );
        let key = super::evidence(&source).key().clone();
        let other_key =
            CalibrationKey::new(key.model().clone(), Horizon::new(HorizonSelection::FewDays));
        let probability =
            AiProbability::parse(&AiProbabilityInput::try_new("50").expect("입력")).expect("확률");
        let other = Calibration::try_new(other_key.clone(), &[])
            .expect("자료")
            .calibrate(&other_key, probability);
        assert_eq!(
            Thresholds::create(
                &crate::CostEligibleTask::assess(task(), &free_cost(Market::Kr))
                    .expect("비용 적격"),
                &other,
                margin("0.10")
            ),
            Err(NoThresholds::HorizonMismatch)
        );
    }
    #[test]
    fn excessive_cost_propagates_from_the_public_task_path() {
        let one = crate::CostRate::try_new(Ratio::new(Decimal::ONE)).expect("요율");
        let zero = crate::CostRate::try_new(Ratio::new(Decimal::ZERO)).expect("요율");
        let cost = crate::TradeCost::from_facts(
            Some(crate::CommissionRateFact::from_commissions_result(
                crate::Market::Kr,
                one,
                crate::CommissionValidity::try_new(None, None).expect("적용 기간"),
            )),
            Some(crate::TradeTaxFact::from_schedule_result(
                crate::Market::Kr,
                zero,
                zero,
            )),
            cost_period(),
        )
        .expect("비용");
        assert_eq!(
            crate::CostEligibleTask::assess(task(), &cost),
            Err(NoThresholds::ExcessiveCost)
        );
    }
    #[test]
    fn numeric_extremes_propagate_a_single_failure_result() {
        let max = ThresholdNumber::new(Decimal::MAX);
        let min = ThresholdNumber::new(Decimal::MIN);
        assert_eq!(
            calculate(number(30, 2), max, number(0, 0), max, margin("0.10")),
            Err(NoThresholds::NumericalDomain)
        );
        assert_eq!(
            calculate(
                min,
                number(1, 0),
                number(0, 0),
                number(-1, 0),
                margin("0.10")
            ),
            Err(NoThresholds::NumericalDomain)
        );
    }
    fn independent_conditions(
        task: &crate::EstimateTask,
        setting: i64,
        count: usize,
        percent: i64,
    ) -> (crate::SignalCondition, crate::SignalCondition) {
        let buy_margin = if count < 30 { setting.max(15) } else { setting };
        let sell_margin = if count < 30 { 15 } else { setting };
        let probability = Decimal::new(percent, 2);
        let base = task.base_probability().as_decimal();
        let buy = if probability
            >= base
                .checked_add(Decimal::new(buy_margin, 2))
                .expect("독립 기준 합")
        {
            crate::SignalCondition::Met
        } else {
            crate::SignalCondition::Unmet
        };
        let sell = if probability
            <= (base
                .checked_sub(Decimal::new(sell_margin, 2))
                .expect("독립 기준 차"))
            .max(Decimal::new(5, 2))
        {
            crate::SignalCondition::Met
        } else {
            crate::SignalCondition::Unmet
        };
        (buy, sell)
    }
    #[test]
    fn public_creation_enforces_side_specific_margins_against_independent_reference() {
        let task = task();
        let eligible = crate::CostEligibleTask::assess(task.clone(), &free_cost(crate::Market::Kr))
            .expect("비용 적격");
        TestRunner::new(Config::with_cases(4096))
            .run(&(5_i64..=25, 1_i64..=99), |(setting, percent)| {
                for count in [29_usize, 30_usize] {
                    let calibrated = calibrated_at(count, percent);
                    let result = crate::Thresholds::create(
                        &eligible,
                        &calibrated,
                        margin(&Decimal::new(setting, 2).to_string()),
                    )
                    .expect("임계값");
                    let (buy_expected, sell_expected) =
                        independent_conditions(&task, setting, count, percent);
                    proptest::prop_assert_eq!(
                        result.buy_condition(&calibrated).expect("매수 판정"),
                        buy_expected
                    );
                    proptest::prop_assert_eq!(
                        result.sell_condition(&calibrated).expect("매도 판정"),
                        sell_expected
                    );
                }
                Ok(())
            })
            .expect("실제 생성 경로의 29/30건 여유분");
    }
    #[test]
    fn table_row_kind_routing_and_both_sides_of_each_boundary() {
        use crate::{SignalCondition, ThresholdKind};
        for (kind, shown, lower, upper) in [
            (ThresholdKind::Breakeven, "≈31.3%", 31_i64, 32_i64),
            (ThresholdKind::Buy, "≈41.3%", 41, 42),
            (ThresholdKind::Sell, "≈17.5%", 17, 18),
        ] {
            for (percent, expected) in [
                (lower, std::cmp::Ordering::Less),
                (upper, std::cmp::Ordering::Greater),
            ] {
                let source = calibrated_at(30, percent);
                let thresholds = super::Thresholds {
                    values: crate::thresholds_arithmetic::ThresholdValues {
                        breakeven: number(31_252_745_751, 11),
                        buy: number(41_252_745_751, 11),
                        sell: number(17_525_965_789, 11),
                    },
                    source: source.clone(),
                };
                assert_eq!(thresholds.display(kind).expect("표시").as_str(), shown);
                assert_eq!(thresholds.compare(&source, kind).expect("비교"), expected);
            }
        }
        let source = calibrated_at(30, 40);
        let thresholds = super::Thresholds {
            values: crate::thresholds_arithmetic::ThresholdValues {
                breakeven: number(40, 2),
                buy: number(40, 2),
                sell: number(40, 2),
            },
            source: source.clone(),
        };
        assert_eq!(thresholds.buy_condition(&source), Ok(SignalCondition::Met));
        assert_eq!(thresholds.sell_condition(&source), Ok(SignalCondition::Met));
        assert_eq!(format!("{thresholds:?}"), "Thresholds(..)");
    }
    #[test]
    fn eligibility_is_bound_to_the_calendar_task_and_hides_debug_values() {
        let original = task();
        let cost = free_cost(crate::Market::Kr);
        let different = task_for(crate::Horizon::new(crate::HorizonSelection::FewDays));
        assert_eq!(
            crate::CostEligibleTask::assess(different, &cost),
            Err(NoThresholds::CostPeriodMismatch)
        );
        let eligible = crate::CostEligibleTask::assess(original, &cost).expect("비용 적격");
        assert_eq!(format!("{eligible:?}"), "CostEligibleTask(..)");
        let source = calibrated(30);
        let thresholds =
            crate::Thresholds::create(&eligible, &source, margin("0.10")).expect("임계값");
        assert_eq!(
            thresholds.buy_condition(&calibrated(29)),
            Err(NoThresholds::CalibrationMismatch)
        );
        assert_eq!(
            thresholds.sell_condition(&calibrated(29)),
            Err(NoThresholds::CalibrationMismatch)
        );
    }
    #[test]
    fn displayed_boundaries_round_toward_the_signal_condition() {
        let source = calibrated_at(30, 40);
        let thresholds = super::Thresholds {
            values: crate::thresholds_arithmetic::ThresholdValues {
                breakeven: number(3131, 4),
                buy: number(4131, 4),
                sell: number(1759, 4),
            },
            source,
        };
        for (kind, expected) in [
            (crate::ThresholdKind::Breakeven, "≈31.4%"),
            (crate::ThresholdKind::Buy, "≈41.4%"),
            (crate::ThresholdKind::Sell, "≈17.5%"),
        ] {
            assert_eq!(
                thresholds.display(kind).expect("방향 반올림").as_str(),
                expected
            );
        }
    }
}
