use crate::script::State;
use application::{
    AiRunId, EstimateCommitOutcome, EstimateSubmissionRepository, ReasoningEffort, SignalCommit,
    SignalCommitEvidence, SignalCommitOutcome, SignalInvalidationOutcome,
    SignalInvalidationRepository, SignalMutation, SignalRepository, SignalRevision, SignalSnapshot,
    SignalSourceFacts, StoredSignal, SystemFailure, SystemPort, SystemPortError,
};
use domain::{
    BuySignalRecord, CalibrationLedgerHead, CurrentBuySignal, EstimateAssignments,
    EstimateSubmissionRecord, HistoricalSignal, HistoricalSignalOccurrence, InstrumentId,
    IssuedEstimateTaskId, NoSignalReason, RaisedSignal, ResolvedOrderInstrument, SignalDecision,
    SignalTransition, signal_transition_from_history,
};

#[derive(Debug, Default)]
struct InstrumentState {
    instrument: Option<InstrumentId>,
    revision: u64,
    current: Option<StoredSignal>,
    evidence: Option<SignalCommitEvidence>,
    discarded: Vec<IssuedEstimateTaskId>,
}

#[derive(Debug, Clone)]
struct SignalHistoryEntry {
    record: BuySignalRecord,
    evidence: domain::StoredSignalOccurrence,
}

#[derive(Debug, Default)]
struct RepositoryState {
    assignments: EstimateAssignments<AiRunId>,
    records: Vec<EstimateSubmissionRecord<AiRunId, ReasoningEffort>>,
    instruments: Vec<InstrumentState>,
    history: Vec<SignalHistoryEntry>,
    sources: Vec<(IssuedEstimateTaskId, SignalSourceFacts)>,
    heads: Vec<CalibrationLedgerHead>,
    masters: Vec<domain::MasterInstrument>,
    fail_next_commit: Option<SystemPortError>,
}

#[derive(Debug, Default)]
pub struct MemorySignalEvaluationRepository {
    state: State<RepositoryState>,
}
impl MemorySignalEvaluationRepository {
    #[must_use]
    pub fn new(assignments: EstimateAssignments<AiRunId>) -> Self {
        Self {
            state: State::new(RepositoryState {
                assignments,
                ..RepositoryState::default()
            }),
        }
    }
    pub fn save_sources(&self, task: IssuedEstimateTaskId, facts: SignalSourceFacts) {
        let mut state = self.state.write();
        state.sources.retain(|(stored, _)| stored != &task);
        state.sources.push((task, facts));
    }
    pub fn advance_calibration_head(
        &self,
        head: CalibrationLedgerHead,
    ) -> Result<(), SystemPortError> {
        let mut state = self.state.write();
        if state
            .heads
            .iter()
            .any(|old| old.key() == head.key() && old.position() >= head.position())
        {
            return Err(invalid());
        }
        state.heads.retain(|old| old.key() != head.key());
        state.heads.push(head);
        Ok(())
    }
    pub fn fail_next_commit(&self, error: SystemPortError) {
        self.state.write().fail_next_commit = Some(error);
    }
    pub fn save_master(&self, master: domain::MasterInstrument) {
        let mut state = self.state.write();
        state
            .masters
            .retain(|old| old.row().instrument_id != master.row().instrument_id);
        state.masters.push(master);
    }
}

