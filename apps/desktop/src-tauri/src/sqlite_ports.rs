use application::{
    AiRunId, AuditEntry, AuditIntegrity, AuditLog, ChatNumberEvidence, ChatNumberEvidenceSet,
    ChatRepository, ChatRunLedger, ChatRunLedgerEntry, ChatSessionId, ChatSessionRecord,
    ChatSessionSummary, ConditionalDraftOutcome, EstimateRepository, FxRateQuery,
    MarketDataRepository, NewsRepository, ProposalRepository, RecordOutcome, RetryDelay,
    SettingsRepository, SignalCommitOutcome, SignalMutation, SignalRepository, SignalSnapshot,
    SignalSourceFacts, StoredChatEntry, StoredUserMessage, SystemFailure, SystemPort,
    SystemPortError, ThesisRepository, TransactionRepository, UserMessageId, ValuationRepository,
    WriteCompleted,
};
use domain::{
    AccountId, Calibration, CalibrationKey, Candle, CandlePeriod, Draft, DraftRestoreEvidence,
    EstimateOutcome, FxPair, FxRate, FxRateKind, HistoricalSignal, HistoricalSignalOccurrence,
    InstrumentId, InvestorProfile, IssuedEstimateTaskId, MasterInstrument, NewsItem,
    PriceAdjustment, Proposal, ProposalId, Quote, ResolvedOrderInstrument, RiskSettings,
    SettledRowSet, SignalSubmission, SignalTaskKey, SyncRecord, Thesis, TimeInterval, Timestamp,
    ValuationSnapshot,
};
use std::sync::Arc;
use store_sqlite::{
    AuditStore, ChatStore, Database, EstimateStore, InformationStore, MarketStore, SettingsStore,
    SignalStore, ThesisStore, TransactionStore, ValuationStore,
};
use tokio::sync::{RwLock, RwLockReadGuard};

#[derive(Clone, Default)]
pub struct SharedDatabase(Arc<RwLock<Option<Database>>>);

impl std::fmt::Debug for SharedDatabase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SharedDatabase(<redacted>)")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SharedClosure {
    NotOpened,
    Closed(WriteCompleted),
}

impl SharedDatabase {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn install(&self, database: Database) -> Option<Database> {
        self.0.write().await.replace(database)
    }

    pub async fn close(&self) -> Result<SharedClosure, SystemPortError> {
        let taken = self.0.write().await.take();
        match taken {
            Some(database) => database.close().await.map(SharedClosure::Closed),
            None => Ok(SharedClosure::NotOpened),
        }
    }

    pub async fn read(&self) -> RwLockReadGuard<'_, Option<Database>> {
        self.0.read().await
    }
}

#[must_use]
pub const fn not_opened(port: SystemPort) -> SystemPortError {
    SystemPortError::new(port, SystemFailure::Unavailable(RetryDelay::Unknown))
}

fn opened(guard: Option<&Database>, port: SystemPort) -> Result<&Database, SystemPortError> {
    guard.ok_or_else(|| not_opened(port))
}

#[derive(Debug, Clone, Default)]
pub struct SqliteRepositories {
    shared: SharedDatabase,
}

impl SqliteRepositories {
    #[must_use]
    pub const fn new(shared: SharedDatabase) -> Self {
        Self { shared }
    }

    #[must_use]
    pub const fn shared(&self) -> &SharedDatabase {
        &self.shared
    }
}

const REPOSITORY: SystemPort = SystemPort::Repository;

impl TransactionRepository for SqliteRepositories {
    async fn record(
        &self,
        account: &AccountId,
        record: &SyncRecord<SettledRowSet>,
    ) -> Result<WriteCompleted, SystemPortError> {
        let guard = self.shared.read().await;
        let database = opened(guard.as_ref(), REPOSITORY)?;
        TransactionRepository::record(&TransactionStore::new(database), account, record).await
    }
    async fn latest(
        &self,
        account: &AccountId,
    ) -> Result<Option<SyncRecord<SettledRowSet>>, SystemPortError> {
        let guard = self.shared.read().await;
        let database = opened(guard.as_ref(), REPOSITORY)?;
        TransactionRepository::latest(&TransactionStore::new(database), account).await
    }
}

