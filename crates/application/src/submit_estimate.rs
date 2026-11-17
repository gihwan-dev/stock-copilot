use crate::{AiRunId, DeliveryReport, EventBus, ReasoningEffort, SystemPortError};
use domain::{
    CalibrationEstimateId, DomainEvent, EstimateAssignmentError, EstimateAssignments,
    EstimateMetadata, EstimateNarrative, EstimateRecordError, EstimateSubmissionRecord,
    SignalSubmission,
};
use std::future::Future;

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
        match self
            .repository
            .commit_submission(&previous, &next, &records)
            .await
            .map_err(SubmitEstimateError::Storage)?
        {
            EstimateCommitOutcome::Conflict => Err(SubmitEstimateError::ConcurrentUpdate),
            EstimateCommitOutcome::Committed => Ok(records
                .iter()
                .map(|record| {
                    self.events.publish(DomainEvent::EstimateSubmitted(
                        record
                            .submission()
                            .task_key()
                            .task()
                            .volatility()
                            .instrument()
                            .clone(),
                    ))
                })
                .collect()),
        }
    }
}
