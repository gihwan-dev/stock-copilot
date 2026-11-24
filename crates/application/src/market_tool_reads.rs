use std::future::Future;

use domain::{DisplayInstrumentName, KnownMarket, Money, Quantity, Ratio, Symbol, Timestamp};

use crate::{AllocationLabel, SystemPortError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueProvenance {
    Official,
    Approximate,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuoteToolView {
    pub symbol: Symbol,
    pub market: KnownMarket,
    pub price: Money,
    pub change_rate: Option<Ratio>,
    pub collected_at: Timestamp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CandleWindow(u16);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CandleWindowError {
    OutOfRange,
}

impl CandleWindow {
    pub const MAX: u16 = 250_u16;

    pub const fn try_new(days: u16) -> Result<Self, CandleWindowError> {
        if days == 0_u16 || days > Self::MAX {
            return Err(CandleWindowError::OutOfRange);
        }
        Ok(Self(days))
    }

    #[must_use]
    pub const fn days(self) -> u16 {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CandleToolLine {
    pub opened_at: Timestamp,
    pub open: Money,
    pub high: Money,
    pub low: Money,
    pub close: Money,
    pub volume: Option<Quantity>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CandlesToolView {
    pub symbol: Symbol,
    pub market: KnownMarket,
    pub collected_at: Timestamp,
    pub lines: Vec<CandleToolLine>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum OverviewItem {
    Kospi,
    Kosdaq,
    KrBond3Y,
    KrBond10Y,
    UsdKrw,
    Sp500,
    Nasdaq100,
    UsTreasury2Y,
    UsTreasury10Y,
    CrudeOil,
    Gold,
    DollarIndex,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverviewUnit {
    IndexPoints,
    YieldPercent,
    KrwPerUsd,
    UsdPrice,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverviewToolLine {
    pub item: OverviewItem,
    pub unit: OverviewUnit,
    pub value: Ratio,
    pub change_rate: Option<Ratio>,
    pub provenance: ValueProvenance,
    pub collected_at: Timestamp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionStatus {
    PreMarket,
    Open,
    AfterHours,
    Closed,
    Holiday,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionToolLine {
    pub market: KnownMarket,
    pub status: SessionStatus,
    pub collected_at: Timestamp,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarketOverviewToolView {
    pub lines: Vec<OverviewToolLine>,
    pub sessions: Vec<SessionToolLine>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RankingToolKind {
    TradingAmount,
    Gainers,
    Losers,
    ForeignNetBuy,
    InstitutionNetBuy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RankingToolLine {
    pub symbol: Symbol,
    pub name: DisplayInstrumentName,
    pub price: Option<Money>,
    pub change_rate: Option<Ratio>,
    pub metric: Option<Money>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RankingsToolView {
    pub kind: RankingToolKind,
    pub market: KnownMarket,
    pub collected_at: Timestamp,
    pub lines: Vec<RankingToolLine>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SectorToolLine {
    pub label: AllocationLabel,
    pub change_rate: Option<Ratio>,
    pub trading_amount: Option<Money>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SectorsToolView {
    pub market: KnownMarket,
    pub collected_at: Timestamp,
    pub lines: Vec<SectorToolLine>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvestorFlowToolLine {
    pub trading_date: Timestamp,
    pub foreign_net: Option<Money>,
    pub institution_net: Option<Money>,
    pub individual_net: Option<Money>,
    pub short_selling_ratio: Option<Ratio>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvestorFlowToolView {
    pub symbol: Symbol,
    pub market: KnownMarket,
    pub collected_at: Timestamp,
    pub lines: Vec<InvestorFlowToolLine>,
}

pub trait MarketToolReads: Send + Sync {
    fn quote(
        &self,
        symbol: &Symbol,
        market: KnownMarket,
    ) -> impl Future<Output = Result<Option<QuoteToolView>, SystemPortError>> + Send;

    fn candles(
        &self,
        symbol: &Symbol,
        market: KnownMarket,
        window: CandleWindow,
    ) -> impl Future<Output = Result<Option<CandlesToolView>, SystemPortError>> + Send;

    fn market_overview(
        &self,
    ) -> impl Future<Output = Result<Option<MarketOverviewToolView>, SystemPortError>> + Send;

    fn rankings(
        &self,
        kind: RankingToolKind,
        market: KnownMarket,
    ) -> impl Future<Output = Result<Option<RankingsToolView>, SystemPortError>> + Send;

    fn sectors(
        &self,
        market: KnownMarket,
    ) -> impl Future<Output = Result<Option<SectorsToolView>, SystemPortError>> + Send;

    fn investor_flow(
        &self,
        symbol: &Symbol,
        market: KnownMarket,
    ) -> impl Future<Output = Result<Option<InvestorFlowToolView>, SystemPortError>> + Send;
}
