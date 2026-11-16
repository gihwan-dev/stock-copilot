use crate::{Script, script::State};
use application::{
    BrokerRead, CollectedObservation, DataAvailability, FeedCursor, FeedSource, FinancialStatement,
    Fundamentals, FxRateQuery, HoldingsObservation, InformationError, MacroObservation,
    MacroSeries, MacroSeriesId, MarketInfo, NewsFeed, NewsMetadata, Notification, Notifier,
    PortError, QuoteObservation, RankingQuery, SectorRankingQuery, SystemPortError,
    TransactionRead, WriteCompleted,
};
use domain::{
    AccountId, BrokerAccountRows, BrokerOrder, BrokerOrderId, BrokerOrderSet, BuyingPower,
    CalendarDate, Calibration, CalibrationKey, Candle, CandlePeriod, ClosedOrderSet,
    CommissionRateFact, CreditTrade, Currency, EstimateOutcome, FlowInterval, FxPair, FxRate,
    FxRateKind, IndicatorCode, InstrumentId, InvestorFlow, InvestorProfile,
    InvestorTradingRankings, KnownMarket, MarketCalendar, MarketIndicatorCandle,
    MarketIndicatorQuote, MasterInstrumentFacts, NewsItem, OrderFxRates, Orderbook, Portfolio,
    PriceAdjustment, PriceLimits, ProgramTrade, ProposalId, Quote, QuoteInstrument, RaisedSignal,
    RankingLimit, ResolvedOrderInstrument, RiskSettings, SectorCatalog, SectorComposition,
    SectorDetail, SectorEtfs, SectorId, SectorRanking, SecuritiesLending, SellableQuantityFact,
    SettledRowSet, SettlementWindow, ShortSelling, SignalSubmission, SignalTaskKey, StockRanking,
    StockWarnings, Symbol, SyncRecord, TimeInterval, Timestamp, TransactionFilter,
    ValuationSnapshot,
};
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BrokerReadCall {
    Accounts(),
    Holdings(AccountId),
    Transactions(AccountId, SettlementWindow, TransactionFilter),
    Quotes(Vec<QuoteInstrument>),
    Candles(QuoteInstrument, TimeInterval, CandlePeriod, PriceAdjustment),
    BuyingPower(AccountId, Currency),
    SellableQuantity(AccountId, ResolvedOrderInstrument),
    OpenOrders(AccountId),
    ClosedOrders(AccountId, TimeInterval),
    Order(AccountId, BrokerOrderId),
    StockWarnings(ResolvedOrderInstrument),
    Instruments(KnownMarket),
    Orderbook(QuoteInstrument),
    PriceLimits(QuoteInstrument),
    CommissionRate(AccountId, KnownMarket),
}
#[derive(Debug, Default)]
pub struct ScriptedBrokerRead {
    accounts: ScriptedBrokerReadAccountsScript,
    holdings: ScriptedBrokerReadHoldingsScript,
    transactions: ScriptedBrokerReadTransactionsScript,
    quotes: ScriptedBrokerReadQuotesScript,
    candles: ScriptedBrokerReadCandlesScript,
    buying_power: ScriptedBrokerReadBuyingPowerScript,
    sellable_quantity: ScriptedBrokerReadSellableQuantityScript,
    open_orders: ScriptedBrokerReadOpenOrdersScript,
    closed_orders: ScriptedBrokerReadClosedOrdersScript,
    order: ScriptedBrokerReadOrderScript,
    stock_warnings: ScriptedBrokerReadStockWarningsScript,
    instruments: ScriptedBrokerReadInstrumentsScript,
    orderbook: ScriptedBrokerReadOrderbookScript,
    price_limits: ScriptedBrokerReadPriceLimitsScript,
    commission_rate: ScriptedBrokerReadCommissionRateScript,
    calls: State<Vec<BrokerReadCall>>,
}
impl ScriptedBrokerRead {
    #[must_use]
    pub fn script_sellable_quantity(&self) -> &ScriptedBrokerReadSellableQuantityScript {
        &self.sellable_quantity
    }

