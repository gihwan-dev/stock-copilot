use crate::{
    CheckResult, Nanosecond, Price, ProposalVolatility, ResolvedOrderInstrument, SpanSeconds,
    TimeError, TimeSpan, Timestamp, TradingDayHorizon,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PriceObservation {
    instrument: ResolvedOrderInstrument,
    instrument_id: Option<crate::InstrumentId>,
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
            instrument_id: None,
            price,
            observed_at,
            fresh_until,
        })
    }
    pub fn from_identified_quote_parts(
        instrument_id: crate::InstrumentId,
        instrument: ResolvedOrderInstrument,
        price: Price,
        market_local_at: Timestamp,
    ) -> Result<Self, TimeError> {
        let offset = market_local_at.offset_seconds();
        let allowed_offsets = if instrument.market() == crate::Market::Kr {
            [32_400_i32, 32_400_i32]
        } else {
            [-18_000_i32, -14_400_i32]
        };
        if !allowed_offsets.contains(offset.as_i32()) {
            return Err(TimeError::InvalidComponent);
        }
        let mut observation = Self::from_quote_parts(instrument, price, market_local_at)?;
        observation.instrument_id = Some(instrument_id);
        Ok(observation)
    }
    #[must_use]
    pub fn instrument_id(&self) -> Option<&crate::InstrumentId> {
        self.instrument_id.as_ref()
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