fn invalid() -> SystemPortError {
    SystemPortError::new(SystemPort::Repository, SystemFailure::InvalidData)
}
fn integrity() -> SystemPortError {
    SystemPortError::new(SystemPort::Repository, SystemFailure::RepositoryIntegrity)
}
fn snapshot(
    state: &RepositoryState,
    instrument: &InstrumentId,
) -> Result<SignalSnapshot, SystemPortError> {
    let entry = state
        .instruments
        .iter()
        .find(|entry| entry.instrument.as_ref() == Some(instrument));
    let submission = state.records.iter().rev().find(|record| {
        record
            .submission()
            .task_key()
            .task()
            .volatility()
            .instrument()
            == instrument
    });
    let current = entry
        .map(|value| checked_current(state, value))
        .transpose()?
        .flatten();
    Ok(SignalSnapshot {
        revision: SignalRevision::new(entry.map_or(0, |value| value.revision)),
        submission: submission.map(|value| value.task_id().clone()),
        current,
    })
}
fn checked_current(
    state: &RepositoryState,
    entry: &InstrumentState,
) -> Result<Option<StoredSignal>, SystemPortError> {
    match (&entry.current, &entry.evidence) {
        (None, None) => Ok(None),
        (Some(current), Some(evidence)) => {
            let restored = restore_signal(&evidence.signal, &state.masters)?;
            if restored != current.signal {
                return Err(integrity());
            }
            if let Some(occurrence) = &current.occurrence {
                let saved = state
                    .history
                    .iter()
                    .find(|value| value.record.id() == occurrence.id())
                    .ok_or_else(integrity)?;
                if checked_occurrence(saved, &state.masters)? != *occurrence {
                    return Err(integrity());
                }
            }
            Ok(Some(current.clone()))
        }
        (None, Some(_)) | (Some(_), None) => Err(integrity()),
    }
}
fn checked_occurrence(
    value: &SignalHistoryEntry,
    masters: &[domain::MasterInstrument],
) -> Result<HistoricalSignalOccurrence, SystemPortError> {
    let restored = restore_occurrence(&value.evidence, masters)?;
    if restored != HistoricalSignalOccurrence::from_record(&value.record) {
        return Err(integrity());
    }
    Ok(restored)
}
fn rebuild_signal(
    evidence: &domain::StoredSignalEvidence,
    masters: &[domain::MasterInstrument],
) -> Result<domain::StoredSignalEvidence, SystemPortError> {
    let mut reconstructed = evidence.clone();
    for assessment in [
        &mut reconstructed.evidence.assessment,
        &mut reconstructed.original.assessment,
    ] {
        if let Some(portfolio) = &assessment.portfolio {
            assessment.portfolio = Some(
                domain::HistoricalPortfolio::from_saved_facts(portfolio.facts().clone(), masters)
                    .map_err(|_error| integrity())?,
            );
        }
    }
    Ok(reconstructed)
}
fn master_for_signal<'a>(
    evidence: &domain::StoredSignalEvidence,
    masters: &'a [domain::MasterInstrument],
) -> Result<&'a domain::MasterInstrument, SystemPortError> {
    let id = &evidence.evidence.estimate.evidence.master.instrument_id;
    masters
        .iter()
        .find(|master| &master.row().instrument_id == id)
        .ok_or_else(integrity)
}
fn restore_signal(
    evidence: &domain::StoredSignalEvidence,
    masters: &[domain::MasterInstrument],
) -> Result<HistoricalSignal, SystemPortError> {
    let current = master_for_signal(evidence, masters)?;
    rebuild_signal(evidence, masters)?
        .restore_history(current)
        .map_err(|_error| integrity())
}
fn restore_occurrence(
    evidence: &domain::StoredSignalOccurrence,
    masters: &[domain::MasterInstrument],
) -> Result<HistoricalSignalOccurrence, SystemPortError> {
    let mut reconstructed = evidence.clone();
    reconstructed.signal = rebuild_signal(&reconstructed.signal, masters)?;
    let current = master_for_signal(&reconstructed.signal, masters)?;
    reconstructed.restore(current).map_err(|_error| integrity())
}
fn signal_submission(signal: &RaisedSignal) -> &domain::SignalSubmission {
    match signal {
        RaisedSignal::Buy(value) => value.facts().submission(),
        RaisedSignal::Sell(value) => value.facts().submission(),
    }
}
fn signal_instrument(signal: &RaisedSignal) -> &InstrumentId {
    signal_submission(signal)
        .task_key()
        .task()
        .volatility()
        .instrument()
}
fn add_occurrence(
    signal: &RaisedSignal,
    prior: Option<&StoredSignal>,
    supplied: Option<&BuySignalRecord>,
    transition: &SignalTransition,
    history: &[SignalHistoryEntry],
) -> Result<(Option<HistoricalSignalOccurrence>, Option<BuySignalRecord>), SystemPortError> {
    match signal {
        RaisedSignal::Sell(_) => {
            if supplied.is_some() {
                return Err(invalid());
            }
            Ok((None, None))
        }
        RaisedSignal::Buy(buy) => {
            let occurrence = match transition {
                SignalTransition::Raised(_) | SignalTransition::Reversed(_) => {
                    let record = supplied.ok_or_else(invalid)?;
                    if history.iter().any(|old| old.record.id() == record.id()) {
                        return Err(invalid());
                    }
                    if record.originating_task().task() != buy.facts().task() {
                        return Err(invalid());
                    }
                    CurrentBuySignal::try_new(buy.clone(), record.clone())
                        .map_err(|_error| invalid())?;
                    return Ok((
                        Some(HistoricalSignalOccurrence::from_record(record)),
                        Some(record.clone()),
                    ));
                }
                SignalTransition::Unchanged | SignalTransition::Updated(_) => {
                    if supplied.is_some() {
                        return Err(invalid());
                    }
                    prior
                        .and_then(|value| value.occurrence.clone())
                        .ok_or_else(invalid)?
                }
                SignalTransition::Cleared(_) => return Err(invalid()),
            };
            CurrentBuySignal::try_from_history(buy.clone(), occurrence.clone())
                .map_err(|_error| invalid())?;
            Ok((Some(occurrence), None))
        }
    }
}
#[derive(Debug)]
struct Change {
    current: Option<StoredSignal>,
    occurrence: Option<SignalHistoryEntry>,
    evidence: Option<SignalCommitEvidence>,
    discarded: Option<IssuedEstimateTaskId>,
    events: Vec<domain::DomainEvent>,
    reason: Option<NoSignalReason>,
}

