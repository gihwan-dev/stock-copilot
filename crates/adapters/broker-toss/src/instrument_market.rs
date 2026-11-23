use crate::{TokenRenewal, TossInstrumentRead};
use application::{
    Clock, InstrumentIdIssuer, InstrumentMasterAuthority, MarketInfo, PortError, PortErrorKind,
    RankingQuery, SecretStore, SectorRankingQuery,
};
use domain::{
    CalendarDate, CandlePeriod, CreditTrade, FlowInterval, FxPair, FxRate, FxRateKind,
    IndicatorCode, InvestorFlow, InvestorTradingRankings, KnownMarket, MarketCalendar,
    MarketIndicatorCandle, MarketIndicatorQuote, OrderFxRates, ProgramTrade, QuoteInstrument,
    RankingLimit, SectorCatalog, SectorComposition, SectorDetail, SectorEtfs, SectorId,
    SectorRanking, SecuritiesLending, ShortSelling, StockRanking, TimeInterval, Timestamp,
};

fn master_error(error: PortError) -> PortError {
    if error.kind() == PortErrorKind::Unsupported(application::BrokerCapability::Instruments) {
        PortError::new(
            error.endpoint(),
            error.status(),
            PortErrorKind::MasterNotReady(application::MasterNotReady::MissingInstrument),
        )
    } else {
        error
    }
}

impl<S: SecretStore, T: Clock, R: TokenRenewal, A: InstrumentMasterAuthority, I: InstrumentIdIssuer>
    MarketInfo for TossInstrumentRead<S, T, R, A, I>
{
    async fn fx_rate(
        &self,
        pair: FxPair,
        kind: FxRateKind,
        at: Option<Timestamp>,
    ) -> Result<FxRate, PortError> {
        self.reader.fx_rate(pair, kind, at).await
    }
    async fn order_fx_rates(&self) -> Result<OrderFxRates, PortError> {
        self.reader.order_fx_rates().await
    }
    async fn program_trades(
        &self,
        instrument: &QuoteInstrument,
        from: CalendarDate,
        through: CalendarDate,
    ) -> Result<Vec<ProgramTrade>, PortError> {
        self.reader.program_trades(instrument, from, through).await
    }
    async fn short_selling(
        &self,
        instrument: &QuoteInstrument,
        from: CalendarDate,
        through: CalendarDate,
    ) -> Result<Vec<ShortSelling>, PortError> {
        self.reader.short_selling(instrument, from, through).await
    }
    async fn credit_trades(
        &self,
        instrument: &QuoteInstrument,
        from: CalendarDate,
        through: CalendarDate,
    ) -> Result<Vec<CreditTrade>, PortError> {
        self.reader.credit_trades(instrument, from, through).await
    }
    async fn securities_lending(
        &self,
        instrument: &QuoteInstrument,
        from: CalendarDate,
        through: CalendarDate,
    ) -> Result<Vec<SecuritiesLending>, PortError> {
        self.reader
            .securities_lending(instrument, from, through)
            .await
    }
    async fn investor_trading_rankings(
        &self,
        limit: RankingLimit,
    ) -> Result<InvestorTradingRankings, PortError> {
        self.reader.investor_trading_rankings(limit).await
    }
    async fn sectors(&self) -> Result<SectorCatalog, PortError> {
        self.reader.sectors().await
    }
    async fn sector(
        &self,
        sector: &SectorId,
        market: KnownMarket,
    ) -> Result<SectorDetail, PortError> {
        self.reader.sector(sector, market).await
    }
    async fn sector_rankings(
        &self,
        query: &SectorRankingQuery,
    ) -> Result<SectorRanking, PortError> {
        self.reader.sector_rankings(query).await
    }
    async fn sector_etfs(
        &self,
        sector: &SectorId,
        market: KnownMarket,
    ) -> Result<SectorEtfs, PortError> {
        self.reader
            .sector_etfs(sector, market)
            .await
            .map_err(master_error)
    }
    async fn indicator_quotes(
        &self,
        codes: &[IndicatorCode],
    ) -> Result<Vec<MarketIndicatorQuote>, PortError> {
        self.reader.indicator_quotes(codes).await
    }
    async fn indicator_candles(
        &self,
        code: IndicatorCode,
        window: TimeInterval,
        period: CandlePeriod,
    ) -> Result<Vec<MarketIndicatorCandle>, PortError> {
        self.reader.indicator_candles(code, window, period).await
    }
    async fn rankings(&self, query: &RankingQuery) -> Result<StockRanking, PortError> {
        self.reader.rankings(query).await
    }
    async fn stock_investor_flow(
        &self,
        instrument: &QuoteInstrument,
        from: CalendarDate,
        through: CalendarDate,
    ) -> Result<Vec<InvestorFlow>, PortError> {
        self.reader
            .stock_investor_flow(instrument, from, through)
            .await
    }
    async fn indicator_investor_flow(
        &self,
        code: IndicatorCode,
        from: CalendarDate,
        through: CalendarDate,
        interval: FlowInterval,
    ) -> Result<Vec<InvestorFlow>, PortError> {
        self.reader
            .indicator_investor_flow(code, from, through, interval)
            .await
    }
    async fn sector_composition(
        &self,
        sector: &SectorId,
        market: KnownMarket,
    ) -> Result<SectorComposition, PortError> {
        self.reader
            .sector_composition(sector, market)
            .await
            .map_err(master_error)
    }
    async fn calendar(&self, market: KnownMarket) -> Result<MarketCalendar, PortError> {
        self.reader.calendar(market).await
    }
}
