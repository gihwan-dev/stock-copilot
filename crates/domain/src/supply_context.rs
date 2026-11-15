use crate::{
    CalendarDate, CreditDetail, KnownMarket, MarketDataError, MarketObservation, SupplyRate,
    Symbol, Timestamp,
};
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SupplyRatio {
    MissingDenominator,
    Reported(SupplyRate),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CreditSide {
    NoData,
    Reported(CreditDetail),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SupplyContext {
    symbol: Symbol,
    date: CalendarDate,
    observation: MarketObservation,
}
impl SupplyContext {
    pub fn from_response(
        symbol: Symbol,
        date: CalendarDate,
        updated: Timestamp,
        collected: Timestamp,
    ) -> Result<Self, MarketDataError> {
        crate::market_ranking::validate_market_symbol(&symbol, KnownMarket::Kr)?;
        if date > updated.offset_calendar_date() {
            return Err(MarketDataError::InvalidFlow);
        }
        Ok(Self {
            symbol,
            date,
            observation: MarketObservation::from_response(Some(updated), collected)?,
        })
    }
    #[must_use]
    pub const fn symbol(&self) -> &Symbol {
        &self.symbol
    }
    #[must_use]
    pub const fn date(&self) -> CalendarDate {
        self.date
    }
    #[must_use]
    pub const fn observation(&self) -> MarketObservation {
        self.observation
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramContext {
    symbol: Symbol,
    date: CalendarDate,
    observation: MarketObservation,
}
impl ProgramContext {
    pub fn from_response(
        symbol: Symbol,
        date: CalendarDate,
        collected: Timestamp,
    ) -> Result<Self, MarketDataError> {
        crate::market_ranking::validate_market_symbol(&symbol, KnownMarket::Kr)?;
        if date > collected.offset_calendar_date() {
            return Err(MarketDataError::InvalidFlow);
        }
        Ok(Self {
            symbol,
            date,
            observation: MarketObservation::collected_only(collected),
        })
    }
    #[must_use]
    pub const fn symbol(&self) -> &Symbol {
        &self.symbol
    }
    #[must_use]
    pub const fn date(&self) -> CalendarDate {
        self.date
    }
    #[must_use]
    pub const fn observation(&self) -> MarketObservation {
        self.observation
    }
}
