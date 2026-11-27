use std::future::Future;

use domain::{
    AiProbability, AiProbabilityError, AiProbabilityInput, CalibrationEstimateId, CalibrationKey,
    CalibrationModel, EstimateAssignmentError, EstimateMetadata, EstimateNarrative, Evidence,
    EvidenceRef, EvidenceUrl, Horizon, InformationSource, SignalSubmission, SignalSubmissionError,
    SignalTaskKey, ThesisError, ToolReference, UntrustedText,
};

use crate::{
    AiRunId, Clock, CollectedEntry, CollectedItemHandle, DeliveryReport, EstimateSubmissionInput,
    EstimateSubmissionRepository, EventBus, ReasoningEffort, SignalSourceFacts, SubmitEstimate,
    SubmitEstimateError, SystemPortError, ToolCallOutcome, ToolCallRecord,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EstimateToolArguments<Source = ToolReference> {
    pub task_id: CalibrationEstimateId,
    pub probability: AiProbabilityInput,
    pub rationale: UntrustedText,
    pub counterargument: UntrustedText,
    pub sources: Vec<Source>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EstimateSource {
    Tool(ToolReference),
    Collected(CollectedItemHandle),
}

impl From<ToolReference> for EstimateSource {
    fn from(reference: ToolReference) -> Self {
        Self::Tool(reference)
    }
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

    pub async fn execute<S: Into<EstimateSource>>(
        &self,
        context: EstimateRunContext,
        arguments: EstimateToolArguments<S>,
    ) -> Result<DeliveryReport, SubmitEstimateToolError> {
        let probability = AiProbability::parse(&arguments.probability)
            .map_err(SubmitEstimateToolError::Probability)?;
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
        let (evidence, sources) = self
            .evidence(&context.run, issued.task().task(), arguments.sources)
            .await?;
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
            match use_case
                .execute_with_sources(&context.run, vec![input.clone()], Some(&sources))
                .await
            {
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

    async fn evidence<S: Into<EstimateSource>>(
        &self,
        run: &AiRunId,
        task: &domain::EstimateTask,
        sources: Vec<S>,
    ) -> Result<(Option<Evidence>, SignalSourceFacts), SubmitEstimateToolError> {
        let mut facts = SignalSourceFacts::default();
        if sources.is_empty() {
            return Ok((None, facts));
        }
        let parsed: Vec<_> = sources.into_iter().map(Into::into).collect();
        let calls = if parsed.iter().any(|source| match source {
            EstimateSource::Tool(_) => true,
            EstimateSource::Collected(_) => false,
        }) {
            self.history
                .calls_in_run(run)
                .await
                .map_err(SubmitEstimateToolError::Ledger)?
        } else {
            Vec::new()
        };
        let mut references = Vec::with_capacity(parsed.len());
        for provided in parsed {
            let reference = match provided {
                EstimateSource::Collected(handle) => {
                    self.citation(run, task, &handle, &mut facts).await?
                }
                EstimateSource::Tool(source) => {
                    verify_tool(run, &calls, &source)?;
                    facts.tools.push(source.clone());
                    EvidenceRef::Tool(source)
                }
            };
            references.push(reference);
        }
        Evidence::try_new(references)
            .map(|evidence| (Some(evidence), facts))
            .map_err(SubmitEstimateToolError::Sources)
    }

    async fn citation(
        &self,
        run: &AiRunId,
        task: &domain::EstimateTask,
        handle: &CollectedItemHandle,
        facts: &mut SignalSourceFacts,
    ) -> Result<EvidenceRef, SubmitEstimateToolError> {
        let found = self
            .repository
            .collected_citation(run, handle)
            .await
            .map_err(SubmitEstimateToolError::Storage)?
            .ok_or(SubmitEstimateToolError::UnknownSource)?;
        if !found.instruments.is_empty()
            && !found.instruments.iter().any(|linked| {
                linked == task.volatility().instrument()
                    || linked.as_str() == task.reference().instrument().symbol().as_str()
            })
        {
            return Err(SubmitEstimateToolError::UnknownSource);
        }
        match found.entry {
            CollectedEntry::News(news) => {
                let item = news.collected().clone();
                let url = EvidenceUrl::try_new(item.url().clone())
                    .map_err(SubmitEstimateToolError::Sources)?;
                let reference = match item.source() {
                    InformationSource::News(source) => EvidenceRef::News(source.clone(), url),
                    InformationSource::Web => EvidenceRef::Web(url),
                    InformationSource::Disclosure(_) => {
                        return Err(SubmitEstimateToolError::UnknownSource);
                    }
                };
                facts.items.push(item);
                Ok(reference)
            }
            CollectedEntry::Disclosure(filing) => {
                let reference = EvidenceRef::from_disclosure(&filing)
                    .map_err(SubmitEstimateToolError::Sources)?;
                facts.disclosures.push(filing);
                Ok(reference)
            }
        }
    }
}

fn verify_tool(
    run: &AiRunId,
    calls: &[ToolCallRecord],
    reference: &ToolReference,
) -> Result<(), SubmitEstimateToolError> {
    let id = reference
        .as_str()
        .strip_prefix("tool:")
        .ok_or(SubmitEstimateToolError::UnknownSource)?;
    calls
        .iter()
        .find(|call| {
            call.run_id() == run
                && call.outcome() == ToolCallOutcome::Succeeded
                && call.request_id().as_str() == id
        })
        .map(|_call| ())
        .ok_or(SubmitEstimateToolError::UnknownSource)
}