    #[must_use]
    pub fn script_accounts(&self) -> &ScriptedBrokerReadAccountsScript {
        &self.accounts
    }
    #[must_use]
    pub fn script_holdings(&self) -> &ScriptedBrokerReadHoldingsScript {
        &self.holdings
    }
    #[must_use]
    pub fn script_transactions(&self) -> &ScriptedBrokerReadTransactionsScript {
        &self.transactions
    }
    #[must_use]
    pub fn script_quotes(&self) -> &ScriptedBrokerReadQuotesScript {
        &self.quotes
    }
    #[must_use]
    pub fn script_candles(&self) -> &ScriptedBrokerReadCandlesScript {
        &self.candles
    }
    #[must_use]
    pub fn script_buying_power(&self) -> &ScriptedBrokerReadBuyingPowerScript {
        &self.buying_power
    }
    #[must_use]
    pub fn script_open_orders(&self) -> &ScriptedBrokerReadOpenOrdersScript {
        &self.open_orders
    }
    #[must_use]
    pub fn script_closed_orders(&self) -> &ScriptedBrokerReadClosedOrdersScript {
        &self.closed_orders
    }
    #[must_use]
    pub fn script_order(&self) -> &ScriptedBrokerReadOrderScript {
        &self.order
    }
    #[must_use]
    pub fn script_stock_warnings(&self) -> &ScriptedBrokerReadStockWarningsScript {
        &self.stock_warnings
    }
    #[must_use]
    pub fn script_instruments(&self) -> &ScriptedBrokerReadInstrumentsScript {
        &self.instruments
    }
    #[must_use]
    pub fn script_orderbook(&self) -> &ScriptedBrokerReadOrderbookScript {
        &self.orderbook
    }
    #[must_use]
    pub fn script_price_limits(&self) -> &ScriptedBrokerReadPriceLimitsScript {
        &self.price_limits
    }
    #[must_use]
    pub fn script_commission_rate(&self) -> &ScriptedBrokerReadCommissionRateScript {
        &self.commission_rate
    }

