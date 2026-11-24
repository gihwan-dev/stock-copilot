use super::codec::{
    StoredCurrentEntry, decode_current_entry, decode_master, decode_stored_occurrence,
    encode_current_entry, encode_master, encode_stored_occurrence,
};
use crate::estimates::codec::invalid;
use crate::estimates::{decode_joined_record, load_latest_calibration};
use crate::sql::signals as sql;
use application::{
    AiRunId, ReasoningEffort, SignalCommit, SignalCommitEvidence, SignalCommitOutcome,
    SignalEvaluation, SignalInvalidationOutcome, SignalMutation, SignalRevision, SignalSnapshot,
    StoredSignal, SystemFailure, SystemPort, SystemPortError,
};
use domain::{
    BuySignalOccurrenceId, BuySignalRecord, CalibrationKey, CalibrationLedgerHead,
    CalibrationPosition, CurrentBuySignal, EstimateSubmissionRecord, HistoricalPortfolio,
    HistoricalSignal, HistoricalSignalOccurrence, InstrumentId, MasterInstrument,
    MasterInstrumentFacts, NoSignalReason, RaisedSignal, ResolvedOrderInstrument, SignalDecision,
    SignalSubmission, SignalTransition, StoredSignalEvidence, StoredSignalOccurrence,
    signal_transition_from_history,
};
use rusqlite::{Connection, OptionalExtension, Transaction, params};

pub(super) fn invalid_data() -> SystemPortError {
    SystemPortError::new(SystemPort::Repository, SystemFailure::InvalidData)
}

pub(super) fn integrity() -> SystemPortError {
    SystemPortError::new(SystemPort::Repository, SystemFailure::RepositoryIntegrity)
}

pub(super) fn all_masters(connection: &Connection) -> rusqlite::Result<Vec<MasterInstrument>> {
    let mut statement = connection.prepare(sql::ALL_MASTERS)?;
    let payloads = statement
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    payloads
        .iter()
        .map(|payload| decode_master(payload))
        .collect()
}

pub(super) fn load_calibration_head(
    connection: &Connection,
    key: &CalibrationKey,
) -> rusqlite::Result<Option<CalibrationLedgerHead>> {
    let Some((position, _)) = load_latest_calibration(connection, key)? else {
        return Ok(None);
    };
    let pos_u64 = u64::try_from(position).map_err(invalid)?;
    Ok(Some(CalibrationLedgerHead::new(
        key.clone(),
        CalibrationPosition::new(pos_u64),
    )))
}

pub(super) fn load_submission_record(
    connection: &Connection,
    task_id: &str,
) -> rusqlite::Result<Option<EstimateSubmissionRecord<AiRunId, ReasoningEffort>>> {
    connection
        .query_row(
            sql::SUBMISSION_RECORD_BY_TASK_ID,
            [task_id],
            decode_joined_record,
        )
        .optional()
}

pub(super) fn load_latest_submission_record(
    connection: &Connection,
    instrument: &str,
) -> rusqlite::Result<Option<EstimateSubmissionRecord<AiRunId, ReasoningEffort>>> {
    connection
        .query_row(
            sql::LATEST_SUBMISSION_RECORD_BY_INSTRUMENT,
            [instrument],
            decode_joined_record,
        )
        .optional()
}

pub(super) fn rebuild_signal(
    evidence: &StoredSignalEvidence,
    masters: &[MasterInstrument],
) -> Result<StoredSignalEvidence, SystemPortError> {
    let mut reconstructed = evidence.clone();
    if let Some(portfolio) = &reconstructed.evidence.assessment.portfolio {
        reconstructed.evidence.assessment.portfolio = Some(
            HistoricalPortfolio::from_saved_facts(portfolio.facts().clone(), masters)
                .map_err(|_error| integrity())?,
        );
    }
    Ok(reconstructed)
}

pub(super) fn master_for_signal<'a>(
    evidence: &StoredSignalEvidence,
    masters: &'a [MasterInstrument],
) -> Result<&'a MasterInstrument, SystemPortError> {
    let id = &evidence.evidence.estimate.evidence.master.instrument_id;
    masters
        .iter()
        .find(|master| &master.row().instrument_id == id)
        .ok_or_else(integrity)
}

