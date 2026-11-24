pub(crate) mod codec;
pub(crate) mod valuation_codec;

use crate::{Database, migration::digest, sql::estimates as sql};
use application::resolve_estimates::{
    EstimateResolutionRepository, ResolutionCalibration, ResolutionCommitOutcome,
    ResolutionDataIssue,
};
use application::{
    AiRunId, EstimateCommitOutcome, EstimateRepository, EstimateSubmissionRepository,
    EstimateTaskRepository, ReasoningEffort, RecordOutcome, SystemPortError,
    ValuationDayRepository, ValuationRepository,
};
use codec::{
    decode_calibration, decode_calibration_key, decode_cost_eligible_task, decode_outcome,
    decode_run_id, decode_signal_submission, decode_submission_record_body, decode_task_key,
    encode_calibration, encode_calibration_key, encode_cost_eligible_task, encode_outcome,
    encode_resolution_issue, encode_run_id, encode_signal_submission,
    encode_submission_record_body, encode_task_key, invalid,
};
use domain::{
    AccountId, CalendarDate, Calibration, CalibrationEstimateId, CalibrationKey,
    CalibrationLedgerHead, CalibrationPosition, CalibrationSnapshot, CostEligibleTask,
    EstimateAssignments, EstimateConsumption, EstimateOutcome, EstimateSubmissionRecord,
    InstrumentId, SavedIssuedEstimateTask, SignalSubmission, SignalTaskKey, TimeInterval,
    ValuationDayResult, ValuationSnapshot,
};
use rusqlite::{Connection, OptionalExtension, Transaction, params};
use valuation_codec::{
    decode_valuation_day, decode_valuation_snapshot, encode_valuation_day,
    encode_valuation_snapshot,
};

#[derive(Debug)]
pub struct EstimateStore<'database> {
    database: &'database Database,
}

impl<'database> EstimateStore<'database> {
    #[must_use]
    pub const fn new(database: &'database Database) -> Self {
        Self { database }
    }
}

#[derive(Debug)]
pub struct ValuationStore<'database> {
    database: &'database Database,
}

impl<'database> ValuationStore<'database> {
    #[must_use]
    pub const fn new(database: &'database Database) -> Self {
        Self { database }
    }

    pub async fn day_result(
        &self,
        account: &AccountId,
        date: CalendarDate,
    ) -> Result<Option<ValuationDayResult>, SystemPortError> {
        let account_id = account.as_i64();
        let year = i64::from(*date.year().as_i32());
        let month = i64::from(*date.month().as_u8());
        let day = i64::from(*date.day().as_u8());
        self.database
            .read(move |connection| {
                let row = connection
                    .query_row(
                        sql::VALUATION_DAY_BY_DATE,
                        params![account_id, year, month, day],
                        |record| Ok((record.get::<_, String>(0)?, record.get::<_, String>(1)?)),
                    )
                    .optional()?;
                let Some((stored_hash, payload)) = row else {
                    return Ok(None);
                };
                if digest(&payload) != stored_hash {
                    return Err(invalid(()));
                }
                let decoded = decode_valuation_day(&payload)?;
                if decoded.date() != date {
                    return Err(invalid(()));
                }
                Ok(Some(decoded))
            })
            .await
    }
}

fn decode_consumption(raw: &str) -> rusqlite::Result<EstimateConsumption> {
    match raw {
        "available" => Ok(EstimateConsumption::Available),
        "consumed" => Ok(EstimateConsumption::Consumed),
        _ => Err(invalid(())),
    }
}

