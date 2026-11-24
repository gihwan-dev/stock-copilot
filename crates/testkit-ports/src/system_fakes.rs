use crate::{Script, memory::MemoryReply, script::State};
use application::{
    AuditEntry, AuditIntegrity, AuditLog, CollectedUrlCatalog, ExternalUrlOpened,
    ExternalUrlOpener, ExternalUrlOpenerError, PeerProcessInfo, PeerProcessInspector,
    PeerProcessLookupError, ProcessId, RecordOutcome, SecretKey, SecretStore, SecretValue,
    SystemPortError, ToolCallHistory, ToolCallLedger, ToolCallRecord, WriteCompleted,
};
use domain::{AuthorizedExternalUrl, NormalizedUrl, ParsedExternalUrl};
use std::sync::Arc;
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuditCall {
    Append(AuditEntry),
    Verify,
}
#[derive(Debug, Default)]
pub struct MemoryAuditLog {
    append: MemoryAuditLogAppendScript,
    verify: MemoryAuditLogVerifyScript,
    entries: State<Vec<AuditEntry>>,
    calls: State<Vec<AuditCall>>,
}
impl MemoryAuditLog {
    #[must_use]
    pub fn script_append(&self) -> &MemoryAuditLogAppendScript {
        &self.append
    }
    #[must_use]
    pub fn script_verify(&self) -> &MemoryAuditLogVerifyScript {
        &self.verify
    }

    #[must_use]
    pub fn entries(&self) -> Vec<AuditEntry> {
        self.entries.write().clone()
    }
    #[must_use]
    pub fn calls(&self) -> Vec<AuditCall> {
        self.calls.write().clone()
    }
}
impl AuditLog for MemoryAuditLog {
    async fn append(&self, entry: &AuditEntry) -> Result<RecordOutcome, SystemPortError> {
        self.calls.write().push(AuditCall::Append(entry.clone()));
        match self.append.run(entry.clone()).await {
            MemoryReply::Return(value) => value,
            MemoryReply::Memory => {
                let mut entries = self.entries.write();
                if entries.contains(entry) {
                    Ok(RecordOutcome::AlreadyRecorded)
                } else {
                    entries.push(entry.clone());
                    Ok(RecordOutcome::Recorded)
                }
            }
        }
    }
    async fn verify(&self) -> Result<AuditIntegrity, SystemPortError> {
        self.calls.write().push(AuditCall::Verify);
        self.verify.run(()).await
    }
}
#[derive(Debug, Clone)]
pub enum SecretCall {
    Load(SecretKey),
    Store(SecretKey, Arc<SecretValue>),
    Remove(SecretKey),
}
#[derive(Debug, Default)]
pub struct MemorySecretStore {
    load: MemorySecretStoreLoadScript,
    store: MemorySecretStoreStoreScript,
    remove: MemorySecretStoreRemoveScript,
    values: State<Vec<(SecretKey, Arc<SecretValue>)>>,
    calls: State<Vec<SecretCall>>,
}
impl MemorySecretStore {
    #[must_use]
    pub fn script_store(&self) -> &MemorySecretStoreStoreScript {
        &self.store
    }

    #[must_use]
    pub fn script_load(&self) -> &MemorySecretStoreLoadScript {
        &self.load
    }
    #[must_use]
    pub fn script_remove(&self) -> &MemorySecretStoreRemoveScript {
        &self.remove
    }

