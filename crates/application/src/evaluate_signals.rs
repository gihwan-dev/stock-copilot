use crate::{
    AiRunId, DeliveryReport, EstimateSubmissionRepository, EventBus, ReasoningEffort,
    SignalCommitEvidence, SignalCommitOutcome, SignalEvaluation, SignalInvalidationOutcome,
    SignalInvalidationRepository, SignalMutation, SignalRepository, SignalSnapshot,
    SystemPortError,
};
use domain::{
    BuyQuantityDecision, BuyQuantityFacts, BuySignalOccurrenceId, BuySignalRecord,
    CalibrationError, CalibrationLedgerHead, CalibrationSnapshot, CommissionRateFact,
    CurrentBuySignal, EstimateAssignmentError, EstimateSubmissionRecord, FxRate, InstrumentId,
    Margin, MarketCalendar, MasterInstrument, NoBuyQuantity, NoSignalReason, OrderFxRates, Policy,
    PolicySecurityFacts, Portfolio, PortfolioValuation, PriceObservation, SectorAssignments,
    SignalAssessmentFacts, SignalContext, SignalDecision, SignalEstimate,
    SignalOccurrenceRestoreError, SignalPreparationError, SignalRestoreError,
    SignalRestoreEvidence, SignalTiming, SignalTransition, SignalTransitionError,
    SignalUnavailable, StoredEstimateEvidence, StoredPriceObservation, StoredSignalOccurrence,
    Timestamp, TradeTaxFact, calculate_buy_quantity_with_history, calibrate_signal_submission,
    signal_transition_from_history,
};
use std::future::Future;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignalQuantityCosts {
    pub fx: OrderFxRates,
    pub commission: CommissionRateFact,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignalRestorationBasis {
    pub master: MasterInstrument,
    pub holding_masters: Vec<MasterInstrument>,
    pub estimate: StoredEstimateEvidence,
    pub commission: CommissionRateFact,
    pub tax: TradeTaxFact,
    pub maturity: Timestamp,
    pub portfolio: Option<Portfolio>,
    pub rate: Option<FxRate>,
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
    pub restoration: Option<SignalRestorationBasis>,
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
    Unavailable {
        cause: Box<EvaluateSignalError>,
        deliveries: Vec<DeliveryReport>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvaluateSignalError {
    Storage(SystemPortError),
    Inputs(SystemPortError),
    Sources(SystemPortError),
    Occurrences(SystemPortError),
    Invalidation {
        cause: Box<Self>,
        error: SystemPortError,
    },
    Assignment(EstimateAssignmentError),
    Preparation(SignalPreparationError),
    Calibration(CalibrationError),
    Transition(SignalTransitionError),
    Occurrence(NoBuyQuantity),
    ConcurrentUpdate,
    RestorationUnavailable,
    RestorationValuationMismatch,
    RestorationMasters(domain::MasterInstrumentError),
    RestorationPortfolio(domain::HistoricalPortfolioError),
    Restoration(SignalRestoreError),
    RestorationOccurrence(SignalOccurrenceRestoreError),
}

#[derive(Debug)]
pub struct EvaluateSignals<'a, Repository, Data> {
    repository: &'a Repository,
    data: &'a Data,
    events: &'a EventBus,
}

impl<
    'a,
    Repository: SignalRepository + SignalInvalidationRepository + EstimateSubmissionRepository,
    Data: SignalEvaluationData,
> EvaluateSignals<'a, Repository, Data>
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
        let records = match self.repository.submission_records(instrument).await {
            Ok(value) => value,
            Err(error) => {
                return self
                    .invalidate(instrument, &snapshot, EvaluateSignalError::Storage(error))
                    .await;
            }
        };
        let Some(record) = records.last() else {
            return Ok(snapshot
                .submission
                .is_none()
                .then_some(EvaluateSignalOutcome::NoSubmission));
        };
        if snapshot.submission.as_ref() != Some(record.task_id()) {
            return Ok(None);
        }
        self.evaluate_record(instrument, &snapshot, record, (now, occurrence_id))
            .await
    }

    async fn evaluate_record(
        &self,
        instrument: &InstrumentId,
        snapshot: &SignalSnapshot,
        record: &EstimateSubmissionRecord<AiRunId, ReasoningEffort>,
        timing: (Timestamp, &BuySignalOccurrenceId),
    ) -> Result<Option<EvaluateSignalOutcome>, EvaluateSignalError> {
        let (now, _) = timing;
        let assignments = match self.repository.assignments().await {
            Ok(value) => value,
            Err(error) => {
                return self
                    .invalidate(instrument, snapshot, EvaluateSignalError::Storage(error))
                    .await;
            }
        };
        let issued = match assignments.lookup(record.task_id().as_estimate_id()) {
            Ok(value) => value,
            Err(error) => {
                return self
                    .invalidate(instrument, snapshot, EvaluateSignalError::Assignment(error))
                    .await;
            }
        };
        let inputs = match self.data.inputs(record, now).await {
            Ok(value) => value,
            Err(error) => {
                return self
                    .invalidate(instrument, snapshot, EvaluateSignalError::Inputs(error))
                    .await;
            }
        };
        let sources = match self.repository.sources(record.task_id()).await {
            Ok(value) => value,
            Err(error) => {
                return self
                    .invalidate(instrument, snapshot, EvaluateSignalError::Sources(error))
                    .await;
            }
        };
        self.evaluate_prepared(
            (instrument, snapshot, record),
            (issued.task(), &inputs, &sources),
            timing,
        )
        .await
    }

    async fn evaluate_prepared(
        &self,
        target: (
            &InstrumentId,
            &SignalSnapshot,
            &EstimateSubmissionRecord<AiRunId, ReasoningEffort>,
        ),
        prepared: (
            &domain::CostEligibleTask,
            &EvaluateSignalInputs,
            &crate::SignalSourceFacts,
        ),
        timing: (Timestamp, &BuySignalOccurrenceId),
    ) -> Result<Option<EvaluateSignalOutcome>, EvaluateSignalError> {
        let (instrument, snapshot, record) = target;
        let (task, inputs, sources) = prepared;
        let (now, occurrence_id) = timing;
        let initial = match assess(record, task, inputs, sources, now) {
            Ok(value) => value,
            Err(error) => return self.invalidate(instrument, snapshot, error).await,
        };
        let (decision, transition, occurrence) =
            prepare_transition(snapshot, initial, (&inputs.calendars, now, occurrence_id))?;
        let quantity = match self
            .quantity((snapshot, &decision, occurrence.as_ref()), (inputs, now))
            .await
        {
            Ok(value) => value,
            Err(error) => return self.invalidate(instrument, snapshot, error).await,
        };
        let evidence = match restoration_evidence(
            record,
            &decision,
            occurrence.as_ref(),
            (inputs, sources, now),
        ) {
            Ok(value) => value,
            Err(error) => return self.invalidate(instrument, snapshot, error).await,
        };
        let mutation = SignalMutation::Evaluate(Box::new(SignalEvaluation {
            submission: record.task_id().clone(),
            decision,
            transition,
            calibration_head: inputs.calibration_head.clone(),
            occurrence,
            evidence,
        }));
        self.commit(instrument, snapshot, &mutation, quantity).await
    }

    async fn invalidate(
        &self,
        instrument: &InstrumentId,
        snapshot: &SignalSnapshot,
        cause: EvaluateSignalError,
    ) -> Result<Option<EvaluateSignalOutcome>, EvaluateSignalError> {
        let outcome = self
            .repository
            .invalidate(instrument, snapshot)
            .await
            .map_err(|error| EvaluateSignalError::Invalidation {
                cause: Box::new(cause.clone()),
                error,
            })?;
        match outcome {
            SignalInvalidationOutcome::Conflict => Ok(None),
            SignalInvalidationOutcome::SubmissionDiscarded => {
                Ok(Some(EvaluateSignalOutcome::SubmissionDiscarded))
            }
            SignalInvalidationOutcome::Applied(events) => {
                let deliveries = events
                    .into_iter()
                    .map(|event| self.events.publish(event))
                    .collect();
                Ok(Some(EvaluateSignalOutcome::Unavailable {
                    cause: Box::new(cause),
                    deliveries,
                }))
            }
        }
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
        state: (&SignalSnapshot, &SignalDecision, Option<&BuySignalRecord>),
        request: (&EvaluateSignalInputs, Timestamp),
    ) -> Result<Option<BuyQuantityDecision>, EvaluateSignalError> {
        let (snapshot, decision, occurrence) = state;
        let (inputs, now) = request;
        let SignalDecision::Buy(buy) = decision else {
            return Ok(None);
        };
        let existing = occurrence
            .map(domain::HistoricalSignalOccurrence::from_record)
            .or_else(|| {
                let entry = snapshot.current.as_ref()?;
                entry.occurrence.clone()
            })
            .ok_or(EvaluateSignalError::Occurrence(
                NoBuyQuantity::Indeterminate,
            ))?;
        let current = CurrentBuySignal::try_from_history(buy.clone(), existing)
            .map_err(EvaluateSignalError::Occurrence)?;
        let history = self
            .repository
            .occurrences(
                buy.facts().task().volatility().instrument(),
                buy.facts().task().reference().instrument(),
            )
            .await
            .map_err(EvaluateSignalError::Occurrences)?;
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
                calculate_buy_quantity_with_history(
                    &current,
                    &facts,
                    now,
                    &inputs.calendars,
                    &history,
                )
            }
            _ => BuyQuantityDecision::None(NoBuyQuantity::Indeterminate),
        };
        Ok(Some(quantity))
    }
}

