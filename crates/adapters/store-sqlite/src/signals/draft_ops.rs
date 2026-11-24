use super::codec::{
    StoredSnapshotBinding, decode_current_entry, decode_draft_evidence, decode_snapshot_binding,
    encode_draft_evidence, encode_snapshot_binding,
};
use super::signal_ops::{
    all_masters, build_snapshot_from_parts, integrity, invalid_data, load_calibration_head,
    load_snapshot,
};
use crate::estimates::codec::invalid;
use crate::sql::signals as sql;
use application::{
    ConditionalDraftOutcome, CurrentDraftOutcome, SignalSnapshot, SystemPortError, WriteCompleted,
};
use domain::{
    AccountId, Draft, DraftRestoreEvidence, InstrumentId, MasterInstrument, Nanosecond,
    OffsetSeconds, OrderSide, PriceObservation, Proposal, ProposalExecutionError, ProposalId,
    Timestamp, UnixSeconds,
};
use rusqlite::{Connection, OptionalExtension, Transaction, params};

pub(super) struct OwnedDraftCheck {
    pub(super) account: AccountId,
    pub(super) at: Timestamp,
    pub(super) observation: PriceObservation,
    pub(super) master: MasterInstrument,
}

pub(super) struct OwnedDraftCandidate {
    pub(super) proposal: Proposal<Draft>,
    pub(super) evidence: DraftRestoreEvidence,
}

fn stored_master_for<'a>(
    masters: &'a [MasterInstrument],
    instrument: &InstrumentId,
) -> Result<&'a MasterInstrument, SystemPortError> {
    masters
        .iter()
        .find(|master| &master.row().instrument_id == instrument)
        .ok_or_else(integrity)
}

fn verify_live_draft_masters(
    masters: &[MasterInstrument],
    instrument: &InstrumentId,
    caller: &MasterInstrument,
    side: OrderSide,
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
    evidence: &DraftRestoreEvidence,
    masters: &[MasterInstrument],
    instrument: &InstrumentId,
    caller: &MasterInstrument,
) -> Result<Proposal<Draft>, SystemPortError> {
    let stored = stored_master_for(masters, instrument)?;
    evidence.restore(stored).map_err(|_error| integrity())?;
    evidence.restore(caller).map_err(|_error| integrity())
}

fn check_expiry_columns(
    evidence: &DraftRestoreEvidence,
    stored_account: i64,
    expires_seconds: i64,
    expires_nanos: i64,
    expires_offset: i64,
) -> rusqlite::Result<()> {
    let nanos = u32::try_from(expires_nanos).map_err(invalid)?;
    let offset = i32::try_from(expires_offset).map_err(invalid)?;
    let col_expiry = Timestamp::from_parts(
        UnixSeconds::new(expires_seconds),
        Nanosecond::new(nanos),
        OffsetSeconds::new(offset),
    )
    .map_err(invalid)?;
    if evidence.account.as_i64() != stored_account
        || evidence.expires_at != col_expiry
        || evidence.expires_at.offset_seconds() != col_expiry.offset_seconds()
    {
        return Err(invalid(()));
    }
    Ok(())
}

pub(super) fn store_draft_standalone_tx(
    tx: &Transaction<'_>,
    proposal: &Proposal<Draft>,
    evidence: &DraftRestoreEvidence,
    master: &MasterInstrument,
) -> rusqlite::Result<Result<WriteCompleted, SystemPortError>> {
    if evidence.verify_matches_proposal(proposal, master).is_err() {
        return Ok(Err(invalid_data()));
    }
    let expires_at = proposal.expires_at();
    if expires_at != evidence.expires_at
        || expires_at.offset_seconds() != evidence.expires_at.offset_seconds()
    {
        return Ok(Err(invalid_data()));
    }
    tx.execute(
        sql::SUPERSEDE_STANDALONE_BY_PROPOSAL_ID,
        [proposal.id().as_client_order_id().as_str()],
    )?;
    let payload = encode_draft_evidence(evidence);
    tx.execute(
        sql::INSERT_DRAFT,
        params![
            proposal.id().as_client_order_id().as_str(),
            "standalone",
            evidence.estimate.evidence.master.instrument_id.as_str(),
            proposal.intent().account_id().as_i64(),
            *expires_at.utc_seconds().as_i64(),
            i64::from(*expires_at.nanosecond().as_u32()),
            i64::from(*expires_at.offset_seconds().as_i32()),
            Option::<String>::None,
            payload
        ],
    )?;
    Ok(Ok(WriteCompleted))
}