pub(super) fn restore_signal(
    evidence: &StoredSignalEvidence,
    masters: &[MasterInstrument],
) -> Result<HistoricalSignal, SystemPortError> {
    let current = master_for_signal(evidence, masters)?;
    rebuild_signal(evidence, masters)?
        .restore_history(current)
        .map_err(|_error| integrity())
}

pub(super) fn restore_occurrence(
    evidence: &StoredSignalOccurrence,
    masters: &[MasterInstrument],
) -> Result<HistoricalSignalOccurrence, SystemPortError> {
    let mut reconstructed = evidence.clone();
    reconstructed.signal = rebuild_signal(&reconstructed.signal, masters)?;
    let current = master_for_signal(&reconstructed.signal, masters)?;
    reconstructed.restore(current).map_err(|_error| integrity())
}

pub(super) fn build_snapshot_from_parts(
    connection: &Connection,
    revision: u64,
    submission_id: Option<&str>,
    current_payload: Option<&str>,
    masters: &[MasterInstrument],
) -> rusqlite::Result<Result<SignalSnapshot, SystemPortError>> {
    let submission = match submission_id {
        Some(task_id) => match load_submission_record(connection, task_id)? {
            Some(record) => Some(record.task_id().clone()),
            None => return Ok(Err(integrity())),
        },
        None => None,
    };
    let current = match current_payload {
        Some(raw) => {
            let entry = decode_current_entry(raw, masters)?;
            let Some(record) = load_submission_record(connection, &entry.submission_id)? else {
                return Ok(Err(integrity()));
            };
            let signal = match restore_signal(&entry.evidence.signal, masters) {
                Ok(value) => value,
                Err(error) => return Ok(Err(error)),
            };
            let occurrence = match &entry.occurrence_id {
                Some(occ_id) => {
                    let stored: Option<String> = connection
                        .query_row(sql::OCCURRENCE_BY_ID, [occ_id.as_str()], |row| row.get(0))
                        .optional()?;
                    let Some(occ_raw) = stored else {
                        return Ok(Err(integrity()));
                    };
                    let decoded = decode_stored_occurrence(&occ_raw, masters)?;
                    match restore_occurrence(&decoded, masters) {
                        Ok(value) => Some(value),
                        Err(error) => return Ok(Err(error)),
                    }
                }
                None => None,
            };
            Some(StoredSignal {
                signal,
                submission: record.task_id().clone(),
                occurrence,
            })
        }
        None => None,
    };
    Ok(Ok(SignalSnapshot {
        revision: SignalRevision::new(revision),
        submission,
        current,
    }))
}

pub(super) fn load_snapshot(
    connection: &Connection,
    instrument: &InstrumentId,
) -> rusqlite::Result<Result<SignalSnapshot, SystemPortError>> {
    let masters = all_masters(connection)?;
    let state_row = connection
        .query_row(sql::STATE_BY_INSTRUMENT, [instrument.as_str()], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, Option<String>>(1)?))
        })
        .optional()?;
    let (revision, current_payload) = match state_row {
        Some((rev, payload)) => (u64::try_from(rev).map_err(invalid)?, payload),
        None => (0_u64, None),
    };
    let latest_submission = load_latest_submission_record(connection, instrument.as_str())?;
    let submission_id = latest_submission
        .as_ref()
        .map(|rec| rec.task_id().as_estimate_id().as_str());
    build_snapshot_from_parts(
        connection,
        revision,
        submission_id,
        current_payload.as_deref(),
        &masters,
    )
}

