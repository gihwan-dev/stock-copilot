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
        _: &QuoteInstrument,
        _: CalendarDate,
        _: CalendarDate,
    ) -> impl Future<Output = Result<Vec<ProgramTrade>, PortError>> + Send {
        std::future::ready(unsupported(ReadEndpoint::ProgramTrades))
    }
    fn short_selling(
        &self,
        _: &QuoteInstrument,
        _: CalendarDate,
        _: CalendarDate,
    ) -> impl Future<Output = Result<Vec<ShortSelling>, PortError>> + Send {
        std::future::ready(unsupported(ReadEndpoint::ShortSelling))
    }
    fn credit_trades(
        &self,
        _: &QuoteInstrument,
        _: CalendarDate,
        _: CalendarDate,
    ) -> impl Future<Output = Result<Vec<CreditTrade>, PortError>> + Send {
        std::future::ready(unsupported(ReadEndpoint::CreditTrades))
    }
    fn securities_lending(
        &self,
        _: &QuoteInstrument,
        _: CalendarDate,
        _: CalendarDate,
    ) -> impl Future<Output = Result<Vec<SecuritiesLending>, PortError>> + Send {
        std::future::ready(unsupported(ReadEndpoint::SecuritiesLending))
    }
    fn investor_trading_rankings(
        &self,
        _: RankingLimit,
    ) -> impl Future<Output = Result<InvestorTradingRankings, PortError>> + Send {
        std::future::ready(unsupported(ReadEndpoint::InvestorTradingRankings))
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
        _: &SectorRankingQuery,
    ) -> impl Future<Output = Result<SectorRanking, PortError>> + Send {
        std::future::ready(unsupported(ReadEndpoint::SectorRankings))
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
        _: &RankingQuery,
    ) -> impl Future<Output = Result<StockRanking, PortError>> + Send {
        std::future::ready(unsupported(ReadEndpoint::Rankings))
    }
    fn stock_investor_flow(
        &self,
        _: &QuoteInstrument,
        _: CalendarDate,
        _: CalendarDate,
    ) -> impl Future<Output = Result<Vec<InvestorFlow>, PortError>> + Send {
        std::future::ready(unsupported(ReadEndpoint::StockInvestorFlow))
    }
    fn indicator_investor_flow(
        &self,
        _: IndicatorCode,
        _: CalendarDate,
        _: CalendarDate,
        _: FlowInterval,
    ) -> impl Future<Output = Result<Vec<InvestorFlow>, PortError>> + Send {
        std::future::ready(unsupported(ReadEndpoint::IndicatorInvestorFlow))
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
