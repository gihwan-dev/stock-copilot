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

#[derive(Debug, Clone)]
struct StoredCurrentEntry {
    submission: IssuedEstimateTaskId,
    occurrence_id: Option<domain::BuySignalOccurrenceId>,
    evidence: SignalCommitEvidence,
}

#[derive(Debug, Default)]
struct InstrumentState {
    instrument: Option<InstrumentId>,
    revision: u64,
    current: Option<StoredCurrentEntry>,
    discarded: Vec<IssuedEstimateTaskId>,
}

#[derive(Debug, Clone)]
struct SignalHistoryEntry {
    instrument_id: InstrumentId,
    instrument: ResolvedOrderInstrument,
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
    drafts: Vec<StoredDraft>,
    selected_account: Option<domain::AccountId>,
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
    evidence: domain::DraftRestoreEvidence,
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
    pub fn save_master(&self, master: domain::MasterInstrument) {
        let mut state = self.state.write();
        state
            .masters
            .retain(|old| old.row().instrument_id != master.row().instrument_id);
        state.masters.push(master);
    }
    pub fn fail_next_draft(&self, error: SystemPortError) {
        self.state.write().fail_next_draft = Some(error);
    }
    pub fn select_account(&self, account: domain::AccountId) {
        self.state.write().selected_account = Some(account);
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
        _evidence: &domain::DraftRestoreEvidence,
        _master: &domain::MasterInstrument,
    ) -> impl Future<Output = Result<application::WriteCompleted, SystemPortError>> + Send {
        std::future::ready(Err(invalid()))
    }

    fn draft(
        &self,
        proposal: &domain::ProposalId,
        master: &domain::MasterInstrument,
    ) -> impl Future<Output = Result<Option<domain::Proposal<domain::Draft>>, SystemPortError>> + Send
    {
        std::future::poll_fn(move |_context| {
            std::task::Poll::Ready(read_draft(&self.state.write(), proposal, master))
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
        candidate: application::DraftStoreCandidate<'_>,
        check: application::DraftCheck<'_>,
    ) -> impl Future<Output = Result<application::ConditionalDraftOutcome, SystemPortError>> + Send
    {
        std::future::poll_fn(move |_context| {
            std::task::Poll::Ready(store_draft_guarded(
                &mut self.state.write(),
                instrument,
                expected,
                candidate,
                check,
            ))
        })
    }
}

fn stored_master_for<'a>(
    masters: &'a [domain::MasterInstrument],
    instrument: &InstrumentId,
) -> Result<&'a domain::MasterInstrument, SystemPortError> {
    masters
        .iter()
        .find(|master| &master.row().instrument_id == instrument)
        .ok_or_else(integrity)
}

fn verify_live_draft_masters(
    masters: &[domain::MasterInstrument],
    instrument: &InstrumentId,
    caller: &domain::MasterInstrument,
    side: domain::OrderSide,
) -> Result<(), SystemPortError> {
    let stored = stored_master_for(masters, instrument)?;
    if caller.row().instrument_id != *instrument {
        return Err(integrity());
    }
    let stored_quote = stored.quote_instrument().map_err(|_error| integrity())?;
    let caller_quote = caller.quote_instrument().map_err(|_error| integrity())?;
    if stored_quote != caller_quote {
        return Err(integrity());
    }
    stored
        .order_gate_facts()
        .and_then(|facts| facts.instrument_for(side))
        .map_err(|_error| integrity())?;
    caller
        .order_gate_facts()
        .and_then(|facts| facts.instrument_for(side))
        .map_err(|_error| integrity())?;
    Ok(())
}

fn restore_draft_with_masters(
    evidence: &domain::DraftRestoreEvidence,
    masters: &[domain::MasterInstrument],
    instrument: &InstrumentId,
    caller: &domain::MasterInstrument,
) -> Result<domain::Proposal<domain::Draft>, SystemPortError> {
    let stored = stored_master_for(masters, instrument)?;
    evidence.restore(stored).map_err(|_error| integrity())?;
    evidence.restore(caller).map_err(|_error| integrity())
}

fn read_draft(
    state: &RepositoryState,
    proposal: &domain::ProposalId,
    master: &domain::MasterInstrument,
) -> Result<Option<domain::Proposal<domain::Draft>>, SystemPortError> {
    let Some(stored) = state
        .drafts
        .iter()
        .rev()
        .find(|value| value.state == DraftState::Current && value.evidence.id == *proposal)
    else {
        return Ok(None);
    };
    if state.selected_account.as_ref() != Some(&stored.evidence.account) {
        return Ok(None);
    }
    let current = snapshot(state, &stored.instrument)?;
    let Some(signal) = stored.snapshot.current.as_ref() else {
        return Ok(None);
    };
    let head = signal.signal.calibration_head();
    if current != stored.snapshot
        || state.heads.iter().find(|item| item.key() == head.key()) != Some(head)
    {
        return Ok(None);
    }
    restore_draft_with_masters(&stored.evidence, &state.masters, &stored.instrument, master)
        .map(Some)
}

