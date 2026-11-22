use crate::{
    AiRunId, DeliveryReport, EstimateSubmissionRepository, EventBus, ReasoningEffort,
    SignalCommitOutcome, SignalEvaluation, SignalMutation, SignalRepository, SignalSnapshot,
    SystemPortError,
};
use domain::{
    BuyQuantityDecision, BuyQuantityFacts, BuySignalOccurrenceId, BuySignalRecord,
    CalibrationError, CalibrationLedgerHead, CalibrationSnapshot, CommissionRateFact,
    CurrentBuySignal, EstimateAssignmentError, EstimateSubmissionRecord, InstrumentId, Margin,
    MarketCalendar, NoBuyQuantity, NoSignalReason, OrderFxRates, Policy, PolicySecurityFacts,
    PortfolioValuation, PriceObservation, SectorAssignments, SignalAssessmentFacts, SignalContext,
    SignalDecision, SignalEstimate, SignalPreparationError, SignalTiming, SignalTransition,
    SignalTransitionError, SignalUnavailable, Timestamp, calculate_buy_quantity,
    calibrate_signal_submission, signal_transition,
};
use std::future::Future;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignalQuantityCosts {
    pub fx: OrderFxRates,
    pub commission: CommissionRateFact,
}

#[derive(Debug, Clone)]
pub struct EvaluateSignalInputs {
    pub calibration: Option<CalibrationSnapshot>,
    pub calibration_head: Option<CalibrationLedgerHead>,
    pub valuation: Option<PortfolioValuation>,
    pub policy: Policy,
    pub assignments: SectorAssignments,
    pub security: PolicySecurityFacts,
    pub target_price: Option<PriceObservation>,
    pub calendars: Vec<MarketCalendar>,
    pub quantity_costs: Option<SignalQuantityCosts>,
}

