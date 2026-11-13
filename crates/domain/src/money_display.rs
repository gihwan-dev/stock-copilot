use rust_decimal::{Decimal, RoundingStrategy};

use crate::{Amount, Currency, Money};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DisplayPrecision(u8);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayPrecisionError {
    OutOfRange,
}

impl DisplayPrecision {
    pub fn try_new(value: u8) -> Result<Self, DisplayPrecisionError> {
        if value > 28 {
            return Err(DisplayPrecisionError::OutOfRange);
        }
        Ok(Self(value))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayDirection {
    Up,
    Down,
    Flat,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisplayDigits(String);

impl DisplayDigits {
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisplayMoney {
    digits: DisplayDigits,
    currency: Currency,
}

impl DisplayMoney {
    #[must_use]
    pub fn digits(&self) -> &DisplayDigits {
        &self.digits
    }

    #[must_use]
    pub fn currency(&self) -> Currency {
        self.currency
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MoneyChange(Money);

impl MoneyChange {
    #[must_use]
    pub fn new(value: Money) -> Self {
        Self(value)
    }

    #[must_use]
    pub fn direction(&self) -> DisplayDirection {
        match self.0.amount().as_decimal().cmp(&Decimal::ZERO) {
            std::cmp::Ordering::Greater => DisplayDirection::Up,
            std::cmp::Ordering::Less => DisplayDirection::Down,
            std::cmp::Ordering::Equal => DisplayDirection::Flat,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisplayMoneyChange {
    money: DisplayMoney,
    direction: DisplayDirection,
}

impl DisplayMoneyChange {
    #[must_use]
    pub fn money(&self) -> &DisplayMoney {
        &self.money
    }

    #[must_use]
    pub fn direction(&self) -> DisplayDirection {
        self.direction
    }
}

#[must_use]
pub fn display_money(money: &Money, precision: DisplayPrecision) -> DisplayMoney {
    DisplayMoney {
        digits: display_digits(money.amount(), precision),
        currency: money.currency(),
    }
}

#[must_use]
pub fn display_change(change: &MoneyChange, precision: DisplayPrecision) -> DisplayMoneyChange {
    DisplayMoneyChange {
        money: display_money(&change.0, precision),
        direction: change.direction(),
    }
}

fn display_digits(amount: Amount, precision: DisplayPrecision) -> DisplayDigits {
    let value = amount.as_decimal();
    let rounded = value.round_dp_with_strategy(
        u32::from(precision.0),
        RoundingStrategy::MidpointAwayFromZero,
    );
    let unsigned_zero = if rounded.is_zero() {
        Decimal::ZERO
    } else {
        rounded
    };
    let mut digits = unsigned_zero.to_string();
    let fraction = if let Some((_, fraction)) = digits.split_once('.') {
        fraction.len()
    } else {
        if precision.0 > 0 {
            digits.push('.');
        }
        0
    };
    digits.extend(std::iter::repeat_n('0', usize::from(precision.0)).skip(fraction));
    DisplayDigits(digits)
}

#[cfg(test)]
mod tests {
    use super::{
        DisplayDirection, DisplayPrecision, DisplayPrecisionError, MoneyChange, display_change,
        display_money,
    };
    use crate::{Amount, AmountInput, Currency, Money};
    use proptest::arbitrary::any;
    use proptest::strategy::Strategy;
    use proptest::test_runner::TestRunner;
    use rust_decimal::Decimal;

    #[test]
    fn display_rounds_without_mutating_money_and_direction_uses_original_change() {
        for (input, expected, direction) in [
            ("1.235", "1.24", DisplayDirection::Up),
            ("-1.235", "-1.24", DisplayDirection::Down),
            ("1.234", "1.23", DisplayDirection::Up),
            ("-0.004", "0.00", DisplayDirection::Down),
            ("-0.0049", "0.00", DisplayDirection::Down),
            ("-0.0001", "0.00", DisplayDirection::Down),
            (
                "-0.0000000000000000000000000001",
                "0.00",
                DisplayDirection::Down,
            ),
            ("0.004", "0.00", DisplayDirection::Up),
            ("-0.005", "-0.01", DisplayDirection::Down),
            ("0", "0.00", DisplayDirection::Flat),
            ("-0", "0.00", DisplayDirection::Flat),
        ] {
            let amount = Amount::parse(&AmountInput::new(input))
                .expect("valid decimal")
                .as_decimal();
            let money = Money::new(Amount::new(amount), Currency::Usd);
            let precision = DisplayPrecision::try_new(2).expect("valid precision");
            let shown = display_money(&money, precision);
            let change = display_change(&MoneyChange::new(money), precision);
            assert_eq!(shown.digits().as_str(), expected);
            assert_eq!(shown.currency(), Currency::Usd);
            assert_eq!(change.direction(), direction);
            assert_eq!(change.money(), &shown);
            assert_eq!(money.amount().as_decimal(), amount);
        }
    }

    #[test]
    fn precision_limits_and_maximum_amount_are_supported() {
        assert_eq!(
            DisplayPrecision::try_new(29),
            Err(DisplayPrecisionError::OutOfRange)
        );
        for precision in [0, 28] {
            assert_eq!(
                DisplayPrecision::try_new(precision).expect("valid precision"),
                DisplayPrecision(precision)
            );
        }
        let maximum = Money::new(Amount::new(Decimal::MAX), Currency::Krw);
        let shown = display_money(
            &maximum,
            DisplayPrecision::try_new(28).expect("valid precision"),
        );
        assert_eq!(
            shown.digits().as_str(),
            "79228162514264337593543950335.0000000000000000000000000000"
        );
    }

    #[test]
    fn arbitrary_display_has_exact_precision_and_unsigned_zero() {
        let full_values = (
            any::<u32>(),
            any::<u32>(),
            any::<u32>(),
            any::<bool>(),
            0_u32..=28_u32,
            0_u8..=28_u8,
        )
            .prop_map(|(lo, mid, hi, negative, scale, precision)| {
                (Decimal::from_parts(lo, mid, hi, negative, scale), precision)
            });
        let values = proptest::prop_oneof![
            full_values,
            (0_u32..10_u32, 0_u8..=27_u8)
                .prop_map(|(lo, precision)| (Decimal::from_parts(lo, 0, 0, true, 28), precision))
        ];
        TestRunner::default()
            .run(&values, |(value, precision)| {
                let money = Money::new(Amount::new(value), Currency::Krw);
                let shown = display_money(
                    &money,
                    DisplayPrecision::try_new(precision).expect("valid precision"),
                );
                let text = shown.digits().as_str();
                let fraction = text
                    .split_once('.')
                    .map_or(0, |(_, fraction)| fraction.len());
                proptest::prop_assert_eq!(fraction, usize::from(precision));
                if precision == 0 {
                    proptest::prop_assert!(!text.contains('.'));
                }
                if text
                    .bytes()
                    .all(|byte| byte == b'-' || byte == b'0' || byte == b'.')
                {
                    proptest::prop_assert!(!text.starts_with('-'));
                }
                Ok(())
            })
            .expect("display properties");
    }

    #[test]
    fn sub_half_unit_values_display_as_unsigned_zero() {
        TestRunner::default()
            .run(
                &(0_u8..=27_u8, 0_u32..5_u32, any::<bool>()),
                |(precision, coefficient, negative)| {
                    let scale = u32::from(precision).checked_add(1).expect("at most 28");
                    let amount = Decimal::from_parts(coefficient, 0, 0, negative, scale);
                    let money = Money::new(Amount::new(amount), Currency::Usd);
                    let shown = display_money(
                        &money,
                        DisplayPrecision::try_new(precision).expect("valid precision"),
                    );
                    proptest::prop_assert_eq!(
                        Amount::parse(&AmountInput::new(shown.digits().as_str()))
                            .expect("display decimal")
                            .as_decimal(),
                        Decimal::ZERO
                    );
                    proptest::prop_assert!(!shown.digits().as_str().starts_with('-'));
                    Ok(())
                },
            )
            .expect("sub half unit rounds to unsigned zero");
    }
}