fn check_existing_for_store(
    state: &RepositoryState,
    instrument: &InstrumentId,
    expected: &SignalSnapshot,
    check: application::DraftCheck<'_>,
) -> Result<Option<application::ConditionalDraftOutcome>, SystemPortError> {
    let Some(old) = state
        .drafts
        .iter()
        .find(|value| value.instrument == *instrument && value.state == DraftState::Current)
    else {
        return Ok(None);
    };
    if old.snapshot != *expected {
        return Ok(None);
    }
    let old_proposal =
        restore_draft_with_masters(&old.evidence, &state.masters, instrument, check.master)?;
    match old_proposal.check_current(check.at, check.observation) {
        Ok(()) if old_proposal.intent().account_id() == check.account => Ok(Some(
            application::ConditionalDraftOutcome::Existing(old_proposal.id().clone()),
        )),
        Ok(())
        | Err(
            domain::ProposalExecutionError::Expired
            | domain::ProposalExecutionError::PriceOutsideRange,
        ) => Ok(None),
        Err(
            error @ (domain::ProposalExecutionError::ClockRollback
            | domain::ProposalExecutionError::ObservationInvalid),
        ) => Ok(Some(application::ConditionalDraftOutcome::Invalid(error))),
    }
}

fn verify_draft_evidence(
    state: &RepositoryState,
    instrument: &InstrumentId,
    candidate: application::DraftStoreCandidate<'_>,
    master: &domain::MasterInstrument,
) -> Result<(), SystemPortError> {
    if candidate.evidence.estimate.evidence.master.instrument_id != *instrument {
        return Err(integrity());
    }
    let stored = stored_master_for(&state.masters, instrument)?;
    let saved_estimate = state
        .instruments
        .iter()
        .find(|entry| entry.instrument.as_ref() == Some(instrument))
        .and_then(|entry| entry.current.as_ref())
        .map(|entry| &entry.evidence.signal.evidence.estimate)
        .ok_or_else(integrity)?;
    candidate
        .evidence
        .estimate
        .verify_same_as(saved_estimate)
        .map_err(|_error| integrity())?;
    candidate
        .evidence
        .verify_matches_proposal(candidate.proposal, stored)
        .map_err(|_error| integrity())?;
    candidate
        .evidence
        .verify_matches_proposal(candidate.proposal, master)
        .map_err(|_error| integrity())
}

fn validate_store_draft(
    state: &RepositoryState,
    instrument: &InstrumentId,
    expected: &SignalSnapshot,
    candidate: application::DraftStoreCandidate<'_>,
    check: application::DraftCheck<'_>,
) -> Result<Option<application::ConditionalDraftOutcome>, SystemPortError> {
    let current = expected.current.as_ref().ok_or_else(invalid)?;
    let expected_side = current.signal.side();
    let head = current.signal.calibration_head();
    if candidate.proposal.intent().side() != expected_side
        || candidate.proposal.intent().account_id() != check.account
    {
        return Err(invalid());
    }
    if snapshot(state, instrument)? != *expected
        || state.heads.iter().find(|value| value.key() == head.key()) != Some(head)
    {
        return Ok(Some(application::ConditionalDraftOutcome::Conflict));
    }
    if state.selected_account.as_ref() != Some(check.account) {
        return Ok(Some(application::ConditionalDraftOutcome::AccountChanged));
    }
    if current
        .signal
        .validate_observation(check.observation, check.at)
        .is_err()
    {
        return Ok(Some(application::ConditionalDraftOutcome::Invalid(
            domain::ProposalExecutionError::ObservationInvalid,
        )));
    }
    if let Err(error) = candidate
        .proposal
        .check_current(check.at, check.observation)
    {
        return Ok(Some(application::ConditionalDraftOutcome::Invalid(error)));
    }
    if state
        .drafts
        .iter()
        .any(|value| value.evidence.id == *candidate.proposal.id())
    {
        return Err(invalid());
    }
    verify_draft_evidence(state, instrument, candidate, check.master)?;
    check_existing_for_store(state, instrument, expected, check)
}

