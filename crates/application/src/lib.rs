mod collected_item_reads;
mod observations;
mod port_error;
pub use collected_item_reads::{
    CollectedItemReads, CollectedKind, CollectedTimes, NoCollectedTimes, ObservedCollectedItem,
};
use domain::{
    AccountId, BrokerAccountRows, BrokerOrder, BrokerOrderId, BrokerOrderSet, BuyingPower,
    CalendarDate, Candle, CandlePeriod, CommissionRateFact, CreditTrade, Currency, FlowInterval,
    FxPair, FxRate, FxRateKind, IndicatorCode, InstrumentMaster, InvestorFlow,
    InvestorTradingRankings, KnownMarket, MarketCalendar, MarketIndicatorCandle,
    MarketIndicatorQuote, OrderFxRates, Orderbook, PriceAdjustment, PriceLimits, ProgramTrade,
    ProvenClosedOrderSet, QuoteInstrument, RankingLimit, ResolvedOrderInstrument, SectorCatalog,
    SectorComposition, SectorDetail, SectorEtfs, SectorId, SectorRanking, SecuritiesLending,
    SellableQuantityFact, SettlementWindow, ShortSelling, StockRanking, StockWarnings,
    TimeInterval, Timestamp, TransactionFilter,
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
    ) -> impl Future<Output = Result<ProvenClosedOrderSet, PortError>> + Send;
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
mod compute_performance;
pub use clock::Clock;
pub use compute_performance::{
    ComputePerformance, PerformanceHistorySource, PerformanceRunError, PerformanceSummary,
    SettledRowsSource,
};
mod diagnostics;
pub use diagnostics::{
    BrokerCall, CallMethod, CallResult, DiagnosticEvent, DiagnosticText, DiagnosticsSink,
    FeedDiagnosis, FeedFailure, FeedRunResult, HttpStatusCode, JobDiagnosis, JobRunNote, LinkProbe,
    SourceCall, SyncDiagnosis, SyncFailureReason, SyncJob, SyncResult, TokenIssue,
    ToolReadDiagnosis, ToolReadResult, TransportFailure, failure_reason, sync_failure_reason,
};
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
    AuditChainHash, AuditEntry, AuditHead, AuditHeadAnchor, AuditHeadError, AuditIntegrity,
    AuditKind, AuditLog, AuditSequence, WebSearchAuditError, WebSearchAuditOutcome,
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

