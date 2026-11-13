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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Money {
    amount: Amount,
    currency: Currency,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoneyError {
    CurrencyMismatch,
    Overflow,
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
            .0
            .checked_add(other.amount.0)
            .map(|value| Self::new(Amount::new(value), self.currency))
            .ok_or(MoneyError::Overflow)
    }

    pub fn checked_sub(&self, other: &Self) -> Result<Self, MoneyError> {
        if self.currency != other.currency {
            return Err(MoneyError::CurrencyMismatch);
        }
        self.amount
            .0
            .checked_sub(other.amount.0)
            .map(|value| Self::new(Amount::new(value), self.currency))
            .ok_or(MoneyError::Overflow)
    }
}

#[cfg(test)]
mod tests {
    use proptest::test_runner::TestRunner;
    use rust_decimal::Decimal;

    use super::{Amount, Currency, Money, MoneyError};

    #[test]
    fn different_currencies_cannot_be_added_or_subtracted() {
        let krw = Money::new(Amount::new(Decimal::ONE), Currency::Krw);
        let usd = Money::new(Amount::new(Decimal::ONE), Currency::Usd);
        assert_eq!(krw.checked_add(&usd), Err(MoneyError::CurrencyMismatch));
        assert_eq!(krw.checked_sub(&usd), Err(MoneyError::CurrencyMismatch));
    }

    #[test]
    fn same_currency_arithmetic_preserves_fractional_amounts() {
        let left = Money::new(
            Amount::new(Decimal::from_str_exact("1.23456789").expect("valid decimal")),
            Currency::Krw,
        );
        let right = Money::new(
            Amount::new(Decimal::from_str_exact("0.00000001").expect("valid decimal")),
            Currency::Krw,
        );
        assert_eq!(
            left.checked_add(&right)
                .expect("representable")
                .amount()
                .as_decimal(),
            Decimal::from_str_exact("1.23456790").expect("valid decimal")
        );
        assert_eq!(
            left.checked_sub(&left)
                .expect("representable")
                .amount()
                .as_decimal(),
            Decimal::ZERO
        );
        assert_eq!(left.currency(), Currency::Krw);
    }

    #[test]
    fn checked_arithmetic_reports_overflow_for_every_positive_increment() {
        let mut runner = TestRunner::default();
        runner
            .run(&(1_u64..=u64::MAX), |increment| {
                let positive = Money::new(Amount::new(Decimal::from(increment)), Currency::Usd);
                let maximum = Money::new(Amount::new(Decimal::MAX), Currency::Usd);
                let minimum = Money::new(Amount::new(Decimal::MIN), Currency::Usd);
                proptest::prop_assert_eq!(
                    maximum.checked_add(&positive),
                    Err(MoneyError::Overflow)
                );
                proptest::prop_assert_eq!(
                    minimum.checked_sub(&positive),
                    Err(MoneyError::Overflow)
                );
                Ok(())
            })
            .expect("all increments overflow");
    }
}
