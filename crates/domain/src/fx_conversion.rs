use crate::{FxRate, Money, MoneyError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValuationAccuracy {
    Approximate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FxConversionError {
    CurrencyMismatch,
    MidRequired,
    Arithmetic(MoneyError),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConvertedMoney {
    money: Money,
    rate: FxRate,
    accuracy: ValuationAccuracy,
}

impl ConvertedMoney {
    pub(crate) const fn from_conversion(money: Money, rate: FxRate) -> Self {
        Self {
            money,
            rate,
            accuracy: ValuationAccuracy::Approximate,
        }
    }

    #[must_use]
    pub const fn rate(self) -> FxRate {
        self.rate
    }
    #[must_use]
    pub const fn accuracy(self) -> ValuationAccuracy {
        self.accuracy
    }

    #[must_use]
    pub const fn into_parts(self) -> (Money, FxRate, ValuationAccuracy) {
        (self.money, self.rate, self.accuracy())
    }

    pub fn checked_add_krw(self, money: Money) -> Result<Self, FxConversionError> {
        self.money
            .checked_add(&money)
            .map(|sum| Self::from_conversion(sum, self.rate))
            .map_err(FxConversionError::Arithmetic)
    }
}