fn prepare_evaluation(
    state: &RepositoryState,
    instrument: &InstrumentId,
    expected: &SignalSnapshot,
    evaluation: &application::SignalEvaluation,
) -> Result<Option<Change>, SystemPortError> {
    let Some(submission_id) = &expected.submission else {
        return Err(invalid());
    };
    if &evaluation.submission != submission_id {
        return Ok(None);
    }
    let record = state
        .records
        .iter()
        .find(|value| value.task_id() == submission_id)
        .ok_or_else(invalid)?;
    let head = state
        .heads
        .iter()
        .find(|value| value.key() == record.submission().calibration_key());
    if head != evaluation.calibration_head.as_ref() {
        return Ok(None);
    }
    let previous = expected.current.as_ref().map(|value| &value.signal);
    if signal_transition_from_history(previous, &evaluation.decision).map_err(|_error| invalid())?
        != evaluation.transition
    {
        return Err(invalid());
    }
    let reason = match evaluation.decision {
        SignalDecision::None(reason) => Some(reason),
        SignalDecision::Buy(_) | SignalDecision::Sell(_) => None,
    };
    let (current, occurrence) =
        prepare_current((state, instrument, expected), evaluation, record, head)?;
    let saved_occurrence = match occurrence {
        Some(history_record) => {
            let evidence = evaluation.evidence.as_ref().ok_or_else(invalid)?;
            Some(SignalHistoryEntry {
                record: history_record,
                evidence: evidence.occurrence.clone().ok_or_else(invalid)?,
            })
        }
        None => None,
    };
    Ok(Some(Change {
        current,
        occurrence: saved_occurrence,
        evidence: evaluation.evidence.clone(),
        discarded: None,
        events: evaluation.transition.events(),
        reason,
    }))
}

