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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProposalPriceRange {
    facts: crate::EstimateRangeFacts,
    card: crate::PriceObservation,
}
impl ProposalPriceRange {
    pub fn from_estimate_task(
        facts: crate::EstimateRangeFacts,
        card: crate::PriceObservation,
    ) -> Result<Self, crate::ProposalError> {
        if facts.instrument() != card.instrument() {
            return Err(crate::ProposalError::InstrumentMismatch);
        }
        if facts.reference().as_money().currency() != card.price().as_money().currency() {
            return Err(crate::ProposalError::CurrencyMismatch);
        }
        Ok(Self { facts, card })
    }
    #[must_use]
    pub fn check(&self, current: Price) -> CheckResult {
        let card = crate::exact_arithmetic::proposal_price_range(
            self.card.price(),
            current,
            self.facts.volatility().0,
            self.facts.horizon().0,
        );
        let estimate = crate::exact_arithmetic::proposal_price_range(
            self.facts.reference(),
            current,
            self.facts.volatility().0,
            self.facts.horizon().0,
        );
        if card == CheckResult::Passed && estimate == CheckResult::Passed {
            CheckResult::Passed
        } else {
            CheckResult::Failed
        }
    }
    pub(crate) fn instrument(&self) -> &crate::ResolvedOrderInstrument {
        self.facts.instrument()
    }
    pub(crate) fn currency(&self) -> crate::Currency {
        self.facts.reference().as_money().currency()
    }
}