impl MarketDataRepository for SqliteRepositories {
    async fn store_quote(
        &self,
        instrument: &InstrumentId,
        quote: &Quote,
    ) -> Result<application::CacheWriteOutcome, SystemPortError> {
        let guard = self.shared.read().await;
        let database = opened(guard.as_ref(), REPOSITORY)?;
        MarketStore::new(database)
            .store_quote(instrument, quote)
            .await
    }
    async fn quote(&self, instrument: &InstrumentId) -> Result<Option<Quote>, SystemPortError> {
        let guard = self.shared.read().await;
        let database = opened(guard.as_ref(), REPOSITORY)?;
        MarketStore::new(database).quote(instrument).await
    }
    async fn store_candles(
        &self,
        instrument: &InstrumentId,
        candles: &[Candle],
    ) -> Result<WriteCompleted, SystemPortError> {
        let guard = self.shared.read().await;
        let database = opened(guard.as_ref(), REPOSITORY)?;
        MarketStore::new(database)
            .store_candles(instrument, candles)
            .await
    }
    async fn candles(
        &self,
        instrument: &InstrumentId,
        window: TimeInterval,
        period: CandlePeriod,
        adjustment: PriceAdjustment,
    ) -> Result<Vec<Candle>, SystemPortError> {
        let guard = self.shared.read().await;
        let database = opened(guard.as_ref(), REPOSITORY)?;
        MarketStore::new(database)
            .candles(instrument, window, period, adjustment)
            .await
    }
    async fn store_fx_rate(
        &self,
        rate: &FxRate,
    ) -> Result<application::CacheWriteOutcome, SystemPortError> {
        let guard = self.shared.read().await;
        let database = opened(guard.as_ref(), REPOSITORY)?;
        MarketStore::new(database).store_fx_rate(rate).await
    }
    async fn fx_rate(
        &self,
        pair: FxPair,
        kind: FxRateKind,
        at: FxRateQuery,
    ) -> Result<Option<FxRate>, SystemPortError> {
        let guard = self.shared.read().await;
        let database = opened(guard.as_ref(), REPOSITORY)?;
        MarketStore::new(database).fx_rate(pair, kind, at).await
    }
}

impl NewsRepository for SqliteRepositories {
    async fn store(&self, item: &NewsItem) -> Result<WriteCompleted, SystemPortError> {
        let guard = self.shared.read().await;
        let database = opened(guard.as_ref(), REPOSITORY)?;
        NewsRepository::store(&InformationStore::new(database), item).await
    }
    async fn for_instrument(
        &self,
        instrument: &InstrumentId,
    ) -> Result<Vec<NewsItem>, SystemPortError> {
        let guard = self.shared.read().await;
        let database = opened(guard.as_ref(), REPOSITORY)?;
        NewsRepository::for_instrument(&InformationStore::new(database), instrument).await
    }
}

impl ThesisRepository for SqliteRepositories {
    async fn store(
        &self,
        instrument: &InstrumentId,
        thesis: &Thesis,
    ) -> Result<WriteCompleted, SystemPortError> {
        let guard = self.shared.read().await;
        let database = opened(guard.as_ref(), REPOSITORY)?;
        ThesisRepository::store(&ThesisStore::new(database), instrument, thesis).await
    }
    async fn load(&self, instrument: &InstrumentId) -> Result<Option<Thesis>, SystemPortError> {
        let guard = self.shared.read().await;
        let database = opened(guard.as_ref(), REPOSITORY)?;
        ThesisRepository::load(&ThesisStore::new(database), instrument).await
    }
}

