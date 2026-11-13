use crate::{
    CheckResult, Nanosecond, Price, ProposalVolatility, ResolvedOrderInstrument, SpanSeconds,
    TimeError, TimeSpan, Timestamp, TradingDayHorizon,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PriceObservation {
    instrument: ResolvedOrderInstrument,
    price: Price,
    observed_at: Timestamp,
    fresh_until: Timestamp,
}
impl PriceObservation {
    pub fn from_quote_parts(
        instrument: ResolvedOrderInstrument,
        price: Price,
        observed_at: Timestamp,
    ) -> Result<Self, TimeError> {
        let fresh_until = TimeSpan::try_new(SpanSeconds::new(5), Nanosecond::new(0))
            .and_then(|span| observed_at.checked_add(span))?;
        Ok(Self {
            instrument,
            price,
            observed_at,
            fresh_until,
        })
    }
    #[must_use]
    pub fn instrument(&self) -> &ResolvedOrderInstrument {
        &self.instrument
    }
    #[must_use]
    pub fn price(&self) -> Price {
        self.price
    }
    #[must_use]
    pub fn observed_at(&self) -> Timestamp {
        self.observed_at
    }
    pub(crate) fn check(
        &self,
        instrument: &ResolvedOrderInstrument,
        now: Timestamp,
    ) -> CheckResult {
        if &self.instrument != instrument || self.observed_at > now || now >= self.fresh_until {
            CheckResult::Failed
        } else {
            CheckResult::Passed
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EstimateRangeFacts {
    pub(crate) instrument: ResolvedOrderInstrument,
    pub(crate) reference: Price,
    pub(crate) volatility: ProposalVolatility,
    pub(crate) horizon: TradingDayHorizon,
}
impl EstimateRangeFacts {
    pub(crate) fn instrument(&self) -> &ResolvedOrderInstrument {
        &self.instrument
    }
    pub(crate) fn reference(&self) -> Price {
        self.reference
    }
    pub(crate) fn volatility(&self) -> ProposalVolatility {
        self.volatility
    }
    pub(crate) fn horizon(&self) -> TradingDayHorizon {
        self.horizon
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProposalLifetime(SpanSeconds);
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProposalLifetimeError {
    OutOfRange,
}
impl Default for ProposalLifetime {
    fn default() -> Self {
        Self(SpanSeconds::new(600))
    }
}
impl ProposalLifetime {
    pub fn try_new(value: SpanSeconds) -> Result<Self, ProposalLifetimeError> {
        if !(1..=600).contains(value.as_u64()) {
            return Err(ProposalLifetimeError::OutOfRange);
        }
        Ok(Self(value))
    }
    pub(crate) fn seconds(self) -> SpanSeconds {
        self.0
    }
}
