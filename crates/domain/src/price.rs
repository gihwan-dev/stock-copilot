use rust_decimal::Decimal;

use crate::{Amount, Currency, Money, MoneyError, NumericError, NumericInput, Quantity};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Price(Money);

impl Price {
    pub fn try_new(value: Money) -> Result<Self, NumericError> {
        if value.amount().as_decimal() <= Decimal::ZERO {
            return Err(NumericError::NonPositivePrice);
        }
        Ok(Self(value))
    }

    pub fn parse(input: &NumericInput, currency: Currency) -> Result<Self, NumericError> {
        Self::try_new(Money::new(input.parse_amount()?, currency))
    }

    pub fn checked_mul(&self, quantity: &Quantity) -> Result<Money, MoneyError> {
        crate::exact_arithmetic::multiply(self.0.amount(), Amount::new(quantity.as_decimal()))
            .map(|amount| Money::new(amount, self.0.currency()))
    }

    #[must_use]
    pub fn as_money(&self) -> Money {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use crate::{Amount, Currency, Money, NumericError, NumericInput, Price};
    use rust_decimal::Decimal;

    #[test]
    fn price_requires_positive_value_and_preserves_currency_and_precision() {
        for currency in [Currency::Krw, Currency::Usd] {
            for (text, expected) in [
                ("0.0000000000000000000000000001", Decimal::new(1, 28)),
                ("1.23456", Decimal::new(123_456, 5)),
                ("79228162514264337593543950335", Decimal::MAX),
            ] {
                let money = Money::new(Amount::new(expected), currency);
                assert_eq!(
                    Price::parse(&NumericInput::try_new(text).expect("길이"), currency)
                        .expect("가격")
                        .as_money(),
                    money
                );
                assert_eq!(Price::try_new(money).expect("내부 가격").as_money(), money);
            }
            for text in ["0", "-0.00", "-1", "-0.0000000000000000000000000001"] {
                assert_eq!(
                    Price::parse(&NumericInput::try_new(text).expect("길이"), currency),
                    Err(NumericError::NonPositivePrice)
                );
            }
        }
    }
}