impl EstimateRepository for SqliteRepositories {
    async fn append_submission(
        &self,
        submission: &SignalSubmission,
    ) -> Result<RecordOutcome, SystemPortError> {
        let guard = self.shared.read().await;
        let database = opened(guard.as_ref(), REPOSITORY)?;
        EstimateRepository::append_submission(&EstimateStore::new(database), submission).await
    }
    async fn submissions(
        &self,
        instrument: &InstrumentId,
    ) -> Result<Vec<SignalSubmission>, SystemPortError> {
        let guard = self.shared.read().await;
        let database = opened(guard.as_ref(), REPOSITORY)?;
        EstimateRepository::submissions(&EstimateStore::new(database), instrument).await
    }
    async fn append_resolution(
        &self,
        task: &SignalTaskKey,
        outcome: &EstimateOutcome,
    ) -> Result<RecordOutcome, SystemPortError> {
        let guard = self.shared.read().await;
        let database = opened(guard.as_ref(), REPOSITORY)?;
        EstimateRepository::append_resolution(&EstimateStore::new(database), task, outcome).await
    }
    async fn resolution(
        &self,
        task: &SignalTaskKey,
    ) -> Result<Option<EstimateOutcome>, SystemPortError> {
        let guard = self.shared.read().await;
        let database = opened(guard.as_ref(), REPOSITORY)?;
        EstimateRepository::resolution(&EstimateStore::new(database), task).await
    }
    async fn append_calibration(
        &self,
        key: &CalibrationKey,
        calibration: &Calibration,
    ) -> Result<RecordOutcome, SystemPortError> {
        let guard = self.shared.read().await;
        let database = opened(guard.as_ref(), REPOSITORY)?;
        EstimateRepository::append_calibration(&EstimateStore::new(database), key, calibration)
            .await
    }
    async fn calibration(
        &self,
        key: &CalibrationKey,
    ) -> Result<Option<Calibration>, SystemPortError> {
        let guard = self.shared.read().await;
        let database = opened(guard.as_ref(), REPOSITORY)?;
        EstimateRepository::calibration(&EstimateStore::new(database), key).await
    }
}

impl SignalRepository for SqliteRepositories {
    async fn snapshot(&self, instrument: &InstrumentId) -> Result<SignalSnapshot, SystemPortError> {
        let guard = self.shared.read().await;
        let database = opened(guard.as_ref(), REPOSITORY)?;
        SignalRepository::snapshot(&SignalStore::new(database), instrument).await
    }
    async fn commit(
        &self,
        instrument: &InstrumentId,
        expected: &SignalSnapshot,
        mutation: &SignalMutation,
    ) -> Result<SignalCommitOutcome, SystemPortError> {
        let guard = self.shared.read().await;
        let database = opened(guard.as_ref(), REPOSITORY)?;
        SignalRepository::commit(&SignalStore::new(database), instrument, expected, mutation).await
    }
    async fn occurrences(
        &self,
        instrument: &InstrumentId,
        identity: &ResolvedOrderInstrument,
    ) -> Result<Vec<HistoricalSignalOccurrence>, SystemPortError> {
        let guard = self.shared.read().await;
        let database = opened(guard.as_ref(), REPOSITORY)?;
        SignalRepository::occurrences(&SignalStore::new(database), instrument, identity).await
    }
    async fn sources(
        &self,
        task: &IssuedEstimateTaskId,
    ) -> Result<SignalSourceFacts, SystemPortError> {
        let guard = self.shared.read().await;
        let database = opened(guard.as_ref(), REPOSITORY)?;
        SignalRepository::sources(&SignalStore::new(database), task).await
    }
    async fn current(
        &self,
        instrument: &InstrumentId,
    ) -> Result<Vec<HistoricalSignal>, SystemPortError> {
        let guard = self.shared.read().await;
        let database = opened(guard.as_ref(), REPOSITORY)?;
        SignalRepository::current(&SignalStore::new(database), instrument).await
    }
}