mod ai_job_record;
pub use ai_job_record::{
    AiJobFinishRecord, AiJobOutcome, AiJobRecordStore, AiJobRecordWrite, AiJobRun, AiJobTiming,
    AiJobTimingError, AiJobUsageMeter, ListAiJobUsage, ListAiJobUsageError, RecordAiJobFinished,
};
mod cli_version_gate;
pub use cli_version_gate::{
    AiGate, CliVersion, CliVersionGateOutcome, CliVersionGateReport, CliVersionSource,
    EnsureVerifiedCliVersion, LoginRecheck, VerifiedCliVersions,
};
mod ai_preferences;
pub use ai_preferences::{
    AiEffortChoice, AiModelChoice, AiPreferences, AiPreferencesRepository, AiTask,
    AiTaskPreference, ClaudeConfigDir, ClaudeConfigDirError, ScheduledBudgetPercent,
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
pub use change_risk_settings::{
    ChangeRiskSettings, RiskAudit, RiskAuditSink, RiskSettingsChangeOutcome, WithoutRiskAudit,
};
mod audit_events;
pub use audit_events::{AuditEventRecorder, AuditEventSink};

mod update_investor_profile;
pub use update_investor_profile::{
    InvestorProfileRequest, UpdateInvestorProfile, profile_confirmations,
};

mod market_overview;
pub use market_overview::{
    GetMarketOverview, MarketOverview, OverviewData, OverviewEntry, OverviewError,
    OverviewFreshness, OverviewObservation, OverviewRequest,
};

mod market_tool_reads;
pub use market_tool_reads::{
    CandleToolLine, CandleWindow, CandleWindowError, CandlesToolView, InvestorFlowToolLine,
    InvestorFlowToolView, MarketOverviewToolView, MarketToolError, MarketToolName, MarketToolReads,
    OverviewItem, OverviewToolLine, OverviewUnit, QuoteToolView, RankingToolKind, RankingToolLine,
    RankingsToolView, SectorToolLine, SectorsToolView, SessionStatus, SessionToolLine,
    ValueProvenance,
};
mod news_tool_reads;
pub use news_tool_reads::{
    CollectedItemHandle, CollectedItemQuery, DisclosureToolLine, LookbackDays, NewsKeyword,
    NewsToolLine, NewsToolReads, ToolQueryTextError,
};
mod article_tool;
pub use article_tool::{
    ArticleBody, ArticleFetchFailure, ArticleLocation, ArticleToolPort, BodyReadingSetting,
};
mod estimate_tool_ports;
pub use estimate_tool_ports::{EstimateRunContextSource, ProfileSource, ReadOnlySettings};
mod portfolio_tool_reads;
mod thesis_history_times;
mod thesis_tool;
pub use thesis_history_times::{ThesisEntryTime, ThesisHistoryTimes, TimedThesisRepository};
pub use thesis_tool::{ThesisRunMetadata, ThesisToolPort, ThesisUpdateFailure, UpdateThesis};
mod web_research_tool;
pub use portfolio_tool_reads::{
    AllocationAxis, AllocationLabel, AllocationLabelError, AllocationLine, AllocationToolView,
    BenchmarkReturnLine, BuyingPowerLine, ConcentrationLine, HoldingToolLine, PerformanceBenchmark,
    PerformancePeriod, PerformanceToolView, PortfolioToolReads, PortfolioToolView, RiskMetrics,
    SampleSufficiency, TransactionAdmission, TransactionToolFilter, TransactionToolKind,
    TransactionToolLine, TransactionsToolView,
};
pub use web_research_tool::{
    ResearchTopic, WebResearchToolFailure, WebResearchToolOutcome, WebResearchToolPort,
    WebResearchToolRequest,
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
    EstimateCommitOutcome, EstimateSubmissionInput, EstimateSubmissionRepository,
    EstimateTaskRepository, SubmitEstimate, SubmitEstimateError,
};

mod analysis_tools;
pub use analysis_tools::{
    GetProfile, GetScorecard, GetScorecardError, ProfileView, ScorecardObservationRepository,
    ScorecardSelection, ScorecardView,
};

mod estimate_cycle;
pub use estimate_cycle::{
    BackgroundBudget, CycleAdmission, CycleProgress, EstimateCycleReport, RunEstimateCycle,
    SingleEstimateOutcome, SingleEstimateRunner,
};

mod skip_signal;
pub use skip_signal::{
    SignalSkip, SignalSkipHistory, SignalSkipRepository, SkipReason, SkipReasonError, SkipSignal,
};

mod prompt_replay;
pub use prompt_replay::{
    ComparePromptReplay, PromptReplayRunner, ReplayCase, ReplayComparison, ReplayCount,
    ReplayMetrics, ReplayOutput,
};

mod prompt_file;
pub use prompt_file::{
    LoadedPrompt, PromptBody, PromptDigest, PromptFile, PromptFileError, PromptHasher, PromptId,
    PromptLocationError, PromptRef, PromptSource, PromptTask, PromptVersion,
};

mod submit_estimate_tool;
pub use submit_estimate_tool::{
    EstimateRunContext, EstimateToolArguments, SubmitEstimateTool, SubmitEstimateToolError,
    ToolCallHistory,
};

mod transaction_observation;
pub use transaction_observation::{
    CompleteTransactionObservation, ObservedTransaction, ObservedTransactionBatch,
    RawTransactionRow,
};

mod valuation_gap;
pub use valuation_gap::{
    BackfillValuationDays, PlanValuationGap, ValuationBackfillError, ValuationCalendarSource,
    ValuationDayInputSource, ValuationDayRepository, ValuationDaySuccessSource, ValuationGapError,
    ValuationInputError,
};
mod briefing;
mod run_full_inspection;
pub use briefing::{
    BriefingContext, BriefingContextSource, BriefingFailure, BriefingOutcome, BriefingRecord,
    BriefingRepository, BriefingRunError, BriefingRunReport, BriefingSource, RunBriefing,
};
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
mod spec_check;
pub use spec_check::{
    CheckTossSpec, HttpLastModified, SchemaReviewState, SpecChangeReason, SpecCheckReport,
    SpecDocumentSnapshot, SpecDriftStatus, SpecFetchError, SpecHash, SpecStatusRecord,
    SpecVersionLabel, TossSpecBaseline, TossSpecObservation, TossSpecSource,
    TossSpecStatusRepository, evaluate_spec_drift,
};
mod open_external;
pub use open_external::{
    CollectedUrlCatalog, ExternalUrlOpened, ExternalUrlOpener, ExternalUrlOpenerError,
    OpenExternalError, OpenExternalOutcome, OpenExternalUrl,
};
mod watchlist;
pub use watchlist::{
    ManageWatchlist, TrackedUniverse, TrackingOrigin, TrackingTarget, Watchlist, WatchlistCount,
    WatchlistEntry, WatchlistError, WatchlistMembership, WatchlistOutcome, WatchlistRepository,
    WatchlistStoreOutcome,
};
mod estimate_tasks;
pub use estimate_tasks::{
    CreateEstimateTask, CreateEstimateTaskError, CreateEstimateTaskInput,
    CreateEstimateTaskOutcome, EstimateAssignmentRepository, EstimateTaskView, GetEstimateTask,
    GetEstimateTaskError, GetEstimateTaskInput,
};

mod source_status;
pub use source_status::{
    GetSourceStatuses, RecordSourceStatus, SourceAttemptOutcome, SourceState, SourceStatus,
    SourceStatusError, SourceStatusReport, SourceStatusRepository,
};

mod collection_schedule;
pub use collection_schedule::{
    CatchUpItems, CollectionCadence, CollectionConditions, CollectionInputs, CollectionMinutes,
    CollectionMinutesError, CollectionPhase, CollectionPlan, CollectionScheduler, CollectionTarget,
    DartCallCount, DartDailyUsage, DartKeyState, DartQuotaLevel, FomcDay, PlanCollectionSchedule,
    PlannedCollection, StartupCatchUp, collection_cadence, collection_phase, startup_catch_up,
};

mod record_wake_trigger;
pub use record_wake_trigger::{
    RecordWakeTriggers, TriggerWrite, WakeTriggerOutcome, WakeTriggerRepository,
};

mod stock_web_collection;
pub use stock_web_collection::{
    RunStockWebCollection, STOCK_WEB_TOPICS, StockWebCollectionOutcome, StockWebCollectionReport,
    StockWebCollectionResult, WebCollectionFailure, WebCollectionReservation, WebCollectionRunner,
    WebCollectionStore, WebFinding, plan_stock_web_collection,
};

mod important_disclosures;
pub use important_disclosures::{
    DisclosureCardFact, DisclosureCardLevel, DisclosureTriggerLog, GetImportantDisclosureCards,
    ImportantDisclosureCard,
};

mod news_data_settings;
pub use news_data_settings::{
    AliasName, GetNewsDataSettings, NewsDataSettings, NewsDataSettingsChange,
    NewsDataSettingsError, NewsDataSettingsRepository, NewsSettingValueError, SecContactSetting,
    SourceSwitch, StockAlias, UpdateNewsDataSettings,
};

mod candidate_discovery;
pub use candidate_discovery::{
    DiscoverCandidates, DiscoveryError, DiscoveryList, DiscoveryLists, DiscoveryReport,
};
mod weekly_review;
pub use weekly_review::{
    RunWeeklyReview, ScorecardRefresh, ScorecardUpdate, WeeklyFindings, WeeklyNarrative,
    WeeklyReviewCard, WeeklyReviewCards, WeeklyReviewError, WeeklyReviewInputs,
    WeeklyReviewNarrator, WeeklyReviewRun, WeeklyReviewSource,
};
mod paired_comparison;
pub use paired_comparison::{
    ComparisonArm, ComparisonArmResult, ComparisonArmRunner, PairedComparison, PairedComparisonLog,
    PairingError, PairingRun, RunPairedComparison,
};
mod deep_research;
pub use deep_research::{
    AnalysisFailure, ClaimVerifier, DeepResearchError, DeepResearchPipeline, DeepResearchRequest,
    DeepResearchRun, DeepResearchStages, DraftResearchCases, FinishResearch, FinishResearchError,
    GatherEvidenceError, GatherResearchEvidence, GatheredEvidence, ResearchAnalyst,
    ResearchCaseError, ResearchCases, ResearchCheckpoint, ResearchCheckpointStore,
    ResearchConcluder, ResearchConclusion, ResearchEvidenceSource, ResearchModels, ResearchOutcome,
    ResearchPrincipleCheck, ResearchStage, ResearchWebCollector, ResearchWebOutcome,
    ResearchWebStore, RunDeepResearch, VerifyResearchClaims,
};
pub use deep_research::{
    DeepResearchCycleError, DeepResearchCycleProgress, DeepResearchCycleReport, DeepResearchJob,
    DeepResearchPause, DeepResearchTargetOutcome, RunDeepResearchCycle,
};
mod web_search_guard;
pub use web_search_guard::{UserDirectWebSearchQuery, WebSearchGuard, WebSearchGuardError};

mod order_gate;
pub use order_gate::{
    IntentRecheckOutcome, OrderGateCollectedFacts, OrderGateError, OrderGateOutcome,
    OrderGatePolicyContext, ValidateOrderProposal,
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

mod investment_principles;
pub use investment_principles::{
    ApplyInvestmentPrinciples, PrinciplesRepository, UpdateInvestmentPrinciples,
};

mod chat_answer_mode;
pub use chat_answer_mode::{ChatAnswerMode, ChatAnswerModeTag, chat_answer_mode};
mod chat_number_check;
pub use chat_number_check::{
    ChatNumberCheck, ChatNumberEvidence, ChatNumberEvidenceSet, NoNumberEvidence,
    check_chat_numbers,
};

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
mod track_orders;
pub use track_orders::{OrderHistoryRow, TrackSubmittedOrders, TrackedOrder, TrackedOrderUpdate};
mod order_reconciliation;
pub use order_reconciliation::{ReconcileOrdersError, ReconcileSubmittedOrders, SubmissionLedger};
mod order_button;
pub use order_button::ReadOrderButton;
mod order_rate_limit;
pub use order_rate_limit::{CheckOrderRateLimit, OrderRateRepository};

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
mod database_restore;
pub use database_restore::{RestoreOutcome, announce_restore};

mod collect_feed;
pub use collect_feed::{
    CollectFeed, CollectedCount, CollectionDue, FeedRound, FeedRoundOutcome, collection_due,
    feed_data_source,
};

mod briefing_context;
pub use briefing_context::HoldingsBriefingContext;
mod briefing_market;
pub use briefing_market::{MarketBriefingContext, market_briefing_facts};

mod headline_grouping;
pub use headline_grouping::{
    HeadlineAdmission, HeadlineKey, RecentHeadlines, StoryGroup, StorySimilarity, group_same_story,
    same_story_window,
};

mod tracked_news;
pub use tracked_news::{CollectedPresence, CollectedPresenceCheck, TrackedInstrument};

mod news_retention;
pub use news_retention::{
    CollectedRetention, PruneCollectedNews, RetentionCutoff, RetentionDays, news_retention_cutoffs,
};

mod intraday_review;
pub use intraday_review::{IntradayReviewOutcome, IntradayReviewSubject, RunIntradayReview};

mod single_estimate;
pub use single_estimate::{
    EstimateRunIdentity, EstimateRunRegistry, EstimateTaskMaterialSource, EstimateTaskMaterials,
    EstimateTrigger, RunSingleEstimate, estimate_prompt_source,
};

mod job_schedule_store;
pub use job_schedule_store::{
    JobClockError, JobClockTime, JobCode, JobCodeError, JobFailureKind, JobHour, JobItemCount,
    JobMinute, JobRunEntry, JobRunResult, JobScheduleStore, JobSummary, JobSwitch,
    SavedJobSchedule,
};
