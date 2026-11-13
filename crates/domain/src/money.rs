use rust_decimal::Decimal;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Currency {
    Krw,
    Usd,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Amount(Decimal);

impl Amount {
    #[must_use]
    pub(crate) fn new(value: Decimal) -> Self {
        Self(value)
    }

    #[must_use]
    pub fn as_decimal(&self) -> Decimal {
        self.0
    }
    pub fn parse(input: &AmountInput) -> Result<Self, AmountError> {
        let unsigned = input.0.strip_prefix('-').unwrap_or(&input.0);
        let (integer, fraction) = unsigned
            .split_once('.')
            .map_or((unsigned, None), |(whole, fraction)| {
                (whole, Some(fraction))
            });
        if integer.is_empty() || !integer.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(AmountError::InvalidSyntax);
        }
        if let Some(digits) = fraction {
            if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
                return Err(AmountError::InvalidSyntax);
            }
            if digits.len() > 28 {
                return Err(AmountError::NotRepresentable);
            }
        }
        Decimal::from_str_exact(&input.0)
            .map(Self::new)
            .map_err(|_error| AmountError::NotRepresentable)
    }

    pub(crate) fn checked_add(&self, other: &Self) -> Result<Self, MoneyError> {
        self.combine(other, Arithmetic::Add)
    }

    pub(crate) fn checked_sub(&self, other: &Self) -> Result<Self, MoneyError> {
        self.combine(other, Arithmetic::Subtract)
    }

    fn combine(&self, other: &Self, operation: Arithmetic) -> Result<Self, MoneyError> {
        if self.range(other, operation) == SumRange::Overflow {
            return Err(MoneyError::Overflow);
        }
        let candidate = match operation {
            Arithmetic::Add => self.0.checked_add(other.0),
            Arithmetic::Subtract => self.0.checked_sub(other.0),
        };
        candidate
            .and_then(|value| self.verify(other, operation, Self::new(value)))
            .ok_or(MoneyError::PrecisionLoss)
    }

    fn verify(&self, other: &Self, operation: Arithmetic, candidate: Self) -> Option<Self> {
        let left = self.0.normalize();
        let right = other.0.normalize();
        let scale = left.scale().max(right.scale());
        let left_coefficient = 10_i128
            .checked_pow(scale.abs_diff(left.scale()))
            .and_then(|factor| left.mantissa().checked_mul(factor));
        let right_coefficient = 10_i128
            .checked_pow(scale.abs_diff(right.scale()))
            .and_then(|factor| right.mantissa().checked_mul(factor));
        let expected =
            left_coefficient
                .zip(right_coefficient)
                .and_then(|(left_integer, right_integer)| match operation {
                    Arithmetic::Add => left_integer.checked_add(right_integer),
                    Arithmetic::Subtract => left_integer.checked_sub(right_integer),
                });
        let verification = candidate.0.normalize();
        let actual = 10_i128
            .checked_pow(scale.abs_diff(verification.scale()))
            .and_then(|factor| verification.mantissa().checked_mul(factor));
        expected
            .zip(actual)
            .filter(|(exact, result)| exact == result)
            .map(|_| candidate)
    }

    fn range(&self, other: &Self, operation: Arithmetic) -> SumRange {
        let grows = match operation {
            Arithmetic::Add => self.0.is_sign_negative() == other.0.is_sign_negative(),
            Arithmetic::Subtract => self.0.is_sign_negative() != other.0.is_sign_negative(),
        };
        let larger = self.0.abs().max(other.0.abs());
        let smaller = self.0.abs().min(other.0.abs());
        if grows
            && Decimal::MAX
                .checked_sub(larger)
                .is_some_and(|capacity| smaller > capacity)
        {
            SumRange::Overflow
        } else {
            SumRange::Within
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Money {
    amount: Amount,
    currency: Currency,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoneyError {
    CurrencyMismatch,
    Overflow,
    PrecisionLoss,
    DivisionByZero,
}

impl Money {
    #[must_use]
    pub(crate) fn new(amount: Amount, currency: Currency) -> Self {
        Self { amount, currency }
    }

    pub fn parse(
        input: &crate::NumericInput,
        currency: Currency,
    ) -> Result<Self, crate::NumericError> {
        input
            .parse_amount()
            .map(|amount| Self::new(amount, currency))
    }

    #[must_use]
    pub fn amount(&self) -> Amount {
        self.amount
    }

    #[must_use]
    pub fn currency(&self) -> Currency {
        self.currency
    }

    pub fn checked_mul(&self, ratio: &crate::Ratio) -> Result<Self, MoneyError> {
        crate::exact_arithmetic::multiply(self.amount, Amount::new(ratio.as_decimal()))
            .map(|amount| Self::new(amount, self.currency))
    }

    pub fn checked_ratio(&self, other: &Self) -> Result<crate::RatioQuotient, MoneyError> {
        if self.currency != other.currency {
            return Err(MoneyError::CurrencyMismatch);
        }
        crate::exact_arithmetic::divide(self.amount, other.amount)
    }

    pub fn checked_add(&self, other: &Self) -> Result<Self, MoneyError> {
        if self.currency != other.currency {
            return Err(MoneyError::CurrencyMismatch);
        }
        self.amount
            .combine(&other.amount, Arithmetic::Add)
            .map(|amount| Self::new(amount, self.currency))
    }

    pub fn checked_sub(&self, other: &Self) -> Result<Self, MoneyError> {
        if self.currency != other.currency {
            return Err(MoneyError::CurrencyMismatch);
        }
        self.amount
            .combine(&other.amount, Arithmetic::Subtract)
            .map(|amount| Self::new(amount, self.currency))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SumRange {
    Within,
    Overflow,
}

#[derive(Debug, Clone, Copy)]
enum Arithmetic {
    Add,
    Subtract,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AmountInput(String);

impl AmountInput {
    #[must_use]
    pub fn new(value: &str) -> Self {
        Self(value.to_owned())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AmountError {
    InvalidSyntax,
    NotRepresentable,
}

#[cfg(test)]
mod tests {
    use super::{Amount, AmountError, AmountInput, Currency, Money, MoneyError};
    use proptest::arbitrary::any;
    use proptest::strategy::Strategy;
    use proptest::test_runner::{Config, TestRunner};
    use rust_decimal::Decimal;

    fn decimals() -> impl Strategy<Value = Decimal> {
        proptest::prop_oneof![
            proptest::strategy::Just(Decimal::MAX),
            proptest::strategy::Just(Decimal::MIN),
            proptest::strategy::Just(Decimal::ZERO),
            (any::<u32>(), 0_u32..=28_u32)
                .prop_map(|(lo, scale)| Decimal::from_parts(lo, 0, 0, false, scale)),
            (
                any::<u32>(),
                any::<u32>(),
                any::<u32>(),
                any::<bool>(),
                0_u32..=28_u32
            )
                .prop_map(|(lo, mid, hi, negative, scale)| Decimal::from_parts(
                    lo, mid, hi, negative, scale
                )),
        ]
    }

    fn money(value: &str) -> Money {
        Money::new(
            Amount::parse(&AmountInput::new(value)).expect("exact input"),
            Currency::Usd,
        )
    }

    #[test]
    fn reviewer_precision_loss_and_overflow_regressions() {
        for (left, right) in [
            ("10", "0.0000000000000000000000000001"),
            ("12345678901234.5678", "0.1234567890123456789012345678"),
            ("10", "0.0000000000000000000000000006"),
        ] {
            assert_eq!(
                money(left).checked_add(&money(right)),
                Err(MoneyError::PrecisionLoss)
            );
            assert_eq!(
                money(left).checked_sub(&money(right)),
                Err(MoneyError::PrecisionLoss)
            );
        }
        assert_eq!(
            money("79228162514264337593543950335").checked_add(&money("1")),
            Err(MoneyError::Overflow)
        );
        assert_eq!(
            money("-79228162514264337593543950335").checked_sub(&money("1")),
            Err(MoneyError::Overflow)
        );
        let tiny = money("0.0000000000000000000000000006");
        assert_eq!(
            tiny.checked_add(&tiny).expect("exact tiny sum"),
            money("0.0000000000000000000000000012")
        );
        assert_eq!(
            money("10").checked_add(&tiny),
            Err(MoneyError::PrecisionLoss)
        );
        assert_eq!(
            money("10").checked_add(&tiny.checked_add(&tiny).expect("exact sum")),
            Err(MoneyError::PrecisionLoss)
        );
    }

    #[test]
    fn arbitrary_decimals_preserve_exact_arithmetic_and_currency() {
        TestRunner::new(Config::with_cases(4096))
            .run(
                &(
                    decimals(),
                    decimals(),
                    decimals(),
                    any::<bool>(),
                    any::<bool>(),
                ),
                |(left, right, third, left_usd, right_usd)| {
                    let left_currency = if left_usd {
                        Currency::Usd
                    } else {
                        Currency::Krw
                    };
                    let right_currency = if right_usd {
                        Currency::Usd
                    } else {
                        Currency::Krw
                    };
                    let first = Money::new(Amount::new(left), left_currency);
                    let second = Money::new(Amount::new(right), right_currency);
                    if left_currency != right_currency {
                        proptest::prop_assert_eq!(
                            first.checked_add(&second),
                            Err(MoneyError::CurrencyMismatch)
                        );
                        proptest::prop_assert_eq!(
                            first.checked_sub(&second),
                            Err(MoneyError::CurrencyMismatch)
                        );
                        return Ok(());
                    }
                    let sum = first.checked_add(&second);
                    proptest::prop_assert_eq!(sum, second.checked_add(&first));
                    if let Ok(value) = sum {
                        proptest::prop_assert_eq!(value.checked_sub(&second), Ok(first));
                        proptest::prop_assert_eq!(value.checked_sub(&first), Ok(second));
                    }
                    if let Ok(value) = first.checked_sub(&second) {
                        proptest::prop_assert_eq!(value.checked_add(&second), Ok(first));
                    }
                    let last = Money::new(Amount::new(third), left_currency);
                    if let (Ok(lhs), Ok(rhs)) = (
                        sum.and_then(|value| value.checked_add(&last)),
                        second
                            .checked_add(&last)
                            .and_then(|value| first.checked_add(&value)),
                    ) {
                        proptest::prop_assert_eq!(lhs, rhs);
                    }
                    if left.checked_add(right).is_none() {
                        proptest::prop_assert_eq!(sum, Err(MoneyError::Overflow));
                    }
                    Ok(())
                },
            )
            .expect("arbitrary decimals obey exact arithmetic");
    }

    #[test]
    fn parsing_accepts_only_exact_canonical_decimal_syntax() {
        for text in [
            "", "-", ".1", "1.", "1.2.3", "1e5", "1_000", "+1", " 1", "1 ", "１２", "--1",
        ] {
            assert_eq!(
                Amount::parse(&AmountInput::new(text)),
                Err(AmountError::InvalidSyntax)
            );
        }
        for text in [
            "0.00000000000000000000000000001",
            "79228162514264337593543950336",
            "1.00000000000000000000000000000",
        ] {
            assert_eq!(
                Amount::parse(&AmountInput::new(text)),
                Err(AmountError::NotRepresentable)
            );
        }
        TestRunner::new(Config::with_cases(4096))
            .run(&decimals(), |value| {
                let text = value.to_string();
                let parsed = Amount::parse(&AmountInput::new(&text)).expect("canonical decimal");
                proptest::prop_assert_eq!(parsed.as_decimal(), value);
                proptest::prop_assert_eq!(
                    Amount::parse(&AmountInput::new(&parsed.as_decimal().to_string())),
                    Ok(parsed)
                );
                Ok(())
            })
            .expect("canonical strings round trip exactly");
    }

    #[test]
    fn exact_cancellation_and_redundant_scales_remain_valid() {
        assert_eq!(
            money("1.23456789").checked_add(&money("0.00000001")),
            Ok(money("1.23456790"))
        );
        assert_eq!(
            money("1.23456789").checked_sub(&money("1.23456789")),
            Ok(money("0"))
        );
        assert_eq!(money("0.10").checked_add(&money("0.90")), Ok(money("1")));
        assert_eq!(
            money("7922816251426433759354395033.5").checked_add(&money("0.5")),
            Ok(money("7922816251426433759354395034"))
        );
        assert_eq!(money("-1").checked_add(&money("2")), Ok(money("1")));
        assert_eq!(money("1").checked_add(&money("-2")), Ok(money("-1")));
    }
}

#[cfg(test)]
mod oracle_tests {
    use proptest::arbitrary::any;
    use proptest::strategy::Strategy;
    use proptest::test_runner::{Config, TestRunner};
    use rust_decimal::Decimal;

    use crate::{Amount, AmountInput, Currency, Money, MoneyError};

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Sign {
        Positive,
        Negative,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Operation {
        Add,
        Subtract,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Reference {
        Value(Decimal),
        Failure(MoneyError),
    }

    impl Reference {
        fn result(self) -> Result<Decimal, MoneyError> {
            match self {
                Self::Value(value) => Ok(value),
                Self::Failure(error) => Err(error),
            }
        }
    }

    fn sign(value: Decimal) -> Sign {
        if value.is_sign_negative() {
            Sign::Negative
        } else {
            Sign::Positive
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct Wide([u64; 4]);

    impl Wide {
        fn coefficient(value: Decimal) -> Self {
            let parts = value.unpack();
            Self([
                u64::from(parts.lo) | u64::from(parts.mid).checked_shl(32).expect("shift"),
                u64::from(parts.hi),
                0,
                0,
            ])
        }

        fn cmp(&self, other: &Self) -> std::cmp::Ordering {
            self.0.iter().rev().cmp(other.0.iter().rev())
        }

        fn scale_up(mut self, places: u32) -> Self {
            for _place in 0..places {
                self.multiply_ten();
            }
            self
        }

        fn multiply_ten(&mut self) {
            let mut carry = 0_u128;
            for limb in &mut self.0 {
                let product = u128::from(*limb)
                    .checked_mul(10)
                    .and_then(|part| part.checked_add(carry))
                    .expect("256 bit coefficient bound");
                *limb = u64::try_from(product & u128::from(u64::MAX)).expect("masked limb");
                carry = product.checked_shr(64).expect("shift");
            }
            assert_eq!(carry, 0);
        }

        fn reduced(&self, scale: u32) -> Option<Self> {
            if scale == 0 {
                return None;
            }
            let mut divided = *self;
            (divided.divide_ten() == 0).then_some(divided)
        }

        fn add(self, other: Self) -> Self {
            let mut result = self;
            let mut carry = 0_u128;
            for (limb, rhs) in result.0.iter_mut().zip(other.0) {
                let total = u128::from(*limb)
                    .checked_add(u128::from(rhs))
                    .and_then(|part| part.checked_add(carry))
                    .expect("two limb sum");
                *limb = u64::try_from(total & u128::from(u64::MAX)).expect("masked limb");
                carry = total.checked_shr(64).expect("shift");
            }
            assert_eq!(carry, 0);
            result
        }

        fn subtract(self, other: Self) -> Self {
            let mut result = self;
            let mut borrow = 0_u64;
            for (limb, rhs) in result.0.iter_mut().zip(other.0) {
                let (first, first_borrow) = limb.overflowing_sub(rhs);
                let (value, second_borrow) = first.overflowing_sub(borrow);
                *limb = value;
                borrow = u64::from(first_borrow || second_borrow);
            }
            assert_eq!(borrow, 0);
            result
        }

        fn divide_ten(&mut self) -> u128 {
            let mut remainder = 0_u128;
            for limb in self.0.iter_mut().rev() {
                let total = remainder
                    .checked_shl(64)
                    .and_then(|high| high.checked_add(u128::from(*limb)))
                    .expect("division carry bound");
                *limb = u64::try_from(total.checked_div(10).expect("nonzero divisor"))
                    .expect("quotient limb");
                remainder = total.checked_rem(10).expect("nonzero divisor");
            }
            remainder
        }

        fn decimal(mut self, negative: Sign, mut scale: u32) -> Reference {
            let maximum = Self::coefficient(Decimal::MAX);
            if self.cmp(&maximum.scale_up(scale)).is_gt() {
                return Reference::Failure(MoneyError::Overflow);
            }
            while let Some(divided) = self.reduced(scale) {
                self = divided;
                scale = scale.checked_sub(1).expect("positive scale");
            }
            if self.cmp(&maximum).is_gt() {
                return Reference::Failure(MoneyError::PrecisionLoss);
            }
            let [low, high, upper, top] = self.0;
            assert_eq!((upper, top), (0, 0));
            let lo = u32::try_from(low & u64::from(u32::MAX)).expect("low limb");
            let mid = u32::try_from(low.checked_shr(32).expect("shift")).expect("middle limb");
            let hi = u32::try_from(high).expect("high limb");
            Reference::Value(Decimal::from_parts(
                lo,
                mid,
                hi,
                negative == Sign::Negative,
                scale,
            ))
        }
    }

    fn reference(left: Decimal, right: Decimal, operation: Operation) -> Reference {
        let scale = left.scale().max(right.scale());
        let first = Wide::coefficient(left).scale_up(scale.abs_diff(left.scale()));
        let second = Wide::coefficient(right).scale_up(scale.abs_diff(right.scale()));
        let first_negative = sign(left);
        let second_negative = if operation == Operation::Subtract {
            if sign(right) == Sign::Negative {
                Sign::Positive
            } else {
                Sign::Negative
            }
        } else {
            sign(right)
        };
        if first_negative == second_negative {
            first.add(second).decimal(first_negative, scale)
        } else if first.cmp(&second).is_lt() {
            second.subtract(first).decimal(second_negative, scale)
        } else {
            first.subtract(second).decimal(first_negative, scale)
        }
    }

    fn decimal(text: &str) -> Decimal {
        Amount::parse(&AmountInput::new(text))
            .expect("exact decimal")
            .as_decimal()
    }

    fn inputs() -> impl Strategy<Value = (Decimal, Decimal)> {
        let all = (
            any::<u32>(),
            any::<u32>(),
            any::<u32>(),
            any::<bool>(),
            0_u32..=28_u32,
        )
            .prop_map(|(lo, mid, hi, negative, scale)| {
                Decimal::from_parts(lo, mid, hi, negative, scale)
            });
        let equal_scale = (
            any::<u32>(),
            any::<u32>(),
            0x8000_0000_u32..=u32::MAX,
            any::<u32>(),
            any::<u32>(),
            0x8000_0000_u32..=u32::MAX,
            any::<bool>(),
            0_u32..=28_u32,
        )
            .prop_map(|(lo, mid, hi, rhs_lo, rhs_mid, rhs_hi, negative, scale)| {
                (
                    Decimal::from_parts(lo, mid, hi, negative, scale),
                    Decimal::from_parts(rhs_lo, rhs_mid, rhs_hi, negative, scale),
                )
            });
        let cancellation = all.clone().prop_map(|value| {
            let parts = value.unpack();
            (
                value,
                Decimal::from_parts(parts.lo, parts.mid, parts.hi, !parts.negative, parts.scale),
            )
        });
        let padded = (1_u32..=7_u32, 0_u32..=28_u32).prop_map(|(small, places)| {
            let coefficient = i128::from(small)
                .checked_mul(10_i128.checked_pow(places).expect("bounded power"))
                .expect("bounded coefficient");
            (
                decimal("100000000000000000000"),
                Decimal::from_i128_with_scale(coefficient, places),
            )
        });
        let near_max =
            (1_u32..=u32::MAX, 0_u32..=28_u32, any::<bool>()).prop_map(|(lo, scale, negative)| {
                (Decimal::MAX, Decimal::from_parts(lo, 0, 0, negative, scale))
            });
        proptest::prop_oneof![2 => (all.clone(), all), 3 => equal_scale, 2 => cancellation, 2 => padded, 3 => near_max]
    }

    #[test]
    fn independent_256_bit_reference_matches_exact_sum_and_difference() {
        TestRunner::new(Config::with_cases(4096))
            .run(&inputs(), |(left, right)| {
                let first = Money::new(Amount::new(left), Currency::Usd);
                let second = Money::new(Amount::new(right), Currency::Usd);
                proptest::prop_assert_eq!(
                    first
                        .checked_add(&second)
                        .map(|money| money.amount().as_decimal()),
                    reference(left, right, Operation::Add).result()
                );
                proptest::prop_assert_eq!(
                    first
                        .checked_sub(&second)
                        .map(|money| money.amount().as_decimal()),
                    reference(left, right, Operation::Subtract).result()
                );
                Ok(())
            })
            .expect("wide integer reference agrees including error classes");
    }

    #[test]
    fn reviewer_m9_same_scale_loss_and_m3_padded_exact_sum_regressions() {
        for (left, right, expected) in [
            (
                "7922816251426433759354395033.5",
                "0.6",
                Err(MoneyError::PrecisionLoss),
            ),
            (
                "100000000000000000000",
                "1.0000000000000000000000000000",
                Ok(decimal("100000000000000000001")),
            ),
        ] {
            let first = Money::new(Amount::new(decimal(left)), Currency::Usd);
            let second = Money::new(Amount::new(decimal(right)), Currency::Usd);
            assert_eq!(
                reference(
                    first.amount().as_decimal(),
                    second.amount().as_decimal(),
                    Operation::Add
                )
                .result(),
                expected
            );
            assert_eq!(
                first
                    .checked_add(&second)
                    .map(|money| money.amount().as_decimal()),
                expected
            );
        }
    }

    fn assert_overflow(fraction: &str, operation: Operation) {
        let left = if operation == Operation::Subtract {
            Decimal::MIN
        } else {
            Decimal::MAX
        };
        let right = decimal(fraction);
        let first = Money::new(Amount::new(left), Currency::Usd);
        let second = Money::new(Amount::new(right), Currency::Usd);
        let result = if operation == Operation::Subtract {
            first.checked_sub(&second)
        } else {
            first.checked_add(&second)
        };
        assert_eq!(
            reference(left, right, operation).result(),
            Err(MoneyError::Overflow)
        );
        assert_eq!(result, Err(MoneyError::Overflow));
    }

    #[test]
    fn exact_magnitude_determines_overflow_before_precision_loss() {
        for fraction in ["0.4", "0.5", "0.6", "0.0000000000000000000000000001"] {
            for operation in [Operation::Add, Operation::Subtract] {
                assert_overflow(fraction, operation);
            }
        }
    }
}