    #[must_use]
    pub fn calls(&self) -> Vec<SecretCall> {
        self.calls.write().clone()
    }
}
impl SecretStore for MemorySecretStore {
    async fn load(&self, key: SecretKey) -> Result<Option<SecretValue>, SystemPortError> {
        self.calls.write().push(SecretCall::Load(key));
        match self.load.run(key).await {
            MemoryReply::Return(value) => value,
            MemoryReply::Memory => Ok(self
                .values
                .write()
                .iter()
                .find(|(stored, _)| *stored == key)
                .map(|(_, value)| SecretValue::new(value.as_exposed_secret().to_vec()))),
        }
    }
    async fn store(
        &self,
        key: SecretKey,
        value: SecretValue,
    ) -> Result<WriteCompleted, SystemPortError> {
        let stored_value = Arc::new(value);
        self.calls
            .write()
            .push(SecretCall::Store(key, Arc::clone(&stored_value)));
        match self.store.run((key, Arc::clone(&stored_value))).await {
            MemoryReply::Return(reply) => reply,
            MemoryReply::Memory => {
                let mut values = self.values.write();
                values.retain(|(stored, _)| *stored != key);
                values.push((key, stored_value));
                Ok(WriteCompleted)
            }
        }
    }
    async fn remove(&self, key: SecretKey) -> Result<WriteCompleted, SystemPortError> {
        self.calls.write().push(SecretCall::Remove(key));
        match self.remove.run(key).await {
            MemoryReply::Return(value) => value,
            MemoryReply::Memory => {
                self.values.write().retain(|(stored, _)| *stored != key);
                Ok(WriteCompleted)
            }
        }
    }
}

pub type MemoryAuditLogAppendScript =
    Script<AuditEntry, MemoryReply<Result<RecordOutcome, SystemPortError>>>;
pub type MemoryAuditLogVerifyScript = Script<(), Result<AuditIntegrity, SystemPortError>>;
pub type MemorySecretStoreLoadScript =
    Script<SecretKey, MemoryReply<Result<Option<SecretValue>, SystemPortError>>>;
pub type MemorySecretStoreRemoveScript =
    Script<SecretKey, MemoryReply<Result<WriteCompleted, SystemPortError>>>;

pub type MemorySecretStoreStoreScript =
    Script<(SecretKey, Arc<SecretValue>), MemoryReply<Result<WriteCompleted, SystemPortError>>>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CollectedUrlCatalogCall {
    FindCollected(ParsedExternalUrl),
}

#[derive(Debug, Default)]
pub struct MemoryCollectedUrlCatalog {
    find_collected: MemoryCollectedUrlCatalogFindScript,
    entries: State<Vec<NormalizedUrl>>,
    calls: State<Vec<CollectedUrlCatalogCall>>,
}

impl MemoryCollectedUrlCatalog {
    pub fn insert(&self, url: NormalizedUrl) {
        self.entries.write().push(url);
    }

    #[must_use]
    pub fn script_find_collected(&self) -> &MemoryCollectedUrlCatalogFindScript {
        &self.find_collected
    }

    #[must_use]
    pub fn calls(&self) -> Vec<CollectedUrlCatalogCall> {
        self.calls.write().clone()
    }
}

