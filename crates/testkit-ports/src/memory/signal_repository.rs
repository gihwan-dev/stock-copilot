use crate::script::State;
use application::{
    AiRunId, EstimateCommitOutcome, EstimateSubmissionRepository, ReasoningEffort, SignalCommit,
    SignalCommitOutcome, SignalInvalidationOutcome, SignalInvalidationRepository, SignalMutation,
    SignalRepository, SignalRevision, SignalSnapshot, SignalSourceFacts, StoredSignal,
    SystemFailure, SystemPort, SystemPortError,
};
use domain::{
    BuySignalRecord, CalibrationLedgerHead, CurrentBuySignal, EstimateAssignments,
    EstimateSubmissionRecord, InstrumentId, IssuedEstimateTaskId, NoSignalReason, RaisedSignal,
    ResolvedOrderInstrument, SignalDecision, SignalTransition, signal_transition,
};

#[derive(Debug, Default)]
struct InstrumentState {
    instrument: Option<InstrumentId>,
    revision: u64,
    current: Option<StoredSignal>,
    discarded: Vec<IssuedEstimateTaskId>,
}

#[derive(Debug, Default)]
struct RepositoryState {
    assignments: EstimateAssignments<AiRunId>,
    records: Vec<EstimateSubmissionRecord<AiRunId, ReasoningEffort>>,
    instruments: Vec<InstrumentState>,
    history: Vec<BuySignalRecord>,
    sources: Vec<(IssuedEstimateTaskId, SignalSourceFacts)>,
    heads: Vec<CalibrationLedgerHead>,
    fail_next_commit: Option<SystemPortError>,
    drafts: Vec<StoredDraft>,
    fail_next_draft: Option<SystemPortError>,
    draft_conflicts: usize,
    draft_attempts: usize,
    snapshot_reads: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DraftState {
    Current,
    Superseded,
}

#[derive(Debug)]
struct StoredDraft {
    instrument: InstrumentId,
    snapshot: SignalSnapshot,
    proposal: domain::Proposal<domain::Draft>,
    state: DraftState,
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
    pub fn fail_next_draft(&self, error: SystemPortError) {
        self.state.write().fail_next_draft = Some(error);
    }
    pub fn conflict_next_drafts(&self, count: usize) {
        self.state.write().draft_conflicts = count;
    }
    #[must_use]
    pub fn draft_attempts(&self) -> usize {
        self.state.write().draft_attempts
    }
    #[must_use]
    pub fn snapshot_reads(&self) -> usize {
        self.state.write().snapshot_reads
    }
}

impl application::ProposalRepository for MemorySignalEvaluationRepository {
    fn store_draft(
        &self,
        _proposal: &domain::Proposal<domain::Draft>,
    ) -> impl Future<Output = Result<application::WriteCompleted, SystemPortError>> + Send {
        std::future::ready(Err(invalid()))
    }

    fn draft(
        &self,
        proposal: &domain::ProposalId,
    ) -> impl Future<Output = Result<Option<domain::Proposal<domain::Draft>>, SystemPortError>> + Send
    {
        std::future::poll_fn(move |_context| {
            std::task::Poll::Ready(Ok(read_draft(&self.state.write(), proposal)))
        })
    }

    fn current_draft(
        &self,
        instrument: &InstrumentId,
        expected: &SignalSnapshot,
        check: application::DraftCheck<'_>,
    ) -> impl Future<Output = Result<application::CurrentDraftOutcome, SystemPortError>> + Send
    {
        std::future::poll_fn(move |_context| {
            std::task::Poll::Ready(current_draft_guarded(
                &mut self.state.write(),
                instrument,
                expected,
                check,
            ))
        })
    }

