use crate::{
    CalendarDate, CreditDetail, KnownMarket, MarketDataError, MarketObservation, OffsetSeconds,
    SupplyRate, Symbol, Timestamp,
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
    market: KnownMarket,
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
        validate_supply_date(date, updated)?;
        Ok(Self {
            market: KnownMarket::Kr,
            symbol,
            date,
            observation: MarketObservation::from_response(Some(updated), collected)?,
        })
    }
    #[must_use]
    pub const fn market(&self) -> KnownMarket {
        self.market
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
    market: KnownMarket,
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
        validate_supply_date(date, collected)?;
        Ok(Self {
            market: KnownMarket::Kr,
            symbol,
            date,
            observation: MarketObservation::collected_only(collected),
        })
    }
    #[must_use]
    pub const fn market(&self) -> KnownMarket {
        self.market
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

fn validate_supply_date(date: CalendarDate, timestamp: Timestamp) -> Result<(), MarketDataError> {
    let market_date = timestamp
        .date_at_offset(OffsetSeconds::new(32_400))
        .or(Err(MarketDataError::InvalidFlow))?;
    if date > market_date {
        return Err(MarketDataError::InvalidFlow);
    }
    Ok(())
}
