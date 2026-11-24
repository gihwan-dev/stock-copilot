use application::{
    AccessDenial, AiEvent, AiEventStream, AiProvider, AiRunId, AiUsage, AuditEntry, AuditIntegrity,
    AuditLog, BrokerRead, ChatHistory, ChatQueueAcquisition, ChatQueuePort, ChatRepository,
    ChatRunLedger, ChatRunLedgerEntry, ChatSessionId, ChatSessionRecord, ChatSessionSummary,
    CollectedObservation, ConditionalDraftOutcome, DataAvailability, EstimateRepository,
    FeedCursor, FeedSource, FinancialStatement, Fundamentals, FxRateQuery, HoldingsObservation,
    HttpStatus, InformationError, IsolationCheck, MacroObservation, MacroSeries, MacroSeriesId,
    MarketDataRepository, MarketInfo, NewsFeed, NewsMetadata, NewsRepository, Notification,
    Notifier, PortError, PortErrorKind, PortfolioRepository, ProposalRepository, QuoteObservation,
    RankingQuery, ReadEndpoint, RecordOutcome, ResetTime, RetryDelay, RunKind, RunSpec, SecretKey,
    SecretStore, SecretValue, SectorRankingQuery, SettingsRepository, SignalCommitOutcome,
    SignalMutation, SignalRepository, SignalSnapshot, SignalSourceFacts, StoredChatEntry,
    StoredUserMessage, SystemFailure, SystemPort, SystemPortError, ThesisRepository,
    TransactionRead, TransactionRepository, UserMessageId, ValuationRepository, WarmChatId,
    WarmChatProbe, WarmChatRequest, WarmChatRuntime, WarmChatSession, WriteCompleted,
};
use domain::{
    AccountId, BrokerAccountRows, BrokerOrder, BrokerOrderId, BrokerOrderSet, BuyingPower,
    CalendarDate, Calibration, CalibrationKey, Candle, CandlePeriod, ClosedOrderSet,
    CommissionRateFact, CreditTrade, Currency, Draft, DraftRestoreEvidence, EstimateOutcome,
    FlowInterval, FxPair, FxRate, FxRateKind, HistoricalSignal, HistoricalSignalOccurrence,
    IndicatorCode, InstrumentId, InstrumentMaster, InvestorFlow, InvestorProfile,
    InvestorTradingRankings, IssuedEstimateTaskId, KnownMarket, MarketCalendar,
    MarketIndicatorCandle, MarketIndicatorQuote, MasterInstrument, NewsItem, OrderFxRates,
    Orderbook, Portfolio, PriceAdjustment, PriceLimits, ProgramTrade, Proposal, ProposalId, Quote,
    QuoteInstrument, RankingLimit, ResolvedOrderInstrument, RiskSettings, SectorCatalog,
    SectorComposition, SectorDetail, SectorEtfs, SectorId, SectorRanking, SecuritiesLending,
    SellableQuantityFact, SettledRowSet, SettlementWindow, ShortSelling, SignalSubmission,
    SignalTaskKey, StockRanking, StockWarnings, Symbol, SyncRecord, Thesis, TimeInterval,
    Timestamp, TransactionFilter, UntrustedText, ValuationSnapshot,
};
impl BrokerRead for Unavailable {
    fn accounts(&self) -> impl Future<Output = Result<BrokerAccountRows, PortError>> + Send {
        std::future::ready(Err(PortError::new(
            ReadEndpoint::Accounts,
            HttpStatus::NotReceived,
            PortErrorKind::AccessDenied(AccessDenial::InvalidCredentials),
        )))
    }
    fn holdings(
        &self,
        _account: &AccountId,
    ) -> impl Future<Output = Result<HoldingsObservation, PortError>> + Send {
        std::future::ready(Err(PortError::new(
            ReadEndpoint::Holdings,
            HttpStatus::NotReceived,
            PortErrorKind::AccessDenied(AccessDenial::InvalidCredentials),
        )))
    }
    fn transactions(
        &self,
        _account: &AccountId,
        _window: SettlementWindow,
        _filter: &TransactionFilter,
    ) -> impl Future<Output = Result<TransactionRead, PortError>> + Send {
        std::future::ready(Err(PortError::new(
            ReadEndpoint::Transactions,
            HttpStatus::NotReceived,
            PortErrorKind::AccessDenied(AccessDenial::InvalidCredentials),
        )))
    }
    fn quotes(
        &self,
        _instruments: &[QuoteInstrument],
    ) -> impl Future<Output = Result<Vec<QuoteObservation>, PortError>> + Send {
        std::future::ready(Err(PortError::new(
            ReadEndpoint::Quotes,
            HttpStatus::NotReceived,
            PortErrorKind::AccessDenied(AccessDenial::InvalidCredentials),
        )))
    }
    fn candles(
        &self,
        _instrument: &QuoteInstrument,
        _window: TimeInterval,
        _period: CandlePeriod,
        _adjustment: PriceAdjustment,
    ) -> impl Future<Output = Result<Vec<Candle>, PortError>> + Send {
        std::future::ready(Err(PortError::new(
            ReadEndpoint::Candles,
            HttpStatus::NotReceived,
            PortErrorKind::AccessDenied(AccessDenial::InvalidCredentials),
        )))
    }
    fn buying_power(
        &self,
        _account: &AccountId,
        _currency: Currency,
    ) -> impl Future<Output = Result<BuyingPower, PortError>> + Send {
        std::future::ready(Err(PortError::new(
            ReadEndpoint::BuyingPower,
            HttpStatus::NotReceived,
            PortErrorKind::AccessDenied(AccessDenial::InvalidCredentials),
        )))
    }
    fn sellable_quantity(
        &self,
        _account: &AccountId,
        _instrument: &ResolvedOrderInstrument,
    ) -> impl Future<Output = Result<SellableQuantityFact, PortError>> + Send {
        std::future::ready(Err(PortError::new(
            ReadEndpoint::SellableQuantity,
            HttpStatus::NotReceived,
            PortErrorKind::AccessDenied(AccessDenial::InvalidCredentials),
        )))
    }
    fn open_orders(
        &self,
        _account: &AccountId,
    ) -> impl Future<Output = Result<BrokerOrderSet, PortError>> + Send {
        std::future::ready(Err(PortError::new(
            ReadEndpoint::OpenOrders,
            HttpStatus::NotReceived,
            PortErrorKind::AccessDenied(AccessDenial::InvalidCredentials),
        )))
    }
    fn closed_orders(
        &self,
        _account: &AccountId,
        _evaluation_window: TimeInterval,
    ) -> impl Future<Output = Result<ClosedOrderSet, PortError>> + Send {
        std::future::ready(Err(PortError::new(
            ReadEndpoint::ClosedOrders,
            HttpStatus::NotReceived,
            PortErrorKind::AccessDenied(AccessDenial::InvalidCredentials),
        )))
    }
    fn order(
        &self,
        _account: &AccountId,
        _order_id: &BrokerOrderId,
    ) -> impl Future<Output = Result<BrokerOrder, PortError>> + Send {
        std::future::ready(Err(PortError::new(
            ReadEndpoint::Order,
            HttpStatus::NotReceived,
            PortErrorKind::AccessDenied(AccessDenial::InvalidCredentials),
        )))
    }
    fn stock_warnings(
        &self,
        _instrument: &ResolvedOrderInstrument,
    ) -> impl Future<Output = Result<StockWarnings, PortError>> + Send {
        std::future::ready(Err(PortError::new(
            ReadEndpoint::StockWarnings,
            HttpStatus::NotReceived,
            PortErrorKind::AccessDenied(AccessDenial::InvalidCredentials),
        )))
    }
    fn instruments(
        &self,
        _market: KnownMarket,
    ) -> impl Future<Output = Result<InstrumentMaster, PortError>> + Send {
        std::future::ready(Err(PortError::new(
            ReadEndpoint::Instruments,
            HttpStatus::NotReceived,
            PortErrorKind::AccessDenied(AccessDenial::InvalidCredentials),
        )))
    }
    fn orderbook(
        &self,
        _instrument: &QuoteInstrument,
    ) -> impl Future<Output = Result<Orderbook, PortError>> + Send {
        std::future::ready(Err(PortError::new(
            ReadEndpoint::Orderbook,
            HttpStatus::NotReceived,
            PortErrorKind::AccessDenied(AccessDenial::InvalidCredentials),
        )))
    }
    fn price_limits(
        &self,
        _instrument: &QuoteInstrument,
    ) -> impl Future<Output = Result<PriceLimits, PortError>> + Send {
        std::future::ready(Err(PortError::new(
            ReadEndpoint::PriceLimits,
            HttpStatus::NotReceived,
            PortErrorKind::AccessDenied(AccessDenial::InvalidCredentials),
        )))
    }
    fn commission_rate(
        &self,
        _account: &AccountId,
        _market: KnownMarket,
    ) -> impl Future<Output = Result<CommissionRateFact, PortError>> + Send {
        std::future::ready(Err(PortError::new(
            ReadEndpoint::CommissionRate,
            HttpStatus::NotReceived,
            PortErrorKind::AccessDenied(AccessDenial::InvalidCredentials),
        )))
    }
}
impl MarketInfo for Unavailable {
    fn program_trades(
        &self,
        _instrument: &QuoteInstrument,
        _from: CalendarDate,
        _through: CalendarDate,
    ) -> impl Future<Output = Result<Vec<ProgramTrade>, PortError>> + Send {
        std::future::ready(Err(PortError::new(
            ReadEndpoint::ProgramTrades,
            HttpStatus::NotReceived,
            PortErrorKind::AccessDenied(AccessDenial::InvalidCredentials),
        )))
    }
    fn short_selling(
        &self,
        _instrument: &QuoteInstrument,
        _from: CalendarDate,
        _through: CalendarDate,
    ) -> impl Future<Output = Result<Vec<ShortSelling>, PortError>> + Send {
        std::future::ready(Err(PortError::new(
            ReadEndpoint::ShortSelling,
            HttpStatus::NotReceived,
            PortErrorKind::AccessDenied(AccessDenial::InvalidCredentials),
        )))
    }
    fn credit_trades(
        &self,
        _instrument: &QuoteInstrument,
        _from: CalendarDate,
        _through: CalendarDate,
    ) -> impl Future<Output = Result<Vec<CreditTrade>, PortError>> + Send {
        std::future::ready(Err(PortError::new(
            ReadEndpoint::CreditTrades,
            HttpStatus::NotReceived,
            PortErrorKind::AccessDenied(AccessDenial::InvalidCredentials),
        )))
    }
    fn securities_lending(
        &self,
        _instrument: &QuoteInstrument,
        _from: CalendarDate,
        _through: CalendarDate,
    ) -> impl Future<Output = Result<Vec<SecuritiesLending>, PortError>> + Send {
        std::future::ready(Err(PortError::new(
            ReadEndpoint::SecuritiesLending,
            HttpStatus::NotReceived,
            PortErrorKind::AccessDenied(AccessDenial::InvalidCredentials),
        )))
    }
    fn investor_trading_rankings(
        &self,
        _limit: RankingLimit,
    ) -> impl Future<Output = Result<InvestorTradingRankings, PortError>> + Send {
        std::future::ready(Err(PortError::new(
            ReadEndpoint::InvestorTradingRankings,
            HttpStatus::NotReceived,
            PortErrorKind::AccessDenied(AccessDenial::InvalidCredentials),
        )))
    }
    fn sectors(&self) -> impl Future<Output = Result<SectorCatalog, PortError>> + Send {
        std::future::ready(Err(PortError::new(
            ReadEndpoint::Sectors,
            HttpStatus::NotReceived,
            PortErrorKind::AccessDenied(AccessDenial::InvalidCredentials),
        )))
    }
    fn sector(
        &self,
        _sector: &SectorId,
        _market: KnownMarket,
    ) -> impl Future<Output = Result<SectorDetail, PortError>> + Send {
        std::future::ready(Err(PortError::new(
            ReadEndpoint::Sector,
            HttpStatus::NotReceived,
            PortErrorKind::AccessDenied(AccessDenial::InvalidCredentials),
        )))
    }
    fn sector_rankings(
        &self,
        _query: &SectorRankingQuery,
    ) -> impl Future<Output = Result<SectorRanking, PortError>> + Send {
        std::future::ready(Err(PortError::new(
            ReadEndpoint::SectorRankings,
            HttpStatus::NotReceived,
            PortErrorKind::AccessDenied(AccessDenial::InvalidCredentials),
        )))
    }
    fn sector_etfs(
        &self,
        _sector: &SectorId,
        _market: KnownMarket,
    ) -> impl Future<Output = Result<SectorEtfs, PortError>> + Send {
        std::future::ready(Err(PortError::new(
            ReadEndpoint::SectorEtfs,
            HttpStatus::NotReceived,
            PortErrorKind::AccessDenied(AccessDenial::InvalidCredentials),
        )))
    }
    fn fx_rate(
        &self,
        _pair: FxPair,
        _kind: FxRateKind,
        _at: Option<Timestamp>,
    ) -> impl Future<Output = Result<FxRate, PortError>> + Send {
        std::future::ready(Err(PortError::new(
            ReadEndpoint::FxRate,
            HttpStatus::NotReceived,
            PortErrorKind::AccessDenied(AccessDenial::InvalidCredentials),
        )))
    }
    fn order_fx_rates(&self) -> impl Future<Output = Result<OrderFxRates, PortError>> + Send {
        std::future::ready(Err(PortError::new(
            ReadEndpoint::OrderFxRates,
            HttpStatus::NotReceived,
            PortErrorKind::AccessDenied(AccessDenial::InvalidCredentials),
        )))
    }
    fn indicator_quotes(
        &self,
        _codes: &[IndicatorCode],
    ) -> impl Future<Output = Result<Vec<MarketIndicatorQuote>, PortError>> + Send {
        std::future::ready(Err(PortError::new(
            ReadEndpoint::IndicatorQuotes,
            HttpStatus::NotReceived,
            PortErrorKind::AccessDenied(AccessDenial::InvalidCredentials),
        )))
    }
    fn indicator_candles(
        &self,
        _code: IndicatorCode,
        _window: TimeInterval,
        _period: CandlePeriod,
    ) -> impl Future<Output = Result<Vec<MarketIndicatorCandle>, PortError>> + Send {
        std::future::ready(Err(PortError::new(
            ReadEndpoint::IndicatorCandles,
            HttpStatus::NotReceived,
            PortErrorKind::AccessDenied(AccessDenial::InvalidCredentials),
        )))
    }
    fn rankings(
        &self,
        _query: &RankingQuery,
    ) -> impl Future<Output = Result<StockRanking, PortError>> + Send {
        std::future::ready(Err(PortError::new(
            ReadEndpoint::Rankings,
            HttpStatus::NotReceived,
            PortErrorKind::AccessDenied(AccessDenial::InvalidCredentials),
        )))
    }
    fn stock_investor_flow(
        &self,
        _instrument: &QuoteInstrument,
        _from: CalendarDate,
        _through: CalendarDate,
    ) -> impl Future<Output = Result<Vec<InvestorFlow>, PortError>> + Send {
        std::future::ready(Err(PortError::new(
            ReadEndpoint::StockInvestorFlow,
            HttpStatus::NotReceived,
            PortErrorKind::AccessDenied(AccessDenial::InvalidCredentials),
        )))
    }
    fn indicator_investor_flow(
        &self,
        _code: IndicatorCode,
        _from: CalendarDate,
        _through: CalendarDate,
        _interval: FlowInterval,
    ) -> impl Future<Output = Result<Vec<InvestorFlow>, PortError>> + Send {
        std::future::ready(Err(PortError::new(
            ReadEndpoint::IndicatorInvestorFlow,
            HttpStatus::NotReceived,
            PortErrorKind::AccessDenied(AccessDenial::InvalidCredentials),
        )))
    }
    fn sector_composition(
        &self,
        _sector: &SectorId,
        _market: KnownMarket,
    ) -> impl Future<Output = Result<SectorComposition, PortError>> + Send {
        std::future::ready(Err(PortError::new(
            ReadEndpoint::SectorComposition,
            HttpStatus::NotReceived,
            PortErrorKind::AccessDenied(AccessDenial::InvalidCredentials),
        )))
    }
    fn calendar(
        &self,
        _market: KnownMarket,
    ) -> impl Future<Output = Result<MarketCalendar, PortError>> + Send {
        std::future::ready(Err(PortError::new(
            ReadEndpoint::Calendar,
            HttpStatus::NotReceived,
            PortErrorKind::AccessDenied(AccessDenial::InvalidCredentials),
        )))
    }
}
impl MacroSeries for Unavailable {
    fn daily_values(
        &self,
        _series: &MacroSeriesId,
        _from: CalendarDate,
        _through: CalendarDate,
    ) -> impl Future<Output = Result<DataAvailability<Vec<MacroObservation>>, InformationError>> + Send
    {
        std::future::ready(Ok(DataAvailability::NotConfigured))
    }
}
impl NewsFeed for UnavailableFeed {
    fn source(&self) -> FeedSource {
        self.source.clone()
    }
    fn metadata(&self) -> NewsMetadata {
        self.metadata.clone()
    }
    fn new_items(
        &self,
        _since: FeedCursor,
    ) -> impl Future<
        Output = Result<
            DataAvailability<(Vec<CollectedObservation>, FeedCursor)>,
            InformationError,
        >,
    > + Send {
        std::future::ready(Ok(DataAvailability::NotConfigured))
    }
}
impl Fundamentals for Unavailable {
    fn statements(
        &self,
        _symbol: &Symbol,
        _period_end: CalendarDate,
    ) -> impl Future<Output = Result<DataAvailability<Vec<FinancialStatement>>, InformationError>> + Send
    {
        std::future::ready(Ok(DataAvailability::NotConfigured))
    }
}
impl AiProvider for Unavailable {
    type Stream = UnavailableStream;
    fn run(
        &self,
        _spec: RunSpec,
    ) -> impl Future<Output = Result<Self::Stream, InformationError>> + Send {
        std::future::ready(Err(InformationError::Unavailable(ResetTime::NotProvided)))
    }
    fn resume(
        &self,
        _spec: RunSpec,
        _history: ChatHistory,
        _message: UntrustedText,
    ) -> impl Future<Output = Result<Self::Stream, InformationError>> + Send {
        std::future::ready(Err(InformationError::Unavailable(ResetTime::NotProvided)))
    }
    fn usage(&self) -> impl Future<Output = Result<AiUsage, InformationError>> + Send {
        std::future::ready(Err(InformationError::Unavailable(ResetTime::NotProvided)))
    }
    fn interrupt(
        &self,
        _run: &AiRunId,
    ) -> impl Future<Output = Result<(), InformationError>> + Send {
        std::future::ready(Err(InformationError::Unavailable(ResetTime::NotProvided)))
    }
    fn verify_isolation(
        &self,
        _kind: RunKind,
    ) -> impl Future<Output = Result<IsolationCheck, InformationError>> + Send {
        std::future::ready(Err(InformationError::Unavailable(ResetTime::NotProvided)))
    }
}
impl PortfolioRepository for Unavailable {
    fn record(
        &self,
        _account: &AccountId,
        _record: &SyncRecord<Portfolio>,
    ) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send {
        std::future::ready(Err(SystemPortError::new(
            SystemPort::Repository,
            SystemFailure::Unavailable(RetryDelay::Unknown),
        )))
    }
    fn latest(
        &self,
        _account: &AccountId,
    ) -> impl Future<Output = Result<Option<SyncRecord<Portfolio>>, SystemPortError>> + Send {
        std::future::ready(Err(SystemPortError::new(
            SystemPort::Repository,
            SystemFailure::Unavailable(RetryDelay::Unknown),
        )))
    }
}
impl TransactionRepository for Unavailable {
    fn record(
        &self,
        _account: &AccountId,
        _record: &SyncRecord<SettledRowSet>,
    ) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send {
        std::future::ready(Err(SystemPortError::new(
            SystemPort::Repository,
            SystemFailure::Unavailable(RetryDelay::Unknown),
        )))
    }
    fn latest(
        &self,
        _account: &AccountId,
    ) -> impl Future<Output = Result<Option<SyncRecord<SettledRowSet>>, SystemPortError>> + Send
    {
        std::future::ready(Err(SystemPortError::new(
            SystemPort::Repository,
            SystemFailure::Unavailable(RetryDelay::Unknown),
        )))
    }
}
impl MarketDataRepository for Unavailable {
    fn store_quote(
        &self,
        _instrument: &InstrumentId,
        _quote: &Quote,
    ) -> impl Future<Output = Result<application::CacheWriteOutcome, SystemPortError>> + Send {
        std::future::ready(Err(SystemPortError::new(
            SystemPort::Repository,
            SystemFailure::Unavailable(RetryDelay::Unknown),
        )))
    }
    fn quote(
        &self,
        _instrument: &InstrumentId,
    ) -> impl Future<Output = Result<Option<Quote>, SystemPortError>> + Send {
        std::future::ready(Err(SystemPortError::new(
            SystemPort::Repository,
            SystemFailure::Unavailable(RetryDelay::Unknown),
        )))
    }
    fn store_candles(
        &self,
        _instrument: &InstrumentId,
        _candles: &[Candle],
    ) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send {
        std::future::ready(Err(SystemPortError::new(
            SystemPort::Repository,
            SystemFailure::Unavailable(RetryDelay::Unknown),
        )))
    }
    fn candles(
        &self,
        _instrument: &InstrumentId,
        _window: TimeInterval,
        _period: CandlePeriod,
        _adjustment: PriceAdjustment,
    ) -> impl Future<Output = Result<Vec<Candle>, SystemPortError>> + Send {
        std::future::ready(Err(SystemPortError::new(
            SystemPort::Repository,
            SystemFailure::Unavailable(RetryDelay::Unknown),
        )))
    }
    fn store_fx_rate(
        &self,
        _rate: &FxRate,
    ) -> impl Future<Output = Result<application::CacheWriteOutcome, SystemPortError>> + Send {
        std::future::ready(Err(SystemPortError::new(
            SystemPort::Repository,
            SystemFailure::Unavailable(RetryDelay::Unknown),
        )))
    }
    fn fx_rate(
        &self,
        _pair: FxPair,
        _kind: FxRateKind,
        _at: FxRateQuery,
    ) -> impl Future<Output = Result<Option<FxRate>, SystemPortError>> + Send {
        std::future::ready(Err(SystemPortError::new(
            SystemPort::Repository,
            SystemFailure::Unavailable(RetryDelay::Unknown),
        )))
    }
}
impl NewsRepository for Unavailable {
    fn store(
        &self,
        _item: &NewsItem,
    ) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send {
        std::future::ready(Err(SystemPortError::new(
            SystemPort::Repository,
            SystemFailure::Unavailable(RetryDelay::Unknown),
        )))
    }
    fn for_instrument(
        &self,
        _instrument: &InstrumentId,
    ) -> impl Future<Output = Result<Vec<NewsItem>, SystemPortError>> + Send {
        std::future::ready(Err(SystemPortError::new(
            SystemPort::Repository,
            SystemFailure::Unavailable(RetryDelay::Unknown),
        )))
    }
}
impl ThesisRepository for Unavailable {
    fn store(
        &self,
        _instrument: &InstrumentId,
        _thesis: &Thesis,
    ) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send {
        std::future::ready(Err(SystemPortError::new(
            SystemPort::Repository,
            SystemFailure::Unavailable(RetryDelay::Unknown),
        )))
    }
    fn load(
        &self,
        _instrument: &InstrumentId,
    ) -> impl Future<Output = Result<Option<Thesis>, SystemPortError>> + Send {
        std::future::ready(Err(SystemPortError::new(
            SystemPort::Repository,
            SystemFailure::Unavailable(RetryDelay::Unknown),
        )))
    }
}
impl EstimateRepository for Unavailable {
    fn append_submission(
        &self,
        _submission: &SignalSubmission,
    ) -> impl Future<Output = Result<RecordOutcome, SystemPortError>> + Send {
        std::future::ready(Err(SystemPortError::new(
            SystemPort::Repository,
            SystemFailure::Unavailable(RetryDelay::Unknown),
        )))
    }
    fn submissions(
        &self,
        _instrument: &InstrumentId,
    ) -> impl Future<Output = Result<Vec<SignalSubmission>, SystemPortError>> + Send {
        std::future::ready(Err(SystemPortError::new(
            SystemPort::Repository,
            SystemFailure::Unavailable(RetryDelay::Unknown),
        )))
    }
    fn append_resolution(
        &self,
        _task: &SignalTaskKey,
        _outcome: &EstimateOutcome,
    ) -> impl Future<Output = Result<RecordOutcome, SystemPortError>> + Send {
        std::future::ready(Err(SystemPortError::new(
            SystemPort::Repository,
            SystemFailure::Unavailable(RetryDelay::Unknown),
        )))
    }
    fn resolution(
        &self,
        _task: &SignalTaskKey,
    ) -> impl Future<Output = Result<Option<EstimateOutcome>, SystemPortError>> + Send {
        std::future::ready(Err(SystemPortError::new(
            SystemPort::Repository,
            SystemFailure::Unavailable(RetryDelay::Unknown),
        )))
    }
    fn append_calibration(
        &self,
        _key: &CalibrationKey,
        _calibration: &Calibration,
    ) -> impl Future<Output = Result<RecordOutcome, SystemPortError>> + Send {
        std::future::ready(Err(SystemPortError::new(
            SystemPort::Repository,
            SystemFailure::Unavailable(RetryDelay::Unknown),
        )))
    }
    fn calibration(
        &self,
        _key: &CalibrationKey,
    ) -> impl Future<Output = Result<Option<Calibration>, SystemPortError>> + Send {
        std::future::ready(Err(SystemPortError::new(
            SystemPort::Repository,
            SystemFailure::Unavailable(RetryDelay::Unknown),
        )))
    }
}
impl SignalRepository for Unavailable {
    fn snapshot(
        &self,
        _instrument: &InstrumentId,
    ) -> impl Future<Output = Result<SignalSnapshot, SystemPortError>> + Send {
        std::future::ready(Err(SystemPortError::new(
            SystemPort::Repository,
            SystemFailure::Unavailable(RetryDelay::Unknown),
        )))
    }
    fn commit(
        &self,
        _instrument: &InstrumentId,
        _expected: &SignalSnapshot,
        _mutation: &SignalMutation,
    ) -> impl Future<Output = Result<SignalCommitOutcome, SystemPortError>> + Send {
        std::future::ready(Err(SystemPortError::new(
            SystemPort::Repository,
            SystemFailure::Unavailable(RetryDelay::Unknown),
        )))
    }
    fn occurrences(
        &self,
        _instrument: &InstrumentId,
        _identity: &ResolvedOrderInstrument,
    ) -> impl Future<Output = Result<Vec<HistoricalSignalOccurrence>, SystemPortError>> + Send {
        std::future::ready(Err(SystemPortError::new(
            SystemPort::Repository,
            SystemFailure::Unavailable(RetryDelay::Unknown),
        )))
    }
    fn sources(
        &self,
        _task: &IssuedEstimateTaskId,
    ) -> impl Future<Output = Result<SignalSourceFacts, SystemPortError>> + Send {
        std::future::ready(Err(SystemPortError::new(
            SystemPort::Repository,
            SystemFailure::Unavailable(RetryDelay::Unknown),
        )))
    }
    fn current(
        &self,
        _instrument: &InstrumentId,
    ) -> impl Future<Output = Result<Vec<HistoricalSignal>, SystemPortError>> + Send {
        std::future::ready(Err(SystemPortError::new(
            SystemPort::Repository,
            SystemFailure::Unavailable(RetryDelay::Unknown),
        )))
    }
}
impl ProposalRepository for Unavailable {
    fn current_draft(
        &self,
        _instrument: &InstrumentId,
        _expected: &SignalSnapshot,
        _check: application::DraftCheck<'_>,
    ) -> impl Future<Output = Result<application::CurrentDraftOutcome, SystemPortError>> + Send
    {
        std::future::ready(Err(SystemPortError::new(
            SystemPort::Repository,
            SystemFailure::Unavailable(RetryDelay::Unknown),
        )))
    }
    fn store_current_draft(
        &self,
        _instrument: &InstrumentId,
        _expected: &SignalSnapshot,
        _candidate: application::DraftStoreCandidate<'_>,
        _check: application::DraftCheck<'_>,
    ) -> impl Future<Output = Result<ConditionalDraftOutcome, SystemPortError>> + Send {
        std::future::ready(Err(SystemPortError::new(
            SystemPort::Repository,
            SystemFailure::Unavailable(RetryDelay::Unknown),
        )))
    }
    fn store_draft(
        &self,
        _proposal: &Proposal<Draft>,
        _evidence: &DraftRestoreEvidence,
        _master: &MasterInstrument,
    ) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send {
        std::future::ready(Err(SystemPortError::new(
            SystemPort::Repository,
            SystemFailure::Unavailable(RetryDelay::Unknown),
        )))
    }
    fn draft(
        &self,
        _proposal: &ProposalId,
        _master: &MasterInstrument,
    ) -> impl Future<Output = Result<Option<Proposal<Draft>>, SystemPortError>> + Send {
        std::future::ready(Err(SystemPortError::new(
            SystemPort::Repository,
            SystemFailure::Unavailable(RetryDelay::Unknown),
        )))
    }
}
impl ValuationRepository for Unavailable {
    fn append(
        &self,
        _account: &AccountId,
        _snapshot: &ValuationSnapshot,
    ) -> impl Future<Output = Result<RecordOutcome, SystemPortError>> + Send {
        std::future::ready(Err(SystemPortError::new(
            SystemPort::Repository,
            SystemFailure::Unavailable(RetryDelay::Unknown),
        )))
    }
    fn history(
        &self,
        _account: &AccountId,
        _window: TimeInterval,
    ) -> impl Future<Output = Result<Vec<ValuationSnapshot>, SystemPortError>> + Send {
        std::future::ready(Err(SystemPortError::new(
            SystemPort::Repository,
            SystemFailure::Unavailable(RetryDelay::Unknown),
        )))
    }
}
impl SettingsRepository for Unavailable {
    fn store_profile(
        &self,
        _profile: &InvestorProfile,
    ) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send {
        std::future::ready(Err(SystemPortError::new(
            SystemPort::Repository,
            SystemFailure::Unavailable(RetryDelay::Unknown),
        )))
    }
    fn profile(
        &self,
    ) -> impl Future<Output = Result<Option<InvestorProfile>, SystemPortError>> + Send {
        std::future::ready(Err(SystemPortError::new(
            SystemPort::Repository,
            SystemFailure::Unavailable(RetryDelay::Unknown),
        )))
    }
    fn store_risk_settings(
        &self,
        _settings: &RiskSettings,
    ) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send {
        std::future::ready(Err(SystemPortError::new(
            SystemPort::Repository,
            SystemFailure::Unavailable(RetryDelay::Unknown),
        )))
    }
    fn risk_settings(
        &self,
    ) -> impl Future<Output = Result<Option<RiskSettings>, SystemPortError>> + Send {
        std::future::ready(Err(SystemPortError::new(
            SystemPort::Repository,
            SystemFailure::Unavailable(RetryDelay::Unknown),
        )))
    }
}
impl AuditLog for Unavailable {
    fn append(
        &self,
        _entry: &AuditEntry,
    ) -> impl Future<Output = Result<RecordOutcome, SystemPortError>> + Send {
        std::future::ready(Err(SystemPortError::new(
            SystemPort::AuditLog,
            SystemFailure::Unavailable(RetryDelay::Unknown),
        )))
    }
    fn verify(&self) -> impl Future<Output = Result<AuditIntegrity, SystemPortError>> + Send {
        std::future::ready(Err(SystemPortError::new(
            SystemPort::AuditLog,
            SystemFailure::Unavailable(RetryDelay::Unknown),
        )))
    }
}
impl SecretStore for Unavailable {
    fn load(
        &self,
        _key: SecretKey,
    ) -> impl Future<Output = Result<Option<SecretValue>, SystemPortError>> + Send {
        std::future::ready(Ok(None))
    }
    fn store(
        &self,
        _key: SecretKey,
        _value: SecretValue,
    ) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send {
        std::future::ready(Err(SystemPortError::new(
            SystemPort::SecretStore,
            SystemFailure::Unavailable(RetryDelay::Unknown),
        )))
    }
    fn remove(
        &self,
        _key: SecretKey,
    ) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send {
        std::future::ready(Err(SystemPortError::new(
            SystemPort::SecretStore,
            SystemFailure::Unavailable(RetryDelay::Unknown),
        )))
    }
}
impl Notifier for Unavailable {
    fn notify(
        &self,
        _notification: Notification,
    ) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send {
        std::future::ready(Err(SystemPortError::new(
            SystemPort::Notifier,
            SystemFailure::Unavailable(RetryDelay::Unknown),
        )))
    }
}
#[derive(Debug)]
pub struct Unavailable;
#[derive(Debug)]
pub struct UnavailableFeed {
    pub source: FeedSource,
    pub metadata: NewsMetadata,
}
#[derive(Debug)]
pub struct UnavailableStream;
impl AiEventStream for UnavailableStream {
    fn next(&mut self) -> impl Future<Output = Result<Option<AiEvent>, InformationError>> + Send {
        std::future::ready(Err(InformationError::Unavailable(ResetTime::NotProvided)))
    }
}
impl ChatRepository for Unavailable {
    fn ensure_session(
        &self,
        _session: &ChatSessionId,
        _now: Timestamp,
    ) -> impl Future<Output = Result<ChatSessionRecord, SystemPortError>> + Send {
        std::future::ready(Err(SystemPortError::new(
            SystemPort::Repository,
            SystemFailure::Unavailable(RetryDelay::Unknown),
        )))
    }
    fn load_session(
        &self,
        _session: &ChatSessionId,
    ) -> impl Future<Output = Result<Option<ChatSessionRecord>, SystemPortError>> + Send {
        std::future::ready(Err(SystemPortError::new(
            SystemPort::Repository,
            SystemFailure::Unavailable(RetryDelay::Unknown),
        )))
    }
    fn list_sessions(
        &self,
    ) -> impl Future<Output = Result<Vec<ChatSessionSummary>, SystemPortError>> + Send {
        std::future::ready(Err(SystemPortError::new(
            SystemPort::Repository,
            SystemFailure::Unavailable(RetryDelay::Unknown),
        )))
    }
    fn append_entry(
        &self,
        _session: &ChatSessionId,
        _entry: &StoredChatEntry,
    ) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send {
        std::future::ready(Err(SystemPortError::new(
            SystemPort::Repository,
            SystemFailure::Unavailable(RetryDelay::Unknown),
        )))
    }
    fn load_user_message(
        &self,
        _session: &ChatSessionId,
        _message_id: &UserMessageId,
    ) -> impl Future<Output = Result<Option<StoredUserMessage>, SystemPortError>> + Send {
        std::future::ready(Err(SystemPortError::new(
            SystemPort::Repository,
            SystemFailure::Unavailable(RetryDelay::Unknown),
        )))
    }
}
impl ChatRunLedger for Unavailable {
    fn record_chat_run(
        &self,
        _entry: &ChatRunLedgerEntry,
    ) -> impl Future<Output = Result<RecordOutcome, SystemPortError>> + Send {
        std::future::ready(Err(SystemPortError::new(
            SystemPort::Repository,
            SystemFailure::Unavailable(RetryDelay::Unknown),
        )))
    }
}
impl ChatQueuePort for Unavailable {
    fn acquire_chat(
        &self,
        _run: &AiRunId,
    ) -> impl Future<Output = Result<ChatQueueAcquisition, SystemPortError>> + Send {
        std::future::ready(Err(SystemPortError::new(
            SystemPort::Repository,
            SystemFailure::Unavailable(RetryDelay::Unknown),
        )))
    }
    fn release_chat(
        &self,
        _run: &AiRunId,
    ) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send {
        std::future::ready(Err(SystemPortError::new(
            SystemPort::Repository,
            SystemFailure::Unavailable(RetryDelay::Unknown),
        )))
    }
}
impl WarmChatRuntime for Unavailable {
    type Stream = UnavailableStream;
    fn prewarm(
        &self,
        _request: WarmChatRequest,
    ) -> impl Future<Output = Result<WarmChatProbe, InformationError>> + Send {
        std::future::ready(Err(InformationError::Unavailable(ResetTime::NotProvided)))
    }
    fn store_ready(
        &self,
        _session: WarmChatSession,
    ) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send {
        std::future::ready(Err(SystemPortError::new(
            SystemPort::Repository,
            SystemFailure::Unavailable(RetryDelay::Unknown),
        )))
    }
    fn take_ready(
        &self,
    ) -> impl Future<Output = Result<Option<WarmChatSession>, SystemPortError>> + Send {
        std::future::ready(Ok(None))
    }
    fn send_on_warm(
        &self,
        _session: WarmChatSession,
        _spec: RunSpec,
        _history: ChatHistory,
        _message: UntrustedText,
    ) -> impl Future<Output = Result<Self::Stream, InformationError>> + Send {
        std::future::ready(Err(InformationError::Unavailable(ResetTime::NotProvided)))
    }
    fn discard(
        &self,
        _id: &WarmChatId,
    ) -> impl Future<Output = Result<(), InformationError>> + Send {
        std::future::ready(Ok(()))
    }
}
impl application::CollectedUrlCatalog for Unavailable {
    fn find_collected(
        &self,
        _url: &domain::ParsedExternalUrl,
    ) -> impl Future<Output = Result<Option<domain::NormalizedUrl>, SystemPortError>> + Send {
        std::future::ready(Err(SystemPortError::new(
            SystemPort::Repository,
            SystemFailure::Unavailable(RetryDelay::Unknown),
        )))
    }
}
impl application::ExternalUrlOpener for Unavailable {
    fn open(
        &self,
        _url: &domain::AuthorizedExternalUrl,
    ) -> impl Future<
        Output = Result<application::ExternalUrlOpened, application::ExternalUrlOpenerError>,
    > + Send {
        std::future::ready(Err(application::ExternalUrlOpenerError::Unavailable(
            RetryDelay::Unknown,
        )))
    }
}
