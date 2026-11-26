use crate::{
    AuditStore, BackupDirectory, BackupOutcome, BriefingStore, ChatStore, Database, EstimateStore,
    HoldingsStore, InformationStore, MarketStore, SettingsStore, SignalStore, SourceStatusStore,
    SpecStatusStore, ThesisStore, TransactionStore, ValuationStore,
};
use application::{
    AiRunId, AuditEntry, AuditIntegrity, AuditLog, BriefingRecord, BriefingRepository,
    ChatNumberEvidence, ChatNumberEvidenceSet, ChatRepository, ChatRunLedger, ChatRunLedgerEntry,
    ChatSessionId, ChatSessionRecord, ChatSessionSummary, Clock, ConditionalDraftOutcome,
    DisclosureRepository, EstimateRepository, FullInspectionRepository, FxRateQuery,
    MarketDataRepository, NewsRepository, ProposalRepository, RecordOutcome, RetryDelay,
    SecretValue, SettingsRepository, SignalCommitOutcome, SignalMutation, SignalRepository,
    SignalSnapshot, SignalSourceFacts, SourceStatus, SourceStatusRepository, SpecStatusRecord,
    StoredChatEntry, StoredUserMessage, SystemFailure, SystemPort, SystemPortError,
    ThesisRepository, TossSpecStatusRepository, TransactionRepository, TransactionSyncRecord,
    TransactionSyncRepository, UserMessageId, ValuationRepository, WriteCompleted,
};
use domain::{
    AccountId, BriefingId, Calibration, CalibrationKey, Candle, CandlePeriod, DataSource,
    Disclosure, DisclosureKey, Draft, DraftRestoreEvidence, EstimateOutcome, FxPair, FxRate,
    FxRateKind, HistoricalSignal, HistoricalSignalOccurrence, InstrumentId, InvestorProfile,
    IssuedEstimateTaskId, MasterInstrument, NewsItem, PriceAdjustment, Proposal, ProposalId, Quote,
    ResolvedOrderInstrument, RiskSettings, SettledRowSet, SignalSubmission, SignalTaskKey,
    SyncRecord, Thesis, TimeInterval, Timestamp, ValuationSnapshot,
};
use std::sync::Arc;
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