fn load_assignments(connection: &Connection) -> rusqlite::Result<EstimateAssignments<AiRunId>> {
    let mut statement = connection.prepare(sql::ALL_TASKS)?;
    let entries = statement
        .query_map([], |row| {
            let task_id =
                CalibrationEstimateId::try_new(&row.get::<_, String>(0)?).map_err(invalid)?;
            let run = decode_run_id(&row.get::<_, String>(1)?)?;
            let consumption = decode_consumption(&row.get::<_, String>(2)?)?;
            let task = decode_cost_eligible_task(&row.get::<_, String>(3)?)?;
            Ok(SavedIssuedEstimateTask {
                id: task_id,
                run,
                task,
                consumption,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    EstimateAssignments::restore_from_saved_state(entries).map_err(invalid)
}

pub(crate) fn decode_joined_record(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<EstimateSubmissionRecord<AiRunId, ReasoningEffort>> {
    let task_id = CalibrationEstimateId::try_new(&row.get::<_, String>(0)?).map_err(invalid)?;
    let run = decode_run_id(&row.get::<_, String>(1)?)?;
    let record_payload: String = row.get(2)?;
    let status = decode_consumption(&row.get::<_, String>(3)?)?;
    let task = decode_cost_eligible_task(&row.get::<_, String>(4)?)?;
    let assignments =
        EstimateAssignments::restore_from_saved_state(vec![SavedIssuedEstimateTask {
            id: task_id.clone(),
            run,
            task,
            consumption: status,
        }])
        .map_err(invalid)?;
    let issued = assignments.lookup(&task_id).map_err(invalid)?;
    let (submission, narrative, metadata) = decode_submission_record_body(&record_payload)?;
    EstimateSubmissionRecord::create(issued, submission, narrative, metadata).map_err(invalid)
}

pub(crate) fn load_latest_calibration(
    connection: &Connection,
    key: &CalibrationKey,
) -> rusqlite::Result<Option<(i64, Calibration)>> {
    let key_payload = encode_calibration_key(key);
    let key_hash = digest(&key_payload);
    let row = connection
        .query_row(sql::LATEST_CALIBRATION, [key_hash], |record| {
            Ok((
                record.get::<_, i64>(0)?,
                record.get::<_, String>(1)?,
                record.get::<_, String>(2)?,
            ))
        })
        .optional()?;
    let Some((position, stored_key, payload)) = row else {
        return Ok(None);
    };
    if position < 0 {
        return Err(invalid(()));
    }
    let decoded_key = decode_calibration_key(&stored_key)?;
    if &decoded_key != key {
        return Err(invalid(()));
    }
    let calibration = decode_calibration(&payload)?;
    if calibration.key() != key {
        return Err(invalid(()));
    }
    Ok(Some((position, calibration)))
}

fn load_resolution_calibration(
    connection: &Connection,
    key: &CalibrationKey,
) -> rusqlite::Result<Option<ResolutionCalibration>> {
    let Some((position, calibration)) = load_latest_calibration(connection, key)? else {
        return Ok(None);
    };
    let pos_u64 = u64::try_from(position).map_err(invalid)?;
    let head = CalibrationLedgerHead::new(key.clone(), CalibrationPosition::new(pos_u64));
    let snapshot = CalibrationSnapshot::try_new(calibration, head.clone()).map_err(invalid)?;
    Ok(Some(ResolutionCalibration { snapshot, head }))
}

fn collect_consumed_task_ids(
    previous: &EstimateAssignments<AiRunId>,
    next: &EstimateAssignments<AiRunId>,
) -> rusqlite::Result<Vec<CalibrationEstimateId>> {
    if next.entries().len() != previous.entries().len() {
        return Err(invalid(()));
    }
    let mut consumed_ids = Vec::new();
    for (prior, updated) in previous.entries().iter().zip(next.entries()) {
        if prior.id() != updated.id()
            || prior.run() != updated.run()
            || prior.task() != updated.task()
        {
            return Err(invalid(()));
        }
        match (prior.consumption(), updated.consumption()) {
            (EstimateConsumption::Available, EstimateConsumption::Consumed) => {
                consumed_ids.push(prior.id().as_estimate_id().clone());
            }
            (EstimateConsumption::Available, EstimateConsumption::Available)
            | (EstimateConsumption::Consumed, EstimateConsumption::Consumed) => {}
            (EstimateConsumption::Consumed, EstimateConsumption::Available) => {
                return Err(invalid(()));
            }
        }
    }
    Ok(consumed_ids)
}

fn commit_submission_tx(
    tx: &Transaction<'_>,
    previous: &EstimateAssignments<AiRunId>,
    next: &EstimateAssignments<AiRunId>,
    records: &[EstimateSubmissionRecord<AiRunId, ReasoningEffort>],
) -> rusqlite::Result<EstimateCommitOutcome> {
    if &load_assignments(tx)? != previous {
        return Ok(EstimateCommitOutcome::Conflict);
    }
    let consumed_ids = collect_consumed_task_ids(previous, next)?;
    if consumed_ids.len() != records.len() {
        return Err(invalid(()));
    }
    for task_id in &consumed_ids {
        if tx.execute(sql::CONSUME_TASK, [task_id.as_str()])? != 1 {
            return Err(invalid(()));
        }
    }
    for record in records {
        let task_id = record.task_id().as_estimate_id();
        if !consumed_ids.contains(task_id) {
            return Err(invalid(()));
        }
        let body = encode_submission_record_body(
            record.submission(),
            record.narrative(),
            record.metadata(),
        );
        tx.execute(
            sql::INSERT_SUBMISSION_RECORD,
            params![
                task_id.as_str(),
                encode_run_id(record.run()),
                record
                    .submission()
                    .task_key()
                    .task()
                    .volatility()
                    .instrument()
                    .as_str(),
                body
            ],
        )?;
    }
    Ok(EstimateCommitOutcome::Committed)
}

fn issue_task_tx(
    tx: &Transaction<'_>,
    task_id: &str,
    run: &AiRunId,
    task: &CostEligibleTask,
) -> rusqlite::Result<RecordOutcome> {
    let existing = tx
        .query_row(sql::TASK_BY_ID, [task_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .optional()?;
    if let Some((stored_run, _status, stored_payload)) = existing {
        let decoded_run = decode_run_id(&stored_run)?;
        let decoded_task = decode_cost_eligible_task(&stored_payload)?;
        if &decoded_run == run && &decoded_task == task {
            return Ok(RecordOutcome::AlreadyRecorded);
        }
        return Err(invalid(()));
    }
    let instrument = task.task().volatility().instrument().as_str();
    let payload = encode_cost_eligible_task(task);
    tx.execute(
        sql::INSERT_TASK,
        params![task_id, encode_run_id(run), instrument, payload],
    )?;
    Ok(RecordOutcome::Recorded)
}

fn load_submissions_for_instrument(
    connection: &Connection,
    instrument_id: &str,
) -> rusqlite::Result<Vec<SignalSubmission>> {
    let mut statement = connection.prepare(sql::SUBMISSIONS_BY_INSTRUMENT)?;
    let rows = statement
        .query_map([instrument_id], |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut results = Vec::with_capacity(rows.len());
    for payload in rows {
        let decoded = decode_signal_submission(&payload)?;
        if decoded.task_key().task().volatility().instrument().as_str() != instrument_id {
            return Err(invalid(()));
        }
        results.push(decoded);
    }
    Ok(results)
}

struct ResolutionCommitInput {
    task_id: String,
    key: CalibrationKey,
    outcome_payload: String,
    previous: ResolutionCalibration,
    next: Calibration,
}

fn commit_resolution_tx(
    tx: &Transaction<'_>,
    input: &ResolutionCommitInput,
) -> rusqlite::Result<ResolutionCommitOutcome> {
    let resolved: Option<String> = tx
        .query_row(sql::TASK_RESOLUTION_BY_ID, [&input.task_id], |row| {
            row.get(0)
        })
        .optional()?;
    if let Some(payload) = resolved {
        let _verified = decode_outcome(&payload)?;
        return Ok(ResolutionCommitOutcome::AlreadyResolved);
    }
    let current = load_resolution_calibration(tx, &input.key)?;
    if current.as_ref() != Some(&input.previous) {
        return Ok(ResolutionCommitOutcome::Conflict);
    }
    let next_position = input
        .previous
        .head
        .position()
        .as_u64()
        .checked_add(1)
        .ok_or_else(|| invalid(()))?;
    let stored_position = i64::try_from(next_position).map_err(invalid)?;
    let next_head =
        CalibrationLedgerHead::new(input.key.clone(), CalibrationPosition::new(next_position));
    let _verified_snapshot =
        CalibrationSnapshot::try_new(input.next.clone(), next_head).map_err(invalid)?;
    let key_payload = encode_calibration_key(&input.key);
    let key_hash = digest(&key_payload);
    let table_payload = encode_calibration(&input.next);
    let table_hash = digest(&table_payload);
    tx.execute(
        sql::INSERT_TASK_RESOLUTION,
        params![&input.task_id, &input.outcome_payload],
    )?;
    tx.execute(
        sql::INSERT_CALIBRATION,
        params![
            key_hash,
            stored_position,
            key_payload,
            table_hash,
            table_payload
        ],
    )?;
    Ok(ResolutionCommitOutcome::Committed)
}

fn load_valuation_history(
    connection: &Connection,
    account_id: i64,
    window: TimeInterval,
) -> rusqlite::Result<Vec<ValuationSnapshot>> {
    let start_sec = *window.start().utc_seconds().as_i64();
    let start_nanos = i64::from(*window.start().nanosecond().as_u32());
    let end_sec = *window.end().utc_seconds().as_i64();
    let end_nanos = i64::from(*window.end().nanosecond().as_u32());
    let mut statement = connection.prepare(sql::VALUATION_HISTORY)?;
    let rows = statement
        .query_map(
            params![account_id, start_sec, start_nanos, end_sec, end_nanos],
            |row| row.get::<_, String>(0),
        )?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut results = Vec::with_capacity(rows.len());
    for payload in rows {
        let snapshot = decode_valuation_snapshot(&payload)?;
        if !(window.start()..window.end()).contains(&snapshot.valuation_at()) {
            return Err(invalid(()));
        }
        results.push(snapshot);
    }
    results.sort_by_key(ValuationSnapshot::valuation_at);
    Ok(results)
}

fn record_valuation_day_tx(
    tx: &Transaction<'_>,
    account_id: i64,
    result: &ValuationDayResult,
) -> rusqlite::Result<RecordOutcome> {
    let year = i64::from(*result.date().year().as_i32());
    let month = i64::from(*result.date().month().as_u8());
    let day = i64::from(*result.date().day().as_u8());
    let existing = tx
        .query_row(
            sql::VALUATION_DAY_BY_DATE,
            params![account_id, year, month, day],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()?;
    if let Some((stored_hash, stored_payload)) = existing {
        if digest(&stored_payload) != stored_hash {
            return Err(invalid(()));
        }
        let decoded = decode_valuation_day(&stored_payload)?;
        if &decoded == result {
            return Ok(RecordOutcome::AlreadyRecorded);
        }
        return Err(invalid(()));
    }
    let payload = encode_valuation_day(result);
    let result_hash = digest(&payload);
    tx.execute(
        sql::INSERT_VALUATION_DAY,
        params![account_id, year, month, day, result_hash, payload],
    )?;
    Ok(RecordOutcome::Recorded)
}

impl EstimateTaskRepository for EstimateStore<'_> {
    async fn issue_task(
        &self,
        id: &CalibrationEstimateId,
        run: &AiRunId,
        task: &CostEligibleTask,
    ) -> Result<RecordOutcome, SystemPortError> {
        let task_id = id.as_str().to_owned();
        let expected_task = task.clone();
        let expected_run = run.clone();
        self.database
            .write(move |tx| issue_task_tx(tx, &task_id, &expected_run, &expected_task))
            .await
    }
}

impl EstimateRepository for EstimateStore<'_> {
    async fn append_submission(
        &self,
        submission: &SignalSubmission,
    ) -> Result<RecordOutcome, SystemPortError> {
        let instrument = submission
            .task_key()
            .task()
            .volatility()
            .instrument()
            .as_str()
            .to_owned();
        let payload = encode_signal_submission(submission);
        let payload_hash = digest(&payload);
        self.database
            .write(move |tx| {
                let exists: Option<i64> = tx
                    .query_row(sql::SUBMISSION_EXISTS, [&payload_hash], |row| row.get(0))
                    .optional()?;
                if exists.is_some() {
                    return Ok(RecordOutcome::AlreadyRecorded);
                }
                tx.execute(
                    sql::INSERT_SUBMISSION,
                    params![instrument, payload_hash, payload],
                )?;
                Ok(RecordOutcome::Recorded)
            })
            .await
    }

    async fn submissions(
        &self,
        instrument: &InstrumentId,
    ) -> Result<Vec<SignalSubmission>, SystemPortError> {
        let instrument_id = instrument.as_str().to_owned();
        self.database
            .read(move |connection| load_submissions_for_instrument(connection, &instrument_id))
            .await
    }

    async fn append_resolution(
        &self,
        task: &SignalTaskKey,
        outcome: &EstimateOutcome,
    ) -> Result<RecordOutcome, SystemPortError> {
        let task_payload = encode_task_key(task);
        let task_key_hash = digest(&task_payload);
        let outcome_payload = encode_outcome(outcome);
        self.database
            .write(move |tx| {
                let existing = tx
                    .query_row(sql::RESOLUTION_BY_TASK_KEY, [&task_key_hash], |row| {
                        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                    })
                    .optional()?;
                if let Some((stored_task, stored_outcome)) = existing {
                    let _verified_task = decode_task_key(&stored_task)?;
                    let _verified_outcome = decode_outcome(&stored_outcome)?;
                    return Ok(RecordOutcome::AlreadyRecorded);
                }
                tx.execute(
                    sql::INSERT_RESOLUTION,
                    params![task_key_hash, task_payload, outcome_payload],
                )?;
                Ok(RecordOutcome::Recorded)
            })
            .await
    }

    async fn resolution(
        &self,
        task: &SignalTaskKey,
    ) -> Result<Option<EstimateOutcome>, SystemPortError> {
        let task_payload = encode_task_key(task);
        let task_key_hash = digest(&task_payload);
        let expected_task = task.clone();
        self.database
            .read(move |connection| {
                let row = connection
                    .query_row(sql::RESOLUTION_BY_TASK_KEY, [&task_key_hash], |record| {
                        Ok((record.get::<_, String>(0)?, record.get::<_, String>(1)?))
                    })
                    .optional()?;
                let Some((stored_task, stored_outcome)) = row else {
                    return Ok(None);
                };
                if decode_task_key(&stored_task)? != expected_task {
                    return Err(invalid(()));
                }
                Ok(Some(decode_outcome(&stored_outcome)?))
            })
            .await
    }

    async fn append_calibration(
        &self,
        key: &CalibrationKey,
        calibration: &Calibration,
    ) -> Result<RecordOutcome, SystemPortError> {
        if calibration.key() != key {
            return Err(SystemPortError::new(
                application::SystemPort::Repository,
                application::SystemFailure::InvalidData,
            ));
        }
        let key_payload = encode_calibration_key(key);
        let key_hash = digest(&key_payload);
        let table_payload = encode_calibration(calibration);
        let table_hash = digest(&table_payload);
        let owned_key = key.clone();
        self.database
            .write(move |tx| {
                let duplicate: Option<i64> = tx
                    .query_row(
                        sql::CALIBRATION_HASH_EXISTS,
                        params![&key_hash, &table_hash],
                        |row| row.get(0),
                    )
                    .optional()?;
                if duplicate.is_some() {
                    return Ok(RecordOutcome::AlreadyRecorded);
                }
                let next_position = match load_latest_calibration(tx, &owned_key)? {
                    Some((last_pos, _)) => last_pos.checked_add(1).ok_or_else(|| invalid(()))?,
                    None => 0_i64,
                };
                tx.execute(
                    sql::INSERT_CALIBRATION,
                    params![
                        key_hash,
                        next_position,
                        key_payload,
                        table_hash,
                        table_payload
                    ],
                )?;
                Ok(RecordOutcome::Recorded)
            })
            .await
    }

    async fn calibration(
        &self,
        key: &CalibrationKey,
    ) -> Result<Option<Calibration>, SystemPortError> {
        let owned_key = key.clone();
        self.database
            .read(move |connection| {
                Ok(load_latest_calibration(connection, &owned_key)?.map(|(_, table)| table))
            })
            .await
    }
}

impl EstimateSubmissionRepository for EstimateStore<'_> {
    async fn submission_records(
        &self,
        instrument: &InstrumentId,
    ) -> Result<Vec<EstimateSubmissionRecord<AiRunId, ReasoningEffort>>, SystemPortError> {
        let instrument_id = instrument.as_str().to_owned();
        self.database
            .read(move |connection| {
                let mut statement = connection.prepare(sql::SUBMISSION_RECORDS_BY_INSTRUMENT)?;
                statement
                    .query_map([&instrument_id], decode_joined_record)?
                    .collect()
            })
            .await
    }

    async fn assignments(&self) -> Result<EstimateAssignments<AiRunId>, SystemPortError> {
        self.database.read(load_assignments).await
    }

    async fn commit_submission(
        &self,
        previous: &EstimateAssignments<AiRunId>,
        next: &EstimateAssignments<AiRunId>,
        records: &[EstimateSubmissionRecord<AiRunId, ReasoningEffort>],
    ) -> Result<EstimateCommitOutcome, SystemPortError> {
        let owned_previous = previous.clone();
        let owned_next = next.clone();
        let owned_records = records.to_vec();
        self.database
            .write(move |tx| commit_submission_tx(tx, &owned_previous, &owned_next, &owned_records))
            .await
    }
}

impl EstimateResolutionRepository for EstimateStore<'_> {
    async fn pending_submissions(
        &self,
    ) -> Result<Vec<EstimateSubmissionRecord<AiRunId, ReasoningEffort>>, SystemPortError> {
        self.database
            .read(|connection| {
                let mut statement = connection.prepare(sql::PENDING_SUBMISSION_RECORDS)?;
                statement.query_map([], decode_joined_record)?.collect()
            })
            .await
    }

    async fn calibration(
        &self,
        key: &CalibrationKey,
    ) -> Result<Option<ResolutionCalibration>, SystemPortError> {
        let owned_key = key.clone();
        self.database
            .read(move |connection| load_resolution_calibration(connection, &owned_key))
            .await
    }

    async fn resolution(
        &self,
        id: &CalibrationEstimateId,
    ) -> Result<Option<EstimateOutcome>, SystemPortError> {
        let task_id = id.as_str().to_owned();
        self.database
            .read(move |connection| {
                let row = connection
                    .query_row(sql::TASK_RESOLUTION_BY_ID, [&task_id], |record| {
                        record.get::<_, String>(0)
                    })
                    .optional()?;
                row.map(|payload| decode_outcome(&payload)).transpose()
            })
            .await
    }

    async fn record_unavailable(
        &self,
        record: &EstimateSubmissionRecord<AiRunId, ReasoningEffort>,
        reason: ResolutionDataIssue,
    ) -> Result<ResolutionCommitOutcome, SystemPortError> {
        let task_id = record.task_id().as_estimate_id().as_str().to_owned();
        let reason_payload = encode_resolution_issue(reason);
        self.database
            .write(move |tx| {
                let resolved: Option<String> = tx
                    .query_row(sql::TASK_RESOLUTION_BY_ID, [&task_id], |row| row.get(0))
                    .optional()?;
                if let Some(payload) = resolved {
                    let _verified = decode_outcome(&payload)?;
                    return Ok(ResolutionCommitOutcome::AlreadyResolved);
                }
                tx.execute(sql::INSERT_UNAVAILABLE, params![task_id, reason_payload])?;
                Ok(ResolutionCommitOutcome::Committed)
            })
            .await
    }

    async fn commit_resolution(
        &self,
        record: &EstimateSubmissionRecord<AiRunId, ReasoningEffort>,
        outcome: &EstimateOutcome,
        previous: &ResolutionCalibration,
        next: &Calibration,
    ) -> Result<ResolutionCommitOutcome, SystemPortError> {
        let input = ResolutionCommitInput {
            task_id: record.task_id().as_estimate_id().as_str().to_owned(),
            key: record.submission().calibration_key().clone(),
            outcome_payload: encode_outcome(outcome),
            previous: previous.clone(),
            next: next.clone(),
        };
        self.database
            .write(move |tx| commit_resolution_tx(tx, &input))
            .await
    }
}

impl ValuationRepository for ValuationStore<'_> {
    async fn append(
        &self,
        account: &AccountId,
        snapshot: &ValuationSnapshot,
    ) -> Result<RecordOutcome, SystemPortError> {
        let account_id = account.as_i64();
        let as_of_seconds = *snapshot.valuation_at().utc_seconds().as_i64();
        let as_of_nanos = i64::from(*snapshot.valuation_at().nanosecond().as_u32());
        let as_of_offset = i64::from(*snapshot.valuation_at().offset_seconds().as_i32());
        let payload = encode_valuation_snapshot(snapshot);
        let snapshot_hash = digest(&payload);
        self.database
            .write(move |tx| {
                let exists: Option<i64> = tx
                    .query_row(
                        sql::VALUATION_EXISTS,
                        params![account_id, &snapshot_hash],
                        |row| row.get(0),
                    )
                    .optional()?;
                if exists.is_some() {
                    return Ok(RecordOutcome::AlreadyRecorded);
                }
                tx.execute(
                    sql::INSERT_VALUATION,
                    params![
                        account_id,
                        as_of_seconds,
                        as_of_nanos,
                        as_of_offset,
                        snapshot_hash,
                        payload
                    ],
                )?;
                Ok(RecordOutcome::Recorded)
            })
            .await
    }

    async fn history(
        &self,
        account: &AccountId,
        window: TimeInterval,
    ) -> Result<Vec<ValuationSnapshot>, SystemPortError> {
        let account_id = account.as_i64();
        self.database
            .read(move |connection| load_valuation_history(connection, account_id, window))
            .await
    }
}

impl ValuationDayRepository for ValuationStore<'_> {
    async fn record(
        &self,
        account: &AccountId,
        result: &ValuationDayResult,
    ) -> Result<RecordOutcome, SystemPortError> {
        let account_id = account.as_i64();
        let owned_result = result.clone();
        self.database
            .write(move |tx| record_valuation_day_tx(tx, account_id, &owned_result))
            .await
    }
}