struct DraftRow {
    mode: String,
    instrument: String,
    account: i64,
    expires_seconds: i64,
    expires_nanos: i64,
    expires_offset: i64,
    snapshot_payload: Option<String>,
    evidence_payload: String,
}

fn load_selected_account(connection: &Connection) -> rusqlite::Result<Option<i64>> {
    connection
        .query_row(sql::SELECTED_ACCOUNT, [], |row| row.get(0))
        .optional()
}

pub(super) fn read_draft_tx(
    connection: &Connection,
    proposal_id: &ProposalId,
    master: &MasterInstrument,
) -> rusqlite::Result<Result<Option<Proposal<Draft>>, SystemPortError>> {
    let row = connection
        .query_row(
            sql::LATEST_CURRENT_DRAFT_BY_ID,
            [proposal_id.as_client_order_id().as_str()],
            |record| {
                Ok(DraftRow {
                    mode: record.get(0)?,
                    instrument: record.get(1)?,
                    account: record.get(2)?,
                    expires_seconds: record.get(3)?,
                    expires_nanos: record.get(4)?,
                    expires_offset: record.get(5)?,
                    snapshot_payload: record.get(6)?,
                    evidence_payload: record.get(7)?,
                })
            },
        )
        .optional()?;
    let Some(loaded) = row else {
        return Ok(Ok(None));
    };
    let evidence = decode_draft_evidence(&loaded.evidence_payload)?;
    check_expiry_columns(
        &evidence,
        loaded.account,
        loaded.expires_seconds,
        loaded.expires_nanos,
        loaded.expires_offset,
    )?;
    let instrument_id = InstrumentId::try_new(&loaded.instrument).map_err(invalid)?;
    if evidence.id != *proposal_id
        || evidence.estimate.evidence.master.instrument_id != instrument_id
    {
        return Err(invalid(()));
    }
    match loaded.mode.as_str() {
        "standalone" => {
            if loaded.snapshot_payload.is_some() {
                return Err(invalid(()));
            }
            match evidence.restore(master) {
                Ok(restored) => Ok(Ok(Some(restored))),
                Err(_) => Ok(Err(integrity())),
            }
        }
        "guarded" => read_guarded_draft(
            connection,
            &instrument_id,
            &evidence,
            loaded.snapshot_payload.as_deref(),
            master,
        ),
        _ => Err(invalid(())),
    }
}

fn read_guarded_draft(
    connection: &Connection,
    instrument: &InstrumentId,
    evidence: &DraftRestoreEvidence,
    snapshot_raw: Option<&str>,
    master: &MasterInstrument,
) -> rusqlite::Result<Result<Option<Proposal<Draft>>, SystemPortError>> {
    let Some(snap_raw) = snapshot_raw else {
        return Err(invalid(()));
    };
    if load_selected_account(connection)? != Some(evidence.account.as_i64()) {
        return Ok(Ok(None));
    }
    let masters = all_masters(connection)?;
    let binding = decode_snapshot_binding(snap_raw)?;
    let stored_snapshot = match build_snapshot_from_parts(
        connection,
        binding.revision,
        binding.submission_id.as_deref(),
        binding.current_payload.as_deref(),
        &masters,
    )? {
        Ok(value) => value,
        Err(error) => return Ok(Err(error)),
    };
    let current = match load_snapshot(connection, instrument)? {
        Ok(value) => value,
        Err(error) => return Ok(Err(error)),
    };
    let Some(signal) = stored_snapshot.current.as_ref() else {
        return Ok(Ok(None));
    };
    let head = signal.signal.calibration_head();
    let live_head = load_calibration_head(connection, head.key())?;
    if current != stored_snapshot || live_head.as_ref() != Some(head) {
        return Ok(Ok(None));
    }
    Ok(restore_draft_with_masters(evidence, &masters, instrument, master).map(Some))
}

