use crate::{AiRunId, DeliveryReport, EventBus, ReasoningEffort, RecordOutcome, SystemPortError};
use domain::{
    CalibrationEstimateId, CostEligibleTask, DomainEvent, EstimateAssignmentError,
    EstimateAssignments, EstimateMetadata, EstimateNarrative, EstimateRecordError,
    EstimateSubmissionRecord, SignalSubmission,
};
use std::future::Future;

pub trait EstimateTaskRepository: Send + Sync {
    fn issue_task(
        &self,
        id: &CalibrationEstimateId,
        run: &AiRunId,
        task: &CostEligibleTask,
    ) -> impl Future<Output = Result<RecordOutcome, SystemPortError>> + Send;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EstimateSubmissionInput {
    pub task_id: CalibrationEstimateId,
    pub submission: SignalSubmission,
    pub narrative: EstimateNarrative,
    pub metadata: EstimateMetadata<ReasoningEffort>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EstimateCommitOutcome {
    Committed,
    Conflict,
}
pub trait EstimateSubmissionRepository: Send + Sync {
    fn submission_records(
        &self,
        instrument: &domain::InstrumentId,
    ) -> impl Future<
        Output = Result<Vec<EstimateSubmissionRecord<AiRunId, ReasoningEffort>>, SystemPortError>,
    > + Send;
    fn assignments(
        &self,
    ) -> impl Future<Output = Result<EstimateAssignments<AiRunId>, SystemPortError>> + Send;
    fn commit_submission(
        &self,
        previous: &EstimateAssignments<AiRunId>,
        next: &EstimateAssignments<AiRunId>,
        records: &[EstimateSubmissionRecord<AiRunId, ReasoningEffort>],
    ) -> impl Future<Output = Result<EstimateCommitOutcome, SystemPortError>> + Send;
    fn collected_citation(
        &self,
        _run: &AiRunId,
        _handle: &crate::CollectedItemHandle,
    ) -> impl Future<Output = Result<Option<crate::ObservedCollectedItem>, SystemPortError>> + Send
    {
        std::future::ready(Ok(None))
    }
    fn commit_with_sources(
        &self,
        previous: &EstimateAssignments<AiRunId>,
        next: &EstimateAssignments<AiRunId>,
        records: &[EstimateSubmissionRecord<AiRunId, ReasoningEffort>],
        sources: &crate::SignalSourceFacts,
    ) -> impl Future<Output = Result<EstimateCommitOutcome, SystemPortError>> + Send {
        async move {
            if !sources.items.is_empty() || !sources.disclosures.is_empty() {
                return Err(SystemPortError::new(
                    crate::SystemPort::Repository,
                    crate::SystemFailure::InvalidData,
                ));
            }
            self.commit_submission(previous, next, records).await
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubmitEstimateError {
    Assignment(EstimateAssignmentError),
    Record(EstimateRecordError),
    Storage(SystemPortError),
    ConcurrentUpdate,
}
#[derive(Debug)]
pub struct SubmitEstimate<'a, Repository> {
    repository: &'a Repository,
    events: &'a EventBus,
}
impl<'a, Repository: EstimateSubmissionRepository> SubmitEstimate<'a, Repository> {
    #[must_use]
    pub const fn new(repository: &'a Repository, events: &'a EventBus) -> Self {
        Self { repository, events }
    }
    pub async fn execute(
        &self,
        run: &AiRunId,
        requests: Vec<EstimateSubmissionInput>,
    ) -> Result<Vec<DeliveryReport>, SubmitEstimateError> {
        self.execute_with_sources(run, requests, None).await
    }
    pub(crate) async fn execute_with_sources(
        &self,
        run: &AiRunId,
        requests: Vec<EstimateSubmissionInput>,
        sources: Option<&crate::SignalSourceFacts>,
    ) -> Result<Vec<DeliveryReport>, SubmitEstimateError> {
        if requests.is_empty() {
            return Ok(Vec::new());
        }
        let previous = self
            .repository
            .assignments()
            .await
            .map_err(SubmitEstimateError::Storage)?;
        let ids: Vec<_> = requests
            .iter()
            .map(|request| request.task_id.clone())
            .collect();
        let next = previous
            .consume(run, &ids)
            .map_err(SubmitEstimateError::Assignment)?;
        let records = requests
            .into_iter()
            .map(|request| {
                let issued = previous
                    .lookup(&request.task_id)
                    .map_err(SubmitEstimateError::Assignment)?;
                EstimateSubmissionRecord::create(
                    issued,
                    request.submission,
                    request.narrative,
                    request.metadata,
                )
                .map_err(SubmitEstimateError::Record)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let committed = match sources {
            Some(facts) => {
                self.repository
                    .commit_with_sources(&previous, &next, &records, facts)
                    .await
            }
            None => {
                self.repository
                    .commit_submission(&previous, &next, &records)
                    .await
            }
        }
        .map_err(SubmitEstimateError::Storage)?;
        match committed {
            EstimateCommitOutcome::Conflict => Err(SubmitEstimateError::ConcurrentUpdate),
            EstimateCommitOutcome::Committed => {
                Ok(records.iter().map(|record| self.publish(record)).collect())
            }
        }
    }

    fn publish(
        &self,
        record: &EstimateSubmissionRecord<AiRunId, ReasoningEffort>,
    ) -> DeliveryReport {
        let instrument = record
            .submission()
            .task_key()
            .task()
            .volatility()
            .instrument()
            .clone();
        self.events
            .publish(DomainEvent::EstimateSubmitted(instrument))
    }
}
