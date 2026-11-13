use rust_decimal::{Decimal, RoundingStrategy};

use crate::{Currency, Money};

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
    direction: DisplayDirection,
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

    #[must_use]
    pub fn direction(&self) -> DisplayDirection {
        self.direction
    }
}

#[must_use]
pub fn display_money(money: &Money, precision: DisplayPrecision) -> DisplayMoney {
    let rounded = money.amount().as_decimal().round_dp_with_strategy(
        u32::from(precision.0),
        RoundingStrategy::MidpointAwayFromZero,
    );
    let (value, direction) = match rounded.cmp(&Decimal::ZERO) {
        std::cmp::Ordering::Greater => (rounded, DisplayDirection::Up),
        std::cmp::Ordering::Less => (rounded, DisplayDirection::Down),
        std::cmp::Ordering::Equal => (Decimal::ZERO, DisplayDirection::Flat),
    };
    let mut digits = value.normalize().to_string();
    let fraction = if let Some((_, fraction)) = digits.split_once('.') {
        fraction.len()
    } else {
        if precision.0 > 0 {
            digits.push('.');
        }
        0
    };
    digits.extend(std::iter::repeat_n('0', usize::from(precision.0)).skip(fraction));
    DisplayMoney {
        digits: DisplayDigits(digits),
        currency: money.currency(),
        direction,
    }
}

#[cfg(test)]
mod tests {
    use super::{DisplayDirection, DisplayPrecision, DisplayPrecisionError, display_money};
    use crate::{Amount, Currency, Money};
    use rust_decimal::Decimal;

    #[test]
    fn display_rounds_midpoints_away_from_zero_without_changing_money() {
        for (input, expected, direction) in [
            ("1.235", "1.24", DisplayDirection::Up),
            ("-1.235", "-1.24", DisplayDirection::Down),
            ("1.234", "1.23", DisplayDirection::Up),
            ("-0.004", "0.00", DisplayDirection::Flat),
            ("0.004", "0.00", DisplayDirection::Flat),
            ("-0.005", "-0.01", DisplayDirection::Down),
            ("0", "0.00", DisplayDirection::Flat),
        ] {
            let amount = Decimal::from_str_exact(input).expect("valid decimal");
            let money = Money::new(Amount::new(amount), Currency::Usd);
            let shown = display_money(
                &money,
                DisplayPrecision::try_new(2).expect("valid precision"),
            );
            assert_eq!(shown.digits().as_str(), expected);
            assert_eq!(shown.direction(), direction);
            assert_eq!(shown.currency(), Currency::Usd);
            assert_eq!(money.amount().as_decimal(), amount);
        }
    }

    #[test]
    fn precision_is_bounded_and_currency_does_not_round_storage() {
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
        let money = Money::new(Amount::new(Decimal::new(1235, 3)), Currency::Krw);
        let shown = display_money(
            &money,
            DisplayPrecision::try_new(0).expect("valid precision"),
        );
        assert_eq!(shown.digits().as_str(), "1");
        assert_eq!(shown.currency(), Currency::Krw);
        let maximum = Money::new(Amount::new(Decimal::MAX), Currency::Usd);
        let maximum_shown = display_money(
            &maximum,
            DisplayPrecision::try_new(28).expect("valid precision"),
        );
        assert_eq!(
            maximum_shown.digits().as_str(),
            "79228162514264337593543950335.0000000000000000000000000000"
        );
    }
}