fn store_draft_guarded(
    state: &mut RepositoryState,
    instrument: &InstrumentId,
    expected: &SignalSnapshot,
    candidate: application::DraftStoreCandidate<'_>,
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
    if let Some(outcome) = validate_store_draft(state, instrument, expected, candidate, check)? {
        return Ok(outcome);
    }
    for value in &mut state.drafts {
        if value.instrument == *instrument && value.state == DraftState::Current {
            value.state = DraftState::Superseded;
        }
    }
    state.drafts.push(StoredDraft {
        instrument: instrument.clone(),
        snapshot: expected.clone(),
        evidence: candidate.evidence.clone(),
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
    if snapshot(state, instrument)? != *expected {
        return Ok(application::CurrentDraftOutcome::Conflict);
    }
    let current = expected.current.as_ref().ok_or_else(invalid)?;
    let head = current.signal.calibration_head();
    if state.heads.iter().find(|value| value.key() == head.key()) != Some(head) {
        return Ok(application::CurrentDraftOutcome::Conflict);
    }
    if state.selected_account.as_ref() != Some(check.account) {
        return Ok(application::CurrentDraftOutcome::AccountChanged);
    }
    if current
        .signal
        .validate_observation(check.observation, check.at)
        .is_err()
    {
        return Ok(application::CurrentDraftOutcome::Invalid(
            domain::ProposalExecutionError::ObservationInvalid,
        ));
    }
    verify_live_draft_masters(
        &state.masters,
        instrument,
        check.master,
        current.signal.side(),
    )?;
    inspect_existing_draft(state, instrument, expected, check)
}

fn inspect_existing_draft(
    state: &mut RepositoryState,
    instrument: &InstrumentId,
    expected: &SignalSnapshot,
    check: application::DraftCheck<'_>,
) -> Result<application::CurrentDraftOutcome, SystemPortError> {
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
    let old_proposal =
        restore_draft_with_masters(&old.evidence, &state.masters, instrument, check.master)?;
    match old_proposal.check_current(check.at, check.observation) {
        Ok(()) if old_proposal.intent().account_id() == check.account => Ok(
            application::CurrentDraftOutcome::Existing(old_proposal.id().clone()),
        ),
        Ok(())
        | Err(
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
    let Some(saved) = &entry.current else {
        return Ok(None);
    };
    let signal = restore_signal(&saved.evidence.signal, &state.masters)?;
    let occurrence = match &saved.occurrence_id {
        Some(id) => {
            let history = state
                .history
                .iter()
                .find(|value| value.evidence.id == *id)
                .ok_or_else(integrity)?;
            Some(restore_occurrence(&history.evidence, &state.masters)?)
        }
        None => None,
    };
    Ok(Some(StoredSignal {
        signal,
        submission: saved.submission.clone(),
        occurrence,
    }))
}
fn rebuild_signal(
    evidence: &domain::StoredSignalEvidence,
    masters: &[domain::MasterInstrument],
) -> Result<domain::StoredSignalEvidence, SystemPortError> {
    let mut reconstructed = evidence.clone();
    if let Some(portfolio) = &reconstructed.evidence.assessment.portfolio {
        reconstructed.evidence.assessment.portfolio = Some(
            domain::HistoricalPortfolio::from_saved_facts(portfolio.facts().clone(), masters)
                .map_err(|_error| integrity())?,
        );
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
fn merge_evidence_masters(
    existing: &[domain::MasterInstrument],
    evidence: &SignalCommitEvidence,
) -> Result<Vec<domain::MasterInstrument>, SystemPortError> {
    let mut incoming = vec![evidence.master.clone()];
    for extra in &evidence.holding_masters {
        if !incoming.iter().any(|seen| {
            seen == extra
                && seen.row().observed_at.offset_seconds()
                    == extra.row().observed_at.offset_seconds()
        }) {
            incoming.push(extra.clone());
        }
    }
    let validated = domain::MasterInstrumentFacts::try_new(incoming).map_err(|_error| invalid())?;
    let mut merged = existing.to_vec();
    for candidate in validated.instruments() {
        if !merged
            .iter()
            .any(|old| old.row().instrument_id == candidate.row().instrument_id)
        {
            merged.push(candidate.clone());
        }
    }
    let non_delisted: Vec<_> = merged
        .iter()
        .filter(|master| match master.row().listing_status {
            domain::ListingStatus::Scheduled | domain::ListingStatus::Active => true,
            domain::ListingStatus::Delisted => false,
        })
        .cloned()
        .collect();
    domain::MasterInstrumentFacts::try_new(non_delisted).map_err(|_error| invalid())?;
    Ok(merged)
}
fn add_occurrence(
    signal: &RaisedSignal,
    prior: Option<&StoredSignal>,
    supplied: Option<&BuySignalRecord>,
    transition: &SignalTransition,
    history: &[SignalHistoryEntry],
) -> Result<
    (
        Option<domain::BuySignalOccurrenceId>,
        Option<BuySignalRecord>,
    ),
    SystemPortError,
> {
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
                    if history.iter().any(|old| old.evidence.id == *record.id()) {
                        return Err(invalid());
                    }
                    if record.originating_task().task() != buy.facts().task() {
                        return Err(invalid());
                    }
                    CurrentBuySignal::try_new(buy.clone(), record.clone())
                        .map_err(|_error| invalid())?;
                    return Ok((Some(record.id().clone()), Some(record.clone())));
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
            Ok((Some(occurrence.id().clone()), None))
        }
    }
}
#[derive(Debug)]
struct Change {
    current: Option<StoredCurrentEntry>,
    occurrence: Option<SignalHistoryEntry>,
    masters: Option<Vec<domain::MasterInstrument>>,
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
    let (current, occurrence, masters) =
        prepare_current((state, instrument, expected), evaluation, record, head)?;
    let saved_occurrence = match occurrence {
        Some(history_record) => {
            let evidence = evaluation.evidence.as_ref().ok_or_else(invalid)?;
            Some(SignalHistoryEntry {
                instrument_id: history_record.instrument_id().clone(),
                instrument: history_record.instrument().clone(),
                evidence: evidence.occurrence.clone().ok_or_else(invalid)?,
            })
        }
        None => None,
    };
    Ok(Some(Change {
        current,
        occurrence: saved_occurrence,
        masters,
        discarded: None,
        events: evaluation.transition.events(),
        reason,
    }))
}

type PreparedCurrent = (
    Option<StoredCurrentEntry>,
    Option<BuySignalRecord>,
    Option<Vec<domain::MasterInstrument>>,
);

fn verify_commit_evidence(
    existing: &[domain::MasterInstrument],
    signal: &RaisedSignal,
    occurrence: Option<&BuySignalRecord>,
    evidence: &SignalCommitEvidence,
) -> Result<Vec<domain::MasterInstrument>, SystemPortError> {
    if evidence.master.row().instrument_id
        != evidence
            .signal
            .evidence
            .estimate
            .evidence
            .master
            .instrument_id
    {
        return Err(invalid());
    }
    let masters = merge_evidence_masters(existing, evidence)?;
    let current_master =
        master_for_signal(&evidence.signal, &masters).map_err(|_error| invalid())?;
    let rebuilt = rebuild_signal(&evidence.signal, &masters).map_err(|_error| invalid())?;
    rebuilt
        .evidence
        .capture(signal, current_master)
        .map_err(|_error| invalid())?;
    let historical = rebuilt
        .restore_history(current_master)
        .map_err(|_error| invalid())?;
    if historical != HistoricalSignal::from_signal(signal) {
        return Err(invalid());
    }
    let expected_occurrence = occurrence.map(HistoricalSignalOccurrence::from_record);
    let stored_occurrence = evidence
        .occurrence
        .as_ref()
        .map(|value| restore_occurrence(value, &masters))
        .transpose()
        .map_err(|_error| invalid())?;
    if expected_occurrence != stored_occurrence {
        return Err(invalid());
    }
    if let (Some(stored), Some(record)) = (&evidence.occurrence, occurrence) {
        stored
            .verify_commit_binding(&evidence.signal, record)
            .map_err(|_error| invalid())?;
    }
    Ok(masters)
}

fn prepare_current(
    context: (&RepositoryState, &InstrumentId, &SignalSnapshot),
    evaluation: &application::SignalEvaluation,
    record: &EstimateSubmissionRecord<AiRunId, ReasoningEffort>,
    head: Option<&CalibrationLedgerHead>,
) -> Result<PreparedCurrent, SystemPortError> {
    let (state, instrument, expected) = context;
    let Some(signal) = evaluation.decision.clone().into_signal() else {
        if evaluation.occurrence.is_some() || evaluation.evidence.is_some() {
            return Err(invalid());
        }
        return Ok((None, None, None));
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
    let (occurrence_id, new_occurrence) = add_occurrence(
        &signal,
        expected.current.as_ref(),
        evaluation.occurrence.as_ref(),
        &evaluation.transition,
        &state.history,
    )?;
    let evidence = evaluation.evidence.as_ref().ok_or_else(invalid)?;
    let masters = verify_commit_evidence(
        &state.masters,
        &signal,
        evaluation.occurrence.as_ref(),
        evidence,
    )?;
    Ok((
        Some(StoredCurrentEntry {
            submission: record.task_id().clone(),
            occurrence_id,
            evidence: evidence.clone(),
        }),
        new_occurrence,
        Some(masters),
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
        masters: None,
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
    if let Some(masters) = change.masters {
        state.masters = masters;
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
            let mut state = self.state.write();
            state.snapshot_reads = state.snapshot_reads.saturating_add(1);
            std::task::Poll::Ready(snapshot(&state, instrument))
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
                .filter(|value| value.instrument_id == *instrument || value.instrument == *identity)
                .map(|value| restore_occurrence(&value.evidence, &state.masters))
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