fn prepare_current(
    context: (&RepositoryState, &InstrumentId, &SignalSnapshot),
    evaluation: &application::SignalEvaluation,
    record: &EstimateSubmissionRecord<AiRunId, ReasoningEffort>,
    head: Option<&CalibrationLedgerHead>,
) -> Result<(Option<StoredSignal>, Option<BuySignalRecord>), SystemPortError> {
    let (state, instrument, expected) = context;
    let Some(signal) = evaluation.decision.clone().into_signal() else {
        if evaluation.occurrence.is_some() || evaluation.evidence.is_some() {
            return Err(invalid());
        }
        return Ok((None, None));
    };
    let signal_head = match &signal {
        RaisedSignal::Buy(value) => value.facts().calibration_head(),
        RaisedSignal::Sell(value) => value.facts().calibration_head(),
    };
    if Some(signal_head) != head
        || signal_instrument(&signal) != instrument
        || signal_submission(&signal) != record.submission()
    {
        return Err(invalid());
    }
    let (occurrence, new_occurrence) = add_occurrence(
        &signal,
        expected.current.as_ref(),
        evaluation.occurrence.as_ref(),
        &evaluation.transition,
        &state.history,
    )?;
    let evidence = evaluation.evidence.as_ref().ok_or_else(invalid)?;
    let mut masters = state.masters.clone();
    if !masters
        .iter()
        .any(|master| master.row().instrument_id == evidence.master.row().instrument_id)
    {
        masters.push(evidence.master.clone());
    }
    let historical = restore_signal(&evidence.signal, &masters).map_err(|_error| invalid())?;
    if historical != HistoricalSignal::from_signal(&signal) {
        return Err(invalid());
    }
    let expected_occurrence = evaluation
        .occurrence
        .as_ref()
        .map(HistoricalSignalOccurrence::from_record);
    let stored_occurrence = evidence
        .occurrence
        .as_ref()
        .map(|value| restore_occurrence(value, &masters))
        .transpose()
        .map_err(|_error| invalid())?;
    if expected_occurrence != stored_occurrence {
        return Err(invalid());
    }
    Ok((
        Some(StoredSignal {
            signal: HistoricalSignal::from_signal(&signal),
            submission: record.task_id().clone(),
            occurrence,
        }),
        new_occurrence,
    ))
}

fn prepare_discard(
    expected: &SignalSnapshot,
    transition: &SignalTransition,
) -> Result<Change, SystemPortError> {
    let Some(current) = &expected.current else {
        return Err(invalid());
    };
    let decision = SignalDecision::None(NoSignalReason::Expired);
    if signal_transition_from_history(Some(&current.signal), &decision)
        .map_err(|_error| invalid())?
        != *transition
    {
        return Err(invalid());
    }
    Ok(Change {
        current: None,
        occurrence: None,
        evidence: None,
        discarded: Some(current.submission.clone()),
        events: transition.events(),
        reason: None,
    })
}

fn apply_change(
    state: &mut RepositoryState,
    instrument: &InstrumentId,
    revision: u64,
    change: Change,
) -> SignalCommitOutcome {
    if let Some(evidence) = &change.evidence
        && !state
            .masters
            .iter()
            .any(|master| master.row().instrument_id == evidence.master.row().instrument_id)
    {
        state.masters.push(evidence.master.clone());
    }
    if let Some(record) = change.occurrence {
        state.history.push(record);
    }
    if let Some(entry) = state
        .instruments
        .iter_mut()
        .find(|entry| entry.instrument.as_ref() == Some(instrument))
    {
        entry.revision = revision;
        entry.current = change.current;
        entry.evidence = change.evidence;
        if let Some(task) = change.discarded {
            entry.discarded.push(task);
        }
    } else {
        state.instruments.push(InstrumentState {
            instrument: Some(instrument.clone()),
            revision,
            current: change.current,
            evidence: change.evidence,
            discarded: change.discarded.into_iter().collect(),
        });
    }
    SignalCommitOutcome::Applied(SignalCommit {
        events: change.events,
        no_signal_reason: change.reason,
    })
}