pub trait SignalEvaluationData: Send + Sync {
    fn inputs(
        &self,
        record: &EstimateSubmissionRecord<AiRunId, ReasoningEffort>,
        now: Timestamp,
    ) -> impl Future<Output = Result<EvaluateSignalInputs, SystemPortError>> + Send;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvaluateSignalOutcome {
    Applied {
        reason: Option<NoSignalReason>,
        quantity: Option<BuyQuantityDecision>,
        deliveries: Vec<DeliveryReport>,
    },
    NoSubmission,
    SubmissionDiscarded,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvaluateSignalError {
    Storage(SystemPortError),
    Assignment(EstimateAssignmentError),
    Preparation(SignalPreparationError),
    Calibration(CalibrationError),
    Transition(SignalTransitionError),
    Occurrence(NoBuyQuantity),
    ConcurrentUpdate,
}

#[derive(Debug)]
pub struct EvaluateSignals<'a, Repository, Data> {
    repository: &'a Repository,
    data: &'a Data,
    events: &'a EventBus,
}

impl<'a, Repository: SignalRepository + EstimateSubmissionRepository, Data: SignalEvaluationData>
    EvaluateSignals<'a, Repository, Data>
{
    #[must_use]
    pub const fn new(repository: &'a Repository, data: &'a Data, events: &'a EventBus) -> Self {
        Self {
            repository,
            data,
            events,
        }
    }

    pub async fn execute(
        &self,
        instrument: &InstrumentId,
        now: Timestamp,
        occurrence_id: BuySignalOccurrenceId,
    ) -> Result<EvaluateSignalOutcome, EvaluateSignalError> {
        for _ in 0_u8..3 {
            if let Some(outcome) = self.attempt(instrument, now, &occurrence_id).await? {
                return Ok(outcome);
            }
        }
        Err(EvaluateSignalError::ConcurrentUpdate)
    }

    async fn attempt(
        &self,
        instrument: &InstrumentId,
        now: Timestamp,
        occurrence_id: &BuySignalOccurrenceId,
    ) -> Result<Option<EvaluateSignalOutcome>, EvaluateSignalError> {
        let snapshot = self
            .repository
            .snapshot(instrument)
            .await
            .map_err(EvaluateSignalError::Storage)?;
        let records = self
            .repository
            .submission_records(instrument)
            .await
            .map_err(EvaluateSignalError::Storage)?;
        let Some(record) = records.last() else {
            return Ok(snapshot
                .submission
                .is_none()
                .then_some(EvaluateSignalOutcome::NoSubmission));
        };
        if snapshot.submission.as_ref() != Some(record.task_id()) {
            return Ok(None);
        }
        let assignments = self
            .repository
            .assignments()
            .await
            .map_err(EvaluateSignalError::Storage)?;
        let issued = assignments
            .lookup(record.task_id().as_estimate_id())
            .map_err(EvaluateSignalError::Assignment)?;
        let inputs = self
            .data
            .inputs(record, now)
            .await
            .map_err(EvaluateSignalError::Storage)?;
        let sources = self
            .repository
            .sources(record.task_id())
            .await
            .map_err(EvaluateSignalError::Storage)?;
        let decision = assess(record, issued.task(), &inputs, &sources, now)?;
        let transition = signal_transition(
            snapshot.current.as_ref().map(|entry| &entry.signal),
            &decision,
        )
        .map_err(EvaluateSignalError::Transition)?;
        let (occurrence, quantity) = self
            .quantity(
                (&snapshot, &transition, &decision),
                (&inputs, now, occurrence_id),
            )
            .await?;
        let mutation = SignalMutation::Evaluate(Box::new(SignalEvaluation {
            submission: record.task_id().clone(),
            decision,
            transition,
            calibration_head: inputs.calibration_head.clone(),
            occurrence,
        }));
        self.commit(instrument, &snapshot, &mutation, quantity)
            .await
    }

    async fn commit(
        &self,
        instrument: &InstrumentId,
        snapshot: &SignalSnapshot,
        mutation: &SignalMutation,
        quantity: Option<BuyQuantityDecision>,
    ) -> Result<Option<EvaluateSignalOutcome>, EvaluateSignalError> {
        let result = self
            .repository
            .commit(instrument, snapshot, mutation)
            .await
            .map_err(EvaluateSignalError::Storage)?;
        match result {
            SignalCommitOutcome::Conflict => Ok(None),
            SignalCommitOutcome::SubmissionDiscarded => {
                Ok(Some(EvaluateSignalOutcome::SubmissionDiscarded))
            }
            SignalCommitOutcome::Applied(commit) => {
                let deliveries = commit
                    .events
                    .into_iter()
                    .map(|event| self.events.publish(event))
                    .collect();
                Ok(Some(EvaluateSignalOutcome::Applied {
                    reason: commit.no_signal_reason,
                    quantity,
                    deliveries,
                }))
            }
        }
    }

    async fn quantity(
        &self,
        state: (&SignalSnapshot, &SignalTransition, &SignalDecision),
        request: (&EvaluateSignalInputs, Timestamp, &BuySignalOccurrenceId),
    ) -> Result<(Option<BuySignalRecord>, Option<BuyQuantityDecision>), EvaluateSignalError> {
        let (snapshot, transition, decision) = state;
        let (inputs, now, occurrence_id) = request;
        let SignalDecision::Buy(buy) = decision else {
            return Ok((None, None));
        };
        let occurrence = match transition {
            SignalTransition::Raised(_) | SignalTransition::Reversed(_) => Some(
                BuySignalRecord::from_signal(occurrence_id.clone(), buy, now, &inputs.calendars)
                    .map_err(EvaluateSignalError::Occurrence)?,
            ),
            SignalTransition::Unchanged | SignalTransition::Updated(_) => None,
            SignalTransition::Cleared(_) => {
                return Err(EvaluateSignalError::Occurrence(
                    NoBuyQuantity::Indeterminate,
                ));
            }
        };
        let existing = occurrence
            .as_ref()
            .or_else(|| {
                let entry = snapshot.current.as_ref()?;
                entry.occurrence.as_ref()
            })
            .ok_or(EvaluateSignalError::Occurrence(
                NoBuyQuantity::Indeterminate,
            ))?;
        let current = CurrentBuySignal::try_new(buy.clone(), existing.clone())
            .map_err(EvaluateSignalError::Occurrence)?;
        let history = self
            .repository
            .occurrences(
                buy.facts().task().volatility().instrument(),
                buy.facts().task().reference().instrument(),
            )
            .await
            .map_err(EvaluateSignalError::Storage)?;
        let quantity = match (
            &inputs.valuation,
            &inputs.target_price,
            &inputs.quantity_costs,
        ) {
            (Some(valuation), Some(price), Some(costs)) => {
                let facts = BuyQuantityFacts::new(
                    valuation,
                    &inputs.policy,
                    (&inputs.assignments, &inputs.security),
                    price,
                    (costs.fx, &costs.commission),
                );
                calculate_buy_quantity(&current, &facts, now, &inputs.calendars, &history)
            }
            _ => BuyQuantityDecision::None(NoBuyQuantity::Indeterminate),
        };
        Ok((occurrence, Some(quantity)))
    }
}

fn assess(
    record: &EstimateSubmissionRecord<AiRunId, ReasoningEffort>,
    task: &domain::CostEligibleTask,
    inputs: &EvaluateSignalInputs,
    sources: &crate::SignalSourceFacts,
    now: Timestamp,
) -> Result<SignalDecision, EvaluateSignalError> {
    let Some(head) = inputs.calibration_head.as_ref() else {
        return Ok(SignalDecision::None(NoSignalReason::Unavailable(
            SignalUnavailable::MissingCalibration,
        )));
    };
    let submitted = match calibrate_signal_submission(
        inputs.calibration.as_ref(),
        head,
        record.submission().clone(),
    ) {
        Ok(value) => value,
        Err(CalibrationError::MissingCalibration) => {
            return Ok(SignalDecision::None(NoSignalReason::Unavailable(
                SignalUnavailable::MissingCalibration,
            )));
        }
        Err(CalibrationError::StaleCalibration) => {
            return Ok(SignalDecision::None(NoSignalReason::Unavailable(
                SignalUnavailable::StaleCalibration,
            )));
        }
        Err(CalibrationError::MismatchedCombination) => {
            return Ok(SignalDecision::None(NoSignalReason::Unavailable(
                SignalUnavailable::CalibrationCombinationMismatch,
            )));
        }
        Err(error) => return Err(EvaluateSignalError::Calibration(error)),
    };
    let estimate = SignalEstimate::create(
        task,
        &submitted,
        Margin::for_appetite(inputs.policy.profile().risk_appetite()),
    )
    .map_err(EvaluateSignalError::Preparation)?;
    let ledger = sources.ledger();
    let facts = SignalAssessmentFacts::new(
        SignalTiming::new(now, &inputs.calendars),
        inputs.target_price.clone(),
        Some(head.clone()),
    );
    let context = SignalContext::new(
        inputs.valuation.as_ref(),
        &inputs.policy,
        (&inputs.assignments, &inputs.security),
        &ledger,
        facts,
    );
    Ok(SignalDecision::assess(&estimate, &context))
}
