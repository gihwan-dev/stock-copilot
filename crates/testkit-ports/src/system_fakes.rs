use crate::{Script, memory::MemoryReply, script::State};
use application::{
    AuditEntry, AuditIntegrity, AuditLog, PeerProcessInfo, PeerProcessInspector,
    PeerProcessLookupError, ProcessId, RecordOutcome, SecretKey, SecretStore, SecretValue,
    SystemPortError, WriteCompleted,
};
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