pub(super) fn load_occurrences_for(
    connection: &Connection,
    instrument: &InstrumentId,
    identity: &ResolvedOrderInstrument,
) -> rusqlite::Result<Result<Vec<HistoricalSignalOccurrence>, SystemPortError>> {
    let masters = all_masters(connection)?;
    let market_code = identity.market().as_code();
    let mut statement = connection.prepare(sql::OCCURRENCES_FOR_INSTRUMENT)?;
    let rows = statement
        .query_map(
            params![
                instrument.as_str(),
                market_code.as_str(),
                identity.symbol().as_str()
            ],
            |row| row.get::<_, String>(0),
        )?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut results = Vec::with_capacity(rows.len());
    for payload in rows {
        let stored = decode_stored_occurrence(&payload, &masters)?;
        match restore_occurrence(&stored, &masters) {
            Ok(restored) => results.push(restored),
            Err(error) => return Ok(Err(error)),
        }
    }
    Ok(Ok(results))
}

fn is_discarded(
    connection: &Connection,
    instrument: &InstrumentId,
    task_id: &str,
) -> rusqlite::Result<bool> {
    let found: Option<i64> = connection
        .query_row(
            sql::IS_DISCARDED,
            params![instrument.as_str(), task_id],
            |row| row.get(0),
        )
        .optional()?;
    Ok(found.is_some())
}

fn load_all_occurrence_ids(
    connection: &Connection,
) -> rusqlite::Result<Vec<BuySignalOccurrenceId>> {
    let mut statement = connection.prepare(sql::ALL_OCCURRENCE_IDS)?;
    let rows = statement
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    rows.iter()
        .map(|value| BuySignalOccurrenceId::try_new(value).map_err(invalid))
        .collect()
}

fn signal_submission(signal: &RaisedSignal) -> &SignalSubmission {
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
    existing: &[MasterInstrument],
    evidence: &SignalCommitEvidence,
) -> Result<Vec<MasterInstrument>, SystemPortError> {
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
    let validated = MasterInstrumentFacts::try_new(incoming).map_err(|_error| invalid_data())?;
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
    MasterInstrumentFacts::try_new(non_delisted).map_err(|_error| invalid_data())?;
    Ok(merged)
}

fn verify_commit_evidence(
    existing: &[MasterInstrument],
    signal: &RaisedSignal,
    occurrence: Option<&BuySignalRecord>,
    evidence: &SignalCommitEvidence,
) -> Result<Vec<MasterInstrument>, SystemPortError> {
    if evidence.master.row().instrument_id
        != evidence
            .signal
            .evidence
            .estimate
            .evidence
            .master
            .instrument_id
    {
        return Err(invalid_data());
    }
    let masters = merge_evidence_masters(existing, evidence)?;
    let current_master =
        master_for_signal(&evidence.signal, &masters).map_err(|_error| invalid_data())?;
    let rebuilt = rebuild_signal(&evidence.signal, &masters).map_err(|_error| invalid_data())?;
    rebuilt
        .evidence
        .capture(signal, current_master)
        .map_err(|_error| invalid_data())?;
    let historical = rebuilt
        .restore_history(current_master)
        .map_err(|_error| invalid_data())?;
    if historical != HistoricalSignal::from_signal(signal) {
        return Err(invalid_data());
    }
    let expected_occ = occurrence.map(HistoricalSignalOccurrence::from_record);
    let stored_occ = evidence
        .occurrence
        .as_ref()
        .map(|value| restore_occurrence(value, &masters))
        .transpose()
        .map_err(|_error| invalid_data())?;
    if expected_occ != stored_occ {
        return Err(invalid_data());
    }
    if let (Some(stored), Some(record)) = (&evidence.occurrence, occurrence) {
        stored
            .verify_commit_binding(&evidence.signal, record)
            .map_err(|_error| invalid_data())?;
    }
    Ok(masters)
}

