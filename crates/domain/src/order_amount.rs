use crate::{Currency, Money, OrderValueError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UsdOrderAmount(Money);

impl UsdOrderAmount {
    pub fn try_new(value: Money) -> Result<Self, OrderValueError> {
        if value.currency() != Currency::Usd {
            return Err(OrderValueError::CurrencyMismatch);
        }
        if value.amount().as_decimal() <= rust_decimal::Decimal::ZERO {
            return Err(OrderValueError::NonPositive);
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn money(self) -> Money {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::{Currency, Money, OrderValueError, UsdOrderAmount};
    use crate::{Amount, AmountInput};

    #[test]
    fn amount_requires_positive_usd_and_thirty_character_wire_value() {
        for text in [
            "0.0000000000000000000000000001",
            "0.01",
            "100.5",
            "79228162514264337593543950335",
        ] {
            let money = Money::new(
                Amount::parse(&AmountInput::new(text)).expect("금액"),
                Currency::Usd,
            );
            assert_eq!(
                UsdOrderAmount::try_new(money).expect("양수 달러").money(),
                money
            );
            assert_eq!(
                UsdOrderAmount::try_new(Money::new(money.amount(), Currency::Krw)),
                Err(OrderValueError::CurrencyMismatch)
            );
        }
        for text in ["0", "-1", "-0.00001"] {
            assert_eq!(
                UsdOrderAmount::try_new(Money::new(
                    Amount::parse(&AmountInput::new(text)).expect("금액"),
                    Currency::Usd
                )),
                Err(OrderValueError::NonPositive)
            );
        }
        assert_eq!(
            UsdOrderAmount::try_new(Money::new(
                Amount::parse(&AmountInput::new("1.0000000000000000000000000001")).expect("금액"),
                Currency::Usd
            ))
            .expect("30자")
            .money()
            .currency(),
            Currency::Usd
        );
    }
}
