use rust_decimal::Decimal;

use crate::{Amount, DisplayDirection, MoneyError, Ratio};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RatioQuotient {
    Exact(Ratio),
    Approximate(ApproximateRatio),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ApproximateRatio(Decimal);

impl ApproximateRatio {
    pub(crate) const fn new(value: Decimal) -> Self {
        Self(value)
    }
    #[must_use]
    pub(crate) fn as_decimal(&self) -> Decimal {
        self.0
    }
    #[must_use]
    pub fn cmp_ratio(&self, other: &Ratio) -> std::cmp::Ordering {
        self.as_decimal().cmp(&other.as_decimal())
    }

    #[must_use]
    pub fn original_direction(&self) -> DisplayDirection {
        if self.as_decimal().is_sign_negative() {
            DisplayDirection::Down
        } else {
            DisplayDirection::Up
        }
    }
}

pub(crate) fn add(left: Amount, right: Amount) -> Result<Amount, MoneyError> {
    left.checked_add(&right)
}

pub(crate) fn subtract(left: Amount, right: Amount) -> Result<Amount, MoneyError> {
    left.checked_sub(&right)
}

pub(crate) fn multiply(left: Amount, right: Amount) -> Result<Amount, MoneyError> {
    let product = Magnitude::product(left, right);
    let scale = product_scale(left, right);
    let maximum = Magnitude::coefficient(Amount::new(Decimal::MAX)).scaled(scale);
    if maximum.is_some_and(|limit| product.cmp(&limit).is_gt()) {
        return Err(MoneyError::Overflow);
    }
    let candidate = left
        .as_decimal()
        .checked_mul(right.as_decimal())
        .map(Amount::new);
    let expected = Some(product)
        .zip(candidate)
        .and_then(|(value, result)| value.scaled(Scale(result.as_decimal().scale())));
    let actual = candidate
        .zip(Some(scale))
        .and_then(|(value, places)| Magnitude::coefficient(value).scaled(places));
    expected
        .zip(actual)
        .filter(|(first, second)| first == second)
        .and(candidate)
        .ok_or(MoneyError::PrecisionLoss)
}

pub(crate) fn divide(left: Amount, right: Amount) -> Result<RatioQuotient, MoneyError> {
    if right.as_decimal().is_zero() {
        return Err(MoneyError::DivisionByZero);
    }
    let numerator = Magnitude::coefficient(left).scaled(Scale(right.as_decimal().scale()));
    let capacity = Magnitude::product(Amount::new(Decimal::MAX), right)
        .scaled(Scale(left.as_decimal().scale()));
    if numerator
        .zip(capacity)
        .is_some_and(|(value, limit)| value.cmp(&limit).is_gt())
    {
        return Err(MoneyError::Overflow);
    }
    left.as_decimal()
        .checked_div(right.as_decimal())
        .map(Amount::new)
        .and_then(|candidate| classify_quotient(left, right, candidate))
        .ok_or(MoneyError::Overflow)
}

fn classify_quotient(left: Amount, right: Amount, candidate: Amount) -> Option<RatioQuotient> {
    let expected = Magnitude::coefficient(left).scaled(product_scale(candidate, right));
    let actual = Magnitude::product(candidate, right).scaled(Scale(left.as_decimal().scale()));
    expected.zip(actual).map(|(first, second)| {
        if first == second {
            RatioQuotient::Exact(Ratio::new(candidate.as_decimal()))
        } else {
            let mut value = maximum_scale(candidate).as_decimal();
            value.set_sign_negative(
                left.as_decimal().is_sign_negative() != right.as_decimal().is_sign_negative(),
            );
            RatioQuotient::Approximate(ApproximateRatio(value))
        }
    })
}

fn maximum_scale(value: Amount) -> Amount {
    let mut result = value.as_decimal();
    for next_scale in 1..=28 {
        if next_scale <= result.scale() {
            continue;
        }
        let Some(next) = result.mantissa().checked_mul(10).and_then(|coefficient| {
            Decimal::try_from_i128_with_scale(coefficient, next_scale).ok()
        }) else {
            break;
        };
        result = next;
    }
    Amount::new(result)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Scale(u32);

fn product_scale(left: Amount, right: Amount) -> Scale {
    Scale(
        left.as_decimal()
            .scale()
            .saturating_add(right.as_decimal().scale()),
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Limb(u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Magnitude([Limb; 12]);

impl Magnitude {
    fn coefficient(value: Amount) -> Self {
        let parts = value.as_decimal().unpack();
        let mut result = Self([Limb(0); 12]);
        for (target, digit) in result.0.iter_mut().zip([parts.lo, parts.mid, parts.hi]) {
            target.0 = digit;
        }
        result
    }

    fn product(left: Amount, right: Amount) -> Self {
        let first = left.as_decimal().unpack();
        let mut result = Self([Limb(0); 12]);
        for (index, left_digit) in [first.lo, first.mid, first.hi].into_iter().enumerate() {
            result.accumulate(Limb(left_digit), right, Offset(index));
        }
        result
    }

    fn accumulate(&mut self, left: Limb, right: Amount, offset: Offset) {
        let parts = right.as_decimal().unpack();
        let mut carry = Limb(0);
        for (target, right_digit) in self
            .0
            .iter_mut()
            .skip(offset.0)
            .zip([parts.lo, parts.mid, parts.hi, 0])
        {
            let total = u64::from(left.0)
                .wrapping_mul(u64::from(right_digit))
                .wrapping_add(u64::from(target.0))
                .wrapping_add(u64::from(carry.0));
            let (low, high) = split_word(Word(total));
            *target = low;
            carry = high;
        }
    }

    fn scaled(mut self, scale: Scale) -> Option<Self> {
        for _place in 0..scale.0 {
            let mut carry = Limb(0);
            for digit in &mut self.0 {
                let total = u64::from(digit.0)
                    .wrapping_mul(10)
                    .wrapping_add(u64::from(carry.0));
                let (low, high) = split_word(Word(total));
                *digit = low;
                carry = high;
            }
            if carry.0 != 0 {
                return None;
            }
        }
        Some(self)
    }

    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0
            .iter()
            .rev()
            .map(|limb| limb.0)
            .cmp(other.0.iter().rev().map(|limb| limb.0))
    }
}

#[derive(Debug, Clone, Copy)]
struct Offset(usize);

#[derive(Debug, Clone, Copy)]
struct Word(u64);

fn split_word(value: Word) -> (Limb, Limb) {
    let [first, second, third, fourth, fifth, sixth, seventh, eighth] = value.0.to_le_bytes();
    (
        Limb(u32::from_le_bytes([first, second, third, fourth])),
        Limb(u32::from_le_bytes([fifth, sixth, seventh, eighth])),
    )
}

#[derive(Clone, Copy)]
struct ProposalMagnitude([Limb; 32]);
impl ProposalMagnitude {
    fn coefficient(value: Amount) -> Self {
        let mut result = Self([Limb(0); 32]);
        for (target, digit) in result.0.iter_mut().zip(Magnitude::coefficient(value).0) {
            *target = digit;
        }
        result
    }
    fn scaled(mut self, scale: Scale) -> Self {
        for _place in 0..scale.0 {
            let mut carry = Limb(0);
            for digit in &mut self.0 {
                let total = u64::from(digit.0)
                    .wrapping_mul(10)
                    .wrapping_add(u64::from(carry.0));
                let (low, high) = split_word(Word(total));
                *digit = low;
                carry = high;
            }
        }
        self
    }
    fn product(self, right: Self) -> Self {
        let mut result = Self([Limb(0); 32]);
        for (index, left) in self.0.iter().enumerate() {
            let mut carry = Limb(0);
            for (target, digit) in result.0.iter_mut().skip(index).zip(right.0) {
                let total = u64::from(left.0)
                    .wrapping_mul(u64::from(digit.0))
                    .wrapping_add(u64::from(target.0))
                    .wrapping_add(u64::from(carry.0));
                let (low, high) = split_word(Word(total));
                *target = low;
                carry = high;
            }
        }
        result
    }
    fn cmp(&self, right: &Self) -> std::cmp::Ordering {
        self.0
            .iter()
            .rev()
            .map(|digit| digit.0)
            .cmp(right.0.iter().rev().map(|digit| digit.0))
    }
    fn distance(self, right: Self) -> Self {
        let (mut high, low) = if self.cmp(&right).is_lt() {
            (right, self)
        } else {
            (self, right)
        };
        let mut borrow = false;
        for (target, digit) in high.0.iter_mut().zip(low.0) {
            let (difference, first) = target.0.overflowing_sub(digit.0);
            let (value, second) = difference.overflowing_sub(u32::from(borrow));
            *target = Limb(value);
            borrow = first || second;
        }
        high
    }
}
pub(crate) fn proposal_price_range(
    reference: crate::Price,
    current: crate::Price,
    sigma: Ratio,
    horizon: Ratio,
) -> crate::CheckResult {
    let base = reference.as_money();
    let quote = current.as_money();
    if base.currency() != quote.currency() {
        return crate::CheckResult::Failed;
    }
    let base_amount = base.amount();
    let quote_amount = quote.amount();
    let common_scale = base_amount
        .as_decimal()
        .scale()
        .max(quote_amount.as_decimal().scale());
    let aligned_base = ProposalMagnitude::coefficient(base_amount).scaled(Scale(
        common_scale.saturating_sub(base_amount.as_decimal().scale()),
    ));
    let aligned_quote = ProposalMagnitude::coefficient(quote_amount).scaled(Scale(
        common_scale.saturating_sub(quote_amount.as_decimal().scale()),
    ));
    let difference = aligned_quote.distance(aligned_base);
    let left = difference
        .product(difference)
        .product(ProposalMagnitude::coefficient(Amount::new(Decimal::from(
            4_i32,
        ))))
        .scaled(Scale(
            sigma
                .as_decimal()
                .scale()
                .saturating_mul(2)
                .saturating_add(horizon.as_decimal().scale()),
        ));
    let volatility = ProposalMagnitude::coefficient(Amount::new(sigma.as_decimal()));
    let reference_coefficient = ProposalMagnitude::coefficient(base_amount);
    let right = volatility
        .product(volatility)
        .product(ProposalMagnitude::coefficient(Amount::new(
            horizon.as_decimal(),
        )))
        .product(reference_coefficient)
        .product(reference_coefficient)
        .scaled(Scale(
            common_scale
                .saturating_sub(base_amount.as_decimal().scale())
                .saturating_mul(2),
        ));
    if left.cmp(&right).is_le() {
        crate::CheckResult::Passed
    } else {
        crate::CheckResult::Failed
    }
}

pub(crate) fn compare_reported(computed: Amount, reported: Amount) -> crate::Comparison {
    let left = computed.as_decimal();
    let right = reported.as_decimal();
    let scale = left.scale().max(right.scale());
    let left_coefficient = left
        .mantissa()
        .checked_mul(10_i128.saturating_pow(scale.abs_diff(left.scale())));
    let right_coefficient = right
        .mantissa()
        .checked_mul(10_i128.saturating_pow(scale.abs_diff(right.scale())));
    let distance = left_coefficient
        .zip(right_coefficient)
        .and_then(|(first, second)| first.checked_sub(second));
    let unit = 10_u128.saturating_pow(scale.abs_diff(right.scale()));
    if distance.is_some_and(|difference| difference.unsigned_abs() <= unit) {
        crate::Comparison::Matches
    } else {
        crate::Comparison::Different
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct AllocationUnits(u32);
impl AllocationUnits {
    pub(crate) const fn as_units(&self) -> u32 {
        self.0
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AllocationRemainder(Magnitude);
impl Ord for AllocationRemainder {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.cmp(&other.0)
    }
}
impl PartialOrd for AllocationRemainder {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
pub(crate) fn allocation_parts(
    value: Amount,
    total: Amount,
) -> (AllocationUnits, AllocationRemainder) {
    let mut numerator = allocation_scaled(value, &AllocationUnits(10000));
    let divisor = allocation_scaled(total, &AllocationUnits(1));
    let mut lower = 0_u32;
    let mut upper = 10001_u32;
    while upper.saturating_sub(lower) != 1 {
        let middle = lower.midpoint(upper);
        let candidate = allocation_multiple(divisor, &AllocationUnits(middle));
        if candidate.cmp(&numerator).is_gt() {
            upper = middle;
        } else {
            lower = middle;
        }
    }
    let denominator = allocation_multiple(divisor, &AllocationUnits(lower));
    let mut borrow = false;
    for (left, right) in numerator.0.iter_mut().zip(denominator.0) {
        let (difference, first) = left.0.overflowing_sub(right.0);
        let (reduced, second) = difference.overflowing_sub(u32::from(borrow));
        left.0 = reduced;
        borrow = first || second;
    }
    let units = AllocationUnits(lower);
    (units, AllocationRemainder(numerator))
}

fn allocation_scaled(value: Amount, units: &AllocationUnits) -> Magnitude {
    let mut result = Magnitude::product(value, Amount::new(Decimal::from(units.0)));
    for _place in value.as_decimal().scale()..28 {
        let mut carry = Limb(0);
        for digit in &mut result.0 {
            let total = u64::from(digit.0)
                .wrapping_mul(10)
                .wrapping_add(u64::from(carry.0));
            let (low, high) = split_word(Word(total));
            *digit = low;
            carry = high;
        }
    }
    result
}

fn allocation_multiple(mut value: Magnitude, units: &AllocationUnits) -> Magnitude {
    let mut carry = Limb(0);
    for digit in &mut value.0 {
        let total = u64::from(digit.0)
            .wrapping_mul(u64::from(units.0))
            .wrapping_add(u64::from(carry.0));
        let (low, high) = split_word(Word(total));
        *digit = low;
        carry = high;
    }
    value
}

#[cfg(test)]
mod allocation_remainder_tests {
    use super::{AllocationRemainder, allocation_parts};
    use crate::Amount;
    use proptest::test_runner::{Config, TestRunner};
    use rust_decimal::Decimal;

    fn assert_integer_remainder(value: u32, total: u32) {
        let numerator = u128::from(value).checked_mul(10000).expect("독립 분자");
        let (units, AllocationRemainder(actual)) = allocation_parts(
            Amount::new(Decimal::from(value)),
            Amount::new(Decimal::from(total)),
        );
        assert_eq!(
            u128::from(units.as_units()),
            numerator.checked_div(u128::from(total)).expect("독립 몫")
        );
        let mut coefficient = 0_u128;
        for limb in actual.0.iter().rev() {
            coefficient = coefficient
                .checked_mul(1_u128 << 32_u32)
                .and_then(|shifted| shifted.checked_add(u128::from(limb.0)))
                .expect("기준 나머지는 128비트 안");
        }
        let expected = numerator
            .checked_rem(u128::from(total))
            .expect("독립 나머지")
            .checked_mul(10_u128.checked_pow(28).expect("공통 스케일"))
            .expect("스케일 정렬");
        assert_eq!(coefficient, expected);
    }
    #[test]
    fn allocation_remainders_match_independent_integer_coefficients() {
        assert_integer_remainder(47, 29016);
        TestRunner::new(Config::with_cases(4096))
            .run(&(0_u32..1_000_000, 1_u32..1_000_000), |(seed, total)| {
                assert_integer_remainder(seed.checked_rem(total).expect("양수 분모"), total);
                Ok(())
            })
            .expect("4096 독립 정수 나머지");
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        Amount, AmountInput, Currency, Money, MoneyError, NumericInput, Price, Quantity, Ratio,
        RatioQuotient,
    };
    use proptest::arbitrary::any;
    use proptest::strategy::Strategy;
    use proptest::test_runner::{Config, TestRunner};
    use rust_decimal::Decimal;

    fn money(text: &str) -> Money {
        Money::new(
            Amount::parse(&AmountInput::new(text)).expect("정확 입력"),
            Currency::Usd,
        )
    }

    fn ratio(text: &str) -> Ratio {
        Ratio::new(money(text).amount().as_decimal())
    }

    #[test]
    fn multiplication_is_exact_or_reports_range_and_precision_loss() {
        for (left, right, expected) in [
            ("1.25", "0.8", Ok(money("1"))),
            ("-1.25", "0.8", Ok(money("-1"))),
            ("79228162514264337593543950335", "0", Ok(money("0"))),
            (
                "79228162514264337593543950335",
                "2",
                Err(MoneyError::Overflow),
            ),
            (
                "0.0000000000000000000000000001",
                "0.1",
                Err(MoneyError::PrecisionLoss),
            ),
            (
                "7922816251426433759354395033.5",
                "1.01",
                Err(MoneyError::PrecisionLoss),
            ),
            (
                "10000000000000000000000000000",
                "0.0000000000000000000000000001",
                Ok(money("1")),
            ),
        ] {
            assert_eq!(money(left).checked_mul(&ratio(right)), expected);
        }
        for currency in [Currency::Krw, Currency::Usd] {
            let price = Price::parse(&NumericInput::try_new("12.5").expect("길이"), currency)
                .expect("단가");
            let quantity =
                Quantity::parse(&NumericInput::try_new("0.4").expect("길이")).expect("수량");
            assert_eq!(
                price.checked_mul(&quantity),
                Ok(Money::new(Amount::new(Decimal::from(5_i32)), currency))
            );
            let negative = Money::new(Amount::new(Decimal::NEGATIVE_ONE), currency);
            assert_eq!(
                negative.checked_mul(&ratio("2")),
                Ok(Money::new(Amount::new(Decimal::from(-2_i32)), currency))
            );
        }
    }

    #[derive(Debug, Clone, Copy)]
    enum Precision {
        Exact,
        Approximate,
    }

    fn assert_quotient(left: &str, right: &str, text: &str, precision: Precision) {
        let expected_value = ratio(text).as_decimal();
        let expected = match precision {
            Precision::Exact => RatioQuotient::Exact(Ratio::new(expected_value)),
            Precision::Approximate => {
                RatioQuotient::Approximate(super::ApproximateRatio(expected_value))
            }
        };
        let result = money(left).checked_ratio(&money(right)).expect("몫");
        assert_eq!(result, expected);
        if let RatioQuotient::Approximate(value) = result {
            assert_eq!(value.as_decimal().scale(), expected_value.scale());
        }
    }

    #[test]
    fn quotient_examples_fix_precision_rounding_and_exactness() {
        for (left, right, text) in [
            ("1", "3", "0.3333333333333333333333333333"),
            ("2", "3", "0.6666666666666666666666666667"),
            ("1", "7", "0.1428571428571428571428571429"),
            ("-2", "3", "-0.6666666666666666666666666667"),
            (
                "0.0000000000000000000000000001",
                "2",
                "0.0000000000000000000000000000",
            ),
            (
                "0.0000000000000000000000000003",
                "2",
                "0.0000000000000000000000000002",
            ),
            (
                "-0.0000000000000000000000000003",
                "2",
                "-0.0000000000000000000000000002",
            ),
            (
                "79228162514264337593543950335",
                "2",
                "39614081257132168796771975168",
            ),
        ] {
            assert_quotient(left, right, text, Precision::Approximate);
        }
        for (left, right, text) in [
            ("1", "2", "0.5"),
            ("0", "3", "0"),
            (
                "79228162514264337593543950335",
                "3",
                "26409387504754779197847983445",
            ),
            (
                "1",
                "0.0000000000000000000000000001",
                "10000000000000000000000000000",
            ),
        ] {
            assert_quotient(left, right, text, Precision::Exact);
        }
    }

    #[test]
    fn quotient_rejects_zero_overflow_and_currency_mismatch() {
        for zero in ["0", "-0.00"] {
            assert_eq!(
                money("1").checked_ratio(&money(zero)),
                Err(MoneyError::DivisionByZero)
            );
            assert_eq!(
                money("0").checked_ratio(&money(zero)),
                Err(MoneyError::DivisionByZero)
            );
        }
        for (left, right) in [
            ("10", "0.0000000000000000000000000001"),
            (
                "79228162514264337593543950335",
                "0.9999999999999999999999999999",
            ),
        ] {
            assert_eq!(
                money(left).checked_ratio(&money(right)),
                Err(MoneyError::Overflow)
            );
        }
        let krw = Money::new(money("1").amount(), Currency::Krw);
        assert_eq!(
            krw.checked_ratio(&money("1")),
            Err(MoneyError::CurrencyMismatch)
        );
        assert_eq!(
            money("1").checked_ratio(&krw),
            Err(MoneyError::CurrencyMismatch)
        );
        assert_eq!(
            krw.checked_ratio(&money("0")),
            Err(MoneyError::CurrencyMismatch)
        );
    }

    #[test]
    fn ratio_addition_and_subtraction_reuse_exact_amount_rules() {
        assert_eq!(ratio("0.1").checked_add(&ratio("0.2")), Ok(ratio("0.3")));
        assert_eq!(ratio("0.1").checked_sub(&ratio("0.2")), Ok(ratio("-0.1")));
        assert_eq!(
            ratio("79228162514264337593543950335").checked_add(&ratio("1")),
            Err(MoneyError::Overflow)
        );
        assert_eq!(
            ratio("-79228162514264337593543950335").checked_sub(&ratio("1")),
            Err(MoneyError::Overflow)
        );
        assert_eq!(
            ratio("10").checked_add(&ratio("0.0000000000000000000000000001")),
            Err(MoneyError::PrecisionLoss)
        );
        assert_eq!(
            ratio("10").checked_sub(&ratio("0.0000000000000000000000000001")),
            Err(MoneyError::PrecisionLoss)
        );
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    struct Integer(Vec<u32>);

    #[derive(Clone, Copy, PartialEq, Eq)]
    enum Sign {
        Positive,
        Negative,
    }

    impl Integer {
        fn new(value: u128) -> Self {
            Self(
                value
                    .to_string()
                    .bytes()
                    .rev()
                    .map(|byte| u32::from(byte.checked_sub(b'0').expect("숫자")))
                    .collect(),
            )
        }

        fn trim(mut self) -> Self {
            if self.0.is_empty() {
                self.0.push(0);
            }
            while self.0.len() > 1 && self.0.last() == Some(&0) {
                let _removed = self.0.pop();
            }
            self
        }

        fn cmp(&self, other: &Self) -> std::cmp::Ordering {
            self.0
                .len()
                .cmp(&other.0.len())
                .then_with(|| self.0.iter().rev().cmp(other.0.iter().rev()))
        }

        fn scaled(&self, scale: u32) -> Self {
            let mut result = vec![0; usize::try_from(scale).expect("scale")];
            result.extend(&self.0);
            Self(result).trim()
        }

        fn multiply(&self, other: &Self) -> Self {
            let mut result = vec![
                0_u32;
                self.0
                    .len()
                    .checked_add(other.0.len())
                    .expect("길이")
                    .checked_add(1)
                    .expect("올림")
            ];
            for (left_index, left) in self.0.iter().enumerate() {
                Self::accumulate_digits(&mut result, left_index, *left, other);
            }
            let mut carry = 0_u32;
            for digit in &mut result {
                let total = digit.checked_add(carry).expect("올림");
                *digit = total.checked_rem(10).expect("기수");
                carry = total.checked_div(10).expect("기수");
            }
            assert_eq!(carry, 0);
            Self(result).trim()
        }

        fn accumulate_digits(result: &mut [u32], left_index: usize, left: u32, other: &Self) {
            for (right_index, right) in other.0.iter().enumerate() {
                let position = left_index.checked_add(right_index).expect("위치");
                result[position] = result[position]
                    .checked_add(left.checked_mul(*right).expect("한 자리 곱"))
                    .expect("합성");
            }
        }

        fn difference(&self, other: &Self) -> Self {
            let (larger, smaller) = if self.cmp(other).is_lt() {
                (other, self)
            } else {
                (self, other)
            };
            let mut borrow = 0_u32;
            let digits = larger
                .0
                .iter()
                .enumerate()
                .map(|(index, digit)| {
                    let rhs = smaller
                        .0
                        .get(index)
                        .copied()
                        .unwrap_or(0)
                        .checked_add(borrow)
                        .expect("빌림");
                    let total = digit
                        .checked_add(10)
                        .expect("기수")
                        .checked_sub(rhs)
                        .expect("차");
                    borrow = u32::from(*digit < rhs);
                    total.checked_rem(10).expect("기수")
                })
                .collect();
            assert_eq!(borrow, 0);
            Self(digits).trim()
        }

        fn decimal(self, negative: Sign, mut scale: u32) -> Result<Decimal, MoneyError> {
            let maximum = Self::new(Decimal::MAX.mantissa().unsigned_abs());
            if self.cmp(&maximum.scaled(scale)).is_gt() {
                return Err(MoneyError::Overflow);
            }
            let mut reduced = self;
            while scale > 0 && reduced.0.first() == Some(&0) {
                reduced = Self(reduced.0.iter().skip(1).copied().collect()).trim();
                scale = scale.checked_sub(1).expect("scale");
            }
            if scale > 28 || reduced.cmp(&maximum).is_gt() {
                return Err(MoneyError::PrecisionLoss);
            }
            let coefficient = reduced.0.iter().rev().fold(0_i128, |value, digit| {
                value
                    .checked_mul(10)
                    .expect("96bit")
                    .checked_add(i128::from(*digit))
                    .expect("96bit")
            });
            let signed = if negative == Sign::Negative {
                coefficient.checked_neg().expect("부호")
            } else {
                coefficient
            };
            Ok(Decimal::from_i128_with_scale(signed, scale))
        }
    }

    fn decimal_strategy() -> impl Strategy<Value = Decimal> {
        proptest::prop_oneof![
            proptest::strategy::Just(Decimal::ZERO),
            proptest::strategy::Just(Decimal::MAX),
            proptest::strategy::Just(Decimal::MIN),
            proptest::strategy::Just(Decimal::new(1, 28)),
            product_samples(ProductTemplate::Small).prop_map(|sample| sample.left),
            product_samples(ProductTemplate::Trailing).prop_map(|sample| sample.left),
            terminating_samples().prop_map(|sample| sample.right),
            (
                any::<u32>(),
                any::<u32>(),
                any::<u32>(),
                any::<bool>(),
                0_u32..=28
            )
                .prop_map(|(low, middle, high, negative, scale)| Decimal::from_parts(
                    low, middle, high, negative, scale
                )),
        ]
    }

    fn product_reference(left: Decimal, right: Decimal) -> Result<Decimal, MoneyError> {
        Integer::new(left.mantissa().unsigned_abs())
            .multiply(&Integer::new(right.mantissa().unsigned_abs()))
            .decimal(
                if left.is_sign_negative() == right.is_sign_negative() {
                    Sign::Positive
                } else {
                    Sign::Negative
                },
                left.scale().checked_add(right.scale()).expect("scale"),
            )
    }

    #[test]
    fn independent_decimal_integer_product_matches_all_public_multiplications() {
        TestRunner::new(Config::with_cases(4096))
            .run(
                &(decimal_strategy(), decimal_strategy(), any::<bool>()),
                |(left, right, usd)| {
                    let currency = if usd { Currency::Usd } else { Currency::Krw };
                    let amount = Money::new(Amount::new(left), currency);
                    let actual = amount.checked_mul(&Ratio::new(right));
                    proptest::prop_assert_eq!(
                        actual,
                        product_reference(left, right)
                            .map(|value| Money::new(Amount::new(value), currency))
                    );
                    if !left.is_zero() {
                        let price = Price::try_new(Money::new(Amount::new(left.abs()), currency))
                            .expect("양수");
                        let quantity = Quantity::try_new(right.abs()).expect("음수 아님");
                        proptest::prop_assert_eq!(
                            price.checked_mul(&quantity),
                            product_reference(left.abs(), right.abs())
                                .map(|value| Money::new(Amount::new(value), currency))
                        );
                    }
                    Ok(())
                },
            )
            .expect("독립 십진 정수 곱");
    }

    fn assert_quotient_against_integer_bound(left: Decimal, right: Decimal) {
        let result = Money::new(Amount::new(left), Currency::Usd)
            .checked_ratio(&Money::new(Amount::new(right), Currency::Usd));
        if right.is_zero() {
            assert_eq!(result, Err(MoneyError::DivisionByZero));
            return;
        }
        let numerator = Integer::new(left.mantissa().unsigned_abs());
        let denominator = Integer::new(right.mantissa().unsigned_abs());
        let maximum = Integer::new(Decimal::MAX.mantissa().unsigned_abs());
        if numerator
            .scaled(right.scale())
            .cmp(&maximum.multiply(&denominator).scaled(left.scale()))
            .is_gt()
        {
            assert_eq!(result, Err(MoneyError::Overflow));
            return;
        }
        let (value, exact) = match result.expect("범위 안") {
            RatioQuotient::Exact(value) => (value.as_decimal(), true),
            RatioQuotient::Approximate(value) => (value.as_decimal(), false),
        };
        let coefficient = Integer::new(value.mantissa().unsigned_abs());
        let residual = numerator
            .scaled(right.scale().checked_add(value.scale()).expect("scale"))
            .difference(&coefficient.multiply(&denominator).scaled(left.scale()));
        assert_eq!(exact, residual == Integer::new(0));
        if !value.is_zero() {
            assert_eq!(
                value.is_sign_negative(),
                left.is_sign_negative() != right.is_sign_negative()
            );
        }
        if !exact {
            let twice = residual.multiply(&Integer::new(2));
            let unit = denominator.scaled(left.scale());
            assert!(!twice.cmp(&unit).is_gt(), "반 단위 오차 한계");
            if twice == unit {
                assert_eq!(value.mantissa().checked_rem(2).expect("동률"), 0);
            }
            assert!(
                value.scale() == 28
                    || coefficient
                        .multiply(&Integer::new(10))
                        .cmp(&maximum)
                        .is_gt(),
                "최대 스케일"
            );
        }
    }

    #[test]
    fn independent_integer_residual_bounds_maximum_precision_quotients() {
        for (left, right) in [
            ("0.0000000000000000000000000001", "2"),
            ("0.0000000000000000000000000003", "2"),
            ("79228162514264337593543950335", "2"),
        ] {
            assert_quotient_against_integer_bound(
                ratio(left).as_decimal(),
                ratio(right).as_decimal(),
            );
        }

        TestRunner::new(Config::with_cases(4096))
            .run(
                &(decimal_strategy(), decimal_strategy()),
                |(left, right)| {
                    assert_quotient_against_integer_bound(left, right);
                    Ok(())
                },
            )
            .expect("독립 정수 잔차와 최대 스케일");
    }

    #[test]
    fn internal_integer_capacity_exceeds_every_decimal_cross_product() {
        let maximum = super::Magnitude::coefficient(Amount::new(Decimal::MAX));
        assert!(maximum.scaled(super::Scale(56)).is_some());
        assert_eq!(maximum.scaled(super::Scale(128)), None);
    }
    fn quotient_value(quotient: RatioQuotient) -> Decimal {
        match quotient {
            RatioQuotient::Exact(value) => value.as_decimal(),
            RatioQuotient::Approximate(value) => value.as_decimal(),
        }
    }

    #[test]
    fn underflow_preserves_the_original_sign() {
        for (left, right, negative) in [
            ("-0.0000000000000000000000000001", "3", true),
            ("0.0000000000000000000000000001", "3", false),
            ("0.0000000000000000000000000001", "-3", true),
            ("-0.0000000000000000000000000001", "-3", false),
            ("0", "3", false),
        ] {
            let value = quotient_value(money(left).checked_ratio(&money(right)).expect("몫"));
            assert!(value.is_zero());
            assert_eq!(value.is_sign_negative(), negative);
        }
    }

    #[test]
    fn reviewed_maximum_boundaries_remain_valid() {
        let maximum = money("79228162514264337593543950335");
        assert_eq!(maximum.checked_mul(&Ratio::new(Decimal::ONE)), Ok(maximum));
        assert_eq!(
            maximum.checked_ratio(&money("1")),
            Ok(RatioQuotient::Exact(Ratio::new(Decimal::MAX)))
        );
        assert_quotient(
            "79228162514264337593543950327",
            "9999999999999999999999999999",
            "7.9228162514264337593543950335",
            Precision::Approximate,
        );
    }
    fn approximate(left: &str, right: &str) -> Option<super::ApproximateRatio> {
        match money(left).checked_ratio(&money(right)).expect("몫") {
            RatioQuotient::Exact(_) => None,
            RatioQuotient::Approximate(value) => Some(value),
        }
    }

    #[test]
    fn approximate_ratios_support_ordering_without_erasing_precision() {
        let lower = approximate("3", "7").expect("근사");
        let upper = approximate("4", "7").expect("근사");
        assert!(lower < upper);
        assert_eq!(lower.cmp(&upper), std::cmp::Ordering::Less);
        assert_eq!(lower.cmp_ratio(&ratio("0.5")), std::cmp::Ordering::Less);
        assert_eq!(upper.cmp_ratio(&ratio("0.5")), std::cmp::Ordering::Greater);
        assert_eq!(
            lower.cmp_ratio(&Ratio::new(lower.as_decimal())),
            std::cmp::Ordering::Equal
        );
        assert!(approximate("1", "2").is_none());
    }

    #[test]
    fn approximate_direction_uses_the_unrounded_quotient_sign() {
        for (left, right, expected) in [
            (
                "-0.0000000000000000000000000001",
                "3",
                crate::DisplayDirection::Down,
            ),
            (
                "0.0000000000000000000000000001",
                "3",
                crate::DisplayDirection::Up,
            ),
            (
                "0.0000000000000000000000000001",
                "-3",
                crate::DisplayDirection::Down,
            ),
            (
                "-0.0000000000000000000000000001",
                "-3",
                crate::DisplayDirection::Up,
            ),
            ("-3", "7", crate::DisplayDirection::Down),
            ("3", "7", crate::DisplayDirection::Up),
        ] {
            assert_eq!(
                approximate(left, right).expect("근사").original_direction(),
                expected
            );
        }
    }

    #[derive(Debug, Clone, Copy)]
    enum ProductTemplate {
        Small,
        Trailing,
        Reduced,
    }

    #[derive(Debug, Clone, Copy)]
    enum CoverageKind {
        Small,
        Trailing,
        Reduced,
        Terminating,
        Constructed,
        Boundary,
    }

    #[derive(Debug)]
    enum Expected {
        Product,
        Quotient(Precision, Decimal),
    }

    #[derive(Debug)]
    struct Sample {
        left: Decimal,
        right: Decimal,
        expected: Expected,
    }

    fn signed_coefficient(value: i128, sign: Sign) -> i128 {
        if sign == Sign::Negative {
            value.checked_neg().expect("부호")
        } else {
            value
        }
    }

    fn trailing_decimal(coefficient: i128, zeros: u32, scale: u32) -> Decimal {
        let factor = 10_i128.checked_pow(zeros).expect("스케일");
        Decimal::from_i128_with_scale(coefficient.checked_mul(factor).expect("계수"), scale)
    }

    fn ending_coefficient(seed: i128, digit: i128) -> i128 {
        seed.checked_mul(10)
            .expect("계수")
            .checked_add(digit)
            .expect("끝자리")
    }

    fn product_samples(template: ProductTemplate) -> proptest::strategy::BoxedStrategy<Sample> {
        (
            1_i128..=9999,
            1_i128..=9999,
            0_u32..=12,
            0_u32..=12,
            1_u32..=8,
            14_u32..=20,
            any::<bool>(),
        )
            .prop_map(
                move |(
                    first_seed,
                    second_seed,
                    first_scale,
                    second_scale,
                    zeros,
                    reduction,
                    negative,
                )| {
                    let magnitude = ending_coefficient(first_seed, 3);
                    let second = ending_coefficient(second_seed, 7);
                    let sign = if negative {
                        Sign::Negative
                    } else {
                        Sign::Positive
                    };
                    let first = signed_coefficient(magnitude, sign);
                    let (left, right) = match template {
                        ProductTemplate::Small => (
                            Decimal::from_i128_with_scale(first, first_scale),
                            Decimal::from_i128_with_scale(second, second_scale),
                        ),
                        ProductTemplate::Trailing => (
                            trailing_decimal(
                                first,
                                zeros,
                                zeros.checked_add(first_scale).expect("스케일"),
                            ),
                            trailing_decimal(
                                second,
                                zeros,
                                zeros.checked_add(second_scale).expect("스케일"),
                            ),
                        ),
                        ProductTemplate::Reduced => (
                            trailing_decimal(first, reduction, 28),
                            trailing_decimal(second, reduction, 28),
                        ),
                    };
                    Sample {
                        left,
                        right,
                        expected: Expected::Product,
                    }
                },
            )
            .boxed()
    }

    fn terminating_samples() -> proptest::strategy::BoxedStrategy<Sample> {
        (
            1_i128..=9999,
            1_u32..=12,
            1_u32..=12,
            0_u32..=8,
            any::<bool>(),
        )
            .prop_map(|(numerator_seed, twos, fives, scale, negative)| {
                let magnitude = numerator_seed
                    .checked_mul(10)
                    .expect("계수")
                    .checked_add(3)
                    .expect("끝자리");
                let sign = if negative {
                    Sign::Negative
                } else {
                    Sign::Positive
                };
                let numerator = signed_coefficient(magnitude, sign);
                let denominator = 2_i128
                    .checked_pow(twos)
                    .expect("2의 거듭제곱")
                    .checked_mul(5_i128.checked_pow(fives).expect("5의 거듭제곱"))
                    .expect("분모");
                let places = twos.max(fives);
                let coefficient = numerator
                    .checked_mul(2_i128.checked_pow(places.abs_diff(twos)).expect("계수"))
                    .and_then(|value| {
                        value.checked_mul(5_i128.checked_pow(places.abs_diff(fives)).expect("계수"))
                    })
                    .expect("분자");
                Sample {
                    left: Decimal::from_i128_with_scale(numerator, scale),
                    right: Decimal::from_i128_with_scale(denominator, 0),
                    expected: Expected::Quotient(
                        Precision::Exact,
                        Decimal::from_i128_with_scale(
                            coefficient,
                            scale.checked_add(places).expect("스케일"),
                        ),
                    ),
                }
            })
            .boxed()
    }

    fn constructed_samples() -> proptest::strategy::BoxedStrategy<Sample> {
        (
            1_i128..=9999,
            1_i128..=9999,
            0_u32..=8,
            0_u32..=8,
            any::<bool>(),
        )
            .prop_map(
                |(quotient_seed, denominator_seed, quotient_scale, denominator_scale, negative)| {
                    let magnitude = quotient_seed
                        .checked_mul(10)
                        .expect("계수")
                        .checked_add(3)
                        .expect("끝자리");
                    let sign = if negative {
                        Sign::Negative
                    } else {
                        Sign::Positive
                    };
                    let quotient = signed_coefficient(magnitude, sign);
                    let denominator = denominator_seed
                        .checked_mul(10)
                        .expect("계수")
                        .checked_add(7)
                        .expect("끝자리");
                    Sample {
                        left: Decimal::from_i128_with_scale(
                            quotient.checked_mul(denominator).expect("정수 곱"),
                            quotient_scale
                                .checked_add(denominator_scale)
                                .expect("스케일"),
                        ),
                        right: Decimal::from_i128_with_scale(denominator, denominator_scale),
                        expected: Expected::Quotient(
                            Precision::Exact,
                            Decimal::from_i128_with_scale(quotient, quotient_scale),
                        ),
                    }
                },
            )
            .boxed()
    }

    fn assert_near_half_boundary(maximum: i128, offset: i128, denominator: i128, adjustment: i128) {
        let residual = maximum
            .checked_mul(offset)
            .expect("넓은 정수")
            .checked_sub(adjustment.checked_mul(denominator).expect("넓은 정수"))
            .expect("잔차")
            .unsigned_abs();
        let unit = u128::try_from(denominator.checked_sub(offset).expect("분모")).expect("양수");
        assert!(residual.checked_mul(1000).expect("하한") > unit.checked_mul(499).expect("하한"));
        assert!(residual.checked_mul(2).expect("상한") < unit);
    }

    fn boundary_samples() -> proptest::strategy::BoxedStrategy<Sample> {
        (
            proptest::sample::select(vec![1030_i128, 2507, 149, 1328]),
            1_u32..=28,
            any::<bool>(),
        )
            .prop_map(|(offset, scale, negative)| {
                let maximum = Decimal::MAX.mantissa();
                let denominator_base = 10_i128.checked_pow(28).expect("분모");
                let adjustment = maximum
                    .checked_mul(offset)
                    .expect("넓은 정수")
                    .checked_add(denominator_base.checked_div(2).expect("절반"))
                    .expect("올림")
                    .checked_div(denominator_base)
                    .expect("분모");
                let sign = if negative {
                    Sign::Negative
                } else {
                    Sign::Positive
                };
                assert_near_half_boundary(maximum, offset, denominator_base, adjustment);
                let numerator =
                    signed_coefficient(maximum.checked_sub(adjustment).expect("분자"), sign);
                Sample {
                    left: Decimal::from_i128_with_scale(numerator, scale),
                    right: Decimal::from_i128_with_scale(
                        denominator_base.checked_sub(offset).expect("분모"),
                        28,
                    ),
                    expected: Expected::Quotient(
                        Precision::Approximate,
                        Decimal::from_i128_with_scale(signed_coefficient(maximum, sign), scale),
                    ),
                }
            })
            .boxed()
    }

    fn samples(kind: CoverageKind) -> proptest::strategy::BoxedStrategy<Sample> {
        match kind {
            CoverageKind::Small => product_samples(ProductTemplate::Small),
            CoverageKind::Trailing => product_samples(ProductTemplate::Trailing),
            CoverageKind::Reduced => product_samples(ProductTemplate::Reduced),
            CoverageKind::Terminating => terminating_samples(),
            CoverageKind::Constructed => constructed_samples(),
            CoverageKind::Boundary => boundary_samples(),
        }
    }

    fn exercise_sample(sample: &Sample) -> Decimal {
        let left = Money::new(Amount::new(sample.left), Currency::Usd);
        match sample.expected {
            Expected::Product => {
                let result = left.checked_mul(&Ratio::new(sample.right));
                assert_eq!(
                    result,
                    product_reference(sample.left, sample.right)
                        .map(|value| Money::new(Amount::new(value), Currency::Usd))
                );
                result.expect("정확한 곱").amount().as_decimal()
            }
            Expected::Quotient(precision, expected_value) => {
                assert_quotient_against_integer_bound(sample.left, sample.right);
                let result =
                    left.checked_ratio(&Money::new(Amount::new(sample.right), Currency::Usd));
                let expected = match precision {
                    Precision::Exact => RatioQuotient::Exact(Ratio::new(expected_value)),
                    Precision::Approximate => {
                        RatioQuotient::Approximate(super::ApproximateRatio(expected_value))
                    }
                };
                assert_eq!(result, Ok(expected));
                quotient_value(result.expect("최대 정밀도 몫"))
            }
        }
    }

    #[test]
    fn structured_categories_each_reach_4096_nonzero_nonunit_successes() {
        for kind in [
            CoverageKind::Small,
            CoverageKind::Trailing,
            CoverageKind::Reduced,
            CoverageKind::Terminating,
            CoverageKind::Constructed,
            CoverageKind::Boundary,
        ] {
            let successes = std::cell::Cell::new(0_usize);
            let nonunit_successes = std::cell::Cell::new(0_usize);
            TestRunner::new(Config::with_cases(4096))
                .run(&samples(kind), |sample| {
                    let value = exercise_sample(&sample);
                    successes.set(
                        successes
                            .get()
                            .checked_add(usize::from(!value.is_zero()))
                            .expect("성공 수"),
                    );
                    let nonunit = !value.is_zero()
                        && sample.left.abs() != Decimal::ONE
                        && sample.right.abs() != Decimal::ONE;
                    nonunit_successes.set(
                        nonunit_successes
                            .get()
                            .checked_add(usize::from(nonunit))
                            .expect("비단위 성공 수"),
                    );
                    Ok(())
                })
                .expect("범주별 독립 기준");
            assert_eq!(successes.get(), 4096);
            assert_eq!(nonunit_successes.get(), 4096);
        }
    }
}