fn add_occurrence(
    signal: &RaisedSignal,
    prior: Option<&StoredSignal>,
    supplied: Option<&BuySignalRecord>,
    transition: &SignalTransition,
    existing_ids: &[BuySignalOccurrenceId],
) -> Result<(Option<BuySignalOccurrenceId>, Option<BuySignalRecord>), SystemPortError> {
    match signal {
        RaisedSignal::Sell(_) => {
            if supplied.is_some() {
                return Err(invalid_data());
            }
            Ok((None, None))
        }
        RaisedSignal::Buy(buy) => {
            let occurrence = match transition {
                SignalTransition::Raised(_) | SignalTransition::Reversed(_) => {
                    let record = supplied.ok_or_else(invalid_data)?;
                    if existing_ids.contains(record.id()) {
                        return Err(invalid_data());
                    }
                    if record.originating_task().task() != buy.facts().task() {
                        return Err(invalid_data());
                    }
                    CurrentBuySignal::try_new(buy.clone(), record.clone())
                        .map_err(|_error| invalid_data())?;
                    return Ok((Some(record.id().clone()), Some(record.clone())));
                }
                SignalTransition::Unchanged | SignalTransition::Updated(_) => {
                    if supplied.is_some() {
                        return Err(invalid_data());
                    }
                    prior
                        .and_then(|value| value.occurrence.clone())
                        .ok_or_else(invalid_data)?
                }
                SignalTransition::Cleared(_) => return Err(invalid_data()),
            };
            CurrentBuySignal::try_from_history(buy.clone(), occurrence.clone())
                .map_err(|_error| invalid_data())?;
            Ok((Some(occurrence.id().clone()), None))
        }
    }
}

struct PreparedChange {
    current: Option<StoredCurrentEntry>,
    occurrence: Option<(BuySignalRecord, StoredSignalOccurrence)>,
    masters: Option<Vec<MasterInstrument>>,
    discarded: Option<String>,
    events: Vec<domain::DomainEvent>,
    reason: Option<NoSignalReason>,
}

fn prepare_discard(
    expected: &SignalSnapshot,
    transition: &SignalTransition,
) -> Result<PreparedChange, SystemPortError> {
    let Some(current) = &expected.current else {
        return Err(invalid_data());
    };
    let decision = SignalDecision::None(NoSignalReason::Expired);
    if signal_transition_from_history(Some(&current.signal), &decision)
        .map_err(|_error| invalid_data())?
        != *transition
    {
        return Err(invalid_data());
    }
    Ok(PreparedChange {
        current: None,
        occurrence: None,
        masters: None,
        discarded: Some(current.submission.as_estimate_id().as_str().to_owned()),
        events: transition.events(),
        reason: None,
    })
}

fn verify_signal_matches_record(
    tx: &Transaction<'_>,
    instrument: &InstrumentId,
    submission_id: &str,
    signal: &RaisedSignal,
    head: Option<&CalibrationLedgerHead>,
) -> rusqlite::Result<Result<(), SystemPortError>> {
    let Some(record) = load_submission_record(tx, submission_id)? else {
        return Ok(Err(invalid_data()));
    };
    let signal_head = match signal {
        RaisedSignal::Buy(value) => value.facts().calibration_head(),
        RaisedSignal::Sell(value) => value.facts().calibration_head(),
    };
    if Some(signal_head) != head
        || signal_instrument(signal) != instrument
        || signal_submission(signal) != record.submission()
    {
        return Ok(Err(invalid_data()));
    }
    Ok(Ok(()))
}

fn build_signal_change(
    tx: &Transaction<'_>,
    expected: &SignalSnapshot,
    evaluation: &SignalEvaluation,
    signal: &RaisedSignal,
    reason: Option<NoSignalReason>,
) -> rusqlite::Result<Result<PreparedChange, SystemPortError>> {
    let submission_id = evaluation.submission.as_estimate_id().as_str();
    let existing_ids = load_all_occurrence_ids(tx)?;
    let (occurrence_id, new_occ) = match add_occurrence(
        signal,
        expected.current.as_ref(),
        evaluation.occurrence.as_ref(),
        &evaluation.transition,
        &existing_ids,
    ) {
        Ok(pair) => pair,
        Err(error) => return Ok(Err(error)),
    };
    let Some(evidence) = &evaluation.evidence else {
        return Ok(Err(invalid_data()));
    };
    let existing_masters = all_masters(tx)?;
    let masters = match verify_commit_evidence(
        &existing_masters,
        signal,
        evaluation.occurrence.as_ref(),
        evidence,
    ) {
        Ok(merged) => merged,
        Err(error) => return Ok(Err(error)),
    };
    let saved_occ = match new_occ {
        Some(record_item) => match &evidence.occurrence {
            Some(stored_occ) => Some((record_item, stored_occ.clone())),
            None => return Ok(Err(invalid_data())),
        },
        None => None,
    };
    Ok(Ok(PreparedChange {
        current: Some(StoredCurrentEntry {
            submission_id: submission_id.to_owned(),
            occurrence_id,
            evidence: evidence.clone(),
        }),
        occurrence: saved_occ,
        masters: Some(masters),
        discarded: None,
        events: evaluation.transition.events(),
        reason,
    }))
}

