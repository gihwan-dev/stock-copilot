use crate::{TokenRenewal, TossAccountRead};
use application::{
    Clock, MarketInfo, PortError, RankingQuery, ReadEndpoint, RequestRejection, SecretStore,
    SectorRankingQuery,
};
use domain::{
    CalendarDate, CandlePeriod, CreditTrade, FlowInterval, FxPair, FxRate, FxRateKind,
    IndicatorCode, InvestorFlow, InvestorTradingRankings, KnownMarket, MarketCalendar,
    MarketIndicatorCandle, MarketIndicatorQuote, OrderFxRates, ProgramTrade, QuoteInstrument,
    RankingLimit, SectorCatalog, SectorComposition, SectorDetail, SectorEtfs, SectorId,
    SectorRanking, SecuritiesLending, ShortSelling, StockRanking, TimeInterval, Timestamp,
};

fn unsupported<T>(endpoint: ReadEndpoint) -> Result<T, PortError> {
    Err(crate::market_read::rejected(
        endpoint,
        RequestRejection::UnsupportedQuery,
    ))
}
impl<S: SecretStore, T: Clock, R: TokenRenewal> MarketInfo for TossAccountRead<S, T, R> {
    async fn fx_rate(
        &self,
        pair: FxPair,
        kind: FxRateKind,
        at: Option<Timestamp>,
    ) -> Result<FxRate, PortError> {
        let (buy, mid) = crate::market_fx::fetch(self, pair, at, ReadEndpoint::FxRate).await?;
        Ok(match kind {
            FxRateKind::Buy => buy,
            FxRateKind::Mid => mid,
        })
    }
    async fn order_fx_rates(&self) -> Result<OrderFxRates, PortError> {
        let (buy, mid) =
            crate::market_fx::fetch(self, FxPair::UsdKrw, None, ReadEndpoint::OrderFxRates).await?;
        Ok(OrderFxRates::from_broker_fx_result(buy, mid))
    }
    fn program_trades(
        &self,
        instrument: &QuoteInstrument,
        from: CalendarDate,
        through: CalendarDate,
    ) -> impl Future<Output = Result<Vec<ProgramTrade>, PortError>> + Send {
        crate::market_supply::programs(self, instrument, from, through)
    }
    fn short_selling(
        &self,
        instrument: &QuoteInstrument,
        from: CalendarDate,
        through: CalendarDate,
    ) -> impl Future<Output = Result<Vec<ShortSelling>, PortError>> + Send {
        crate::market_supply::shorts(self, instrument, from, through)
    }
    fn credit_trades(
        &self,
        instrument: &QuoteInstrument,
        from: CalendarDate,
        through: CalendarDate,
    ) -> impl Future<Output = Result<Vec<CreditTrade>, PortError>> + Send {
        crate::market_supply::credits(self, instrument, from, through)
    }
    fn securities_lending(
        &self,
        instrument: &QuoteInstrument,
        from: CalendarDate,
        through: CalendarDate,
    ) -> impl Future<Output = Result<Vec<SecuritiesLending>, PortError>> + Send {
        crate::market_supply::loans(self, instrument, from, through)
    }
    fn investor_trading_rankings(
        &self,
        limit: RankingLimit,
    ) -> impl Future<Output = Result<InvestorTradingRankings, PortError>> + Send {
        crate::market_investor_rankings::fetch(self, limit)
    }
    fn sectors(&self) -> impl Future<Output = Result<SectorCatalog, PortError>> + Send {
        crate::market_sector::sectors(self)
    }
    fn sector(
        &self,
        sector: &SectorId,
        market: KnownMarket,
    ) -> impl Future<Output = Result<SectorDetail, PortError>> + Send {
        crate::market_sector::detail(self, sector, market)
    }
    fn sector_rankings(
        &self,
        query: &SectorRankingQuery,
    ) -> impl Future<Output = Result<SectorRanking, PortError>> + Send {
        crate::market_sector_rankings::fetch(self, query)
    }
    fn sector_etfs(
        &self,
        sector: &SectorId,
        market: KnownMarket,
    ) -> impl Future<Output = Result<SectorEtfs, PortError>> + Send {
        crate::market_sector::etfs(self, sector, market)
    }
    async fn indicator_quotes(
        &self,
        codes: &[IndicatorCode],
    ) -> Result<Vec<MarketIndicatorQuote>, PortError> {
        crate::market_indicators::quotes(self, codes).await
    }
    fn indicator_candles(
        &self,
        _: IndicatorCode,
        _: TimeInterval,
        _: CandlePeriod,
    ) -> impl Future<Output = Result<Vec<MarketIndicatorCandle>, PortError>> + Send {
        std::future::ready(unsupported(ReadEndpoint::IndicatorCandles))
    }
    fn rankings(
        &self,
        query: &RankingQuery,
    ) -> impl Future<Output = Result<StockRanking, PortError>> + Send {
        crate::market_rankings::rankings(self, query)
    }
    fn stock_investor_flow(
        &self,
        instrument: &QuoteInstrument,
        from: CalendarDate,
        through: CalendarDate,
    ) -> impl Future<Output = Result<Vec<InvestorFlow>, PortError>> + Send {
        crate::market_flow::stocks(self, instrument, from, through)
    }
    fn indicator_investor_flow(
        &self,
        code: IndicatorCode,
        from: CalendarDate,
        through: CalendarDate,
        interval: FlowInterval,
    ) -> impl Future<Output = Result<Vec<InvestorFlow>, PortError>> + Send {
        crate::market_flow::indices(self, code, from, through, interval)
    }
    fn sector_composition(
        &self,
        sector: &SectorId,
        market: KnownMarket,
    ) -> impl Future<Output = Result<SectorComposition, PortError>> + Send {
        crate::market_sector::composition(self, sector, market)
    }
    async fn calendar(&self, market: KnownMarket) -> Result<MarketCalendar, PortError> {
        crate::market_calendar::fetch(self, market).await
    }
}
