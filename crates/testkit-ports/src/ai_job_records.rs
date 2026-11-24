use crate::script::State;
use application::{AiJobFinishRecord, AiJobRecordStore, AiJobRecordWrite, SystemPortError};
use domain::AiJobId;
use std::collections::VecDeque;

#[derive(Debug, Default)]
pub struct MemoryAiJobRecords {
    records: State<Vec<AiJobFinishRecord>>,
    store_failures: State<VecDeque<SystemPortError>>,
    lookup_failures: State<VecDeque<SystemPortError>>,
}

impl MemoryAiJobRecords {
    pub fn fail_next_store(&self, error: SystemPortError) {
        self.store_failures.write().push_back(error);
    }
    pub fn fail_next_lookup(&self, error: SystemPortError) {
        self.lookup_failures.write().push_back(error);
    }
    #[must_use]
    pub fn records(&self) -> Vec<AiJobFinishRecord> {
        self.records.write().clone()
    }
    fn store_now(&self, record: &AiJobFinishRecord) -> Result<AiJobRecordWrite, SystemPortError> {
        let failure = self.store_failures.write().pop_front();
        if let Some(error) = failure {
            return Err(error);
        }
        let mut records = self.records.write();
        if records.iter().any(|stored| stored.job() == record.job()) {
            return Ok(AiJobRecordWrite::AlreadyStored);
        }
        records.push(record.clone());
        Ok(AiJobRecordWrite::Stored)
    }

    fn lookup_now(&self, job: &AiJobId) -> Result<Option<AiJobFinishRecord>, SystemPortError> {
        let failure = self.lookup_failures.write().pop_front();
        if let Some(error) = failure {
            return Err(error);
        }
        let found = self
            .records
            .write()
            .iter()
            .find(|stored| stored.job() == job)
            .cloned();
        Ok(found)
    }
}

impl AiJobRecordStore for MemoryAiJobRecords {
    fn store_finished(
        &self,
        record: &AiJobFinishRecord,
    ) -> impl Future<Output = Result<AiJobRecordWrite, SystemPortError>> + Send {
        std::future::ready(self.store_now(record))
    }

    fn finished(
        &self,
        job: &AiJobId,
    ) -> impl Future<Output = Result<Option<AiJobFinishRecord>, SystemPortError>> + Send {
        std::future::ready(self.lookup_now(job))
    }
}
