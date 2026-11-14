use domain::{
    AccountDisplayFact, AccountId, BuyingPower, CalendarDate, Candle, CandlePeriod,
    CompleteTransactionResponse, Currency, FxPair, FxRate, FxRateKind, InstrumentId, Market,
    MarketCalendar, OrderFxRates, Portfolio, PriceAdjustment, Quote, ResolvedOrderInstrument,
    SettlementWindow, TimeInterval, Timestamp, TransactionFilter,
};
use std::{collections::BTreeSet, future::Future};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortError {
    AccessDenied,
    Unavailable,
    InvalidResponse,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum BrokerCapability {
    KrLimit,
    KrMarket,
    UsLimit,
    UsMarketBuy,
    UsMarketSell,
    UsMarketAmountBuy,
    UsFractionalMarketSell,
    DailyCandles,
    MinuteCandles,
    AdjustedCandles,
    RawCandles,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrokerCapabilities(BTreeSet<BrokerCapability>);

impl BrokerCapabilities {
    #[must_use]
    pub const fn new(supported: BTreeSet<BrokerCapability>) -> Self {
        Self(supported)
    }

    #[must_use]
    pub const fn supported(&self) -> &BTreeSet<BrokerCapability> {
        &self.0
    }
}

pub trait BrokerRead: Send + Sync {
    fn accounts(&self) -> impl Future<Output = Result<Vec<AccountId>, PortError>> + Send;
    fn account_display(
        &self,
        account: &AccountId,
    ) -> impl Future<Output = Result<AccountDisplayFact, PortError>> + Send;
    fn portfolio(
        &self,
        account: &AccountId,
    ) -> impl Future<Output = Result<Portfolio, PortError>> + Send;
    fn transactions(
        &self,
        account: &AccountId,
        window: SettlementWindow,
        filter: &TransactionFilter,
    ) -> impl Future<Output = Result<CompleteTransactionResponse, PortError>> + Send;
    fn quote(
        &self,
        instrument: &ResolvedOrderInstrument,
    ) -> impl Future<Output = Result<Quote, PortError>> + Send;
    fn candles(
        &self,
        instrument: (&ResolvedOrderInstrument, &InstrumentId),
        window: TimeInterval,
        period: CandlePeriod,
        adjustment: PriceAdjustment,
    ) -> impl Future<Output = Result<Vec<Candle>, PortError>> + Send;
    fn buying_power(
        &self,
        account: &AccountId,
        currency: Currency,
    ) -> impl Future<Output = Result<BuyingPower, PortError>> + Send;
}

pub trait MarketInfo: Send + Sync {
    fn calendar(
        &self,
        market: &Market,
        date: CalendarDate,
    ) -> impl Future<Output = Result<MarketCalendar, PortError>> + Send;
    fn fx_rate(
        &self,
        pair: FxPair,
        kind: FxRateKind,
        at: Option<Timestamp>,
    ) -> impl Future<Output = Result<FxRate, PortError>> + Send;
    fn order_fx_rates(&self) -> impl Future<Output = Result<OrderFxRates, PortError>> + Send;
}
