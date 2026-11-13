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
    pub fn new(value: Decimal) -> Self {
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

    fn combine(&self, other: &Self, operation: Arithmetic) -> Result<Self, MoneyError> {
        let candidate = match operation {
            Arithmetic::Add => self.0.checked_add(other.0),
            Arithmetic::Subtract => self.0.checked_sub(other.0),
        }
        .ok_or(MoneyError::Overflow)?;
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
        let verification = candidate.normalize();
        let actual = 10_i128
            .checked_pow(scale.abs_diff(verification.scale()))
            .and_then(|factor| verification.mantissa().checked_mul(factor));
        if expected
            .zip(actual)
            .is_some_and(|(exact, result)| exact == result)
        {
            Ok(Self::new(candidate))
        } else {
            Err(MoneyError::PrecisionLoss)
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
}

impl Money {
    #[must_use]
    pub fn new(amount: Amount, currency: Currency) -> Self {
        Self { amount, currency }
    }

    #[must_use]
    pub fn amount(&self) -> Amount {
        self.amount
    }

    #[must_use]
    pub fn currency(&self) -> Currency {
        self.currency
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
    use proptest::test_runner::TestRunner;
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
            ("79228162514264337593543950335", "0.4"),
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
        TestRunner::default()
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
        TestRunner::default()
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