fn commit_signal(
    state: &mut RepositoryState,
    instrument: &InstrumentId,
    expected: &SignalSnapshot,
    mutation: &SignalMutation,
) -> Result<SignalCommitOutcome, SystemPortError> {
    if let Some(error) = state.fail_next_commit.take() {
        return Err(error);
    }
    if snapshot(state, instrument)? != *expected {
        return Ok(SignalCommitOutcome::Conflict);
    }
    if let SignalMutation::Evaluate(evaluation) = mutation
        && expected.submission.as_ref() == Some(&evaluation.submission)
        && state
            .instruments
            .iter()
            .find(|entry| entry.instrument.as_ref() == Some(instrument))
            .is_some_and(|entry| entry.discarded.contains(&evaluation.submission))
    {
        return Ok(SignalCommitOutcome::SubmissionDiscarded);
    }
    if let SignalMutation::Discard(transition) = mutation
        && expected.current.is_none()
        && **transition == SignalTransition::Unchanged
    {
        return Ok(SignalCommitOutcome::Applied(SignalCommit {
            events: vec![],
            no_signal_reason: None,
        }));
    }
    let revision = expected
        .revision
        .as_u64()
        .checked_add(1)
        .ok_or_else(invalid)?;
    let change = match mutation {
        SignalMutation::Discard(transition) => Some(prepare_discard(expected, transition)?),
        SignalMutation::Evaluate(evaluation) => {
            prepare_evaluation(state, instrument, expected, evaluation)?
        }
    };
    match change {
        Some(value) => Ok(apply_change(state, instrument, revision, value)),
        None => Ok(SignalCommitOutcome::Conflict),
    }
}

fn validate_submission_records(
    state: &RepositoryState,
    previous: &EstimateAssignments<AiRunId>,
    next: &EstimateAssignments<AiRunId>,
    records: &[EstimateSubmissionRecord<AiRunId, ReasoningEffort>],
) -> Result<(), SystemPortError> {
    let Some(first) = records.first() else {
        return if previous == next {
            Ok(())
        } else {
            Err(invalid())
        };
    };
    let ids: Vec<_> = records
        .iter()
        .map(|record| record.task_id().as_estimate_id().clone())
        .collect();
    if records.iter().any(|record| {
        record.run() != first.run()
            || state
                .records
                .iter()
                .any(|old| old.task_id() == record.task_id())
    }) || previous
        .consume(first.run(), &ids)
        .map_err(|_error| invalid())?
        != *next
    {
        return Err(invalid());
    }
    for record in records {
        let issued = previous
            .lookup(record.task_id().as_estimate_id())
            .map_err(|_error| invalid())?;
        if issued.run() != record.run()
            || issued.task().task() != record.submission().task_key().task()
        {
            return Err(invalid());
        }
    }
    Ok(())
}

impl SignalRepository for MemorySignalEvaluationRepository {
    fn snapshot(
        &self,
        instrument: &InstrumentId,
    ) -> impl Future<Output = Result<SignalSnapshot, SystemPortError>> + Send {
        std::future::poll_fn(move |_context| {
            std::task::Poll::Ready(snapshot(&self.state.write(), instrument))
        })
    }
    fn current(
        &self,
        instrument: &InstrumentId,
    ) -> impl Future<Output = Result<Vec<HistoricalSignal>, SystemPortError>> + Send {
        std::future::poll_fn(move |_context| {
            let result = snapshot(&self.state.write(), instrument).map(|value| {
                value
                    .current
                    .into_iter()
                    .map(|entry| entry.signal)
                    .collect()
            });
            std::task::Poll::Ready(result)
        })
    }
    fn occurrences(
        &self,
        instrument: &InstrumentId,
        identity: &ResolvedOrderInstrument,
    ) -> impl Future<Output = Result<Vec<HistoricalSignalOccurrence>, SystemPortError>> + Send {
        std::future::poll_fn(move |_context| {
            let state = self.state.write();
            let result = state
                .history
                .iter()
                .filter(|value| {
                    value.record.instrument_id() == instrument
                        || value.record.instrument() == identity
                })
                .map(|value| checked_occurrence(value, &state.masters))
                .collect();
            std::task::Poll::Ready(result)
        })
    }
    fn sources(
        &self,
        task: &IssuedEstimateTaskId,
    ) -> impl Future<Output = Result<SignalSourceFacts, SystemPortError>> + Send {
        std::future::poll_fn(move |_context| {
            std::task::Poll::Ready(
                self.state
                    .write()
                    .sources
                    .iter()
                    .find(|(id, _)| id == task)
                    .map(|(_, facts)| facts.clone())
                    .ok_or_else(invalid),
            )
        })
    }
    fn commit(
        &self,
        instrument: &InstrumentId,
        expected: &SignalSnapshot,
        mutation: &SignalMutation,
    ) -> impl Future<Output = Result<SignalCommitOutcome, SystemPortError>> + Send {
        std::future::poll_fn(move |_context| {
            std::task::Poll::Ready(commit_signal(
                &mut self.state.write(),
                instrument,
                expected,
                mutation,
            ))
        })
    }
}
fn invalidate_signal(
    state: &mut RepositoryState,
    instrument: &InstrumentId,
    expected: &SignalSnapshot,
) -> Result<SignalInvalidationOutcome, SystemPortError> {
    if let Some(error) = state.fail_next_commit.take() {
        return Err(error);
    }
    if snapshot(state, instrument)? != *expected {
        return Ok(SignalInvalidationOutcome::Conflict);
    }
    if expected.submission.as_ref().is_some_and(|submission| {
        state
            .instruments
            .iter()
            .find(|entry| entry.instrument.as_ref() == Some(instrument))
            .is_some_and(|entry| entry.discarded.contains(submission))
    }) {
        return Ok(SignalInvalidationOutcome::SubmissionDiscarded);
    }
    if expected.current.is_none() {
        return Ok(SignalInvalidationOutcome::Applied(Vec::new()));
    }
    let revision = expected
        .revision
        .as_u64()
        .checked_add(1)
        .ok_or_else(invalid)?;
    let transition = SignalTransition::Cleared(instrument.clone());
    let mut change = prepare_discard(expected, &transition)?;
    change.discarded = None;
    let SignalCommitOutcome::Applied(commit) = apply_change(state, instrument, revision, change)
    else {
        return Err(invalid());
    };
    Ok(SignalInvalidationOutcome::Applied(commit.events))
}