    fn store_current_draft(
        &self,
        instrument: &InstrumentId,
        expected: &SignalSnapshot,
        proposal: &domain::Proposal<domain::Draft>,
        check: application::DraftCheck<'_>,
    ) -> impl Future<Output = Result<application::ConditionalDraftOutcome, SystemPortError>> + Send
    {
        std::future::poll_fn(move |_context| {
            std::task::Poll::Ready(store_draft_guarded(
                &mut self.state.write(),
                instrument,
                expected,
                proposal,
                check,
            ))
        })
    }
}

fn read_draft(
    state: &RepositoryState,
    proposal: &domain::ProposalId,
) -> Option<domain::Proposal<domain::Draft>> {
    let stored = state
        .drafts
        .iter()
        .rev()
        .find(|value| value.state == DraftState::Current && value.proposal.id() == proposal)?;
    let current = snapshot(state, &stored.instrument);
    let signal = stored.snapshot.current.as_ref()?;
    let head = match &signal.signal {
        RaisedSignal::Buy(buy_signal) => buy_signal.facts().calibration_head(),
        RaisedSignal::Sell(sell_signal) => sell_signal.facts().calibration_head(),
    };
    if current != stored.snapshot
        || state.heads.iter().find(|item| item.key() == head.key()) != Some(head)
    {
        return None;
    }
    Some(stored.proposal.clone())
}

fn store_draft_guarded(
    state: &mut RepositoryState,
    instrument: &InstrumentId,
    expected: &SignalSnapshot,
    proposal: &domain::Proposal<domain::Draft>,
    check: application::DraftCheck<'_>,
) -> Result<application::ConditionalDraftOutcome, SystemPortError> {
    state.draft_attempts = state.draft_attempts.saturating_add(1);
    if let Some(error) = state.fail_next_draft.take() {
        return Err(error);
    }
    if state.draft_conflicts > 0 {
        state.draft_conflicts = state.draft_conflicts.saturating_sub(1);
        return Ok(application::ConditionalDraftOutcome::Conflict);
    }
    match current_draft_guarded(state, instrument, expected, check)? {
        application::CurrentDraftOutcome::Absent => {}
        application::CurrentDraftOutcome::Existing(id) => {
            return Ok(application::ConditionalDraftOutcome::Existing(id));
        }
        application::CurrentDraftOutcome::Conflict => {
            return Ok(application::ConditionalDraftOutcome::Conflict);
        }
        application::CurrentDraftOutcome::Invalid(error) => {
            return Ok(application::ConditionalDraftOutcome::Invalid(error));
        }
    }
    let current = expected.current.as_ref().ok_or_else(invalid)?;
    let expected_side = match &current.signal {
        RaisedSignal::Buy(_) => domain::OrderSide::Buy,
        RaisedSignal::Sell(_) => domain::OrderSide::Sell,
    };
    if proposal.intent().side() != expected_side {
        return Err(invalid());
    }
    if let Err(error) = proposal.check_current(check.at, check.observation) {
        return Ok(application::ConditionalDraftOutcome::Invalid(error));
    }
    if state
        .drafts
        .iter()
        .any(|value| value.proposal.id() == proposal.id())
    {
        return Err(invalid());
    }
    for value in &mut state.drafts {
        if value.instrument == *instrument && value.state == DraftState::Current {
            value.state = DraftState::Superseded;
        }
    }
    state.drafts.push(StoredDraft {
        instrument: instrument.clone(),
        snapshot: expected.clone(),
        proposal: proposal.clone(),
        state: DraftState::Current,
    });
    Ok(application::ConditionalDraftOutcome::Stored)
}

fn current_draft_guarded(
    state: &mut RepositoryState,
    instrument: &InstrumentId,
    expected: &SignalSnapshot,
    check: application::DraftCheck<'_>,
) -> Result<application::CurrentDraftOutcome, SystemPortError> {
    if snapshot(state, instrument) != *expected {
        return Ok(application::CurrentDraftOutcome::Conflict);
    }
    let current = expected.current.as_ref().ok_or_else(invalid)?;
    let (head, task) = match &current.signal {
        RaisedSignal::Buy(signal) => (signal.facts().calibration_head(), signal.facts().task()),
        RaisedSignal::Sell(signal) => (signal.facts().calibration_head(), signal.facts().task()),
    };
    if state.heads.iter().find(|value| value.key() == head.key()) != Some(head) {
        return Ok(application::CurrentDraftOutcome::Conflict);
    }
    if check.observation.validate_for_task(task, check.at).is_err() {
        return Ok(application::CurrentDraftOutcome::Invalid(
            domain::ProposalExecutionError::ObservationInvalid,
        ));
    }
    let Some(old) = state
        .drafts
        .iter_mut()
        .find(|value| value.instrument == *instrument && value.state == DraftState::Current)
    else {
        return Ok(application::CurrentDraftOutcome::Absent);
    };
    if old.snapshot != *expected {
        old.state = DraftState::Superseded;
        return Ok(application::CurrentDraftOutcome::Absent);
    }
    match old.proposal.check_current(check.at, check.observation) {
        Ok(()) => Ok(application::CurrentDraftOutcome::Existing(
            old.proposal.id().clone(),
        )),
        Err(
            domain::ProposalExecutionError::Expired
            | domain::ProposalExecutionError::PriceOutsideRange,
        ) => {
            old.state = DraftState::Superseded;
            Ok(application::CurrentDraftOutcome::Absent)
        }
        Err(
            error @ (domain::ProposalExecutionError::ClockRollback
            | domain::ProposalExecutionError::ObservationInvalid),
        ) => Ok(application::CurrentDraftOutcome::Invalid(error)),
    }
}

fn invalid() -> SystemPortError {
    SystemPortError::new(SystemPort::Repository, SystemFailure::InvalidData)
}
fn snapshot(state: &RepositoryState, instrument: &InstrumentId) -> SignalSnapshot {
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
    SignalSnapshot {
        revision: SignalRevision::new(entry.map_or(0, |value| value.revision)),
        submission: submission.map(|value| value.task_id().clone()),
        current: entry.and_then(|value| value.current.clone()),
    }
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
    history: &[BuySignalRecord],
) -> Result<(Option<BuySignalRecord>, Option<BuySignalRecord>), SystemPortError> {
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
                    if history.iter().any(|old| old.id() == record.id()) {
                        return Err(invalid());
                    }
                    if record.originating_task().task() != buy.facts().task() {
                        return Err(invalid());
                    }
                    CurrentBuySignal::try_new(buy.clone(), record.clone())
                        .map_err(|_error| invalid())?;
                    return Ok((Some(record.clone()), Some(record.clone())));
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
            CurrentBuySignal::try_new(buy.clone(), occurrence.clone())
                .map_err(|_error| invalid())?;
            Ok((Some(occurrence), None))
        }
    }
}
#[derive(Debug)]
struct Change {
    current: Option<StoredSignal>,
    occurrence: Option<BuySignalRecord>,
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
    if signal_transition(previous, &evaluation.decision).map_err(|_error| invalid())?
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
    Ok(Some(Change {
        current,
        occurrence,
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
        if evaluation.occurrence.is_some() {
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
    Ok((
        Some(StoredSignal {
            signal,
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
    if signal_transition(Some(&current.signal), &decision).map_err(|_error| invalid())?
        != *transition
    {
        return Err(invalid());
    }
    Ok(Change {
        current: None,
        occurrence: None,
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
        if let Some(task) = change.discarded {
            entry.discarded.push(task);
        }
    } else {
        state.instruments.push(InstrumentState {
            instrument: Some(instrument.clone()),
            revision,
            current: change.current,
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
    if snapshot(state, instrument) != *expected {
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
            let mut state = self.state.write();
            state.snapshot_reads = state.snapshot_reads.saturating_add(1);
            std::task::Poll::Ready(Ok(snapshot(&state, instrument)))
        })
    }
    fn current(
        &self,
        instrument: &InstrumentId,
    ) -> impl Future<Output = Result<Vec<RaisedSignal>, SystemPortError>> + Send {
        std::future::poll_fn(move |_context| {
            let current = snapshot(&self.state.write(), instrument).current;
            std::task::Poll::Ready(Ok(current.into_iter().map(|entry| entry.signal).collect()))
        })
    }
    fn occurrences(
        &self,
        instrument: &InstrumentId,
        identity: &ResolvedOrderInstrument,
    ) -> impl Future<Output = Result<Vec<BuySignalRecord>, SystemPortError>> + Send {
        std::future::poll_fn(move |_context| {
            std::task::Poll::Ready(Ok(self
                .state
                .write()
                .history
                .iter()
                .filter(|value| {
                    value.instrument_id() == instrument || value.instrument() == identity
                })
                .cloned()
                .collect()))
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
    if snapshot(state, instrument) != *expected {
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
