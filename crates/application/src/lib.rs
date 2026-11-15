mod observations;
mod port_error;
use domain::{
    AccountId, BrokerAccountRows, BrokerOrder, BrokerOrderId, BrokerOrderSet, BuyingPower,
    CalendarDate, Candle, CandlePeriod, ClosedOrderSet, CommissionRateFact, Currency, FlowInterval,
    FxPair, FxRate, FxRateKind, IndicatorCode, InvestorFlow, KnownMarket, MarketCalendar,
    MarketIndicatorCandle, MarketIndicatorQuote, MasterInstrumentFacts, OrderFxRates, Orderbook,
    PriceAdjustment, PriceLimits, QuoteInstrument, ResolvedOrderInstrument, SectorComposition,
    SectorId, SellableQuantityFact, SettlementWindow, StockRanking, StockWarnings, TimeInterval,
    Timestamp, TransactionFilter,
};
pub use observations::{
    HoldingsObservation, IncompleteReason, IncompleteTransactions, QuarantinedTransaction,
    QuoteObservation, TransactionRead,
};
pub use port_error::{
    AccessDenial, HttpStatus, MissingData, PortError, PortErrorKind, ReadEndpoint,
    RequestRejection, ResponseMismatch, RetryDelay, UnavailableCause,
};
use std::{collections::BTreeSet, future::Future};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum BrokerCapability {
    DailyCandles,
    MinuteCandles,
    AdjustedHistory,
    RawHistory,
    SellableQuantity,
    OpenOrders,
    ClosedOrders,
    Order,
    StockWarnings,
    Instruments,
    Orderbook,
    PriceLimits,
    CommissionRate,
    IndicatorQuotes,
    IndicatorCandles,
    Rankings,
    StockInvestorFlow,
    IndicatorInvestorFlow,
    SectorComposition,
    Calendar,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapabilitySupport {
    Supported,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrokerCapabilities(BTreeSet<BrokerCapability>);
impl BrokerCapabilities {
    #[must_use]
    pub const fn new(supported: BTreeSet<BrokerCapability>) -> Self {
        Self(supported)
    }

    #[must_use]
    pub fn support_for(&self, capability: BrokerCapability) -> CapabilitySupport {
        if self.0.contains(&capability) {
            CapabilitySupport::Supported
        } else {
            CapabilitySupport::Unsupported
        }
    }
}

pub trait BrokerRead: Send + Sync {
    fn accounts(&self) -> impl Future<Output = Result<BrokerAccountRows, PortError>> + Send;
    fn holdings(
        &self,
        account: &AccountId,
    ) -> impl Future<Output = Result<HoldingsObservation, PortError>> + Send;
    fn transactions(
        &self,
        account: &AccountId,
        window: SettlementWindow,
        filter: &TransactionFilter,
    ) -> impl Future<Output = Result<TransactionRead, PortError>> + Send;
    fn quotes(
        &self,
        instruments: &[QuoteInstrument],
    ) -> impl Future<Output = Result<Vec<QuoteObservation>, PortError>> + Send;
    fn candles(
        &self,
        instrument: &QuoteInstrument,
        window: TimeInterval,
        period: CandlePeriod,
        adjustment: PriceAdjustment,
    ) -> impl Future<Output = Result<Vec<Candle>, PortError>> + Send;
    fn buying_power(
        &self,
        account: &AccountId,
        currency: Currency,
    ) -> impl Future<Output = Result<BuyingPower, PortError>> + Send;
    fn sellable_quantity(
        &self,
        account: &AccountId,
        instrument: &ResolvedOrderInstrument,
    ) -> impl Future<Output = Result<SellableQuantityFact, PortError>> + Send;
    fn open_orders(
        &self,
        account: &AccountId,
    ) -> impl Future<Output = Result<BrokerOrderSet, PortError>> + Send;
    fn closed_orders(
        &self,
        account: &AccountId,
        ordered_window: TimeInterval,
    ) -> impl Future<Output = Result<ClosedOrderSet, PortError>> + Send;
    fn order(
        &self,
        account: &AccountId,
        order_id: &BrokerOrderId,
    ) -> impl Future<Output = Result<BrokerOrder, PortError>> + Send;
    fn stock_warnings(
        &self,
        instrument: &ResolvedOrderInstrument,
    ) -> impl Future<Output = Result<StockWarnings, PortError>> + Send;
    fn instruments(
        &self,
        market: KnownMarket,
    ) -> impl Future<Output = Result<MasterInstrumentFacts, PortError>> + Send;
    fn orderbook(
        &self,
        instrument: &QuoteInstrument,
    ) -> impl Future<Output = Result<Orderbook, PortError>> + Send;
    fn price_limits(
        &self,
        instrument: &QuoteInstrument,
    ) -> impl Future<Output = Result<PriceLimits, PortError>> + Send;
    fn commission_rate(
        &self,
        account: &AccountId,
        market: KnownMarket,
    ) -> impl Future<Output = Result<CommissionRateFact, PortError>> + Send;
}

pub trait MarketInfo: Send + Sync {
    fn fx_rate(
        &self,
        pair: FxPair,
        kind: FxRateKind,
        at: Option<Timestamp>,
    ) -> impl Future<Output = Result<FxRate, PortError>> + Send;
    fn order_fx_rates(&self) -> impl Future<Output = Result<OrderFxRates, PortError>> + Send;
    fn indicator_quotes(
        &self,
        codes: &[IndicatorCode],
    ) -> impl Future<Output = Result<Vec<MarketIndicatorQuote>, PortError>> + Send;
    fn indicator_candles(
        &self,
        code: IndicatorCode,
        window: TimeInterval,
        period: CandlePeriod,
    ) -> impl Future<Output = Result<Vec<MarketIndicatorCandle>, PortError>> + Send;
    fn rankings(
        &self,
        query: &RankingQuery,
    ) -> impl Future<Output = Result<StockRanking, PortError>> + Send;
    fn stock_investor_flow(
        &self,
        instrument: &QuoteInstrument,
        from: CalendarDate,
        through: CalendarDate,
    ) -> impl Future<Output = Result<Vec<InvestorFlow>, PortError>> + Send;
    fn indicator_investor_flow(
        &self,
        code: IndicatorCode,
        from: CalendarDate,
        through: CalendarDate,
        interval: FlowInterval,
    ) -> impl Future<Output = Result<Vec<InvestorFlow>, PortError>> + Send;
    fn sector_composition(
        &self,
        sector: &SectorId,
        market: KnownMarket,
    ) -> impl Future<Output = Result<SectorComposition, PortError>> + Send;
    fn calendar(
        &self,
        market: KnownMarket,
    ) -> impl Future<Output = Result<MarketCalendar, PortError>> + Send;
}

mod error_class;
pub use error_class::{
    BugDiagnostic, ClassifiedError, ErrorClass, UserAction, classify_port_error,
};

mod information_error;
pub use information_error::{
    CredentialSource, DataAvailability, IdentifierError, InformationError,
};

mod news_source;
pub use news_source::{
    AiReadPolicy, CollectedEntry, CollectedObservation, ConditionalGet, FeedCursor, FeedPosition,
    FeedSource, NewsFeed, NewsMetadata, PublicationTime, Retention, UserAgentRequirement,
};

mod fundamentals;
pub use fundamentals::{
    FinancialFact, FinancialLine, FinancialSource, FinancialStatement, FinancialUnit, Fundamentals,
    MetricValue,
};

mod macro_series;
pub use macro_series::{
    MacroFact, MacroObservation, MacroSeries, MacroSeriesId, MacroSource, MacroUnit,
};

mod ai_request;
pub use ai_request::{
    AiRunId, AnalysisPrompt, ChatHistory, ChatMessage, ChatRole, CollectionQuery, CollectionTopic,
    ModelName, ModelSelection, PromptError, ReasoningEffort, RunInput, RunKind, RunPurpose,
    RunSpec, StoredUserMessage, UserMessageId, Utilization,
};

mod ai_provider;
pub use ai_provider::{
    AiEvent, AiEventStream, AiProvider, AiUsage, IsolationCheck, IsolationFailure, ResetTime,
    TokenCount, TokenUsage, ToolStatus,
};

mod event_bus;
pub use event_bus::{
    DeliveryFailure, DeliveryReport, EventBus, EventBusBuilder, EventSubscriber, SubscriberFailure,
    SubscriptionId,
};

mod read_queries;
pub use read_queries::RankingQuery;