struct GuardedDraftRow {
    row_id: i64,
    account: i64,
    expires_seconds: i64,
    expires_nanos: i64,
    expires_offset: i64,
    snapshot_payload: Option<String>,
    evidence_payload: String,
}

fn load_current_guarded_row(
    connection: &Connection,
    instrument: &InstrumentId,
) -> rusqlite::Result<Option<GuardedDraftRow>> {
    connection
        .query_row(
            sql::CURRENT_GUARDED_DRAFT_BY_INSTRUMENT,
            [instrument.as_str()],
            |row| {
                Ok(GuardedDraftRow {
                    row_id: row.get(0)?,
                    account: row.get(2)?,
                    expires_seconds: row.get(3)?,
                    expires_nanos: row.get(4)?,
                    expires_offset: row.get(5)?,
                    snapshot_payload: row.get(6)?,
                    evidence_payload: row.get(7)?,
                })
            },
        )
        .optional()
}

fn inspect_existing_draft_tx(
    tx: &Transaction<'_>,
    instrument: &InstrumentId,
    expected: &SignalSnapshot,
    check: &OwnedDraftCheck,
    masters: &[MasterInstrument],
) -> rusqlite::Result<Result<CurrentDraftOutcome, SystemPortError>> {
    let Some(old) = load_current_guarded_row(tx, instrument)? else {
        return Ok(Ok(CurrentDraftOutcome::Absent));
    };
    let evidence = decode_draft_evidence(&old.evidence_payload)?;
    check_expiry_columns(
        &evidence,
        old.account,
        old.expires_seconds,
        old.expires_nanos,
        old.expires_offset,
    )?;
    let Some(snap_raw) = &old.snapshot_payload else {
        return Err(invalid(()));
    };
    let binding = decode_snapshot_binding(snap_raw)?;
    let old_snapshot = match build_snapshot_from_parts(
        tx,
        binding.revision,
        binding.submission_id.as_deref(),
        binding.current_payload.as_deref(),
        masters,
    )? {
        Ok(value) => value,
        Err(error) => return Ok(Err(error)),
    };
    if &old_snapshot != expected {
        tx.execute(sql::SUPERSEDE_DRAFT_BY_ROW_ID, [old.row_id])?;
        return Ok(Ok(CurrentDraftOutcome::Absent));
    }
    let old_proposal =
        match restore_draft_with_masters(&evidence, masters, instrument, &check.master) {
            Ok(value) => value,
            Err(error) => return Ok(Err(error)),
        };
    match old_proposal.check_current(check.at, &check.observation) {
        Ok(()) if old_proposal.intent().account_id() == &check.account => {
            Ok(Ok(CurrentDraftOutcome::Existing(old_proposal.id().clone())))
        }
        Ok(())
        | Err(ProposalExecutionError::Expired | ProposalExecutionError::PriceOutsideRange) => {
            tx.execute(sql::SUPERSEDE_DRAFT_BY_ROW_ID, [old.row_id])?;
            Ok(Ok(CurrentDraftOutcome::Absent))
        }
        Err(
            error @ (ProposalExecutionError::ClockRollback
            | ProposalExecutionError::ObservationInvalid),
        ) => Ok(Ok(CurrentDraftOutcome::Invalid(error))),
    }
}