impl SignalInvalidationRepository for MemorySignalEvaluationRepository {
    fn invalidate(
        &self,
        instrument: &InstrumentId,
        expected: &SignalSnapshot,
    ) -> impl Future<Output = Result<SignalInvalidationOutcome, SystemPortError>> + Send {
        std::future::poll_fn(move |_context| {
            std::task::Poll::Ready(invalidate_signal(
                &mut self.state.write(),
                instrument,
                expected,
            ))
        })
    }
}

impl EstimateSubmissionRepository for MemorySignalEvaluationRepository {
    fn submission_records(
        &self,
        instrument: &InstrumentId,
    ) -> impl Future<
        Output = Result<Vec<EstimateSubmissionRecord<AiRunId, ReasoningEffort>>, SystemPortError>,
    > + Send {
        std::future::poll_fn(move |_context| {
            std::task::Poll::Ready(Ok(self
                .state
                .write()
                .records
                .iter()
                .filter(|record| {
                    record
                        .submission()
                        .task_key()
                        .task()
                        .volatility()
                        .instrument()
                        == instrument
                })
                .cloned()
                .collect()))
        })
    }
    fn assignments(
        &self,
    ) -> impl Future<Output = Result<EstimateAssignments<AiRunId>, SystemPortError>> + Send {
        std::future::poll_fn(move |_context| {
            std::task::Poll::Ready(Ok(self.state.write().assignments.clone()))
        })
    }
    fn commit_submission(
        &self,
        previous: &EstimateAssignments<AiRunId>,
        next: &EstimateAssignments<AiRunId>,
        records: &[EstimateSubmissionRecord<AiRunId, ReasoningEffort>],
    ) -> impl Future<Output = Result<EstimateCommitOutcome, SystemPortError>> + Send {
        std::future::poll_fn(move |_context| {
            let mut state = self.state.write();
            if &state.assignments != previous {
                return std::task::Poll::Ready(Ok(EstimateCommitOutcome::Conflict));
            }
            if let Err(error) = validate_submission_records(&state, previous, next, records) {
                return std::task::Poll::Ready(Err(error));
            }
            state.assignments = next.clone();
            state.records.extend_from_slice(records);
            std::task::Poll::Ready(Ok(EstimateCommitOutcome::Committed))
        })
    }
}