fn prepare_evaluation_signal(
    tx: &Transaction<'_>,
    instrument: &InstrumentId,
    expected: &SignalSnapshot,
    evaluation: &SignalEvaluation,
    head: Option<&CalibrationLedgerHead>,
) -> rusqlite::Result<Result<PreparedChange, SystemPortError>> {
    let reason = match evaluation.decision {
        SignalDecision::None(value) => Some(value),
        SignalDecision::Buy(_) | SignalDecision::Sell(_) => None,
    };
    let Some(signal) = evaluation.decision.clone().into_signal() else {
        if evaluation.occurrence.is_some() || evaluation.evidence.is_some() {
            return Ok(Err(invalid_data()));
        }
        return Ok(Ok(PreparedChange {
            current: None,
            occurrence: None,
            masters: None,
            discarded: None,
            events: evaluation.transition.events(),
            reason,
        }));
    };
    let submission_id = evaluation.submission.as_estimate_id().as_str();
    if let Err(error) = verify_signal_matches_record(tx, instrument, submission_id, &signal, head)?
    {
        return Ok(Err(error));
    }
    build_signal_change(tx, expected, evaluation, &signal, reason)
}

fn prepare_evaluation_tx(
    tx: &Transaction<'_>,
    instrument: &InstrumentId,
    expected: &SignalSnapshot,
    evaluation: &SignalEvaluation,
) -> rusqlite::Result<Result<Option<PreparedChange>, SystemPortError>> {
    let Some(submission_id) = &expected.submission else {
        return Ok(Err(invalid_data()));
    };
    if &evaluation.submission != submission_id {
        return Ok(Ok(None));
    }
    let Some(record) = load_submission_record(tx, submission_id.as_estimate_id().as_str())? else {
        return Ok(Err(invalid_data()));
    };
    let head = load_calibration_head(tx, record.submission().calibration_key())?;
    if head != evaluation.calibration_head {
        return Ok(Ok(None));
    }
    let previous = expected.current.as_ref().map(|value| &value.signal);
    match signal_transition_from_history(previous, &evaluation.decision) {
        Ok(computed) if computed == evaluation.transition => {}
        Ok(_) | Err(_) => return Ok(Err(invalid_data())),
    }
    match prepare_evaluation_signal(tx, instrument, expected, evaluation, head.as_ref())? {
        Ok(change) => Ok(Ok(Some(change))),
        Err(error) => Ok(Err(error)),
    }
}

fn apply_change_tx(
    tx: &Transaction<'_>,
    instrument: &InstrumentId,
    revision: i64,
    change: PreparedChange,
) -> rusqlite::Result<SignalCommitOutcome> {
    if let Some(masters) = &change.masters {
        for master in masters {
            tx.execute(
                sql::UPSERT_MASTER,
                params![master.row().instrument_id.as_str(), encode_master(master)],
            )?;
        }
    }
    if let Some((record, stored_occ)) = &change.occurrence {
        let market_code = record.instrument().market().as_code();
        tx.execute(
            sql::INSERT_OCCURRENCE,
            params![
                stored_occ.id.as_str(),
                record.instrument_id().as_str(),
                market_code.as_str(),
                record.instrument().symbol().as_str(),
                encode_stored_occurrence(stored_occ)
            ],
        )?;
    }
    let current_payload = change.current.as_ref().map(encode_current_entry);
    tx.execute(
        sql::UPSERT_STATE,
        params![instrument.as_str(), revision, current_payload],
    )?;
    if let Some(discarded) = &change.discarded {
        tx.execute(
            sql::INSERT_DISCARDED,
            params![instrument.as_str(), discarded],
        )?;
    }
    Ok(SignalCommitOutcome::Applied(SignalCommit {
        events: change.events,
        no_signal_reason: change.reason,
    }))
}

