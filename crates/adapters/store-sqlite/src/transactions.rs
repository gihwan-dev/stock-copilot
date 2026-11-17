mod codec;
mod observation;
mod rows;
mod state;
mod tags;
use crate::{Database, sql::transactions as sql};
use application::{
    FullInspectionRepository, SystemPortError, TransactionRepository, TransactionSyncRecord,
    TransactionSyncRepository, WriteCompleted,
};
use codec::{Fields, invalid, read_time, time_text};
use domain::{AccountId, SettledRowSet, SyncOutcome, SyncRecord, Timestamp};
use rusqlite::{Connection, OptionalExtension, Transaction, params};
#[derive(Debug)]
pub struct TransactionStore<'database> {
    database: &'database Database,
}
impl<'database> TransactionStore<'database> {
    #[must_use]
    pub const fn new(database: &'database Database) -> Self {
        Self { database }
    }
}
struct Entry {
    summary: SyncRecord<SettledRowSet>,
    state: SettledRowSet,
    detail: Option<TransactionSyncRecord>,
}
fn summary_tag(record: &SyncRecord<SettledRowSet>) -> &'static str {
    match record.outcome() {
        SyncOutcome::Complete(_) => "complete",
        SyncOutcome::Partial(_) => "partial",
        SyncOutcome::Failed => "failed",
        SyncOutcome::AccessBlocked => "blocked",
    }
}
fn summary(
    at: Timestamp,
    tag: &str,
    state: &SettledRowSet,
) -> rusqlite::Result<SyncRecord<SettledRowSet>> {
    Ok(SyncRecord::new(
        at,
        match tag {
            "complete" => SyncOutcome::Complete(state.clone()),
            "partial" => SyncOutcome::Partial(state.clone()),
            "failed" => SyncOutcome::Failed,
            "blocked" => SyncOutcome::AccessBlocked,
            _ => return Err(invalid(())),
        },
    ))
}
#[derive(Clone, Copy)]
enum Selection {
    Latest,
    Detail,
}
type Header = (i64, i64, String, String);
fn header(
    connection: &Connection,
    account: &AccountId,
    selection: Selection,
) -> rusqlite::Result<Option<Header>> {
    let decode = |record_row: &rusqlite::Row<'_>| {
        Ok((
            record_row.get::<_, i64>(0)?,
            record_row.get::<_, i64>(1)?,
            record_row.get::<_, String>(2)?,
            record_row.get::<_, String>(3)?,
        ))
    };
    match selection {
        Selection::Latest => connection.query_row(sql::LATEST, [account.as_i64()], decode),
        Selection::Detail => connection.query_row(sql::LATEST_SYNC, [account.as_i64()], decode),
    }
    .optional()
}
fn load(
    connection: &Connection,
    account: &AccountId,
    selection: Selection,
) -> rusqlite::Result<Option<Entry>> {
    let header = header(connection, account, selection)?;
    let Some((id, stored_account, mode, payload)) = header else {
        return Ok(None);
    };
    if stored_account != account.as_i64() {
        return Err(invalid(()));
    }
    let mut fields = Fields::decode(&payload)?;
    if fields.take()? != "1" {
        return Err(invalid(()));
    }
    let at = fields.time()?;
    let tag = fields.take()?;
    let (previous, before_count) = state::get(connection, id, "before", account, &mut fields)?;
    let (state, after_count) = state::get(connection, id, "after", account, &mut fields)?;
    validate_record_time(&previous, &state, at)?;
    let summary = summary(at, &tag, &state)?;
    let (detail, observed) = match mode.as_str() {
        "sync" => {
            let window = observation::window(&mut fields)?;
            let (observation, count) = observation::get(connection, id, &mut fields)?;
            let record =
                TransactionSyncRecord::from_observation(&previous, window, at, observation);
            if record.state() != &state || record.summary() != summary {
                return Err(invalid(()));
            }
            (Some(record), count)
        }
        "summary" => {
            match summary.outcome() {
                SyncOutcome::Partial(_) | SyncOutcome::Failed | SyncOutcome::AccessBlocked
                    if previous != state =>
                {
                    return Err(invalid(()));
                }
                SyncOutcome::Complete(_)
                | SyncOutcome::Partial(_)
                | SyncOutcome::Failed
                | SyncOutcome::AccessBlocked => {}
            }
            (None, 0)
        }
        _ => return Err(invalid(())),
    };
    fields.finish()?;
    validate_counts(
        connection,
        id,
        before_count
            .checked_add(after_count)
            .ok_or_else(|| invalid(()))?,
        observed,
    )?;
    Ok(Some(Entry {
        summary,
        state,
        detail,
    }))
}
fn validate_record_time(
    previous: &SettledRowSet,
    state: &SettledRowSet,
    at: Timestamp,
) -> rusqlite::Result<()> {
    if previous
        .last_synchronized_at()
        .is_some_and(|last| last > at)
        || state.last_synchronized_at().is_some_and(|last| last > at)
    {
        return Err(invalid(()));
    }
    Ok(())
}
fn validate_counts(
    connection: &Connection,
    id: i64,
    states: usize,
    observed: usize,
) -> rusqlite::Result<()> {
    let expected = states.checked_add(observed).ok_or_else(|| invalid(()))?;
    let stored_count =
        usize::try_from(connection.query_row(sql::ROW_COUNT, [id], |record_row| {
            record_row.get::<_, i64>(0)
        })?)
        .map_err(invalid)?;
    let source_count =
        usize::try_from(connection.query_row(sql::SOURCE_COUNT, [id], |record_row| {
            record_row.get::<_, i64>(0)
        })?)
        .map_err(invalid)?;
    if stored_count != expected || source_count != observed {
        return Err(invalid(()));
    }
    Ok(())
}
fn current(connection: &Connection, account: &AccountId) -> rusqlite::Result<SettledRowSet> {
    Ok(load(connection, account, Selection::Latest)?
        .map_or_else(|| SettledRowSet::new(account.clone()), |entry| entry.state))
}
fn latest_detail(
    connection: &Connection,
    account: &AccountId,
) -> rusqlite::Result<Option<TransactionSyncRecord>> {
    if let Some(detail) =
        load(connection, account, Selection::Latest)?.and_then(|entry| entry.detail)
    {
        return Ok(Some(detail));
    }
    Ok(load(connection, account, Selection::Detail)?.and_then(|entry| entry.detail))
}
#[derive(Clone, Copy)]
struct EntryInput<'a> {
    account: &'a AccountId,
    previous: &'a SettledRowSet,
    state: &'a SettledRowSet,
    summary: &'a SyncRecord<SettledRowSet>,
    detail: Option<&'a TransactionSyncRecord>,
}
fn save(tx: &Transaction<'_>, input: EntryInput<'_>) -> rusqlite::Result<WriteCompleted> {
    let EntryInput {
        account,
        previous,
        state,
        summary,
        detail,
    } = input;
    if previous.account_id() != account || state.account_id() != account {
        return Err(invalid(()));
    }
    let mut fields = Fields::default();
    fields.push(&("1"));
    fields.put_time(summary.recorded_at());
    fields.push(&(summary_tag(summary)));
    state::put(&mut fields, previous);
    state::put(&mut fields, state);
    if let Some(record) = detail {
        observation::put_window(&mut fields, record.window());
        observation::put(&mut fields, record.observation());
    }
    tx.execute(
        sql::INSERT_ENTRY,
        params![
            account.as_i64(),
            if detail.is_some() { "sync" } else { "summary" },
            fields.encode()
        ],
    )?;
    let id = tx.last_insert_rowid();
    state::save(tx, id, "before", previous)?;
    state::save(tx, id, "after", state)?;
    if let Some(record) = detail {
        observation::save(tx, id, record.observation())?;
    }
    let loaded = load(tx, account, Selection::Latest)?.ok_or_else(|| invalid(()))?;
    if loaded.state != *state || loaded.summary != *summary || loaded.detail.as_ref() != detail {
        return Err(invalid(()));
    }
    Ok(WriteCompleted)
}
fn record_summary(
    tx: &Transaction<'_>,
    account: &AccountId,
    record: &SyncRecord<SettledRowSet>,
) -> rusqlite::Result<WriteCompleted> {
    let latest = load(tx, account, Selection::Latest)?;
    let latest_at = latest.as_ref().map(|entry| entry.summary.recorded_at());
    let previous = latest.map_or_else(|| SettledRowSet::new(account.clone()), |entry| entry.state);
    if latest_at.is_some_and(|at| at > record.recorded_at())
        || previous
            .last_synchronized_at()
            .is_some_and(|at| at > record.recorded_at())
    {
        return Err(invalid(()));
    }
    let state = match record.outcome() {
        SyncOutcome::Complete(value) => value,
        SyncOutcome::Partial(value) if value != &previous => return Err(invalid(())),
        SyncOutcome::Partial(_) | SyncOutcome::Failed | SyncOutcome::AccessBlocked => &previous,
    };
    validate_record_time(&previous, state, record.recorded_at())?;
    save(
        tx,
        EntryInput {
            account,
            previous: &previous,
            state,
            summary: record,
            detail: None,
        },
    )
}
struct SyncBaseline {
    state: SettledRowSet,
    observed_at: Option<Timestamp>,
    summary_at: Option<Timestamp>,
}
fn sync_baseline(connection: &Connection, account: &AccountId) -> rusqlite::Result<SyncBaseline> {
    let latest = load(connection, account, Selection::Latest)?;
    let summary_at = latest.as_ref().map(|entry| entry.summary.recorded_at());
    let observed_at = match latest.as_ref().and_then(|entry| entry.detail.as_ref()) {
        Some(detail) => Some(detail.recorded_at()),
        None => load(connection, account, Selection::Detail)?
            .and_then(|entry| entry.detail)
            .map(|record| record.recorded_at()),
    };
    Ok(SyncBaseline {
        state: latest.map_or_else(|| SettledRowSet::new(account.clone()), |entry| entry.state),
        observed_at,
        summary_at,
    })
}
impl TransactionRepository for TransactionStore<'_> {
    async fn record(
        &self,
        account: &AccountId,
        record: &SyncRecord<SettledRowSet>,
    ) -> Result<WriteCompleted, SystemPortError> {
        let owned_account = account.clone();
        let owned_record = record.clone();
        self.database
            .write(move |tx| record_summary(tx, &owned_account, &owned_record))
            .await
    }
    async fn latest(
        &self,
        account: &AccountId,
    ) -> Result<Option<SyncRecord<SettledRowSet>>, SystemPortError> {
        let owned_account = account.clone();
        self.database
            .read(move |connection| {
                Ok(load(connection, &owned_account, Selection::Latest)?.map(|entry| entry.summary))
            })
            .await
    }
}
impl TransactionSyncRepository for TransactionStore<'_> {
    async fn state(&self, account: &AccountId) -> Result<SettledRowSet, SystemPortError> {
        let owned_account = account.clone();
        self.database
            .read(move |connection| current(connection, &owned_account))
            .await
    }
    async fn latest_sync(
        &self,
        account: &AccountId,
    ) -> Result<Option<TransactionSyncRecord>, SystemPortError> {
        let owned_account = account.clone();
        self.database
            .read(move |connection| latest_detail(connection, &owned_account))
            .await
    }
    async fn record_sync(
        &self,
        account: &AccountId,
        previous: &SettledRowSet,
        previous_recorded_at: Option<Timestamp>,
        record: &TransactionSyncRecord,
    ) -> Result<WriteCompleted, SystemPortError> {
        let owned_account = account.clone();
        let owned_previous = previous.clone();
        let owned_record = record.clone();
        self.database
            .write(move |tx| {
                let baseline = sync_baseline(tx, &owned_account)?;
                if baseline.state != owned_previous
                    || baseline.observed_at != previous_recorded_at
                    || previous_recorded_at.is_some_and(|at| at > owned_record.recorded_at())
                    || baseline
                        .state
                        .last_synchronized_at()
                        .is_some_and(|at| at > owned_record.recorded_at())
                    || baseline
                        .summary_at
                        .is_some_and(|at| at > owned_record.recorded_at())
                {
                    return Err(invalid(()));
                }
                let rebuilt = TransactionSyncRecord::from_observation(
                    &owned_previous,
                    owned_record.window(),
                    owned_record.recorded_at(),
                    owned_record.observation().clone(),
                );
                if rebuilt != owned_record {
                    return Err(invalid(()));
                }
                save(
                    tx,
                    EntryInput {
                        account: &owned_account,
                        previous: &owned_previous,
                        state: owned_record.state(),
                        summary: &owned_record.summary(),
                        detail: Some(&owned_record),
                    },
                )
            })
            .await
    }
}
fn completion(
    connection: &Connection,
    owned_account: &AccountId,
) -> rusqlite::Result<Option<Timestamp>> {
    connection
        .query_row(sql::COMPLETION, [owned_account.as_i64()], |row| {
            row.get::<_, String>(0)
        })
        .optional()?
        .as_deref()
        .map(read_time)
        .transpose()
}
impl FullInspectionRepository for TransactionStore<'_> {
    async fn last_completed(
        &self,
        account: &AccountId,
    ) -> Result<Option<Timestamp>, SystemPortError> {
        let owned_account = account.clone();
        self.database
            .read(move |connection| completion(connection, &owned_account))
            .await
    }
    async fn record_completed(
        &self,
        account: &AccountId,
        previous: Option<Timestamp>,
        completed_at: Timestamp,
    ) -> Result<WriteCompleted, SystemPortError> {
        let owned_account = account.clone();
        self.database
            .write(move |tx| {
                if completion(tx, &owned_account)? != previous
                    || previous.is_some_and(|at| at > completed_at)
                {
                    return Err(invalid(()));
                }
                tx.execute(
                    sql::COMPLETE,
                    params![owned_account.as_i64(), time_text(completed_at)],
                )?;
                if completion(tx, &owned_account)? != Some(completed_at) {
                    return Err(invalid(()));
                }
                Ok(WriteCompleted)
            })
            .await
    }
}
