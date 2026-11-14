use crate::{ConcentrationThreshold, Money, RatioQuotient, ValuationAccuracy};
use rust_decimal::Decimal;
use std::{cmp::Ordering, fmt};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct CompositionInput {
    numerator: Statistic,
    denominator: Statistic,
    estimate: Statistic,
}
impl fmt::Debug for CompositionInput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CompositionInput")
    }
}
impl CompositionInput {
    pub(crate) fn from_evaluation(
        numerator: Money,
        denominator: Money,
        quotient: RatioQuotient,
    ) -> Self {
        let estimate = match quotient {
            RatioQuotient::Exact(value) => Statistic(value.as_decimal()),
            RatioQuotient::Approximate(value) => Statistic(value.as_decimal()),
        };
        Self {
            estimate,
            numerator: Statistic(numerator.amount().as_decimal()),
            denominator: Statistic(denominator.amount().as_decimal()),
        }
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
struct Statistic(Decimal);
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConcentrationArithmeticError {
    InvalidInput,
    Calculation,
    Range,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConcentrationDisplay(String);
impl ConcentrationDisplay {
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
#[derive(Clone, PartialEq, Eq)]
struct MetricValue {
    estimate: Statistic,
    display: ConcentrationDisplay,
    numerator: Wide,
    denominator: Wide,
}
impl fmt::Debug for MetricValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ApproximateConcentration")
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConcentrationHhi(MetricValue);
impl ConcentrationHhi {
    #[must_use]
    pub fn display(&self) -> ConcentrationDisplay {
        self.0.display.clone()
    }
    #[must_use]
    pub fn compare(&self, other: &Self) -> Ordering {
        self.0.estimate.0.cmp(&other.0.estimate.0)
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectiveInstrumentCount(MetricValue);
impl EffectiveInstrumentCount {
    #[must_use]
    pub fn display(&self) -> ConcentrationDisplay {
        self.0.display.clone()
    }
    #[must_use]
    pub fn compare(&self, other: &Self) -> Ordering {
        self.0.estimate.0.cmp(&other.0.estimate.0)
    }
    pub(crate) fn below(&self, threshold: &ConcentrationThreshold) -> Ordering {
        self.0.numerator.cmp(
            &self
                .0
                .denominator
                .multiply(&Wide::integer(Coefficient(u128::from(threshold.as_u16())))),
        )
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConcentrationWeight(MetricValue);
impl ConcentrationWeight {
    #[must_use]
    pub fn display(&self) -> ConcentrationDisplay {
        self.0.display.clone()
    }
    #[must_use]
    pub fn compare(&self, other: &Self) -> Ordering {
        self.0.estimate.0.cmp(&other.0.estimate.0)
    }
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
#[derive(Clone, Copy)]
struct PositionCount(u16);
pub(crate) fn calculate(
    inputs: &[CompositionInput],
) -> Result<ConcentrationMetrics, ConcentrationArithmeticError> {
    let first = validate(inputs)?;
    let mut rows = inputs.to_vec();
    rows.sort_by_key(|value| std::cmp::Reverse(value.numerator.0));
    let denominator = Wide::decimal(first.denominator);
    let numerators: Vec<_> = rows
        .iter()
        .map(|value| Wide::decimal(value.numerator))
        .collect();
    let total = numerators
        .iter()
        .fold(Wide::integer(Coefficient(0)), |sum, value| sum.add(value));
    if total != denominator {
        return Err(ConcentrationArithmeticError::InvalidInput);
    }
    let weights: Vec<_> = rows.iter().map(|value| value.estimate).collect();
    let squares = numerators
        .iter()
        .fold(Wide::integer(Coefficient(0)), |sum, value| {
            sum.add(&value.multiply(value))
        });
    let squared_total = denominator.multiply(&denominator);
    match estimates(&weights) {
        Some(approximate) => plan(
            approximate,
            &numerators,
            ReferenceTotals {
                denominator,
                squares,
                squared_total,
            },
            PositionCount(
                inputs
                    .iter()
                    .fold(0_u16, |count, _| count.saturating_add(1)),
            ),
        )
        .finish(),
        None => Err(ConcentrationArithmeticError::Calculation),
    }
}
#[derive(Clone, Copy)]
struct Estimates {
    hhi: Statistic,
    count: Statistic,
    one: Statistic,
    three: Statistic,
    five: Statistic,
}
fn estimates(weights: &[Statistic]) -> Option<Estimates> {
    let zero = Statistic(Decimal::ZERO);
    let one = weights.first().copied().unwrap_or(zero);
    let three = weights
        .iter()
        .take(3)
        .try_fold(Decimal::ZERO, |sum, value| sum.checked_add(value.0))?;
    let five = weights
        .iter()
        .take(5)
        .try_fold(Decimal::ZERO, |sum, value| sum.checked_add(value.0))?;
    let hhi = weights.iter().try_fold(Decimal::ZERO, |sum, value| {
        let square = value.0.checked_mul(value.0)?;
        sum.checked_add(square)
    })?;
    let count = Decimal::ONE.checked_div(hhi)?;
    Some(Estimates {
        hhi: Statistic(hhi),
        count: Statistic(count),
        one,
        three: Statistic(three),
        five: Statistic(five),
    })
}
struct Fraction {
    estimate: Statistic,
    numerator: Wide,
    denominator: Wide,
    precision: Precision,
}
impl Fraction {
    fn finish(self) -> Result<MetricValue, ConcentrationArithmeticError> {
        metric(
            self.estimate,
            self.numerator,
            self.denominator,
            self.precision,
        )
    }
}
struct Plan {
    hhi: Fraction,
    count: Fraction,
    one: Fraction,
    three: Fraction,
    five: Fraction,
    positions: PositionCount,
}
impl Plan {
    fn finish(self) -> Result<ConcentrationMetrics, ConcentrationArithmeticError> {
        if !(Decimal::ONE..=Decimal::from(self.positions.0)).contains(&self.count.estimate.0)
            || self.count.numerator.cmp(&self.count.denominator).is_lt()
            || self
                .count
                .numerator
                .cmp(
                    &self
                        .count
                        .denominator
                        .multiply(&Wide::integer(Coefficient(u128::from(self.positions.0)))),
                )
                .is_gt()
        {
            return Err(ConcentrationArithmeticError::Range);
        }
        Ok(ConcentrationMetrics {
            accuracy: ValuationAccuracy::Approximate,
            hhi: ConcentrationHhi(self.hhi.finish()?),
            effective_count: EffectiveInstrumentCount(self.count.finish()?),
            top_one: ConcentrationWeight(self.one.finish()?),
            top_three: ConcentrationWeight(self.three.finish()?),
            top_five: ConcentrationWeight(self.five.finish()?),
        })
    }
}
struct ReferenceTotals {
    denominator: Wide,
    squares: Wide,
    squared_total: Wide,
}
fn plan(
    approximate: Estimates,
    numerators: &[Wide],
    totals: ReferenceTotals,
    positions: PositionCount,
) -> Plan {
    let ReferenceTotals {
        denominator,
        squares,
        squared_total,
    } = totals;
    let top = |estimate: Statistic, count: PositionCount| Fraction {
        estimate,
        numerator: numerators
            .iter()
            .take(usize::from(count.0))
            .fold(Wide::integer(Coefficient(0)), |sum, value| sum.add(value)),
        denominator: denominator.clone(),
        precision: Precision::Weight,
    };
    Plan {
        hhi: Fraction {
            estimate: approximate.hhi,
            numerator: squares.clone(),
            denominator: squared_total.clone(),
            precision: Precision::Hhi,
        },
        count: Fraction {
            estimate: Statistic(
                approximate
                    .count
                    .0
                    .clamp(Decimal::ONE, Decimal::from(positions.0)),
            ),
            numerator: squared_total,
            denominator: squares,
            precision: Precision::Count,
        },
        one: top(approximate.one, PositionCount(1)),
        three: top(approximate.three, PositionCount(3)),
        five: top(approximate.five, PositionCount(5)),
        positions,
    }
}

#[derive(Clone, Copy)]
enum Precision {
    Hhi,
    Count,
    Weight,
}
impl Precision {
    const fn places(self) -> Places {
        match self {
            Self::Hhi => Places(4),
            Self::Count => Places(6),
            Self::Weight => Places(2),
        }
    }
    const fn factor(self) -> Units {
        match self {
            Self::Hhi => Units(10000),
            Self::Count => Units(1_000_000),
            Self::Weight => Units(100),
        }
    }
    const fn maximum(self) -> Units {
        match self {
            Self::Hhi | Self::Weight => Units(1),
            Self::Count => Units(4096),
        }
    }
}
fn metric(
    estimate: Statistic,
    numerator: Wide,
    denominator: Wide,
    precision: Precision,
) -> Result<MetricValue, ConcentrationArithmeticError> {
    let units = rounded_units(&numerator, &denominator, precision)?;
    let whole = units.0.div_euclid(precision.factor().0);
    let fraction = units.0.rem_euclid(precision.factor().0);
    let display = ConcentrationDisplay(format!(
        "{whole}.{fraction:0width$}",
        width = usize::from(precision.places().0)
    ));
    Ok(MetricValue {
        estimate,
        display,
        numerator,
        denominator,
    })
}
fn rounded_units(
    numerator: &Wide,
    denominator: &Wide,
    precision: Precision,
) -> Result<Units, ConcentrationArithmeticError> {
    if denominator.0.is_empty()
        || numerator
            .cmp(&denominator.multiply(&Wide::integer(Coefficient(u128::from(
                precision.maximum().0,
            )))))
            .is_gt()
    {
        return Err(ConcentrationArithmeticError::Range);
    }
    let scaled = numerator.multiply(&Wide::integer(Coefficient(u128::from(
        precision.factor().0,
    ))));
    let mut lower = 0_u64;
    let mut upper = precision
        .maximum()
        .0
        .wrapping_mul(precision.factor().0)
        .wrapping_add(1);
    while upper.abs_diff(lower) > 1 {
        let middle = lower.midpoint(upper);
        if denominator
            .multiply(&Wide::integer(Coefficient(u128::from(middle))))
            .cmp(&scaled)
            .is_gt()
        {
            upper = middle;
        } else {
            lower = middle;
        }
    }
    let midpoint = denominator.multiply(&Wide::integer(Coefficient(u128::from(
        lower.wrapping_mul(2).wrapping_add(1),
    ))));
    if scaled
        .multiply(&Wide::integer(Coefficient(2)))
        .cmp(&midpoint)
        .is_ge()
    {
        lower = lower.wrapping_add(1);
    }
    Ok(Units(lower))
}
#[derive(Clone, Copy, PartialEq, Eq)]
struct Limb(u32);
#[derive(Clone, PartialEq, Eq)]
struct Wide(Vec<Limb>);
impl Wide {
    fn integer(value: Coefficient) -> Self {
        let bytes = value.0.to_le_bytes();
        Self(
            bytes
                .as_chunks::<4>()
                .0
                .iter()
                .map(|part| Limb(u32::from_le_bytes(*part)))
                .collect(),
        )
        .normalized()
    }
    fn decimal(value: Statistic) -> Self {
        let mut result = Self::integer(Coefficient(value.0.mantissa().unsigned_abs()));
        for _ in value.0.scale()..28 {
            result = result.digit(Limb(10));
        }
        result
    }
    fn normalized(mut self) -> Self {
        while self.0.last() == Some(&Limb(0)) {
            let _removed = self.0.pop();
        }
        self
    }
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.len().cmp(&other.0.len()).then_with(|| {
            self.0
                .iter()
                .rev()
                .map(|value| value.0)
                .cmp(other.0.iter().rev().map(|value| value.0))
        })
    }
    fn add(&self, other: &Self) -> Self {
        let mut carry = Limb(0);
        let mut result = Vec::new();
        for (left, right) in self
            .0
            .iter()
            .copied()
            .chain(std::iter::repeat(Limb(0)))
            .zip(other.0.iter().copied().chain(std::iter::repeat(Limb(0))))
            .take(self.0.len().max(other.0.len()))
        {
            let (low, high) = split(Word(
                u64::from(left.0)
                    .wrapping_add(u64::from(right.0))
                    .wrapping_add(u64::from(carry.0)),
            ));
            result.push(low);
            carry = high;
        }
        result.push(carry);
        Self(result).normalized()
    }
    fn digit(&self, value: Limb) -> Self {
        let mut carry = Limb(0);
        let mut result = Vec::new();
        for item in &self.0 {
            let (low, high) = split(Word(
                u64::from(item.0)
                    .wrapping_mul(u64::from(value.0))
                    .wrapping_add(u64::from(carry.0)),
            ));
            result.push(low);
            carry = high;
        }
        result.push(carry);
        Self(result).normalized()
    }
    fn multiply(&self, other: &Self) -> Self {
        self.0
            .iter()
            .enumerate()
            .fold(Self::integer(Coefficient(0)), |sum, (offset, value)| {
                let row = Self(
                    std::iter::repeat_n(Limb(0), offset)
                        .chain(other.digit(*value).0)
                        .collect(),
                );
                sum.add(&row)
            })
            .normalized()
    }
}
#[derive(Clone, Copy)]
struct Word(u64);
fn split(value: Word) -> (Limb, Limb) {
    let [first, second, third, fourth, fifth, sixth, seventh, eighth] = value.0.to_le_bytes();
    (
        Limb(u32::from_le_bytes([first, second, third, fourth])),
        Limb(u32::from_le_bytes([fifth, sixth, seventh, eighth])),
    )
}

#[derive(Clone, Copy)]
struct Coefficient(u128);
#[derive(Clone, Copy)]
struct Places(u8);
#[derive(Clone, Copy)]
struct Units(u64);

fn validate(
    inputs: &[CompositionInput],
) -> Result<&CompositionInput, ConcentrationArithmeticError> {
    let first = inputs
        .first()
        .ok_or(ConcentrationArithmeticError::InvalidInput)?;
    if inputs.len() > 4096
        || first.denominator.0 <= Decimal::ZERO
        || inputs.iter().any(|value| {
            value.denominator != first.denominator || value.numerator.0 < Decimal::ZERO
        })
    {
        return Err(ConcentrationArithmeticError::InvalidInput);
    }
    Ok(first)
}

#[cfg(test)]
mod tests {
    use super::{
        Coefficient, CompositionInput, ConcentrationArithmeticError, Plan, PositionCount,
        Precision, ReferenceTotals, Statistic, Wide, calculate, estimates, plan, rounded_units,
    };
    use rust_decimal::Decimal;
    fn input(numerator: Decimal, denominator: Decimal, estimate: Decimal) -> CompositionInput {
        CompositionInput {
            numerator: Statistic(numerator),
            denominator: Statistic(denominator),
            estimate: Statistic(estimate),
        }
    }
    fn valid_plan() -> Plan {
        plan(
            estimates(&[Statistic(Decimal::ONE)]).expect("통계"),
            &[Wide::integer(Coefficient(1))],
            ReferenceTotals {
                denominator: Wide::integer(Coefficient(1)),
                squares: Wide::integer(Coefficient(1)),
                squared_total: Wide::integer(Coefficient(1)),
            },
            PositionCount(1),
        )
    }
    #[test]
    fn invalid_inputs_and_checked_statistic_failures() {
        let one = input(Decimal::ONE, Decimal::ONE, Decimal::ONE);
        assert_eq!(format!("{one:?}"), "CompositionInput");
        for rows in [
            vec![],
            vec![one; 4097],
            vec![
                input(
                    Decimal::ONE,
                    Decimal::from(4097_u16),
                    Decimal::ONE
                        .checked_div(Decimal::from(4097_u16))
                        .expect("근사 비중")
                );
                4097
            ],
            vec![input(Decimal::ONE, Decimal::ZERO, Decimal::ONE)],
            vec![input(Decimal::ZERO, Decimal::ZERO, Decimal::ZERO)],
            vec![
                input(Decimal::new(5, 1), Decimal::ONE, Decimal::new(5, 1)),
                input(Decimal::new(5, 1), Decimal::TWO, Decimal::new(25, 2)),
            ],
            vec![input(Decimal::NEGATIVE_ONE, Decimal::ONE, Decimal::ONE)],
            vec![input(Decimal::TWO, Decimal::ONE, Decimal::ONE)],
            vec![one, input(Decimal::ONE, Decimal::TWO, Decimal::ONE)],
            vec![input(Decimal::ZERO, Decimal::ONE, Decimal::ZERO)],
        ] {
            assert_eq!(
                calculate(&rows),
                Err(ConcentrationArithmeticError::InvalidInput)
            );
        }
        assert_eq!(
            calculate(&[input(Decimal::ONE, Decimal::ONE, Decimal::ZERO)]),
            Err(ConcentrationArithmeticError::Calculation)
        );
        for weights in [
            vec![],
            vec![Statistic(Decimal::MAX)],
            vec![Statistic(Decimal::MAX); 2],
            vec![Statistic(Decimal::ZERO); 3]
                .into_iter()
                .chain([Statistic(Decimal::MAX); 2])
                .collect(),
            vec![Statistic(Decimal::from(200_000_000_000_000_u64)); 2],
        ] {
            assert!(estimates(&weights).is_none());
        }
    }
    #[test]
    fn every_metric_rejects_invalid_ranges_without_formatting_decimal() {
        for index in 0_u8..5 {
            let mut value = valid_plan();
            let target = match index {
                0 => &mut value.hhi,
                1 => &mut value.count,
                2 => &mut value.one,
                3 => &mut value.three,
                _ => &mut value.five,
            };
            target.numerator = Wide::integer(Coefficient(0));
            target.denominator = Wide::integer(Coefficient(0));
            assert_eq!(value.finish(), Err(ConcentrationArithmeticError::Range));
        }
        for estimate in [Decimal::ZERO, Decimal::MAX] {
            let mut value = valid_plan();
            value.count.estimate = Statistic(estimate);
            assert_eq!(value.finish(), Err(ConcentrationArithmeticError::Range));
        }
        for (numerator, denominator) in [(0, 1), (2, 1)] {
            let mut value = valid_plan();
            value.count.numerator = Wide::integer(Coefficient(numerator));
            value.count.denominator = Wide::integer(Coefficient(denominator));
            assert_eq!(value.finish(), Err(ConcentrationArithmeticError::Range));
        }
        assert!(
            rounded_units(
                &Wide::integer(Coefficient(2)),
                &Wide::integer(Coefficient(1)),
                Precision::Hhi
            )
            .is_err()
        );
    }
    #[test]
    fn wide_words_match_independent_u128_arithmetic() {
        use proptest::test_runner::{Config, TestRunner};
        TestRunner::new(Config::with_cases(4096))
            .run(
                &(
                    proptest::arbitrary::any::<u64>(),
                    proptest::arbitrary::any::<u64>(),
                ),
                |(left, right)| {
                    let first = Wide::integer(Coefficient(u128::from(left)));
                    let second = Wide::integer(Coefficient(u128::from(right)));
                    assert_eq!(
                        first.add(&second).cmp(&Wide::integer(Coefficient(
                            u128::from(left)
                                .checked_add(u128::from(right))
                                .expect("독립 합")
                        ))),
                        std::cmp::Ordering::Equal
                    );
                    assert_eq!(
                        first.multiply(&second).cmp(&Wide::integer(Coefficient(
                            u128::from(left)
                                .checked_mul(u128::from(right))
                                .expect("독립 곱")
                        ))),
                        std::cmp::Ordering::Equal
                    );
                    Ok(())
                },
            )
            .expect("4096 정수 자리 검증");
        let maximum = Wide::integer(Coefficient(u128::MAX));
        let expanded = maximum.add(&Wide::integer(Coefficient(1)));
        assert_eq!(expanded.0.len(), 5);
    }
}