    #[must_use]
    pub fn calls(&self) -> Vec<BrokerReadCall> {
        self.calls.write().clone()
    }
}
impl BrokerRead for ScriptedBrokerRead {
    async fn accounts(&self) -> Result<BrokerAccountRows, PortError> {
        self.calls.write().push(BrokerReadCall::Accounts());
        self.accounts.run(()).await
    }
    async fn holdings(&self, account: &AccountId) -> Result<HoldingsObservation, PortError> {
        self.calls
            .write()
            .push(BrokerReadCall::Holdings(account.clone()));
        self.holdings.run((account.clone(),)).await
    }
    async fn transactions(
        &self,
        account: &AccountId,
        window: SettlementWindow,
        filter: &TransactionFilter,
    ) -> Result<TransactionRead, PortError> {
        self.calls.write().push(BrokerReadCall::Transactions(
            account.clone(),
            window,
            filter.clone(),
        ));
        self.transactions
            .run((account.clone(), window, filter.clone()))
            .await
    }
    async fn quotes(
        &self,
        instruments: &[QuoteInstrument],
    ) -> Result<Vec<QuoteObservation>, PortError> {
        self.calls
            .write()
            .push(BrokerReadCall::Quotes(instruments.to_vec()));
        self.quotes.run((instruments.to_vec(),)).await
    }
    async fn candles(
        &self,
        instrument: &QuoteInstrument,
        window: TimeInterval,
        period: CandlePeriod,
        adjustment: PriceAdjustment,
    ) -> Result<Vec<Candle>, PortError> {
        self.calls.write().push(BrokerReadCall::Candles(
            instrument.clone(),
            window,
            period,
            adjustment,
        ));
        self.candles
            .run((instrument.clone(), window, period, adjustment))
            .await
    }
    async fn buying_power(
        &self,
        account: &AccountId,
        currency: Currency,
    ) -> Result<BuyingPower, PortError> {
        self.calls
            .write()
            .push(BrokerReadCall::BuyingPower(account.clone(), currency));
        self.buying_power.run((account.clone(), currency)).await
    }
    async fn sellable_quantity(
        &self,
        account: &AccountId,
        instrument: &ResolvedOrderInstrument,
    ) -> Result<SellableQuantityFact, PortError> {
        self.calls.write().push(BrokerReadCall::SellableQuantity(
            account.clone(),
            instrument.clone(),
        ));
        self.sellable_quantity
            .run((account.clone(), instrument.clone()))
            .await
    }
    async fn open_orders(&self, account: &AccountId) -> Result<BrokerOrderSet, PortError> {
        self.calls
            .write()
            .push(BrokerReadCall::OpenOrders(account.clone()));
        self.open_orders.run((account.clone(),)).await
    }
    async fn closed_orders(
        &self,
        account: &AccountId,
        evaluation_window: TimeInterval,
    ) -> Result<ClosedOrderSet, PortError> {
        self.calls.write().push(BrokerReadCall::ClosedOrders(
            account.clone(),
            evaluation_window,
        ));
        self.closed_orders
            .run((account.clone(), evaluation_window))
            .await
    }
    async fn order(
        &self,
        account: &AccountId,
        order_id: &BrokerOrderId,
    ) -> Result<BrokerOrder, PortError> {
        self.calls
            .write()
            .push(BrokerReadCall::Order(account.clone(), order_id.clone()));
        self.order.run((account.clone(), order_id.clone())).await
    }
    async fn stock_warnings(
        &self,
        instrument: &ResolvedOrderInstrument,
    ) -> Result<StockWarnings, PortError> {
        self.calls
            .write()
            .push(BrokerReadCall::StockWarnings(instrument.clone()));
        self.stock_warnings.run((instrument.clone(),)).await
    }
    async fn instruments(&self, market: KnownMarket) -> Result<MasterInstrumentFacts, PortError> {
        self.calls.write().push(BrokerReadCall::Instruments(market));
        self.instruments.run((market,)).await
    }
    async fn orderbook(&self, instrument: &QuoteInstrument) -> Result<Orderbook, PortError> {
        self.calls
            .write()
            .push(BrokerReadCall::Orderbook(instrument.clone()));
        self.orderbook.run((instrument.clone(),)).await
    }
    async fn price_limits(&self, instrument: &QuoteInstrument) -> Result<PriceLimits, PortError> {
        self.calls
            .write()
            .push(BrokerReadCall::PriceLimits(instrument.clone()));
        self.price_limits.run((instrument.clone(),)).await
    }
    async fn commission_rate(
        &self,
        account: &AccountId,
        market: KnownMarket,
    ) -> Result<CommissionRateFact, PortError> {
        self.calls
            .write()
            .push(BrokerReadCall::CommissionRate(account.clone(), market));
        self.commission_rate.run((account.clone(), market)).await
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MarketInfoCall {
    SecuritiesLending(QuoteInstrument, CalendarDate, CalendarDate),
    CreditTrades(QuoteInstrument, CalendarDate, CalendarDate),
    ShortSelling(QuoteInstrument, CalendarDate, CalendarDate),
    ProgramTrades(QuoteInstrument, CalendarDate, CalendarDate),
    InvestorTradingRankings(RankingLimit),
    Sectors(),
    Sector(SectorId, KnownMarket),
    SectorRankings(SectorRankingQuery),
    SectorEtfs(SectorId, KnownMarket),

    FxRate(FxPair, FxRateKind, Option<Timestamp>),
    OrderFxRates(),
    IndicatorQuotes(Vec<IndicatorCode>),
    IndicatorCandles(IndicatorCode, TimeInterval, CandlePeriod),
    Rankings(RankingQuery),
    StockInvestorFlow(QuoteInstrument, CalendarDate, CalendarDate),
    IndicatorInvestorFlow(IndicatorCode, CalendarDate, CalendarDate, FlowInterval),
    SectorComposition(SectorId, KnownMarket),
    Calendar(KnownMarket),
}
#[derive(Debug, Default)]
pub struct ScriptedMarketInfo {
    securities_lending: ScriptedMarketInfoSecuritiesLendingScript,
    credit_trades: ScriptedMarketInfoCreditTradesScript,
    short_selling: ScriptedMarketInfoShortSellingScript,
    program_trades: ScriptedMarketInfoProgramTradesScript,
    investor_trading_rankings: ScriptedMarketInfoInvestorTradingRankingsScript,
    sectors: ScriptedMarketInfoSectorsScript,
    sector: ScriptedMarketInfoSectorScript,
    sector_rankings: ScriptedMarketInfoSectorRankingsScript,
    sector_etfs: ScriptedMarketInfoSectorEtfsScript,

    fx_rate: ScriptedMarketInfoFxRateScript,
    order_fx_rates: ScriptedMarketInfoOrderFxRatesScript,
    indicator_quotes: ScriptedMarketInfoIndicatorQuotesScript,
    indicator_candles: ScriptedMarketInfoIndicatorCandlesScript,
    rankings: ScriptedMarketInfoRankingsScript,
    stock_investor_flow: ScriptedMarketInfoStockInvestorFlowScript,
    indicator_investor_flow: ScriptedMarketInfoIndicatorInvestorFlowScript,
    sector_composition: ScriptedMarketInfoSectorCompositionScript,
    calendar: ScriptedMarketInfoCalendarScript,
    calls: State<Vec<MarketInfoCall>>,
}
impl ScriptedMarketInfo {
    #[must_use]
    pub fn script_securities_lending(&self) -> &ScriptedMarketInfoSecuritiesLendingScript {
        &self.securities_lending
    }
    #[must_use]
    pub fn script_credit_trades(&self) -> &ScriptedMarketInfoCreditTradesScript {
        &self.credit_trades
    }
    #[must_use]
    pub fn script_short_selling(&self) -> &ScriptedMarketInfoShortSellingScript {
        &self.short_selling
    }
    #[must_use]
    pub fn script_program_trades(&self) -> &ScriptedMarketInfoProgramTradesScript {
        &self.program_trades
    }
    #[must_use]
    pub fn script_investor_trading_rankings(
        &self,
    ) -> &ScriptedMarketInfoInvestorTradingRankingsScript {
        &self.investor_trading_rankings
    }
    #[must_use]
    pub fn script_sectors(&self) -> &ScriptedMarketInfoSectorsScript {
        &self.sectors
    }
    #[must_use]
    pub fn script_sector(&self) -> &ScriptedMarketInfoSectorScript {
        &self.sector
    }
    #[must_use]
    pub fn script_sector_rankings(&self) -> &ScriptedMarketInfoSectorRankingsScript {
        &self.sector_rankings
    }
    #[must_use]
    pub fn script_sector_etfs(&self) -> &ScriptedMarketInfoSectorEtfsScript {
        &self.sector_etfs
    }

    #[must_use]
    pub fn script_indicator_quotes(&self) -> &ScriptedMarketInfoIndicatorQuotesScript {
        &self.indicator_quotes
    }
    #[must_use]
    pub fn script_stock_investor_flow(&self) -> &ScriptedMarketInfoStockInvestorFlowScript {
        &self.stock_investor_flow
    }

    #[must_use]
    pub fn script_fx_rate(&self) -> &ScriptedMarketInfoFxRateScript {
        &self.fx_rate
    }
    #[must_use]
    pub fn script_order_fx_rates(&self) -> &ScriptedMarketInfoOrderFxRatesScript {
        &self.order_fx_rates
    }
    #[must_use]
    pub fn script_indicator_candles(&self) -> &ScriptedMarketInfoIndicatorCandlesScript {
        &self.indicator_candles
    }
    #[must_use]
    pub fn script_rankings(&self) -> &ScriptedMarketInfoRankingsScript {
        &self.rankings
    }
    #[must_use]
    pub fn script_indicator_investor_flow(&self) -> &ScriptedMarketInfoIndicatorInvestorFlowScript {
        &self.indicator_investor_flow
    }
    #[must_use]
    pub fn script_sector_composition(&self) -> &ScriptedMarketInfoSectorCompositionScript {
        &self.sector_composition
    }
    #[must_use]
    pub fn script_calendar(&self) -> &ScriptedMarketInfoCalendarScript {
        &self.calendar
    }

    #[must_use]
    pub fn calls(&self) -> Vec<MarketInfoCall> {
        self.calls.write().clone()
    }
}
impl MarketInfo for ScriptedMarketInfo {
    async fn securities_lending(
        &self,
        instrument: &QuoteInstrument,
        from: CalendarDate,
        through: CalendarDate,
    ) -> Result<Vec<SecuritiesLending>, PortError> {
        self.calls.write().push(MarketInfoCall::SecuritiesLending(
            instrument.clone(),
            from,
            through,
        ));
        self.securities_lending
            .run((instrument.clone(), from, through))
            .await
    }
    async fn credit_trades(
        &self,
        instrument: &QuoteInstrument,
        from: CalendarDate,
        through: CalendarDate,
    ) -> Result<Vec<CreditTrade>, PortError> {
        self.calls.write().push(MarketInfoCall::CreditTrades(
            instrument.clone(),
            from,
            through,
        ));
        self.credit_trades
            .run((instrument.clone(), from, through))
            .await
    }
    async fn short_selling(
        &self,
        instrument: &QuoteInstrument,
        from: CalendarDate,
        through: CalendarDate,
    ) -> Result<Vec<ShortSelling>, PortError> {
        self.calls.write().push(MarketInfoCall::ShortSelling(
            instrument.clone(),
            from,
            through,
        ));
        self.short_selling
            .run((instrument.clone(), from, through))
            .await
    }
    async fn program_trades(
        &self,
        instrument: &QuoteInstrument,
        from: CalendarDate,
        through: CalendarDate,
    ) -> Result<Vec<ProgramTrade>, PortError> {
        self.calls.write().push(MarketInfoCall::ProgramTrades(
            instrument.clone(),
            from,
            through,
        ));
        self.program_trades
            .run((instrument.clone(), from, through))
            .await
    }
    async fn investor_trading_rankings(
        &self,
        limit: RankingLimit,
    ) -> Result<InvestorTradingRankings, PortError> {
        self.calls
            .write()
            .push(MarketInfoCall::InvestorTradingRankings(limit));
        self.investor_trading_rankings.run((limit,)).await
    }
    async fn sectors(&self) -> Result<SectorCatalog, PortError> {
        self.calls.write().push(MarketInfoCall::Sectors());
        self.sectors.run(()).await
    }
    async fn sector(
        &self,
        sector: &SectorId,
        market: KnownMarket,
    ) -> Result<SectorDetail, PortError> {
        self.calls
            .write()
            .push(MarketInfoCall::Sector(sector.clone(), market));
        self.sector.run((sector.clone(), market)).await
    }
    async fn sector_rankings(
        &self,
        query: &SectorRankingQuery,
    ) -> Result<SectorRanking, PortError> {
        self.calls
            .write()
            .push(MarketInfoCall::SectorRankings(*query));
        self.sector_rankings.run((*query,)).await
    }
    async fn sector_etfs(
        &self,
        sector: &SectorId,
        market: KnownMarket,
    ) -> Result<SectorEtfs, PortError> {
        self.calls
            .write()
            .push(MarketInfoCall::SectorEtfs(sector.clone(), market));
        self.sector_etfs.run((sector.clone(), market)).await
    }

    async fn fx_rate(
        &self,
        pair: FxPair,
        kind: FxRateKind,
        at: Option<Timestamp>,
    ) -> Result<FxRate, PortError> {
        self.calls
            .write()
            .push(MarketInfoCall::FxRate(pair, kind, at));
        self.fx_rate.run((pair, kind, at)).await
    }
    async fn order_fx_rates(&self) -> Result<OrderFxRates, PortError> {
        self.calls.write().push(MarketInfoCall::OrderFxRates());
        self.order_fx_rates.run(()).await
    }
    async fn indicator_quotes(
        &self,
        codes: &[IndicatorCode],
    ) -> Result<Vec<MarketIndicatorQuote>, PortError> {
        self.calls
            .write()
            .push(MarketInfoCall::IndicatorQuotes(codes.to_vec()));
        self.indicator_quotes.run((codes.to_vec(),)).await
    }
    async fn indicator_candles(
        &self,
        code: IndicatorCode,
        window: TimeInterval,
        period: CandlePeriod,
    ) -> Result<Vec<MarketIndicatorCandle>, PortError> {
        self.calls
            .write()
            .push(MarketInfoCall::IndicatorCandles(code, window, period));
        self.indicator_candles.run((code, window, period)).await
    }
    async fn rankings(&self, query: &RankingQuery) -> Result<StockRanking, PortError> {
        self.calls.write().push(MarketInfoCall::Rankings(*query));
        self.rankings.run((*query,)).await
    }
    async fn stock_investor_flow(
        &self,
        instrument: &QuoteInstrument,
        from: CalendarDate,
        through: CalendarDate,
    ) -> Result<Vec<InvestorFlow>, PortError> {
        self.calls.write().push(MarketInfoCall::StockInvestorFlow(
            instrument.clone(),
            from,
            through,
        ));
        self.stock_investor_flow
            .run((instrument.clone(), from, through))
            .await
    }
    async fn indicator_investor_flow(
        &self,
        code: IndicatorCode,
        from: CalendarDate,
        through: CalendarDate,
        interval: FlowInterval,
    ) -> Result<Vec<InvestorFlow>, PortError> {
        self.calls
            .write()
            .push(MarketInfoCall::IndicatorInvestorFlow(
                code, from, through, interval,
            ));
        self.indicator_investor_flow
            .run((code, from, through, interval))
            .await
    }
    async fn sector_composition(
        &self,
        sector: &SectorId,
        market: KnownMarket,
    ) -> Result<SectorComposition, PortError> {
        self.calls
            .write()
            .push(MarketInfoCall::SectorComposition(sector.clone(), market));
        self.sector_composition.run((sector.clone(), market)).await
    }
    async fn calendar(&self, market: KnownMarket) -> Result<MarketCalendar, PortError> {
        self.calls.write().push(MarketInfoCall::Calendar(market));
        self.calendar.run((market,)).await
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NewsFeedCall {
    Source(),
    Metadata(),
    NewItems(FeedCursor),
}
#[derive(Debug)]
pub struct ScriptedNewsFeed {
    new_items: ScriptedNewsFeedNewItemsScript,
    calls: State<Vec<NewsFeedCall>>,
    source: FeedSource,
    metadata: NewsMetadata,
}
impl ScriptedNewsFeed {
    #[must_use]
    pub fn script_new_items(&self) -> &ScriptedNewsFeedNewItemsScript {
        &self.new_items
    }

    #[must_use]
    pub fn new(source: FeedSource, metadata: NewsMetadata) -> Self {
        Self {
            source,
            metadata,
            new_items: Script::default(),
            calls: State::default(),
        }
    }

    #[must_use]
    pub fn calls(&self) -> Vec<NewsFeedCall> {
        self.calls.write().clone()
    }
}
impl NewsFeed for ScriptedNewsFeed {
    fn source(&self) -> FeedSource {
        self.calls.write().push(NewsFeedCall::Source());
        self.source.clone()
    }
    fn metadata(&self) -> NewsMetadata {
        self.calls.write().push(NewsFeedCall::Metadata());
        self.metadata.clone()
    }
    async fn new_items(
        &self,
        since: FeedCursor,
    ) -> Result<DataAvailability<(Vec<CollectedObservation>, FeedCursor)>, InformationError> {
        self.calls
            .write()
            .push(NewsFeedCall::NewItems(since.clone()));
        self.new_items.run((since,)).await
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FundamentalsCall {
    Statements(Symbol, CalendarDate),
}
#[derive(Debug, Default)]
pub struct ScriptedFundamentals {
    statements: ScriptedFundamentalsStatementsScript,
    calls: State<Vec<FundamentalsCall>>,
}
impl ScriptedFundamentals {
    #[must_use]
    pub fn script_statements(&self) -> &ScriptedFundamentalsStatementsScript {
        &self.statements
    }

    #[must_use]
    pub fn calls(&self) -> Vec<FundamentalsCall> {
        self.calls.write().clone()
    }
}
impl Fundamentals for ScriptedFundamentals {
    async fn statements(
        &self,
        symbol: &Symbol,
        period_end: CalendarDate,
    ) -> Result<DataAvailability<Vec<FinancialStatement>>, InformationError> {
        self.calls
            .write()
            .push(FundamentalsCall::Statements(symbol.clone(), period_end));
        self.statements.run((symbol.clone(), period_end)).await
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MacroSeriesCall {
    DailyValues(MacroSeriesId, CalendarDate, CalendarDate),
}
#[derive(Debug, Default)]
pub struct ScriptedMacroSeries {
    daily_values: ScriptedMacroSeriesDailyValuesScript,
    calls: State<Vec<MacroSeriesCall>>,
}
impl ScriptedMacroSeries {
    #[must_use]
    pub fn script_daily_values(&self) -> &ScriptedMacroSeriesDailyValuesScript {
        &self.daily_values
    }

    #[must_use]
    pub fn calls(&self) -> Vec<MacroSeriesCall> {
        self.calls.write().clone()
    }
}
impl MacroSeries for ScriptedMacroSeries {
    async fn daily_values(
        &self,
        series: &MacroSeriesId,
        from: CalendarDate,
        through: CalendarDate,
    ) -> Result<DataAvailability<Vec<MacroObservation>>, InformationError> {
        self.calls
            .write()
            .push(MacroSeriesCall::DailyValues(series.clone(), from, through));
        self.daily_values.run((series.clone(), from, through)).await
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PortfolioRepositoryCall {
    Record(AccountId, Box<SyncRecord<Portfolio>>),
    Latest(AccountId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransactionRepositoryCall {
    Record(AccountId, SyncRecord<SettledRowSet>),
    Latest(AccountId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MarketDataRepositoryCall {
    StoreQuote(InstrumentId, Quote),
    Quote(InstrumentId),
    StoreCandles(InstrumentId, Vec<Candle>),
    Candles(InstrumentId, TimeInterval, CandlePeriod, PriceAdjustment),
    StoreFxRate(FxRate),
    FxRate(FxPair, FxRateKind, FxRateQuery),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NewsRepositoryCall {
    Store(NewsItem),
    ForInstrument(InstrumentId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ThesisRepositoryCall {
    Store(InstrumentId, crate::ThesisRecord),
    Load(InstrumentId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EstimateRepositoryCall {
    AppendSubmission(SignalSubmission),
    Submissions(InstrumentId),
    AppendResolution(SignalTaskKey, EstimateOutcome),
    Resolution(SignalTaskKey),
    AppendCalibration(CalibrationKey, Calibration),
    Calibration(CalibrationKey),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SignalRepositoryCall {
    Replace(InstrumentId, Vec<RaisedSignal>),
    Current(InstrumentId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProposalRepositoryCall {
    StoreDraft(Box<crate::DraftRecord>),
    Draft(ProposalId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValuationRepositoryCall {
    Append(AccountId, Box<ValuationSnapshot>),
    History(AccountId, TimeInterval),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SettingsRepositoryCall {
    StoreProfile(InvestorProfile),
    Profile(),
    StoreRiskSettings(RiskSettings),
    RiskSettings(),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotifierCall {
    Notify(Notification),
}
#[derive(Debug, Default)]
pub struct ScriptedNotifier {
    notify: ScriptedNotifierNotifyScript,
    calls: State<Vec<NotifierCall>>,
}
impl ScriptedNotifier {
    #[must_use]
    pub fn script_notify(&self) -> &ScriptedNotifierNotifyScript {
        &self.notify
    }

    #[must_use]
    pub fn calls(&self) -> Vec<NotifierCall> {
        self.calls.write().clone()
    }
}
impl Notifier for ScriptedNotifier {
    async fn notify(&self, notification: Notification) -> Result<WriteCompleted, SystemPortError> {
        self.calls.write().push(NotifierCall::Notify(notification));
        self.notify.run((notification,)).await
    }
}

pub type ScriptedBrokerReadAccountsScript = Script<(), Result<BrokerAccountRows, PortError>>;
pub type ScriptedBrokerReadHoldingsScript =
    Script<(AccountId,), Result<HoldingsObservation, PortError>>;
pub type ScriptedBrokerReadTransactionsScript =
    Script<(AccountId, SettlementWindow, TransactionFilter), Result<TransactionRead, PortError>>;
pub type ScriptedBrokerReadQuotesScript =
    Script<(Vec<QuoteInstrument>,), Result<Vec<QuoteObservation>, PortError>>;
pub type ScriptedBrokerReadCandlesScript = Script<
    (QuoteInstrument, TimeInterval, CandlePeriod, PriceAdjustment),
    Result<Vec<Candle>, PortError>,
>;
pub type ScriptedBrokerReadBuyingPowerScript =
    Script<(AccountId, Currency), Result<BuyingPower, PortError>>;
pub type ScriptedBrokerReadOpenOrdersScript =
    Script<(AccountId,), Result<BrokerOrderSet, PortError>>;
pub type ScriptedBrokerReadClosedOrdersScript =
    Script<(AccountId, TimeInterval), Result<ClosedOrderSet, PortError>>;
pub type ScriptedBrokerReadOrderScript =
    Script<(AccountId, BrokerOrderId), Result<BrokerOrder, PortError>>;
pub type ScriptedBrokerReadStockWarningsScript =
    Script<(ResolvedOrderInstrument,), Result<StockWarnings, PortError>>;
pub type ScriptedBrokerReadInstrumentsScript =
    Script<(KnownMarket,), Result<MasterInstrumentFacts, PortError>>;
pub type ScriptedBrokerReadOrderbookScript =
    Script<(QuoteInstrument,), Result<Orderbook, PortError>>;
pub type ScriptedBrokerReadPriceLimitsScript =
    Script<(QuoteInstrument,), Result<PriceLimits, PortError>>;
pub type ScriptedBrokerReadCommissionRateScript =
    Script<(AccountId, KnownMarket), Result<CommissionRateFact, PortError>>;
pub type ScriptedMarketInfoFxRateScript =
    Script<(FxPair, FxRateKind, Option<Timestamp>), Result<FxRate, PortError>>;
pub type ScriptedMarketInfoOrderFxRatesScript = Script<(), Result<OrderFxRates, PortError>>;
pub type ScriptedMarketInfoIndicatorCandlesScript = Script<
    (IndicatorCode, TimeInterval, CandlePeriod),
    Result<Vec<MarketIndicatorCandle>, PortError>,
>;
pub type ScriptedMarketInfoRankingsScript =
    Script<(RankingQuery,), Result<StockRanking, PortError>>;
pub type ScriptedMarketInfoIndicatorInvestorFlowScript = Script<
    (IndicatorCode, CalendarDate, CalendarDate, FlowInterval),
    Result<Vec<InvestorFlow>, PortError>,
>;
pub type ScriptedMarketInfoSectorCompositionScript =
    Script<(SectorId, KnownMarket), Result<SectorComposition, PortError>>;
pub type ScriptedMarketInfoCalendarScript =
    Script<(KnownMarket,), Result<MarketCalendar, PortError>>;
pub type ScriptedNewsFeedNewItemsScript = Script<
    (FeedCursor,),
    Result<DataAvailability<(Vec<CollectedObservation>, FeedCursor)>, InformationError>,
>;
pub type ScriptedFundamentalsStatementsScript = Script<
    (Symbol, CalendarDate),
    Result<DataAvailability<Vec<FinancialStatement>>, InformationError>,
>;
pub type ScriptedMacroSeriesDailyValuesScript = Script<
    (MacroSeriesId, CalendarDate, CalendarDate),
    Result<DataAvailability<Vec<MacroObservation>>, InformationError>,
>;

pub type ScriptedNotifierNotifyScript =
    Script<(Notification,), Result<WriteCompleted, SystemPortError>>;

pub type ScriptedBrokerReadSellableQuantityScript =
    Script<(AccountId, ResolvedOrderInstrument), Result<SellableQuantityFact, PortError>>;
pub type ScriptedMarketInfoIndicatorQuotesScript =
    Script<(Vec<IndicatorCode>,), Result<Vec<MarketIndicatorQuote>, PortError>>;
pub type ScriptedMarketInfoStockInvestorFlowScript =
    Script<(QuoteInstrument, CalendarDate, CalendarDate), Result<Vec<InvestorFlow>, PortError>>;
pub type ScriptedMarketInfoInvestorTradingRankingsScript =
    Script<(RankingLimit,), Result<InvestorTradingRankings, PortError>>;
pub type ScriptedMarketInfoSectorsScript = Script<(), Result<SectorCatalog, PortError>>;
pub type ScriptedMarketInfoSectorScript =
    Script<(SectorId, KnownMarket), Result<SectorDetail, PortError>>;
pub type ScriptedMarketInfoSectorRankingsScript =
    Script<(SectorRankingQuery,), Result<SectorRanking, PortError>>;
pub type ScriptedMarketInfoSectorEtfsScript =
    Script<(SectorId, KnownMarket), Result<SectorEtfs, PortError>>;

pub type ScriptedMarketInfoProgramTradesScript =
    Script<(QuoteInstrument, CalendarDate, CalendarDate), Result<Vec<ProgramTrade>, PortError>>;

pub type ScriptedMarketInfoShortSellingScript =
    Script<(QuoteInstrument, CalendarDate, CalendarDate), Result<Vec<ShortSelling>, PortError>>;

pub type ScriptedMarketInfoCreditTradesScript =
    Script<(QuoteInstrument, CalendarDate, CalendarDate), Result<Vec<CreditTrade>, PortError>>;

pub type ScriptedMarketInfoSecuritiesLendingScript = Script<
    (QuoteInstrument, CalendarDate, CalendarDate),
    Result<Vec<SecuritiesLending>, PortError>,
>;