pub(crate) fn opened(
    guard: Option<&Database>,
    port: SystemPort,
) -> Result<&Database, SystemPortError> {
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

    pub async fn compact_intraday(
        &self,
        clock: &dyn Clock,
    ) -> Result<WriteCompleted, SystemPortError> {
        let guard = self.shared.read().await;
        let database = opened(guard.as_ref(), REPOSITORY)?;
        HoldingsStore::new(database).compact_holdings(clock).await
    }

    pub async fn reclaim_space(&self) -> Result<WriteCompleted, SystemPortError> {
        let guard = self.shared.read().await;
        let database = opened(guard.as_ref(), REPOSITORY)?;
        database.reclaim_space().await
    }

    pub async fn back_up(
        &self,
        directory: std::path::PathBuf,
        key: &SecretValue,
        clock: &dyn Clock,
    ) -> Result<BackupOutcome, SystemPortError> {
        let guard = self.shared.read().await;
        let database = opened(guard.as_ref(), REPOSITORY)?;
        crate::create_backup(database, &BackupDirectory::new(directory), key, clock).await
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

impl application::SignalSkipRepository for SqliteRepositories {
    async fn record_skip(
        &self,
        skip: &application::SignalSkip,
    ) -> Result<WriteCompleted, SystemPortError> {
        let guard = self.shared.read().await;
        let database = opened(guard.as_ref(), REPOSITORY)?;
        application::SignalSkipRepository::record_skip(&crate::SignalSkipStore::new(database), skip)
            .await
    }
}

impl application::SignalSkipHistory for SqliteRepositories {
    async fn latest_skip(
        &self,
        instrument: &InstrumentId,
    ) -> Result<Option<Timestamp>, SystemPortError> {
        let guard = self.shared.read().await;
        let database = opened(guard.as_ref(), REPOSITORY)?;
        let skips = crate::SignalSkipStore::new(database).skips().await?;
        Ok(skips
            .into_iter()
            .filter(|skip| skip.instrument == *instrument)
            .map(|skip| skip.skipped_at)
            .max())
    }
}

impl application::ThesisHistoryTimes for SqliteRepositories {
    async fn record_times(
        &self,
        instrument: &InstrumentId,
        sequences: &[domain::ThesisVersion],
        recorded_at: Timestamp,
    ) -> Result<WriteCompleted, SystemPortError> {
        let guard = self.shared.read().await;
        let database = opened(guard.as_ref(), REPOSITORY)?;
        application::ThesisHistoryTimes::record_times(
            &ThesisStore::new(database),
            instrument,
            sequences,
            recorded_at,
        )
        .await
    }
    async fn entry_times(
        &self,
        instrument: &InstrumentId,
    ) -> Result<Vec<application::ThesisEntryTime>, SystemPortError> {
        let guard = self.shared.read().await;
        let database = opened(guard.as_ref(), REPOSITORY)?;
        application::ThesisHistoryTimes::entry_times(&ThesisStore::new(database), instrument).await
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

impl application::SignalInvalidationRepository for SqliteRepositories {
    async fn invalidate(
        &self,
        instrument: &InstrumentId,
        expected: &SignalSnapshot,
    ) -> Result<application::SignalInvalidationOutcome, SystemPortError> {
        let guard = self.shared.read().await;
        let database = opened(guard.as_ref(), REPOSITORY)?;
        application::SignalInvalidationRepository::invalidate(
            &SignalStore::new(database),
            instrument,
            expected,
        )
        .await
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

impl TransactionSyncRepository for SqliteRepositories {
    async fn state(&self, account: &AccountId) -> Result<SettledRowSet, SystemPortError> {
        let guard = self.shared.read().await;
        let database = opened(guard.as_ref(), REPOSITORY)?;
        TransactionSyncRepository::state(&TransactionStore::new(database), account).await
    }
    async fn latest_sync(
        &self,
        account: &AccountId,
    ) -> Result<Option<TransactionSyncRecord>, SystemPortError> {
        let guard = self.shared.read().await;
        let database = opened(guard.as_ref(), REPOSITORY)?;
        TransactionSyncRepository::latest_sync(&TransactionStore::new(database), account).await
    }
    async fn record_sync(
        &self,
        account: &AccountId,
        previous: &SettledRowSet,
        previous_recorded_at: Option<Timestamp>,
        record: &TransactionSyncRecord,
    ) -> Result<WriteCompleted, SystemPortError> {
        let guard = self.shared.read().await;
        let database = opened(guard.as_ref(), REPOSITORY)?;
        TransactionSyncRepository::record_sync(
            &TransactionStore::new(database),
            account,
            previous,
            previous_recorded_at,
            record,
        )
        .await
    }
}

impl FullInspectionRepository for SqliteRepositories {
    async fn last_completed(
        &self,
        account: &AccountId,
    ) -> Result<Option<Timestamp>, SystemPortError> {
        let guard = self.shared.read().await;
        let database = opened(guard.as_ref(), REPOSITORY)?;
        FullInspectionRepository::last_completed(&TransactionStore::new(database), account).await
    }
    async fn record_completed(
        &self,
        account: &AccountId,
        previous: Option<Timestamp>,
        completed_at: Timestamp,
    ) -> Result<WriteCompleted, SystemPortError> {
        let guard = self.shared.read().await;
        let database = opened(guard.as_ref(), REPOSITORY)?;
        FullInspectionRepository::record_completed(
            &TransactionStore::new(database),
            account,
            previous,
            completed_at,
        )
        .await
    }
}

impl TossSpecStatusRepository for SqliteRepositories {
    async fn record(&self, status: &SpecStatusRecord) -> Result<WriteCompleted, SystemPortError> {
        let guard = self.shared.read().await;
        let database = opened(guard.as_ref(), REPOSITORY)?;
        TossSpecStatusRepository::record(&SpecStatusStore::new(database), status).await
    }
    async fn latest(&self) -> Result<Option<SpecStatusRecord>, SystemPortError> {
        let guard = self.shared.read().await;
        let database = opened(guard.as_ref(), REPOSITORY)?;
        TossSpecStatusRepository::latest(&SpecStatusStore::new(database)).await
    }
}

impl BriefingRepository for SqliteRepositories {
    async fn save(&self, record: &BriefingRecord) -> Result<WriteCompleted, SystemPortError> {
        let guard = self.shared.read().await;
        let database = opened(guard.as_ref(), REPOSITORY)?;
        BriefingRepository::save(&BriefingStore::new(database), record).await
    }
    async fn find(&self, id: &BriefingId) -> Result<Option<BriefingRecord>, SystemPortError> {
        let guard = self.shared.read().await;
        let database = opened(guard.as_ref(), REPOSITORY)?;
        BriefingRepository::find(&BriefingStore::new(database), id).await
    }
    async fn recent(&self) -> Result<Vec<BriefingRecord>, SystemPortError> {
        let guard = self.shared.read().await;
        let database = opened(guard.as_ref(), REPOSITORY)?;
        BriefingRepository::recent(&BriefingStore::new(database)).await
    }
}

impl DisclosureRepository for SqliteRepositories {
    async fn store_disclosure(&self, item: &Disclosure) -> Result<WriteCompleted, SystemPortError> {
        let guard = self.shared.read().await;
        let database = opened(guard.as_ref(), REPOSITORY)?;
        InformationStore::new(database).store_disclosure(item).await
    }
    async fn disclosure(&self, key: &DisclosureKey) -> Result<Option<Disclosure>, SystemPortError> {
        let guard = self.shared.read().await;
        let database = opened(guard.as_ref(), REPOSITORY)?;
        InformationStore::new(database).disclosure(key).await
    }
    async fn disclosures_for_instrument(
        &self,
        instrument: &InstrumentId,
    ) -> Result<Vec<Disclosure>, SystemPortError> {
        let guard = self.shared.read().await;
        let database = opened(guard.as_ref(), REPOSITORY)?;
        InformationStore::new(database)
            .disclosures_for_instrument(instrument)
            .await
    }
}

impl SourceStatusRepository for SqliteRepositories {
    async fn find(&self, source: DataSource) -> Result<Option<SourceStatus>, SystemPortError> {
        let guard = self.shared.read().await;
        let database = opened(guard.as_ref(), REPOSITORY)?;
        SourceStatusStore::new(database).find(source).await
    }
    async fn list_all(&self) -> Result<Vec<SourceStatus>, SystemPortError> {
        let guard = self.shared.read().await;
        let database = opened(guard.as_ref(), REPOSITORY)?;
        SourceStatusStore::new(database).list_all().await
    }
    async fn save(&self, status: &SourceStatus) -> Result<WriteCompleted, SystemPortError> {
        let guard = self.shared.read().await;
        let database = opened(guard.as_ref(), REPOSITORY)?;
        SourceStatusRepository::save(&SourceStatusStore::new(database), status).await
    }
}

impl application::ToolCallLedger for SqliteRepositories {
    async fn record_call(
        &self,
        record: &application::ToolCallRecord,
    ) -> Result<RecordOutcome, SystemPortError> {
        let guard = self.shared.read().await;
        let database = opened(guard.as_ref(), REPOSITORY)?;
        application::ToolCallLedger::record_call(&ChatStore::new(database), record).await
    }
}

impl application::EstimateAssignmentRepository for SqliteRepositories {
    async fn load_assignments(
        &self,
    ) -> Result<domain::EstimateAssignments<AiRunId>, SystemPortError> {
        let guard = self.shared.read().await;
        let database = opened(guard.as_ref(), REPOSITORY)?;
        application::EstimateAssignmentRepository::load_assignments(&EstimateStore::new(database))
            .await
    }
    async fn commit_assignments(
        &self,
        previous: &domain::EstimateAssignments<AiRunId>,
        next: &domain::EstimateAssignments<AiRunId>,
    ) -> Result<application::EstimateCommitOutcome, SystemPortError> {
        let guard = self.shared.read().await;
        let database = opened(guard.as_ref(), REPOSITORY)?;
        application::EstimateAssignmentRepository::commit_assignments(
            &EstimateStore::new(database),
            previous,
            next,
        )
        .await
    }
}

impl application::EstimateSubmissionRepository for SqliteRepositories {
    async fn submission_records(
        &self,
        instrument: &InstrumentId,
    ) -> Result<
        Vec<domain::EstimateSubmissionRecord<AiRunId, application::ReasoningEffort>>,
        SystemPortError,
    > {
        let guard = self.shared.read().await;
        let database = opened(guard.as_ref(), REPOSITORY)?;
        application::EstimateSubmissionRepository::submission_records(
            &EstimateStore::new(database),
            instrument,
        )
        .await
    }
    async fn assignments(&self) -> Result<domain::EstimateAssignments<AiRunId>, SystemPortError> {
        let guard = self.shared.read().await;
        let database = opened(guard.as_ref(), REPOSITORY)?;
        application::EstimateSubmissionRepository::assignments(&EstimateStore::new(database)).await
    }
    async fn commit_submission(
        &self,
        previous: &domain::EstimateAssignments<AiRunId>,
        next: &domain::EstimateAssignments<AiRunId>,
        records: &[domain::EstimateSubmissionRecord<AiRunId, application::ReasoningEffort>],
    ) -> Result<application::EstimateCommitOutcome, SystemPortError> {
        let guard = self.shared.read().await;
        let database = opened(guard.as_ref(), REPOSITORY)?;
        application::EstimateSubmissionRepository::commit_submission(
            &EstimateStore::new(database),
            previous,
            next,
            records,
        )
        .await
    }
}

impl application::ScorecardObservationRepository for SqliteRepositories {
    async fn scorecard_observations(
        &self,
    ) -> Result<Vec<domain::ScorecardObservation>, SystemPortError> {
        let guard = self.shared.read().await;
        let database = opened(guard.as_ref(), REPOSITORY)?;
        application::ScorecardObservationRepository::scorecard_observations(&EstimateStore::new(
            database,
        ))
        .await
    }
}

impl application::ValuationCloseEvidenceRepository for SqliteRepositories {
    async fn observed_close_evidence(
        &self,
        instrument: &ResolvedOrderInstrument,
        date: domain::CalendarDate,
        as_of: Timestamp,
    ) -> Result<Vec<domain::ObservedCloseEvidence>, SystemPortError> {
        let guard = self.shared.read().await;
        let database = opened(guard.as_ref(), REPOSITORY)?;
        application::ValuationCloseEvidenceRepository::observed_close_evidence(
            &MarketStore::new(database),
            instrument,
            date,
            as_of,
        )
        .await
    }
}

impl application::ToolCallHistory for SqliteRepositories {
    async fn calls_in_run(
        &self,
        run: &AiRunId,
    ) -> Result<Vec<application::ToolCallRecord>, SystemPortError> {
        let guard = self.shared.read().await;
        let database = opened(guard.as_ref(), REPOSITORY)?;
        application::ToolCallHistory::calls_in_run(&ChatStore::new(database), run).await
    }
}

impl application::CollectedTimes for SqliteRepositories {
    async fn record_times(
        &self,
        entry: &application::CollectedEntry,
        published_at: application::PublicationTime,
        collected_at: Timestamp,
    ) -> Result<WriteCompleted, SystemPortError> {
        let guard = self.shared.read().await;
        let database = opened(guard.as_ref(), REPOSITORY)?;
        application::CollectedTimes::record_times(
            &InformationStore::new(database),
            entry,
            published_at,
            collected_at,
        )
        .await
    }
}

impl application::CollectedUrlCatalog for SqliteRepositories {
    async fn find_collected(
        &self,
        url: &domain::ParsedExternalUrl,
    ) -> Result<Option<domain::NormalizedUrl>, SystemPortError> {
        let guard = self.shared.read().await;
        let database = opened(guard.as_ref(), REPOSITORY)?;
        application::CollectedUrlCatalog::find_collected(&InformationStore::new(database), url)
            .await
    }
}

impl application::CollectedRetention for SqliteRepositories {
    async fn prune_news(
        &self,
        cutoffs: &[application::RetentionCutoff],
    ) -> Result<application::CollectedCount, SystemPortError> {
        let guard = self.shared.read().await;
        let database = opened(guard.as_ref(), REPOSITORY)?;
        application::CollectedRetention::prune_news(&InformationStore::new(database), cutoffs).await
    }
}

impl application::CollectedItemReads for SqliteRepositories {
    async fn collected_since(
        &self,
        kind: application::CollectedKind,
        since: Timestamp,
    ) -> Result<Vec<application::ObservedCollectedItem>, SystemPortError> {
        let guard = self.shared.read().await;
        let database = opened(guard.as_ref(), REPOSITORY)?;
        application::CollectedItemReads::collected_since(
            &InformationStore::new(database),
            kind,
            since,
        )
        .await
    }

    async fn collected_item(
        &self,
        handle: &application::CollectedItemHandle,
    ) -> Result<Option<application::ObservedCollectedItem>, SystemPortError> {
        let guard = self.shared.read().await;
        let database = opened(guard.as_ref(), REPOSITORY)?;
        application::CollectedItemReads::collected_item(&InformationStore::new(database), handle)
            .await
    }
}

impl application::AiPreferencesRepository for SqliteRepositories {
    async fn load_ai_preferences(
        &self,
    ) -> Result<Option<application::AiPreferences>, SystemPortError> {
        let guard = self.shared.read().await;
        let database = opened(guard.as_ref(), REPOSITORY)?;
        application::AiPreferencesRepository::load_ai_preferences(&crate::AiPreferencesStore::new(
            database,
        ))
        .await
    }
    async fn store_ai_preferences(
        &self,
        preferences: &application::AiPreferences,
    ) -> Result<WriteCompleted, SystemPortError> {
        let guard = self.shared.read().await;
        let database = opened(guard.as_ref(), REPOSITORY)?;
        application::AiPreferencesRepository::store_ai_preferences(
            &crate::AiPreferencesStore::new(database),
            preferences,
        )
        .await
    }
}

impl application::PrinciplesRepository for SqliteRepositories {
    async fn store_principles(
        &self,
        principles: &domain::InvestmentPrinciples,
    ) -> Result<WriteCompleted, SystemPortError> {
        let guard = self.shared.read().await;
        let database = opened(guard.as_ref(), REPOSITORY)?;
        application::PrinciplesRepository::store_principles(
            &crate::PrinciplesStore::new(database),
            principles,
        )
        .await
    }
    async fn principles(&self) -> Result<Option<domain::InvestmentPrinciples>, SystemPortError> {
        let guard = self.shared.read().await;
        let database = opened(guard.as_ref(), REPOSITORY)?;
        application::PrinciplesRepository::principles(&crate::PrinciplesStore::new(database)).await
    }
}

impl application::NewsDataSettingsRepository for SqliteRepositories {
    async fn load(&self) -> Result<Option<application::NewsDataSettings>, SystemPortError> {
        let guard = self.shared.read().await;
        let database = opened(guard.as_ref(), REPOSITORY)?;
        application::NewsDataSettingsRepository::load(&crate::NewsDataSettingsStore::new(database))
            .await
    }

    async fn store(
        &self,
        settings: &application::NewsDataSettings,
    ) -> Result<WriteCompleted, SystemPortError> {
        let guard = self.shared.read().await;
        let database = opened(guard.as_ref(), REPOSITORY)?;
        application::NewsDataSettingsRepository::store(
            &crate::NewsDataSettingsStore::new(database),
            settings,
        )
        .await
    }
}

impl application::WatchlistRepository for SqliteRepositories {
    async fn load(&self) -> Result<application::Watchlist, SystemPortError> {
        let guard = self.shared.read().await;
        let database = opened(guard.as_ref(), REPOSITORY)?;
        application::WatchlistRepository::load(&crate::WatchlistStore::new(database)).await
    }

    async fn store(
        &self,
        previous: &application::Watchlist,
        next: &application::Watchlist,
    ) -> Result<application::WatchlistStoreOutcome, SystemPortError> {
        let guard = self.shared.read().await;
        let database = opened(guard.as_ref(), REPOSITORY)?;
        application::WatchlistRepository::store(
            &crate::WatchlistStore::new(database),
            previous,
            next,
        )
        .await
    }
}

impl application::JobScheduleStore for SqliteRepositories {
    async fn save_schedule(
        &self,
        schedule: &application::SavedJobSchedule,
    ) -> Result<WriteCompleted, SystemPortError> {
        let guard = self.shared.read().await;
        let database = opened(guard.as_ref(), REPOSITORY)?;
        crate::JobScheduleSqlStore::new(database)
            .save_schedule(schedule)
            .await
    }
    async fn schedules(&self) -> Result<Vec<application::SavedJobSchedule>, SystemPortError> {
        let guard = self.shared.read().await;
        let database = opened(guard.as_ref(), REPOSITORY)?;
        application::JobScheduleStore::schedules(&crate::JobScheduleSqlStore::new(database)).await
    }
    async fn append_run(
        &self,
        entry: &application::JobRunEntry,
    ) -> Result<WriteCompleted, SystemPortError> {
        let guard = self.shared.read().await;
        let database = opened(guard.as_ref(), REPOSITORY)?;
        crate::JobScheduleSqlStore::new(database)
            .append_run(entry)
            .await
    }
    async fn recent_runs(
        &self,
        job: &application::JobCode,
    ) -> Result<Vec<application::JobRunEntry>, SystemPortError> {
        let guard = self.shared.read().await;
        let database = opened(guard.as_ref(), REPOSITORY)?;
        crate::JobScheduleSqlStore::new(database)
            .recent_runs(job)
            .await
    }
}

impl application::InformationLinks for SqliteRepositories {
    async fn set_instruments(
        &self,
        item: &application::InformationKey,
        instruments: &[InstrumentId],
    ) -> Result<WriteCompleted, SystemPortError> {
        let guard = self.shared.read().await;
        let database = opened(guard.as_ref(), REPOSITORY)?;
        application::InformationLinks::set_instruments(
            &InformationStore::new(database),
            item,
            instruments,
        )
        .await
    }
}

impl application::resolve_estimates::EstimateResolutionRepository for SqliteRepositories {
    async fn pending_submissions(
        &self,
    ) -> Result<
        Vec<domain::EstimateSubmissionRecord<AiRunId, application::ReasoningEffort>>,
        SystemPortError,
    > {
        let guard = self.shared.read().await;
        let database = opened(guard.as_ref(), REPOSITORY)?;
        application::resolve_estimates::EstimateResolutionRepository::pending_submissions(
            &EstimateStore::new(database),
        )
        .await
    }

    async fn calibration(
        &self,
        key: &CalibrationKey,
    ) -> Result<Option<application::resolve_estimates::ResolutionCalibration>, SystemPortError>
    {
        let guard = self.shared.read().await;
        let database = opened(guard.as_ref(), REPOSITORY)?;
        application::resolve_estimates::EstimateResolutionRepository::calibration(
            &EstimateStore::new(database),
            key,
        )
        .await
    }

    async fn resolution(
        &self,
        id: &domain::CalibrationEstimateId,
    ) -> Result<Option<EstimateOutcome>, SystemPortError> {
        let guard = self.shared.read().await;
        let database = opened(guard.as_ref(), REPOSITORY)?;
        application::resolve_estimates::EstimateResolutionRepository::resolution(
            &EstimateStore::new(database),
            id,
        )
        .await
    }

    async fn record_unavailable(
        &self,
        record: &domain::EstimateSubmissionRecord<AiRunId, application::ReasoningEffort>,
        reason: application::resolve_estimates::ResolutionDataIssue,
    ) -> Result<application::resolve_estimates::ResolutionCommitOutcome, SystemPortError> {
        let guard = self.shared.read().await;
        let database = opened(guard.as_ref(), REPOSITORY)?;
        application::resolve_estimates::EstimateResolutionRepository::record_unavailable(
            &EstimateStore::new(database),
            record,
            reason,
        )
        .await
    }

    async fn commit_resolution(
        &self,
        record: &domain::EstimateSubmissionRecord<AiRunId, application::ReasoningEffort>,
        outcome: &EstimateOutcome,
        previous: &application::resolve_estimates::ResolutionCalibration,
        next: &Calibration,
    ) -> Result<application::resolve_estimates::ResolutionCommitOutcome, SystemPortError> {
        let guard = self.shared.read().await;
        let database = opened(guard.as_ref(), REPOSITORY)?;
        application::resolve_estimates::EstimateResolutionRepository::commit_resolution(
            &EstimateStore::new(database),
            record,
            outcome,
            previous,
            next,
        )
        .await
    }
}
