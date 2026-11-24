use crate::{DeliveryReport, EventBus, RunKind, RunPurpose, SystemPortError};
use domain::{AiJobId, DomainEvent, Timestamp};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiJobOutcome {
    Completed,
    Interrupted,
    IsolationRejected,
    Waiting,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiJobFinishRecord {
    job: AiJobId,
    kind: RunKind,
    purpose: RunPurpose,
    outcome: AiJobOutcome,
    finished_at: Timestamp,
}

impl AiJobFinishRecord {
    #[must_use]
    pub const fn new(
        job: AiJobId,
        kind: RunKind,
        purpose: RunPurpose,
        outcome: AiJobOutcome,
        finished_at: Timestamp,
    ) -> Self {
        Self {
            job,
            kind,
            purpose,
            outcome,
            finished_at,
        }
    }
    #[must_use]
    pub const fn job(&self) -> &AiJobId {
        &self.job
    }
    #[must_use]
    pub const fn kind(&self) -> RunKind {
        self.kind
    }
    #[must_use]
    pub const fn purpose(&self) -> RunPurpose {
        self.purpose
    }
    #[must_use]
    pub const fn outcome(&self) -> AiJobOutcome {
        self.outcome
    }
    #[must_use]
    pub const fn finished_at(&self) -> Timestamp {
        self.finished_at
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiJobRecordWrite {
    Stored,
    AlreadyStored,
}

pub trait AiJobRecordStore: Send + Sync {
    fn store_finished(
        &self,
        record: &AiJobFinishRecord,
    ) -> impl Future<Output = Result<AiJobRecordWrite, SystemPortError>> + Send;
    fn finished(
        &self,
        job: &AiJobId,
    ) -> impl Future<Output = Result<Option<AiJobFinishRecord>, SystemPortError>> + Send;
}

#[derive(Debug)]
pub struct RecordAiJobFinished<'a, Store> {
    store: &'a Store,
    events: &'a EventBus,
}

impl<'a, Store: AiJobRecordStore> RecordAiJobFinished<'a, Store> {
    #[must_use]
    pub const fn new(store: &'a Store, events: &'a EventBus) -> Self {
        Self { store, events }
    }

    pub async fn execute(
        &self,
        record: AiJobFinishRecord,
    ) -> Result<DeliveryReport, SystemPortError> {
        match self.store.store_finished(&record).await? {
            AiJobRecordWrite::Stored => Ok(self
                .events
                .publish(DomainEvent::AiJobFinished(record.job().clone()))),
            AiJobRecordWrite::AlreadyStored => Ok(DeliveryReport::default()),
        }
    }
}