fn prepare_transition(
    snapshot: &SignalSnapshot,
    decision: SignalDecision,
    timing: (&[MarketCalendar], Timestamp, &BuySignalOccurrenceId),
) -> Result<(SignalDecision, SignalTransition, Option<BuySignalRecord>), EvaluateSignalError> {
    let previous = snapshot.current.as_ref().map(|entry| &entry.signal);
    let transition = signal_transition_from_history(previous, &decision)
        .map_err(EvaluateSignalError::Transition)?;
    let (calendars, now, occurrence_id) = timing;
    let result = match (&decision, &transition) {
        (SignalDecision::Buy(buy), SignalTransition::Raised(_) | SignalTransition::Reversed(_)) => {
            Some(BuySignalRecord::from_signal_checked(
                occurrence_id.clone(),
                buy,
                now,
                calendars,
            ))
        }
        _ => None,
    };
    match result {
        Some(Ok(occurrence)) => Ok((decision, transition, Some(occurrence))),
        None => Ok((decision, transition, None)),
        Some(Err(reason)) => {
            let rejected = SignalDecision::None(reason);
            let cleared = signal_transition_from_history(previous, &rejected)
                .map_err(EvaluateSignalError::Transition)?;
            Ok((rejected, cleared, None))
        }
    }
}

