use super::{
    codec::{Fields, invalid},
    rows::{self, RowEvidence, RowTarget},
};
use domain::{AccountId, SettledRowSet, StoredTransaction, TransactionBatch};
use rusqlite::{Connection, Transaction};
pub(super) fn put(fields: &mut Fields, state: &SettledRowSet) {
    fields.put_optional_time(state.last_synchronized_at());
    fields.push(&(state.history_rows().len()));
    fields.push(&(state.pending_rows().as_slice().len()));
}
pub(super) fn save(
    tx: &Transaction<'_>,
    entry: i64,
    scope: &str,
    state: &SettledRowSet,
) -> rusqlite::Result<usize> {
    for (position, row) in state.history_rows().iter().enumerate() {
        rows::save(
            tx,
            RowTarget {
                entry,
                scope,
                page: 0,
                position,
            },
            row.transaction(),
            RowEvidence {
                first: Some(row.first_seen()),
                gone: row.disappeared_at(),
                source: None,
            },
        )?;
    }
    for (position, row) in state.pending_rows().as_slice().iter().enumerate() {
        rows::save(
            tx,
            RowTarget {
                entry,
                scope,
                page: 1,
                position,
            },
            row,
            RowEvidence {
                first: None,
                gone: None,
                source: None,
            },
        )?;
    }
    state
        .history_rows()
        .len()
        .checked_add(state.pending_rows().as_slice().len())
        .ok_or_else(|| invalid(()))
}
pub(super) fn get(
    connection: &Connection,
    entry: i64,
    scope: &str,
    account: &AccountId,
    fields: &mut Fields,
) -> rusqlite::Result<(SettledRowSet, usize)> {
    let at = fields.optional_time()?;
    let history_count: usize = fields.number()?;
    let pending_count: usize = fields.number()?;
    if history_count > 1_000_000 || pending_count > 4096 {
        return Err(invalid(()));
    }
    let history = rows::load(connection, entry, scope, 0, history_count)?
        .into_iter()
        .map(|row| {
            if row.source.is_some() {
                return Err(invalid(()));
            }
            StoredTransaction::try_restore(
                row.transaction,
                row.first_seen.ok_or_else(|| invalid(()))?,
                row.disappeared_at,
            )
            .map_err(invalid)
        })
        .collect::<rusqlite::Result<_>>()?;
    let pending = rows::load(connection, entry, scope, 1, pending_count)?
        .into_iter()
        .map(|row| {
            if row.source.is_some() {
                return Err(invalid(()));
            }
            if row.first_seen.is_some() || row.disappeared_at.is_some() {
                return Err(invalid(()));
            }
            Ok(row.transaction)
        })
        .collect::<rusqlite::Result<_>>()?;
    Ok((
        SettledRowSet::try_restore(
            account.clone(),
            history,
            TransactionBatch::try_new(pending).map_err(invalid)?,
            at,
        )
        .map_err(invalid)?,
        history_count
            .checked_add(pending_count)
            .ok_or_else(|| invalid(()))?,
    ))
}
