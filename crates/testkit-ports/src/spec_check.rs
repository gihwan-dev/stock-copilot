use crate::Script;
use crate::script::State;
use application::{
    SpecFetchError, SpecStatusRecord, SystemPortError, TossSpecObservation, TossSpecSource,
    TossSpecStatusRepository, WriteCompleted,
};
use std::future::Future;
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Debug, Default)]
pub struct ScriptedTossSpecSource {
    replies: Script<(), Result<TossSpecObservation, SpecFetchError>>,
    calls: AtomicUsize,
}

impl ScriptedTossSpecSource {
    #[must_use]
    pub fn script_fetch(&self) -> &Script<(), Result<TossSpecObservation, SpecFetchError>> {
        &self.replies
    }

    #[must_use]
    pub fn call_count(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

impl TossSpecSource for ScriptedTossSpecSource {
    async fn fetch(&self) -> Result<TossSpecObservation, SpecFetchError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.replies.run(()).await
    }
}

#[derive(Debug, Default)]
pub struct MemoryTossSpecStatusRepository {
    records: State<Vec<SpecStatusRecord>>,
    record_error: State<Option<SystemPortError>>,
    latest_error: State<Option<SystemPortError>>,
}

impl MemoryTossSpecStatusRepository {
    pub fn fail_record(&self, error: Option<SystemPortError>) {
        *self.record_error.write() = error;
    }

    pub fn fail_latest(&self, error: Option<SystemPortError>) {
        *self.latest_error.write() = error;
    }

    #[must_use]
    pub fn records(&self) -> Vec<SpecStatusRecord> {
        self.records.write().clone()
    }
}

impl TossSpecStatusRepository for MemoryTossSpecStatusRepository {
    fn record(
        &self,
        status: &SpecStatusRecord,
    ) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send {
        let error_override = *self.record_error.write();
        if let Some(error) = error_override {
            return std::future::ready(Err(error));
        }
        self.records.write().push(status.clone());
        std::future::ready(Ok(WriteCompleted))
    }

    fn latest(
        &self,
    ) -> impl Future<Output = Result<Option<SpecStatusRecord>, SystemPortError>> + Send {
        let error_override = *self.latest_error.write();
        if let Some(error) = error_override {
            return std::future::ready(Err(error));
        }
        let latest = self.records.write().last().cloned();
        std::future::ready(Ok(latest))
    }
}
