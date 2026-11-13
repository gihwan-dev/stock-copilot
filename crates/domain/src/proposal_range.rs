use crate::{CheckResult, Price, Ratio};
use rust_decimal::Decimal;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProposalRangeError {
    NonPositive,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProposalVolatility(Ratio);
impl ProposalVolatility {
    pub fn try_new(value: Ratio) -> Result<Self, ProposalRangeError> {
        if value.as_decimal() <= Decimal::ZERO {
            return Err(ProposalRangeError::NonPositive);
        }
        Ok(Self(value))
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TradingDayHorizon(Ratio);
impl TradingDayHorizon {
    pub fn try_new(value: Ratio) -> Result<Self, ProposalRangeError> {
        if value.as_decimal() <= Decimal::ZERO {
            return Err(ProposalRangeError::NonPositive);
        }
        Ok(Self(value))
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProposalPriceRange {
    card_reference: Price,
    estimate_reference: Price,
    volatility: ProposalVolatility,
    horizon: TradingDayHorizon,
}
impl ProposalPriceRange {
    #[must_use]
    pub fn new(
        card_reference: Price,
        estimate_reference: Price,
        volatility: ProposalVolatility,
        horizon: TradingDayHorizon,
    ) -> Self {
        Self {
            card_reference,
            estimate_reference,
            volatility,
            horizon,
        }
    }
    #[must_use]
    pub fn check(&self, current: Price) -> CheckResult {
        let card = crate::exact_arithmetic::proposal_price_range(
            self.card_reference,
            current,
            self.volatility.0,
            self.horizon.0,
        );
        let estimate = crate::exact_arithmetic::proposal_price_range(
            self.estimate_reference,
            current,
            self.volatility.0,
            self.horizon.0,
        );
        if card == CheckResult::Passed && estimate == CheckResult::Passed {
            CheckResult::Passed
        } else {
            CheckResult::Failed
        }
    }
    pub(crate) fn currency(&self) -> Result<crate::Currency, crate::ProposalError> {
        let currency = self.card_reference.as_money().currency();
        if currency != self.estimate_reference.as_money().currency() {
            return Err(crate::ProposalError::CurrencyMismatch);
        }
        Ok(currency)
    }
}
