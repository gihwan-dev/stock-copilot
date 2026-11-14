use crate::{
    Allocation, AllocationError, Amount, AssetInstrument, MoneyError, RatioQuotient,
    ValuationAccuracy,
};
use std::cmp::Ordering;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConcentrationDisplay(String);
impl ConcentrationDisplay {
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConcentrationHhi(Amount);
impl ConcentrationHhi {
    #[must_use]
    pub fn display(&self) -> ConcentrationDisplay {
        display(self.0, DisplayPlaces(4))
    }
    #[must_use]
    pub fn compare(&self, other: &Self) -> Ordering {
        self.0.as_decimal().cmp(&other.0.as_decimal())
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EffectiveInstrumentCount(Amount);
impl EffectiveInstrumentCount {
    #[must_use]
    pub fn display(&self) -> ConcentrationDisplay {
        display(self.0, DisplayPlaces(6))
    }
    #[must_use]
    pub fn compare(&self, other: &Self) -> Ordering {
        self.0.as_decimal().cmp(&other.0.as_decimal())
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConcentrationWeight(Amount);
impl ConcentrationWeight {
    #[must_use]
    pub fn display(&self) -> ConcentrationDisplay {
        display(self.0, DisplayPlaces(2))
    }
    #[must_use]
    pub fn compare(&self, other: &Self) -> Ordering {
        self.0.as_decimal().cmp(&other.0.as_decimal())
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConcentrationError {
    Allocation(AllocationError),
    Arithmetic(MoneyError),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConcentrationWarning {
    BelowFive,
    AtLeastFive,
    NotApplicable,
    Unavailable,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConcentrationMetrics {
    accuracy: ValuationAccuracy,
    hhi: ConcentrationHhi,
    effective_count: EffectiveInstrumentCount,
    top_one: ConcentrationWeight,
    top_three: ConcentrationWeight,
    top_five: ConcentrationWeight,
}
impl ConcentrationMetrics {
    #[must_use]
    pub const fn hhi(&self) -> &ConcentrationHhi {
        &self.hhi
    }
    #[must_use]
    pub const fn effective_count(&self) -> &EffectiveInstrumentCount {
        &self.effective_count
    }
    #[must_use]
    pub const fn top_one(&self) -> &ConcentrationWeight {
        &self.top_one
    }
    #[must_use]
    pub const fn top_three(&self) -> &ConcentrationWeight {
        &self.top_three
    }
    #[must_use]
    pub const fn top_five(&self) -> &ConcentrationWeight {
        &self.top_five
    }
    #[must_use]
    pub const fn accuracy(&self) -> ValuationAccuracy {
        self.accuracy
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConcentrationStatus {
    Available(ConcentrationMetrics),
    NotApplicable,
    Unavailable(ConcentrationError),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Concentration(ConcentrationStatus);
impl Concentration {
    #[must_use]
    pub fn new(allocation: &Allocation) -> Self {
        let values = match allocation.assets().instruments() {
            Err(AllocationError::ZeroTotal) => return Self(ConcentrationStatus::NotApplicable),
            Err(error) => {
                return Self(ConcentrationStatus::Unavailable(
                    ConcentrationError::Allocation(*error),
                ));
            }
            Ok(values) => values,
        };
        if values.entries().iter().all(|entry| match entry.key() {
            AssetInstrument::Holding(_) => {
                entry.display().range() == crate::AllocationDisplayRange::Zero
            }
            AssetInstrument::Cash(_) => true,
        }) {
            return Self(ConcentrationStatus::NotApplicable);
        }
        let mut weights: Vec<_> = values
            .entries()
            .iter()
            .filter_map(|entry| match entry.key() {
                AssetInstrument::Holding(_) => Some(entry.weight().concentration_input()),
                AssetInstrument::Cash(_) => None,
            })
            .collect();
        weights.sort_by_key(|weight| std::cmp::Reverse(weight.as_decimal()));
        Self(match calculate(&weights) {
            Ok(metrics) => ConcentrationStatus::Available(metrics),
            Err(error) => ConcentrationStatus::Unavailable(ConcentrationError::Arithmetic(error)),
        })
    }
    #[must_use]
    pub const fn status(&self) -> &ConcentrationStatus {
        &self.0
    }
    #[must_use]
    pub fn warning(&self) -> ConcentrationWarning {
        match &self.0 {
            ConcentrationStatus::Available(value) => {
                if value.hhi.0.as_decimal() > rust_decimal::Decimal::new(20, 2) {
                    ConcentrationWarning::BelowFive
                } else {
                    ConcentrationWarning::AtLeastFive
                }
            }
            ConcentrationStatus::NotApplicable => ConcentrationWarning::NotApplicable,
            ConcentrationStatus::Unavailable(_) => ConcentrationWarning::Unavailable,
        }
    }
}
fn calculate(weights: &[Amount]) -> Result<ConcentrationMetrics, MoneyError> {
    let zero = Amount::new(rust_decimal::Decimal::ZERO);
    let top_one = ConcentrationWeight(weights.first().copied().unwrap_or(zero));
    let top_three = sum_weights(weights, TopCount(3))?;
    let top_five = sum_weights(weights, TopCount(5))?;
    let hhi = weights.iter().try_fold(zero, |sum, weight| {
        let square = crate::exact_arithmetic::concentration_square(*weight)?;
        sum.checked_add(&square)
    })?;
    let reciprocal = crate::exact_arithmetic::divide(Amount::new(rust_decimal::Decimal::ONE), hhi)
        .map_err(|_error| MoneyError::PrecisionLoss)?;
    let effective = match reciprocal {
        RatioQuotient::Exact(value) => Amount::new(value.as_decimal()),
        RatioQuotient::Approximate(value) => Amount::new(value.as_decimal()),
    };
    Ok(ConcentrationMetrics {
        accuracy: ValuationAccuracy::Approximate,
        hhi: ConcentrationHhi(hhi),
        effective_count: EffectiveInstrumentCount(effective),
        top_one,
        top_three,
        top_five,
    })
}
fn sum_weights(weights: &[Amount], limit: TopCount) -> Result<ConcentrationWeight, MoneyError> {
    weights
        .iter()
        .take(limit.0)
        .try_fold(Amount::new(rust_decimal::Decimal::ZERO), |sum, weight| {
            sum.checked_add(weight)
        })
        .map(ConcentrationWeight)
}
#[derive(Clone, Copy)]
struct TopCount(usize);
#[cfg(test)]
mod tests {
    use super::{
        Concentration, ConcentrationError, ConcentrationMetrics, ConcentrationStatus,
        ConcentrationWarning, calculate,
    };
    use crate::{
        AcquisitionPrice, BuyingPower, Currency, Holding, HoldingObservation, Market, Money,
        Nanosecond, NumericInput, OffsetSeconds, Portfolio, Quantity, Ratio, ReportedHolding,
        ResolvedOrderInstrument, SectorAssignments, Symbol, Timestamp, UnixSeconds,
    };
    use crate::{Allocation, AllocationError, Amount, MoneyError, ValuationAccuracy};
    use proptest::test_runner::{Config, TestRunner};
    use std::cmp::Ordering;

    fn allocation(values: &[Option<&str>], cash: &str) -> Allocation {
        let input = |text: &str| NumericInput::try_new(text).expect("입력");
        let money = |text: &str| Money::parse(&input(text), Currency::Krw).expect("금액");
        let time = Timestamp::from_parts(
            UnixSeconds::new(1000),
            Nanosecond::new(0),
            OffsetSeconds::new(0),
        )
        .expect("시각");
        let holdings = values
            .iter()
            .enumerate()
            .map(|(index, value)| {
                let instrument = ResolvedOrderInstrument::from_master_lookup_result(
                    Symbol::try_new(&format!("S{index}")).expect("심볼"),
                    Market::Kr,
                )
                .expect("종목");
                let reported = value.map(|text| {
                    ReportedHolding::new(
                        money(text),
                        money("0"),
                        Ratio::parse(&input("0")).expect("율"),
                    )
                    .expect("평가")
                });
                Holding::new(
                    instrument,
                    Quantity::parse(&input("1")).expect("수량"),
                    AcquisitionPrice::try_new(money("0")).expect("원가"),
                    HoldingObservation::new(None, reported, time),
                )
                .expect("보유")
            })
            .collect();
        let valuation = Portfolio::new(
            holdings,
            BuyingPower::new(money(cash), time),
            BuyingPower::new(
                Money::parse(&input("0"), Currency::Usd).expect("달러 현금"),
                time,
            ),
        )
        .expect("포트폴리오")
        .evaluate(None);
        Allocation::new(&valuation, &SectorAssignments::default())
    }
    fn metrics(concentration: &Concentration) -> &ConcentrationMetrics {
        match concentration.status() {
            ConcentrationStatus::Available(value) => value,
            other @ (ConcentrationStatus::NotApplicable | ConcentrationStatus::Unavailable(_)) => {
                panic!("계산 가능 기대: {other:?}")
            }
        }
    }
    #[test]
    fn r11_example_eight_matches_every_digit_and_top_n() {
        let value = Concentration::new(&allocation(
            &[
                Some("35"),
                Some("25"),
                Some("15"),
                Some("10"),
                Some("10"),
                Some("5"),
            ],
            "0",
        ));
        let result = metrics(&value);
        assert_eq!(result.hhi().display().as_str(), "0.2300");
        assert_eq!(result.effective_count().display().as_str(), "4.347826");
        assert_eq!(result.top_one().display().as_str(), "0.35");
        assert_eq!(result.top_three().display().as_str(), "0.75");
        assert_eq!(result.top_five().display().as_str(), "0.95");
        assert_eq!(value.warning(), ConcentrationWarning::BelowFive);
        assert_eq!(result.accuracy(), ValuationAccuracy::Approximate);
    }
    #[test]
    fn warning_boundaries_single_empty_zero_and_cash() {
        for (values, cash, expected, count) in [
            (
                vec![Some("1"); 5],
                "0",
                ConcentrationWarning::AtLeastFive,
                Some("5.000000"),
            ),
            (
                vec![Some("1.000001"), Some("1"), Some("1"), Some("1"), Some("1")],
                "0",
                ConcentrationWarning::BelowFive,
                Some("5.000000"),
            ),
            (
                vec![Some("1")],
                "0",
                ConcentrationWarning::BelowFive,
                Some("1.000000"),
            ),
            (vec![], "0", ConcentrationWarning::NotApplicable, None),
            (vec![], "100", ConcentrationWarning::NotApplicable, None),
            (
                vec![Some("0")],
                "100",
                ConcentrationWarning::NotApplicable,
                None,
            ),
            (
                vec![Some("1")],
                "9",
                ConcentrationWarning::AtLeastFive,
                Some("100.000000"),
            ),
        ] {
            let value = Concentration::new(&allocation(&values, cash));
            assert_eq!(value.warning(), expected);
            if let Some(expected_count) = count {
                assert_eq!(
                    metrics(&value).effective_count().display().as_str(),
                    expected_count
                );
            } else {
                assert_eq!(value.status(), &ConcentrationStatus::NotApplicable);
            }
        }
    }
    #[test]
    fn incomplete_valuation_propagates_without_a_metric() {
        let value = Concentration::new(&allocation(&[Some("10"), None], "10"));
        assert_eq!(
            value.status(),
            &ConcentrationStatus::Unavailable(ConcentrationError::Allocation(
                AllocationError::Incomplete
            ))
        );
        assert_eq!(value.warning(), ConcentrationWarning::Unavailable);
    }
    #[test]
    fn all_equal_holdings_have_their_actual_count() {
        for count in [1, 2, 3, 5, 7, 10, 37, 4096] {
            let value = Concentration::new(&allocation(&vec![Some("1"); count], "0"));
            assert_eq!(
                metrics(&value).effective_count().display().as_str(),
                format!("{count}.000000")
            );
            if count == 10 {
                assert_eq!(metrics(&value).hhi().display().as_str(), "0.1000");
            }
        }
    }
    #[test]
    fn independent_integer_rational_reference_and_permutations() {
        TestRunner::new(Config::with_cases(4096))
            .run(
                &(
                    proptest::collection::vec(
                        proptest::prop_oneof![proptest::strategy::Just(0), 1_u32..10000],
                        1..30,
                    ),
                    0_u32..10000,
                ),
                |(mut values, cash)| {
                    let total = values
                        .iter()
                        .map(|value| u128::from(*value))
                        .sum::<u128>()
                        .checked_add(u128::from(cash))
                        .expect("분모");
                    let squares = values
                        .iter()
                        .map(|value| {
                            u128::from(*value)
                                .checked_mul(u128::from(*value))
                                .expect("정수 제곱")
                        })
                        .sum::<u128>();
                    let actual = generated(&values, cash);
                    if squares == 0 {
                        assert_eq!(actual.status(), &ConcentrationStatus::NotApplicable);
                        return Ok(());
                    }
                    let result = metrics(&actual);
                    let denominator = total.checked_mul(total).expect("분모 제곱");
                    assert_display(result.hhi().display().as_str(), squares, denominator, 4);
                    assert_rational(result.hhi().0, squares, denominator, 24);
                    assert_display(
                        result.effective_count().display().as_str(),
                        denominator,
                        squares,
                        6,
                    );
                    assert_rational(result.effective_count().0, denominator, squares, 18);
                    values.sort_by(|left, right| right.cmp(left));
                    for (size, weight) in [
                        (1, result.top_one()),
                        (3, result.top_three()),
                        (5, result.top_five()),
                    ] {
                        let sum = values
                            .iter()
                            .take(size)
                            .map(|value| u128::from(*value))
                            .sum();
                        assert_display(weight.display().as_str(), sum, total, 2);
                        assert_rational(weight.0, sum, total, 24);
                    }
                    assert_warning(&actual, squares, denominator);
                    assert_permutation(&actual, values, cash);
                    Ok(())
                },
            )
            .expect("4096 독립 유리수 기준");
    }

    #[test]
    #[should_panic(expected = "계산 가능 기대")]
    fn metrics_helper_rejects_unavailable_results() {
        let _value = metrics(&Concentration::new(&allocation(&[], "0")));
    }
    #[test]
    fn midpoint_reference_handles_the_known_rational_tie() {
        let value = Concentration::new(&allocation(
            &[
                Some("2865"),
                Some("2762"),
                Some("404"),
                Some("5443"),
                Some("5760"),
            ],
            "2462",
        ));
        assert_display(metrics(&value).top_five().display().as_str(), 7, 8, 2);
    }
    #[test]
    fn checked_arithmetic_rejects_square_sum_and_top_sum_overflow() {
        let amount = |text| {
            Money::parse(&NumericInput::try_new(text).expect("입력"), Currency::Krw)
                .expect("금액")
                .amount()
        };
        let maximum = amount("79228162514264337593543950335");
        let one = amount("1");
        let large = amount("200000000000000");
        for weights in [
            vec![maximum],
            vec![large, large],
            vec![maximum, one],
            vec![one, one, one, maximum],
        ] {
            assert_eq!(calculate(&weights), Err(MoneyError::Overflow));
        }
    }
    #[test]
    fn comparison_accessors_and_precision_failures_are_closed() {
        let one = Concentration::new(&allocation(&[Some("1")], "0"));
        let two = Concentration::new(&allocation(&[Some("1"), Some("1")], "0"));
        assert_eq!(
            metrics(&one).hhi().compare(metrics(&two).hhi()),
            Ordering::Greater
        );
        assert_eq!(
            metrics(&one)
                .effective_count()
                .compare(metrics(&two).effective_count()),
            Ordering::Less
        );
        assert_eq!(
            metrics(&one).top_one().compare(metrics(&two).top_one()),
            Ordering::Greater
        );
        assert_eq!(
            metrics(&one).hhi().compare(metrics(&one).hhi()),
            Ordering::Equal
        );
        let (holding, cash) = ("0.0000000000000000000000000001", "1");
        let tiny = Concentration::new(&allocation(&[Some(holding)], cash));
        assert_eq!(tiny.warning(), ConcentrationWarning::Unavailable);
        assert_eq!(
            tiny.status(),
            &ConcentrationStatus::Unavailable(ConcentrationError::Arithmetic(
                MoneyError::PrecisionLoss
            ))
        );
        assert_eq!(calculate(&[]), Err(MoneyError::PrecisionLoss));
        assert_eq!(
            crate::exact_arithmetic::concentration_square(Amount::new(rust_decimal::Decimal::MAX)),
            Err(MoneyError::Overflow)
        );
    }

    fn reference(numerator: u128, denominator: u128, scale: u32) -> String {
        let factor = 10_u128.checked_pow(scale).expect("자릿수");
        let scaled = numerator.checked_mul(factor).expect("분자");
        let quotient = scaled.checked_div(denominator).expect("몫");
        let remainder = scaled.checked_rem(denominator).expect("나머지");
        let rounded = quotient
            .checked_add(u128::from(
                remainder.checked_mul(2).expect("반올림") >= denominator,
            ))
            .expect("표시");
        format!(
            "{}.{:0width$}",
            rounded.checked_div(factor).expect("정수부"),
            rounded.checked_rem(factor).expect("소수부"),
            width = usize::try_from(scale).expect("폭")
        )
    }

    fn assert_permutation(actual: &Concentration, mut values: Vec<u32>, cash: u32) {
        values.reverse();
        let texts: Vec<_> = values.iter().map(ToString::to_string).collect();
        let inputs: Vec<_> = texts.iter().map(|value| Some(value.as_str())).collect();
        assert_eq!(
            actual,
            &Concentration::new(&allocation(&inputs, &cash.to_string()))
        );
    }

    fn assert_display(actual: &str, numerator: u128, denominator: u128, scale: u32) {
        let expected = reference(numerator, denominator, scale);
        if actual == expected {
            return;
        }
        let scaled = numerator
            .checked_mul(10_u128.checked_pow(scale).expect("표시 배율"))
            .expect("분자");
        assert_eq!(
            scaled
                .checked_rem(denominator)
                .expect("나머지")
                .checked_mul(2),
            Some(denominator)
        );
        let units = actual.replace('.', "").parse::<u128>().expect("표시 단위");
        assert_eq!(
            units,
            scaled.checked_div(denominator).expect("동률 아래 단위")
        );
    }
    fn assert_rational(value: Amount, numerator: u128, denominator: u128, scale: u32) {
        let decimal = value.as_decimal();
        let coefficient = decimal.mantissa().unsigned_abs();
        let units = if decimal.scale() >= scale {
            coefficient
                .checked_div(
                    10_u128
                        .checked_pow(decimal.scale().checked_sub(scale).expect("차이"))
                        .expect("배율"),
                )
                .expect("정수 절삭")
        } else {
            coefficient
                .checked_mul(
                    10_u128
                        .checked_pow(scale.checked_sub(decimal.scale()).expect("차이"))
                        .expect("배율"),
                )
                .expect("정수 정렬")
        };
        let actual = units.checked_mul(denominator).expect("실제 분자");
        let expected = numerator
            .checked_mul(10_u128.checked_pow(scale).expect("기준 배율"))
            .expect("기준 분자");
        assert!(
            actual.abs_diff(expected) <= denominator.checked_add(numerator).expect("오차 상한")
        );
    }

    fn generated(values: &[u32], cash: u32) -> Concentration {
        let texts: Vec<_> = values.iter().map(ToString::to_string).collect();
        let inputs: Vec<_> = texts.iter().map(|value| Some(value.as_str())).collect();
        Concentration::new(&allocation(&inputs, &cash.to_string()))
    }

    fn assert_warning(actual: &Concentration, squares: u128, denominator: u128) {
        let expected = if squares.checked_mul(5).expect("경계") > denominator {
            ConcentrationWarning::BelowFive
        } else {
            ConcentrationWarning::AtLeastFive
        };
        assert_eq!(actual.warning(), expected);
    }
}

#[derive(Clone, Copy)]
struct DisplayPlaces(u8);
fn display(value: Amount, places: DisplayPlaces) -> ConcentrationDisplay {
    let rounded = value.as_decimal().round_dp_with_strategy(
        u32::from(places.0),
        rust_decimal::RoundingStrategy::MidpointAwayFromZero,
    );
    ConcentrationDisplay(format!("{:.*}", usize::from(places.0), rounded))
}