impl CollectedUrlCatalog for MemoryCollectedUrlCatalog {
    async fn find_collected(
        &self,
        url: &ParsedExternalUrl,
    ) -> Result<Option<NormalizedUrl>, SystemPortError> {
        self.calls
            .write()
            .push(CollectedUrlCatalogCall::FindCollected(url.clone()));
        match self.find_collected.run(url.clone()).await {
            MemoryReply::Return(value) => value,
            MemoryReply::Memory => Ok(self
                .entries
                .write()
                .iter()
                .find(|stored| ParsedExternalUrl::from_collected(stored).ok().as_ref() == Some(url))
                .cloned()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExternalUrlOpenerCall {
    Open(AuthorizedExternalUrl),
}

#[derive(Debug, Default)]
pub struct ScriptedExternalUrlOpener {
    open: ScriptedExternalUrlOpenerScript,
    calls: State<Vec<ExternalUrlOpenerCall>>,
}

impl ScriptedExternalUrlOpener {
    #[must_use]
    pub fn script_open(&self) -> &ScriptedExternalUrlOpenerScript {
        &self.open
    }

    #[must_use]
    pub fn calls(&self) -> Vec<ExternalUrlOpenerCall> {
        self.calls.write().clone()
    }

    #[must_use]
    pub fn opened_urls(&self) -> Vec<String> {
        self.calls
            .write()
            .iter()
            .map(|ExternalUrlOpenerCall::Open(url)| url.canonical_url().as_str().to_owned())
            .collect()
    }
}

impl ExternalUrlOpener for ScriptedExternalUrlOpener {
    async fn open(
        &self,
        url: &AuthorizedExternalUrl,
    ) -> Result<ExternalUrlOpened, ExternalUrlOpenerError> {
        self.calls
            .write()
            .push(ExternalUrlOpenerCall::Open(url.clone()));
        self.open.run(url.clone()).await
    }
}

pub type MemoryCollectedUrlCatalogFindScript =
    Script<ParsedExternalUrl, MemoryReply<Result<Option<NormalizedUrl>, SystemPortError>>>;
pub type ScriptedExternalUrlOpenerScript =
    Script<AuthorizedExternalUrl, Result<ExternalUrlOpened, ExternalUrlOpenerError>>;
pub type PeerProcessInspectorScript =
    Script<ProcessId, Result<PeerProcessInfo, PeerProcessLookupError>>;

#[derive(Debug, Default)]
pub struct ScriptedPeerProcessInspector {
    inspect: PeerProcessInspectorScript,
}

impl ScriptedPeerProcessInspector {
    pub fn new<Replies: IntoIterator<Item = Result<PeerProcessInfo, PeerProcessLookupError>>>(
        replies: Replies,
    ) -> Self {
        Self {
            inspect: Script::new(replies),
        }
    }

    #[must_use]
    pub fn script(&self) -> &PeerProcessInspectorScript {
        &self.inspect
    }

    #[must_use]
    pub fn calls(&self) -> Vec<ProcessId> {
        self.inspect.calls()
    }
}

impl PeerProcessInspector for ScriptedPeerProcessInspector {
    async fn inspect(&self, pid: ProcessId) -> Result<PeerProcessInfo, PeerProcessLookupError> {
        self.inspect.run(pid).await
    }
}

pub type MemoryToolCallLedgerScript =
    Script<ToolCallRecord, MemoryReply<Result<RecordOutcome, SystemPortError>>>;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
enum AutoRecordMode {
    #[default]
    Scripted,
    RecordAll,
}

#[derive(Debug, Default)]
pub struct MemoryToolCallLedger {
    record: MemoryToolCallLedgerScript,
    records: State<Vec<ToolCallRecord>>,
    mode: AutoRecordMode,
}

impl MemoryToolCallLedger {
    #[must_use]
    pub fn recording() -> Self {
        Self {
            record: MemoryToolCallLedgerScript::default(),
            records: State::default(),
            mode: AutoRecordMode::RecordAll,
        }
    }

    #[must_use]
    pub fn script_record(&self) -> &MemoryToolCallLedgerScript {
        &self.record
    }

    #[must_use]
    pub fn records(&self) -> Vec<ToolCallRecord> {
        self.records.write().clone()
    }
}

impl ToolCallHistory for MemoryToolCallLedger {
    fn calls_in_run(
        &self,
        run: &application::AiRunId,
    ) -> impl Future<Output = Result<Vec<ToolCallRecord>, SystemPortError>> + Send {
        std::future::ready(Ok(self
            .records
            .write()
            .iter()
            .filter(|record| record.run_id() == run)
            .cloned()
            .collect()))
    }
}

impl ToolCallLedger for MemoryToolCallLedger {
    async fn record_call(&self, record: &ToolCallRecord) -> Result<RecordOutcome, SystemPortError> {
        match self.mode {
            AutoRecordMode::RecordAll => {
                let mut entries = self.records.write();
                entries.push(record.clone());
                Ok(RecordOutcome::Recorded)
            }
            AutoRecordMode::Scripted => match self.record.run(record.clone()).await {
                MemoryReply::Return(value) => value,
                MemoryReply::Memory => {
                    let mut entries = self.records.write();
                    entries.push(record.clone());
                    Ok(RecordOutcome::Recorded)
                }
            },
        }
    }
}