pub(super) fn commit_signal_tx(
    tx: &Transaction<'_>,
    instrument: &InstrumentId,
    expected: &SignalSnapshot,
    mutation: &SignalMutation,
) -> rusqlite::Result<Result<SignalCommitOutcome, SystemPortError>> {
    let live = match load_snapshot(tx, instrument)? {
        Ok(value) => value,
        Err(error) => return Ok(Err(error)),
    };
    if &live != expected {
        return Ok(Ok(SignalCommitOutcome::Conflict));
    }
    if let SignalMutation::Evaluate(evaluation) = mutation
        && expected.submission.as_ref() == Some(&evaluation.submission)
        && is_discarded(
            tx,
            instrument,
            evaluation.submission.as_estimate_id().as_str(),
        )?
    {
        return Ok(Ok(SignalCommitOutcome::SubmissionDiscarded));
    }
    if let SignalMutation::Discard(transition) = mutation
        && expected.current.is_none()
        && **transition == SignalTransition::Unchanged
    {
        return Ok(Ok(SignalCommitOutcome::Applied(SignalCommit {
            events: Vec::new(),
            no_signal_reason: None,
        })));
    }
    let Some(advanced) = expected.revision.as_u64().checked_add(1) else {
        return Ok(Err(invalid_data()));
    };
    let Ok(stored_revision) = i64::try_from(advanced) else {
        return Ok(Err(invalid_data()));
    };
    let prepared = match mutation {
        SignalMutation::Discard(transition) => match prepare_discard(expected, transition) {
            Ok(change) => Some(change),
            Err(error) => return Ok(Err(error)),
        },
        SignalMutation::Evaluate(evaluation) => {
            match prepare_evaluation_tx(tx, instrument, expected, evaluation)? {
                Ok(value) => value,
                Err(error) => return Ok(Err(error)),
            }
        }
    };
    match prepared {
        Some(change) => Ok(Ok(apply_change_tx(
            tx,
            instrument,
            stored_revision,
            change,
        )?)),
        None => Ok(Ok(SignalCommitOutcome::Conflict)),
    }
}

pub(super) fn invalidate_signal_tx(
    tx: &Transaction<'_>,
    instrument: &InstrumentId,
    expected: &SignalSnapshot,
) -> rusqlite::Result<Result<SignalInvalidationOutcome, SystemPortError>> {
    let live = match load_snapshot(tx, instrument)? {
        Ok(value) => value,
        Err(error) => return Ok(Err(error)),
    };
    if &live != expected {
        return Ok(Ok(SignalInvalidationOutcome::Conflict));
    }
    if let Some(sub) = &expected.submission
        && is_discarded(tx, instrument, sub.as_estimate_id().as_str())?
    {
        return Ok(Ok(SignalInvalidationOutcome::SubmissionDiscarded));
    }
    if expected.current.is_none() {
        return Ok(Ok(SignalInvalidationOutcome::Applied(Vec::new())));
    }
    let Some(advanced) = expected.revision.as_u64().checked_add(1) else {
        return Ok(Err(invalid_data()));
    };
    let Ok(stored_revision) = i64::try_from(advanced) else {
        return Ok(Err(invalid_data()));
    };
    let transition = SignalTransition::Cleared(instrument.clone());
    let mut change = match prepare_discard(expected, &transition) {
        Ok(value) => value,
        Err(error) => return Ok(Err(error)),
    };
    change.discarded = None;
    let SignalCommitOutcome::Applied(commit) =
        apply_change_tx(tx, instrument, stored_revision, change)?
    else {
        return Ok(Err(invalid_data()));
    };
    Ok(Ok(SignalInvalidationOutcome::Applied(commit.events)))
}