impl ProposalRepository for SqliteRepositories {
    async fn current_draft(
        &self,
        instrument: &InstrumentId,
        expected: &SignalSnapshot,
        check: application::DraftCheck<'_>,
    ) -> Result<application::CurrentDraftOutcome, SystemPortError> {
        let guard = self.shared.read().await;
        let database = opened(guard.as_ref(), REPOSITORY)?;
        ProposalRepository::current_draft(&SignalStore::new(database), instrument, expected, check)
            .await
    }
    async fn store_current_draft(
        &self,
        instrument: &InstrumentId,
        expected: &SignalSnapshot,
        candidate: application::DraftStoreCandidate<'_>,
        check: application::DraftCheck<'_>,
    ) -> Result<ConditionalDraftOutcome, SystemPortError> {
        let guard = self.shared.read().await;
        let database = opened(guard.as_ref(), REPOSITORY)?;
        ProposalRepository::store_current_draft(
            &SignalStore::new(database),
            instrument,
            expected,
            candidate,
            check,
        )
        .await
    }
    async fn store_draft(
        &self,
        proposal: &Proposal<Draft>,
        evidence: &DraftRestoreEvidence,
        master: &MasterInstrument,
    ) -> Result<WriteCompleted, SystemPortError> {
        let guard = self.shared.read().await;
        let database = opened(guard.as_ref(), REPOSITORY)?;
        ProposalRepository::store_draft(&SignalStore::new(database), proposal, evidence, master)
            .await
    }
    async fn draft(
        &self,
        proposal: &ProposalId,
        master: &MasterInstrument,
    ) -> Result<Option<Proposal<Draft>>, SystemPortError> {
        let guard = self.shared.read().await;
        let database = opened(guard.as_ref(), REPOSITORY)?;
        ProposalRepository::draft(&SignalStore::new(database), proposal, master).await
    }
}

impl ValuationRepository for SqliteRepositories {
    async fn append(
        &self,
        account: &AccountId,
        snapshot: &ValuationSnapshot,
    ) -> Result<RecordOutcome, SystemPortError> {
        let guard = self.shared.read().await;
        let database = opened(guard.as_ref(), REPOSITORY)?;
        ValuationRepository::append(&ValuationStore::new(database), account, snapshot).await
    }
    async fn history(
        &self,
        account: &AccountId,
        window: TimeInterval,
    ) -> Result<Vec<ValuationSnapshot>, SystemPortError> {
        let guard = self.shared.read().await;
        let database = opened(guard.as_ref(), REPOSITORY)?;
        ValuationRepository::history(&ValuationStore::new(database), account, window).await
    }
}

impl SettingsRepository for SqliteRepositories {
    async fn store_profile(
        &self,
        profile: &InvestorProfile,
    ) -> Result<WriteCompleted, SystemPortError> {
        let guard = self.shared.read().await;
        let database = opened(guard.as_ref(), REPOSITORY)?;
        SettingsRepository::store_profile(&SettingsStore::new(database), profile).await
    }
    async fn profile(&self) -> Result<Option<InvestorProfile>, SystemPortError> {
        let guard = self.shared.read().await;
        let database = opened(guard.as_ref(), REPOSITORY)?;
        SettingsRepository::profile(&SettingsStore::new(database)).await
    }
    async fn store_risk_settings(
        &self,
        settings: &RiskSettings,
    ) -> Result<WriteCompleted, SystemPortError> {
        let guard = self.shared.read().await;
        let database = opened(guard.as_ref(), REPOSITORY)?;
        SettingsRepository::store_risk_settings(&SettingsStore::new(database), settings).await
    }
    async fn risk_settings(&self) -> Result<Option<RiskSettings>, SystemPortError> {
        let guard = self.shared.read().await;
        let database = opened(guard.as_ref(), REPOSITORY)?;
        SettingsRepository::risk_settings(&SettingsStore::new(database)).await
    }
}

impl AuditLog for SqliteRepositories {
    async fn append(&self, entry: &AuditEntry) -> Result<RecordOutcome, SystemPortError> {
        let guard = self.shared.read().await;
        let database = opened(guard.as_ref(), SystemPort::AuditLog)?;
        AuditLog::append(&AuditStore::new(database), entry).await
    }
    async fn verify(&self) -> Result<AuditIntegrity, SystemPortError> {
        let guard = self.shared.read().await;
        let database = opened(guard.as_ref(), SystemPort::AuditLog)?;
        AuditLog::verify(&AuditStore::new(database)).await
    }
}