pub(super) fn current_draft_tx(
    tx: &Transaction<'_>,
    instrument: &InstrumentId,
    expected: &SignalSnapshot,
    check: &OwnedDraftCheck,
) -> rusqlite::Result<Result<CurrentDraftOutcome, SystemPortError>> {
    let live = match load_snapshot(tx, instrument)? {
        Ok(value) => value,
        Err(error) => return Ok(Err(error)),
    };
    if &live != expected {
        return Ok(Ok(CurrentDraftOutcome::Conflict));
    }
    let Some(current) = expected.current.as_ref() else {
        return Ok(Err(invalid_data()));
    };
    let head = current.signal.calibration_head();
    if load_calibration_head(tx, head.key())?.as_ref() != Some(head) {
        return Ok(Ok(CurrentDraftOutcome::Conflict));
    }
    if load_selected_account(tx)? != Some(check.account.as_i64()) {
        return Ok(Ok(CurrentDraftOutcome::AccountChanged));
    }
    if current
        .signal
        .validate_observation(&check.observation, check.at)
        .is_err()
    {
        return Ok(Ok(CurrentDraftOutcome::Invalid(
            ProposalExecutionError::ObservationInvalid,
        )));
    }
    let masters = all_masters(tx)?;
    if let Err(error) =
        verify_live_draft_masters(&masters, instrument, &check.master, current.signal.side())
    {
        return Ok(Err(error));
    }
    inspect_existing_draft_tx(tx, instrument, expected, check, &masters)
}

fn verify_draft_evidence_tx(
    tx: &Transaction<'_>,
    instrument: &InstrumentId,
    candidate: &OwnedDraftCandidate,
    caller_master: &MasterInstrument,
    masters: &[MasterInstrument],
) -> rusqlite::Result<Result<(), SystemPortError>> {
    if candidate.evidence.estimate.evidence.master.instrument_id != *instrument {
        return Ok(Err(integrity()));
    }
    let stored = match stored_master_for(masters, instrument) {
        Ok(value) => value,
        Err(error) => return Ok(Err(error)),
    };
    let current_payload: Option<Option<String>> = tx
        .query_row(sql::STATE_BY_INSTRUMENT, [instrument.as_str()], |row| {
            row.get(1)
        })
        .optional()?;
    let Some(Some(raw_entry)) = current_payload else {
        return Ok(Err(integrity()));
    };
    let entry = decode_current_entry(&raw_entry, masters)?;
    let saved_estimate = &entry.evidence.signal.evidence.estimate;
    if candidate
        .evidence
        .estimate
        .verify_same_as(saved_estimate)
        .is_err()
        || candidate
            .evidence
            .verify_matches_proposal(&candidate.proposal, stored)
            .is_err()
        || candidate
            .evidence
            .verify_matches_proposal(&candidate.proposal, caller_master)
            .is_err()
    {
        return Ok(Err(integrity()));
    }
    Ok(Ok(()))
}

fn check_existing_for_store_tx(
    tx: &Transaction<'_>,
    instrument: &InstrumentId,
    expected: &SignalSnapshot,
    check: &OwnedDraftCheck,
    masters: &[MasterInstrument],
) -> rusqlite::Result<Result<Option<ConditionalDraftOutcome>, SystemPortError>> {
    let Some(old) = load_current_guarded_row(tx, instrument)? else {
        return Ok(Ok(None));
    };
    let evidence = decode_draft_evidence(&old.evidence_payload)?;
    check_expiry_columns(
        &evidence,
        old.account,
        old.expires_seconds,
        old.expires_nanos,
        old.expires_offset,
    )?;
    let Some(snap_raw) = &old.snapshot_payload else {
        return Err(invalid(()));
    };
    let binding = decode_snapshot_binding(snap_raw)?;
    let old_snapshot = match build_snapshot_from_parts(
        tx,
        binding.revision,
        binding.submission_id.as_deref(),
        binding.current_payload.as_deref(),
        masters,
    )? {
        Ok(value) => value,
        Err(error) => return Ok(Err(error)),
    };
    if &old_snapshot != expected {
        return Ok(Ok(None));
    }
    let old_proposal =
        match restore_draft_with_masters(&evidence, masters, instrument, &check.master) {
            Ok(value) => value,
            Err(error) => return Ok(Err(error)),
        };
    match old_proposal.check_current(check.at, &check.observation) {
        Ok(()) if old_proposal.intent().account_id() == &check.account => Ok(Ok(Some(
            ConditionalDraftOutcome::Existing(old_proposal.id().clone()),
        ))),
        Ok(())
        | Err(ProposalExecutionError::Expired | ProposalExecutionError::PriceOutsideRange) => {
            Ok(Ok(None))
        }
        Err(
            error @ (ProposalExecutionError::ClockRollback
            | ProposalExecutionError::ObservationInvalid),
        ) => Ok(Ok(Some(ConditionalDraftOutcome::Invalid(error)))),
    }
}

