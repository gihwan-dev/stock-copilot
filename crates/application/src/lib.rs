mod observations;
mod port_error;
use domain::{
    AccountId, BrokerAccountRows, BrokerOrder, BrokerOrderId, BrokerOrderSet, BuyingPower,
    CalendarDate, Candle, CandlePeriod, ClosedOrderSet, CommissionRateFact, CreditTrade, Currency,
    FlowInterval, FxPair, FxRate, FxRateKind, IndicatorCode, InstrumentMaster, InvestorFlow,
    InvestorTradingRankings, KnownMarket, MarketCalendar, MarketIndicatorCandle,
    MarketIndicatorQuote, OrderFxRates, Orderbook, PriceAdjustment, PriceLimits, ProgramTrade,
    QuoteInstrument, RankingLimit, ResolvedOrderInstrument, SectorCatalog, SectorComposition,
    SectorDetail, SectorEtfs, SectorId, SectorRanking, SecuritiesLending, SellableQuantityFact,
    SettlementWindow, ShortSelling, StockRanking, StockWarnings, TimeInterval, Timestamp,
    TransactionFilter,
};
pub use observations::{
    HoldingsObservation, IncompleteReason, IncompleteTransactions, QuarantinedTransaction,
    QuoteObservation, TransactionRead,
};
pub use port_error::{
    AccessDenial, HttpStatus, MasterNotReady, MissingData, PortError, PortErrorKind, ReadEndpoint,
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
    InvestorTradingRankings,
    Sectors,
    Sector,
    SectorRankings,
    SectorEtfs,
    ProgramTrades,
    ShortSelling,
    CreditTrades,
    SecuritiesLending,
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
    pub fn with_supported(mut self, capability: BrokerCapability) -> Self {
        self.0.insert(capability);
        self
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
        evaluation_window: TimeInterval,
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
    ) -> impl Future<Output = Result<InstrumentMaster, PortError>> + Send;
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
    fn program_trades(
        &self,
        instrument: &QuoteInstrument,
        from: CalendarDate,
        through: CalendarDate,
    ) -> impl Future<Output = Result<Vec<ProgramTrade>, PortError>> + Send;
    fn short_selling(
        &self,
        instrument: &QuoteInstrument,
        from: CalendarDate,
        through: CalendarDate,
    ) -> impl Future<Output = Result<Vec<ShortSelling>, PortError>> + Send;
    fn credit_trades(
        &self,
        instrument: &QuoteInstrument,
        from: CalendarDate,
        through: CalendarDate,
    ) -> impl Future<Output = Result<Vec<CreditTrade>, PortError>> + Send;
    fn securities_lending(
        &self,
        instrument: &QuoteInstrument,
        from: CalendarDate,
        through: CalendarDate,
    ) -> impl Future<Output = Result<Vec<SecuritiesLending>, PortError>> + Send;

    fn investor_trading_rankings(
        &self,
        limit: RankingLimit,
    ) -> impl Future<Output = Result<InvestorTradingRankings, PortError>> + Send;
    fn sectors(&self) -> impl Future<Output = Result<SectorCatalog, PortError>> + Send;
    fn sector(
        &self,
        sector: &SectorId,
        market: KnownMarket,
    ) -> impl Future<Output = Result<SectorDetail, PortError>> + Send;
    fn sector_rankings(
        &self,
        query: &SectorRankingQuery,
    ) -> impl Future<Output = Result<SectorRanking, PortError>> + Send;
    fn sector_etfs(
        &self,
        sector: &SectorId,
        market: KnownMarket,
    ) -> impl Future<Output = Result<SectorEtfs, PortError>> + Send;

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

mod system_port_error;
pub use system_port_error::{
    RecordOutcome, SystemDiagnostic, SystemFailure, SystemPort, SystemPortError, WriteCompleted,
};
mod clock;
pub use clock::Clock;
mod notifier;
pub use notifier::{Notification, Notifier};
mod secret_store;
pub use secret_store::SecretValue;
mod secret_port;
pub use secret_port::{SecretKey, SecretStore};
pub mod user_presence;
pub use user_presence::{OrderConfirmation, RiskChangeConfirmation};
pub mod identity_confirmation_prompt;
pub use identity_confirmation_prompt::{
    IdentifierChangePrompt, IdentityConfirmationDecision, LinkCorrectionPrompt,
    RetirementCorrectionPrompt,
};
mod confirm_instrument_identity;
pub use confirm_instrument_identity::{ConfirmInstrumentIdentity, IdentityConfirmationOutcome};
mod audit_log;
pub use audit_log::{
    AuditEntry, AuditIntegrity, AuditKind, AuditLog, WebSearchAuditError, WebSearchAuditOutcome,
    WebSearchAuditRecord, WebSearchQueryMode,
};
mod instrument_mapping;
pub use instrument_mapping::{
    InstrumentIdIssuer, InstrumentMappingCorrectionRepository, InstrumentMappingHistoryRepository,
    InstrumentMappingRepository, InstrumentMasterAuthority, InstrumentMasterObservation,
    InstrumentMasterSession, MappingStoreError, MappingTransitionStoreError,
};
mod storage_ports;
pub use storage_ports::ValuationCloseEvidenceRepository;
pub use storage_ports::{
    CacheWriteOutcome, ConditionalDraftOutcome, CurrentDraftOutcome, DraftCheck,
    DraftStoreCandidate, EstimateRepository, FxRateQuery, MarketDataRepository, NewsRepository,
    PortfolioRepository, ProposalRepository, SettingsRepository, SignalRepository,
    ThesisRepository, TransactionRepository, ValuationRepository,
};

mod buy_limit_intent;
pub use buy_limit_intent::{BuyLimitIntentError, buy_limit_intent};
mod sell_limit_intent;
pub use sell_limit_intent::{SellLimitIntentError, sell_limit_intent};
mod draft_proposal;
pub use draft_proposal::{
    DraftProposal, DraftProposalData, DraftProposalError, DraftProposalInputs,
    DraftProposalOutcome, ReestimateScheduler,
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
pub use read_queries::{RankingQuery, SectorRankingQuery};
mod self_check;
pub use self_check::{
    AiCheckObservation, AiEnvironmentProblem, CheckState, ClaudeInstallProblem, ClaudeLoginProblem,
    DuplicateKeyProblem, IpProblem, NewsCheckState, OrderPrerequisiteProblem, SelfCheckAction,
    SelfCheckChange, SelfCheckInput, SelfCheckResult, SourceCheckRecord, TossCheckObservation,
    TossKeyProblem, evaluate_self_check, isolation_check_observation, self_check_change,
    toss_check_observation,
};
mod sync_transactions;
pub use sync_transactions::{
    SyncTransactions, TransactionSyncError, TransactionSyncRecord, TransactionSyncRejection,
    TransactionSyncRepository,
};

mod sync_portfolio;
pub use sync_portfolio::{PortfolioSyncReport, SyncPortfolio};
mod change_risk_settings;
pub use change_risk_settings::{ChangeRiskSettings, RiskSettingsChangeOutcome};

mod market_overview;
pub use market_overview::{
    GetMarketOverview, MarketOverview, OverviewData, OverviewEntry, OverviewError,
    OverviewFreshness, OverviewObservation, OverviewRequest,
};

mod portfolio_views;
pub use portfolio_views::{
    GetAllocationView, GetPortfolioView, RecordedView, StoredView, ViewFreshness,
};
mod valuation_snapshot;
pub use valuation_snapshot::{
    TakeValuationSnapshot, ValuationSnapshotError, ValuationSnapshotReport,
};

mod transaction_schedule;
pub use transaction_schedule::{
    FullInspectionRepository, TransactionSchedule, TransactionScheduleError,
};

mod submit_estimate;
pub use submit_estimate::{
    EstimateCommitOutcome, EstimateSubmissionInput, EstimateSubmissionRepository, SubmitEstimate,
    SubmitEstimateError,
};

mod transaction_observation;
pub use transaction_observation::{
    CompleteTransactionObservation, ObservedTransaction, ObservedTransactionBatch,
    RawTransactionRow,
};

mod valuation_gap;
pub use valuation_gap::{
    PlanValuationGap, ValuationCalendarSource, ValuationDayRepository, ValuationGapError,
};
mod run_full_inspection;
pub use run_full_inspection::{
    FullInspectionReport, FullInspectionRun, FullInspectionRunError, RunFullInspection,
};

mod store_fx_rate;
pub use store_fx_rate::{RefreshFxRate, RefreshFxRateError, RefreshFxRateReport, StoreFxRate};
mod signal_ports;
pub use signal_ports::{
    SignalCommit, SignalCommitEvidence, SignalCommitOutcome, SignalEvaluation,
    SignalInvalidationOutcome, SignalInvalidationRepository, SignalMutation, SignalRevision,
    SignalSnapshot, SignalSourceFacts, StoredSignal,
};

pub mod resolve_estimates;

mod information_storage;
pub use information_storage::{
    DisclosureRepository, InformationKey, InformationLinks, InformationSearch, SearchLimit,
    SearchLimitError,
};

mod evaluate_signals;
pub use evaluate_signals::{
    EvaluateSignalError, EvaluateSignalInputs, EvaluateSignalOutcome, EvaluateSignals,
    SignalEvaluationData, SignalQuantityCosts, SignalRestorationBasis,
};

mod proposal_storage;
pub use proposal_storage::{
    ConsumptionOutcome, ConsumptionStatus, ProposalConsumptionRepository,
    ValidationReportRepository,
};
mod open_external;
pub use open_external::{
    CollectedUrlCatalog, ExternalUrlOpened, ExternalUrlOpener, ExternalUrlOpenerError,
    OpenExternalError, OpenExternalOutcome, OpenExternalUrl,
};

mod source_status;
pub use source_status::{
    GetSourceStatuses, RecordSourceStatus, SourceAttemptOutcome, SourceState, SourceStatus,
    SourceStatusError, SourceStatusReport, SourceStatusRepository,
};

mod web_search_guard;
pub use web_search_guard::{UserDirectWebSearchQuery, WebSearchGuard, WebSearchGuardError};
mod order_gate;
pub use order_gate::{
    OrderGateCollectedFacts, OrderGateError, OrderGateOutcome, OrderGatePolicyContext,
    ValidateOrderProposal,
};

mod chat_types;
pub use chat_types::{
    BackgroundPreemption, ChatCard, ChatCardId, ChatCardKind, ChatChunk, ChatExecutionMode,
    ChatFailureReason, ChatQueueAcquisition, ChatRunLedgerEntry, ChatRunModelSpec, ChatRunProfile,
    ChatRunTiming, ChatScreenContext, ChatSequence, ChatSessionId, ChatSessionRecord,
    ChatSessionSummary, ChatSignalState, ChatSinkClosed, ChatSource, ChatSourceKind, ChatTurnId,
    ChatTurnIdentity, ChatTurnMetrics, ChatTurnStatus, ChatTurnSummary, StoredAssistantAttachments,
    StoredAssistantReply, StoredChatContent, StoredChatEntry,
};

mod chat_ports;
pub use chat_ports::{ChatChunkSink, ChatQueuePort, ChatRepository, ChatRunLedger, ChatStopSignal};

mod send_chat_message;
pub use send_chat_message::{
    AnalysisToolAllowance, ChatModelConfig, SendChatMessage, SendChatOutcome, SendChatRequest,
    build_chat_prompt, check_analysis_tool,
};

mod chat_web_research;
pub use chat_web_research::{
    ChatWebResearch, ChatWebResearchFailure, ChatWebResearchFindings, ChatWebResearchRequest,
};

mod peer_process;
pub use peer_process::{
    ExecutablePath, PeerProcessError, PeerProcessInfo, PeerProcessInspector,
    PeerProcessLookupError, ProcessId,
};
mod tool_call_ledger;
pub use tool_call_ledger::{
    ToolCallLedger, ToolCallOutcome, ToolCallRecord, ToolCallRecordInput, ToolLedgerError,
    ToolName, ToolPayloadJson, ToolProtocolVersion, ToolRequestId,
};

mod warm_chat;
pub use warm_chat::{
    McpConnectionState, PrewarmChat, PrewarmChatOutcome, SendWarmChatMessage, WarmChatId,
    WarmChatProbe, WarmChatReadiness, WarmChatRejection, WarmChatRequest, WarmChatRuntime,
    WarmChatSession, WarmFreshness, WarmMatch,
};