pub(crate) fn assess(
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

fn restoration_evidence(
    record: &EstimateSubmissionRecord<AiRunId, ReasoningEffort>,
    decision: &SignalDecision,
    occurrence: Option<&BuySignalRecord>,
    context: (&EvaluateSignalInputs, &crate::SignalSourceFacts, Timestamp),
) -> Result<Option<SignalCommitEvidence>, EvaluateSignalError> {
    let (inputs, sources, now) = context;
    let Some(signal) = decision.clone().into_signal() else {
        return Ok(None);
    };
    let basis = inputs
        .restoration
        .as_ref()
        .ok_or(EvaluateSignalError::RestorationUnavailable)?;
    let valuation = basis
        .portfolio
        .as_ref()
        .map(|portfolio| portfolio.evaluate(basis.rate));
    if valuation != inputs.valuation {
        return Err(EvaluateSignalError::RestorationValuationMismatch);
    }
    let combined = combine_masters(&basis.master, &basis.holding_masters)
        .map_err(EvaluateSignalError::RestorationMasters)?;
    let snapshot = inputs
        .calibration
        .clone()
        .ok_or(EvaluateSignalError::RestorationUnavailable)?;
    let submission = record.submission();
    let evidence = SignalRestoreEvidence {
        estimate: basis.estimate.clone(),
        commission: basis.commission.clone(),
        tax: basis.tax.clone(),
        maturity: basis.maturity,
        calibration: snapshot,
        calibration_key: submission.calibration_key().clone(),
        probability: submission.probability(),
        source: submission.evidence().cloned(),
        submitted_at: submission.submitted_at(),
        margin: Margin::for_appetite(inputs.policy.profile().risk_appetite()),
        assessment: historical_context(basis, combined.instruments(), inputs, sources, now)?,
    }
    .capture(&signal, &basis.master)
    .map_err(EvaluateSignalError::Restoration)?;
    let stored_occurrence = occurrence
        .map(|value| StoredSignalOccurrence::capture(value, evidence.clone(), &basis.master))
        .transpose()
        .map_err(EvaluateSignalError::RestorationOccurrence)?;
    Ok(Some(SignalCommitEvidence {
        master: basis.master.clone(),
        holding_masters: basis.holding_masters.clone(),
        signal: evidence,
        occurrence: stored_occurrence,
    }))
}

fn combine_masters(
    primary: &MasterInstrument,
    extras: &[MasterInstrument],
) -> Result<domain::MasterInstrumentFacts, domain::MasterInstrumentError> {
    let mut all = vec![primary.clone()];
    for extra in extras {
        if !all.iter().any(|existing| {
            existing == extra
                && existing.row().observed_at.offset_seconds()
                    == extra.row().observed_at.offset_seconds()
        }) {
            all.push(extra.clone());
        }
    }
    domain::MasterInstrumentFacts::try_new(all)
}

fn historical_context(
    basis: &SignalRestorationBasis,
    masters: &[MasterInstrument],
    inputs: &EvaluateSignalInputs,
    sources: &crate::SignalSourceFacts,
    now: Timestamp,
) -> Result<domain::HistoricalSignalContext, EvaluateSignalError> {
    let portfolio = basis
        .portfolio
        .as_ref()
        .map(|value| domain::HistoricalPortfolio::capture(value, masters))
        .transpose()
        .map_err(EvaluateSignalError::RestorationPortfolio)?;
    Ok(domain::HistoricalSignalContext {
        portfolio,
        rate: basis.rate,
        policy: inputs.policy.clone(),
        classifications: domain::HistoricalClassifications::capture(
            &inputs.assignments,
            &inputs.security,
        ),
        items: sources.items.clone(),
        disclosures: sources.disclosures.clone(),
        tools: sources.tools.clone(),
        calendars: inputs.calendars.clone(),
        evaluated_at: now,
        target: inputs
            .target_price
            .as_ref()
            .map(StoredPriceObservation::from_price_observation),
        calibration_head: inputs.calibration_head.clone(),
    })
}
