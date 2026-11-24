use crate::{AiEvent, DeliveryReport, EventBus, RunKind, RunPurpose, SystemPortError, TokenUsage};
use domain::{AiJobId, DomainEvent, TimeSpan, Timestamp};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiJobOutcome {
    Completed,
    Interrupted,
    IsolationRejected,
    Waiting,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiJobTimingError {
    EndsBeforeStart,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AiJobTiming {
    started_at: Timestamp,
    finished_at: Timestamp,
    span: TimeSpan,
}

impl AiJobTiming {
    pub fn try_new(
        started_at: Timestamp,
        finished_at: Timestamp,
    ) -> Result<Self, AiJobTimingError> {
        let span = started_at
            .elapsed_until(finished_at)
            .map_err(|_error| AiJobTimingError::EndsBeforeStart)?;
        Ok(Self {
            started_at,
            finished_at,
            span,
        })
    }
    #[must_use]
    pub const fn started_at(self) -> Timestamp {
        self.started_at
    }
    #[must_use]
    pub const fn finished_at(self) -> Timestamp {
        self.finished_at
    }
    #[must_use]
    pub const fn span(self) -> TimeSpan {
        self.span
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiJobRun {
    job: AiJobId,
    kind: RunKind,
    purpose: RunPurpose,
}

impl AiJobRun {
    #[must_use]
    pub const fn new(job: AiJobId, kind: RunKind, purpose: RunPurpose) -> Self {
        Self { job, kind, purpose }
    }
    #[must_use]
    pub fn of_spec(spec: &crate::RunSpec) -> Self {
        Self::new(spec.id().job_id().clone(), spec.kind(), spec.purpose())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiJobFinishRecord {
    job: AiJobId,
    kind: RunKind,
    purpose: RunPurpose,
    outcome: AiJobOutcome,
    timing: AiJobTiming,
    tokens: TokenUsage,
}

impl AiJobFinishRecord {
    #[must_use]
    pub fn new(
        run: AiJobRun,
        outcome: AiJobOutcome,
        timing: AiJobTiming,
        tokens: TokenUsage,
    ) -> Self {
        Self {
            job: run.job,
            kind: run.kind,
            purpose: run.purpose,
            outcome,
            timing,
            tokens,
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
    pub const fn timing(&self) -> AiJobTiming {
        self.timing
    }
    #[must_use]
    pub const fn finished_at(&self) -> Timestamp {
        self.timing.finished_at
    }
    #[must_use]
    pub const fn tokens(&self) -> &TokenUsage {
        &self.tokens
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiJobUsageMeter {
    started_at: Timestamp,
    tokens: TokenUsage,
}

impl AiJobUsageMeter {
    #[must_use]
    pub const fn start(started_at: Timestamp) -> Self {
        Self {
            started_at,
            tokens: TokenUsage::zero(),
        }
    }

    pub fn observe(&mut self, event: &AiEvent) {
        match event {
            AiEvent::TokenUsage(usage) => {
                self.tokens = self.tokens.saturating_add(usage);
            }
            AiEvent::IsolationChecked(_)
            | AiEvent::TextDelta(_)
            | AiEvent::ToolCall { .. }
            | AiEvent::WebSearch { .. }
            | AiEvent::Quota(_)
            | AiEvent::RateLimited { .. }
            | AiEvent::ModelRerouted(_)
            | AiEvent::ActualModel(_)
            | AiEvent::McpUnavailable
            | AiEvent::Unknown(_) => {}
        }
    }

    #[must_use]
    pub const fn tokens(&self) -> &TokenUsage {
        &self.tokens
    }

    pub fn finish(
        self,
        finished_at: Timestamp,
    ) -> Result<(AiJobTiming, TokenUsage), AiJobTimingError> {
        AiJobTiming::try_new(self.started_at, finished_at).map(|timing| (timing, self.tokens))
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
    fn finished_between(
        &self,
        from: Timestamp,
        through: Timestamp,
    ) -> impl Future<Output = Result<Vec<AiJobFinishRecord>, SystemPortError>> + Send;
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListAiJobUsageError {
    InvalidWindow,
    Storage(SystemPortError),
}

#[derive(Debug)]
pub struct ListAiJobUsage<'a, Store> {
    store: &'a Store,
}

impl<'a, Store: AiJobRecordStore> ListAiJobUsage<'a, Store> {
    #[must_use]
    pub const fn new(store: &'a Store) -> Self {
        Self { store }
    }

    pub async fn execute(
        &self,
        from: Timestamp,
        through: Timestamp,
    ) -> Result<Vec<AiJobFinishRecord>, ListAiJobUsageError> {
        if through < from {
            return Err(ListAiJobUsageError::InvalidWindow);
        }
        let mut records = self
            .store
            .finished_between(from, through)
            .await
            .map_err(ListAiJobUsageError::Storage)?;
        records.retain(|record| record.finished_at() >= from && record.finished_at() <= through);
        records.sort_by(|left, right| {
            right
                .finished_at()
                .cmp(&left.finished_at())
                .then_with(|| left.job().cmp(right.job()))
        });
        Ok(records)
    }
}
