use rust_decimal::Decimal;

use crate::{Amount, MoneyError, Ratio};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RatioQuotient {
    Exact(Ratio),
    Approximate(ApproximateRatio),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ApproximateRatio(Decimal);

impl ApproximateRatio {
    #[must_use]
    pub fn as_decimal(&self) -> Decimal {
        self.0
    }
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
            RatioQuotient::Approximate(ApproximateRatio(maximum_scale(candidate).as_decimal()))
        }
    })
}

fn maximum_scale(value: Amount) -> Amount {
    let mut coefficient = value.as_decimal().mantissa();
    let mut scale = value.as_decimal().scale();
    for next_scale in 1..=28 {
        if next_scale <= scale {
            continue;
        }
        let Some(next) = coefficient
            .checked_mul(10)
            .filter(|next| next.unsigned_abs() <= Decimal::MAX.mantissa().unsigned_abs())
        else {
            break;
        };
        coefficient = next;
        scale = next_scale;
    }
    Amount::new(Decimal::from_i128_with_scale(coefficient, scale))
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

    #[derive(Clone, Copy)]
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
}
