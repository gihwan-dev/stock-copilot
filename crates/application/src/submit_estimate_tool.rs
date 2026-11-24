use std::future::Future;

use domain::{
    AiProbability, AiProbabilityError, AiProbabilityInput, CalibrationEstimateId, CalibrationKey,
    CalibrationModel, EstimateAssignmentError, EstimateMetadata, EstimateNarrative, Evidence,
    EvidenceRef, Horizon, SignalSubmission, SignalSubmissionError, SignalTaskKey, ThesisError,
    ToolReference, UntrustedText,
};

use crate::{
    AiRunId, Clock, DeliveryReport, EstimateSubmissionInput, EstimateSubmissionRepository,
    EventBus, ReasoningEffort, SubmitEstimate, SubmitEstimateError, SystemPortError,
    ToolCallOutcome, ToolCallRecord,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EstimateToolArguments {
    pub task_id: CalibrationEstimateId,
    pub probability: AiProbabilityInput,
    pub rationale: UntrustedText,
    pub counterargument: UntrustedText,
    pub sources: Vec<ToolReference>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EstimateRunContext {
    pub run: AiRunId,
    pub model: CalibrationModel,
    pub horizon: Horizon,
    pub metadata: EstimateMetadata<ReasoningEffort>,
}

pub trait ToolCallHistory: Send + Sync {
    fn calls_in_run(
        &self,
        run: &AiRunId,
    ) -> impl Future<Output = Result<Vec<ToolCallRecord>, SystemPortError>> + Send;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubmitEstimateToolError {
    Probability(AiProbabilityError),
    UnknownSource,
    Sources(ThesisError),
    Assignment(EstimateAssignmentError),
    Submission(SignalSubmissionError),
    Submit(SubmitEstimateError),
    Ledger(SystemPortError),
    Storage(SystemPortError),
}

#[derive(Debug)]
pub struct SubmitEstimateTool<'a, Repository, History, TimeSource> {
    repository: &'a Repository,
    history: &'a History,
    clock: &'a TimeSource,
    events: &'a EventBus,
}

impl<'a, Repository, History, TimeSource> SubmitEstimateTool<'a, Repository, History, TimeSource>
where
    Repository: EstimateSubmissionRepository,
    History: ToolCallHistory,
    TimeSource: Clock,
{
    #[must_use]
    pub const fn new(
        repository: &'a Repository,
        history: &'a History,
        clock: &'a TimeSource,
        events: &'a EventBus,
    ) -> Self {
        Self {
            repository,
            history,
            clock,
            events,
        }
    }

    pub async fn execute(
        &self,
        context: EstimateRunContext,
        arguments: EstimateToolArguments,
    ) -> Result<DeliveryReport, SubmitEstimateToolError> {
        let probability = AiProbability::parse(&arguments.probability)
            .map_err(SubmitEstimateToolError::Probability)?;
        let evidence = self.evidence(&context.run, arguments.sources).await?;
        let assignments = self
            .repository
            .assignments()
            .await
            .map_err(SubmitEstimateToolError::Storage)?;
        let issued = assignments
            .lookup(&arguments.task_id)
            .map_err(SubmitEstimateToolError::Assignment)?;
        if issued.run() != &context.run {
            return Err(SubmitEstimateToolError::Assignment(
                EstimateAssignmentError::WrongRun,
            ));
        }
        let submission = SignalSubmission::create(
            SignalTaskKey::from_task(issued.task()),
            CalibrationKey::new(context.model, context.horizon),
            probability,
            evidence,
            self.clock.now(),
        )
        .map_err(SubmitEstimateToolError::Submission)?;
        let input = EstimateSubmissionInput {
            task_id: arguments.task_id,
            submission,
            narrative: EstimateNarrative {
                rationale: arguments.rationale,
                counterargument: arguments.counterargument,
            },
            metadata: context.metadata,
        };
        let use_case = SubmitEstimate::new(self.repository, self.events);
        loop {
            match use_case.execute(&context.run, vec![input.clone()]).await {
                Err(SubmitEstimateError::ConcurrentUpdate) => {}
                Err(error) => return Err(SubmitEstimateToolError::Submit(error)),
                Ok(mut reports) => {
                    return reports.pop().ok_or(SubmitEstimateToolError::Submit(
                        SubmitEstimateError::ConcurrentUpdate,
                    ));
                }
            }
        }
    }

    async fn evidence(
        &self,
        run: &AiRunId,
        sources: Vec<ToolReference>,
    ) -> Result<Option<Evidence>, SubmitEstimateToolError> {
        if sources.is_empty() {
            return Ok(None);
        }
        let calls = self
            .history
            .calls_in_run(run)
            .await
            .map_err(SubmitEstimateToolError::Ledger)?;
        let found = sources.iter().all(|source| {
            let suffix = source.as_str().strip_prefix("tool:");
            calls.iter().any(|call| {
                call.run_id() == run
                    && call.outcome() == ToolCallOutcome::Succeeded
                    && suffix == Some(call.request_id().as_str())
            })
        });
        if !found {
            return Err(SubmitEstimateToolError::UnknownSource);
        }
        Evidence::try_new(sources.into_iter().map(EvidenceRef::Tool).collect())
            .map(Some)
            .map_err(SubmitEstimateToolError::Sources)
    }
}