impl ChatRepository for SqliteRepositories {
    async fn ensure_session(
        &self,
        session: &ChatSessionId,
        now: Timestamp,
    ) -> Result<ChatSessionRecord, SystemPortError> {
        let guard = self.shared.read().await;
        let database = opened(guard.as_ref(), REPOSITORY)?;
        ChatRepository::ensure_session(&ChatStore::new(database), session, now).await
    }
    async fn load_session(
        &self,
        session: &ChatSessionId,
    ) -> Result<Option<ChatSessionRecord>, SystemPortError> {
        let guard = self.shared.read().await;
        let database = opened(guard.as_ref(), REPOSITORY)?;
        ChatRepository::load_session(&ChatStore::new(database), session).await
    }
    async fn list_sessions(&self) -> Result<Vec<ChatSessionSummary>, SystemPortError> {
        let guard = self.shared.read().await;
        let database = opened(guard.as_ref(), REPOSITORY)?;
        ChatRepository::list_sessions(&ChatStore::new(database)).await
    }
    async fn append_entry(
        &self,
        session: &ChatSessionId,
        entry: &StoredChatEntry,
    ) -> Result<WriteCompleted, SystemPortError> {
        let guard = self.shared.read().await;
        let database = opened(guard.as_ref(), REPOSITORY)?;
        ChatRepository::append_entry(&ChatStore::new(database), session, entry).await
    }
    async fn load_user_message(
        &self,
        session: &ChatSessionId,
        message_id: &UserMessageId,
    ) -> Result<Option<StoredUserMessage>, SystemPortError> {
        let guard = self.shared.read().await;
        let database = opened(guard.as_ref(), REPOSITORY)?;
        ChatRepository::load_user_message(&ChatStore::new(database), session, message_id).await
    }
}

impl ChatRunLedger for SqliteRepositories {
    async fn record_chat_run(
        &self,
        entry: &ChatRunLedgerEntry,
    ) -> Result<RecordOutcome, SystemPortError> {
        let guard = self.shared.read().await;
        let database = opened(guard.as_ref(), REPOSITORY)?;
        ChatRunLedger::record_chat_run(&ChatStore::new(database), entry).await
    }
}

impl ChatNumberEvidence for SqliteRepositories {
    fn evidence_for_run(
        &self,
        _run: &AiRunId,
    ) -> impl Future<Output = Result<ChatNumberEvidenceSet, SystemPortError>> + Send {
        std::future::ready(Ok(ChatNumberEvidenceSet::NotConfigured))
    }
}

pub struct SharedDatabaseShutdownHook {
    inner: Arc<dyn crate::platform::process_cleanup::ShutdownHook>,
    shared: SharedDatabase,
    closure: tokio::sync::Mutex<Option<Result<SharedClosure, SystemPortError>>>,
}

impl std::fmt::Debug for SharedDatabaseShutdownHook {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SharedDatabaseShutdownHook")
    }
}

impl SharedDatabaseShutdownHook {
    #[must_use]
    pub fn new(
        inner: Arc<dyn crate::platform::process_cleanup::ShutdownHook>,
        shared: SharedDatabase,
    ) -> Self {
        Self {
            inner,
            shared,
            closure: tokio::sync::Mutex::new(None),
        }
    }

    pub async fn closure(&self) -> Option<Result<SharedClosure, SystemPortError>> {
        *self.closure.lock().await
    }
}

impl crate::platform::process_cleanup::ShutdownHook for SharedDatabaseShutdownHook {
    fn on_shutdown(
        &self,
    ) -> crate::platform::process_cleanup::CleanupFuture<
        '_,
        crate::platform::process_cleanup::ProcessShutdownSummary,
    > {
        Box::pin(async move {
            let summary = self.inner.on_shutdown().await;
            let closed = self.shared.close().await;
            *self.closure.lock().await = Some(closed);
            summary
        })
    }
}

mod briefing;
mod maintenance;
pub use maintenance::{back_up, compact_intraday, reclaim_space};

#[cfg(test)]
mod briefing_tests;
#[cfg(test)]
mod tests;