fn validate_store_draft_tx(
    tx: &Transaction<'_>,
    instrument: &InstrumentId,
    expected: &SignalSnapshot,
    candidate: &OwnedDraftCandidate,
    check: &OwnedDraftCheck,
) -> rusqlite::Result<Result<Option<ConditionalDraftOutcome>, SystemPortError>> {
    let Some(current) = expected.current.as_ref() else {
        return Ok(Err(invalid_data()));
    };
    if candidate.proposal.intent().side() != current.signal.side()
        || candidate.proposal.intent().account_id() != &check.account
    {
        return Ok(Err(invalid_data()));
    }
    let live = match load_snapshot(tx, instrument)? {
        Ok(value) => value,
        Err(error) => return Ok(Err(error)),
    };
    let head = current.signal.calibration_head();
    if &live != expected || load_calibration_head(tx, head.key())?.as_ref() != Some(head) {
        return Ok(Ok(Some(ConditionalDraftOutcome::Conflict)));
    }
    if load_selected_account(tx)? != Some(check.account.as_i64()) {
        return Ok(Ok(Some(ConditionalDraftOutcome::AccountChanged)));
    }
    if current
        .signal
        .validate_observation(&check.observation, check.at)
        .is_err()
    {
        return Ok(Ok(Some(ConditionalDraftOutcome::Invalid(
            ProposalExecutionError::ObservationInvalid,
        ))));
    }
    if let Err(error) = candidate
        .proposal
        .check_current(check.at, &check.observation)
    {
        return Ok(Ok(Some(ConditionalDraftOutcome::Invalid(error))));
    }
    let duplicate: Option<i64> = tx
        .query_row(
            sql::DRAFT_EXISTS_BY_ID,
            [candidate.proposal.id().as_client_order_id().as_str()],
            |row| row.get(0),
        )
        .optional()?;
    if duplicate.is_some() {
        return Ok(Err(invalid_data()));
    }
    let masters = all_masters(tx)?;
    if let Err(error) =
        verify_draft_evidence_tx(tx, instrument, candidate, &check.master, &masters)?
    {
        return Ok(Err(error));
    }
    check_existing_for_store_tx(tx, instrument, expected, check, &masters)
}

pub(super) fn store_current_draft_tx(
    tx: &Transaction<'_>,
    instrument: &InstrumentId,
    expected: &SignalSnapshot,
    candidate: &OwnedDraftCandidate,
    check: &OwnedDraftCheck,
) -> rusqlite::Result<Result<ConditionalDraftOutcome, SystemPortError>> {
    match validate_store_draft_tx(tx, instrument, expected, candidate, check)? {
        Ok(Some(outcome)) => return Ok(Ok(outcome)),
        Ok(None) => {}
        Err(error) => return Ok(Err(error)),
    }
    tx.execute(sql::SUPERSEDE_GUARDED_BY_INSTRUMENT, [instrument.as_str()])?;
    let current_payload: Option<String> = tx
        .query_row(sql::STATE_BY_INSTRUMENT, [instrument.as_str()], |row| {
            row.get(1)
        })
        .optional()?
        .flatten();
    let binding = StoredSnapshotBinding {
        revision: *expected.revision.as_u64(),
        submission_id: expected
            .submission
            .as_ref()
            .map(|id| id.as_estimate_id().as_str().to_owned()),
        current_payload,
    };
    let expires_at = candidate.proposal.expires_at();
    tx.execute(
        sql::INSERT_DRAFT,
        params![
            candidate.proposal.id().as_client_order_id().as_str(),
            "guarded",
            instrument.as_str(),
            candidate.proposal.intent().account_id().as_i64(),
            *expires_at.utc_seconds().as_i64(),
            i64::from(*expires_at.nanosecond().as_u32()),
            i64::from(*expires_at.offset_seconds().as_i32()),
            Some(encode_snapshot_binding(&binding)),
            encode_draft_evidence(&candidate.evidence)
        ],
    )?;
    Ok(Ok(ConditionalDraftOutcome::Stored))
}
