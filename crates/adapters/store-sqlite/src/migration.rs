use crate::{failure, sql::migration as sql};
use application::SystemPortError;
use domain::Timestamp;
use rusqlite::{Connection, TransactionBehavior};
use sha2::{Digest, Sha256};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

pub(crate) fn digest(source: &str) -> String {
    Sha256::digest(source.as_bytes())
        .iter()
        .flat_map(|byte| {
            [
                char::from_digit(u32::from(byte >> 4_u8), 16),
                char::from_digit(u32::from(byte & 15), 16),
            ]
        })
        .flatten()
        .collect()
}

pub(crate) const MIGRATIONS: &[(&str, &str)] = &[
    ("0001_migration_ledger.sql", sql::CREATE_LEDGER),
    ("0002_holdings_snapshots.sql", crate::sql::holdings::CREATE),
    ("0003_transactions.sql", crate::sql::transactions::CREATE),
];

pub(crate) fn run(connection: &mut Connection, at: Timestamp) -> Result<(), SystemPortError> {
    apply(connection, at, MIGRATIONS).map_err(|error| failure::sqlite(&error))
}

pub(crate) fn apply(
    connection: &mut Connection,
    at: Timestamp,
    migrations: &[(&str, &str)],
) -> rusqlite::Result<()> {
    let guarded = Arc::new(AtomicBool::new(false));
    let policy = Arc::clone(&guarded);
    connection.authorizer(Some(move |context: rusqlite::hooks::AuthContext<'_>| {
        authorize(context.action, &policy)
    }))?;
    let result = apply_guarded(connection, at, migrations, &guarded);
    connection.authorizer(
        None::<fn(rusqlite::hooks::AuthContext<'_>) -> rusqlite::hooks::Authorization>,
    )?;
    result.map_err(|error| {
        if error
            .sqlite_error()
            .is_some_and(|cause| cause.extended_code == rusqlite::ffi::SQLITE_AUTH)
        {
            rusqlite::Error::InvalidQuery
        } else {
            error
        }
    })
}

fn apply_guarded(
    connection: &mut Connection,
    at: Timestamp,
    migrations: &[(&str, &str)],
    guarded: &AtomicBool,
) -> rusqlite::Result<()> {
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let before: i64 = transaction.query_row(crate::sql::SCHEMA, [], |row| row.get(0))?;
    let version: usize =
        usize::try_from(transaction.query_row(sql::VERSION, [], |row| row.get::<_, i64>(0))?)
            .map_err(|_error| rusqlite::Error::InvalidQuery)?;
    if version > migrations.len() || (before == 0 && version != 0) {
        return Err(rusqlite::Error::InvalidQuery);
    }
    if before == 0 {
        transaction.execute_batch(sql::CREATE_LEDGER)?;
    }
    let rows = ledger_rows(&transaction)?;
    if rows.len() != version || (before != 0 && rows.is_empty()) {
        return Err(rusqlite::Error::InvalidQuery);
    }
    verify_rows(&rows, migrations)?;
    for (index, (name, source)) in migrations.iter().enumerate() {
        let prefix = name
            .split_once('_')
            .and_then(|(number, _)| number.parse::<usize>().ok());
        if prefix != Some(index.checked_add(1).ok_or(rusqlite::Error::InvalidQuery)?) {
            return Err(rusqlite::Error::InvalidQuery);
        }
        if index >= rows.len() {
            if index != 0 {
                guarded.store(true, Ordering::Relaxed);
                let applied = transaction.execute_batch(source);
                guarded.store(false, Ordering::Relaxed);
                applied?;
            }
            transaction.execute(
                sql::INSERT_LEDGER,
                rusqlite::params![
                    i64::try_from(index.checked_add(1).ok_or(rusqlite::Error::InvalidQuery)?)
                        .map_err(|_error| rusqlite::Error::InvalidQuery)?,
                    name,
                    digest(source),
                    at.utc_seconds().as_i64()
                ],
            )?;
        }
    }
    transaction.pragma_update(
        None,
        "user_version",
        i64::try_from(migrations.len()).map_err(|_error| rusqlite::Error::InvalidQuery)?,
    )?;
    validate_schema(&transaction)?;
    verify_applied(&transaction, at, &rows, migrations)?;
    transaction.commit()
}

pub(crate) fn validate_schema(connection: &Connection) -> rusqlite::Result<()> {
    let tables = connection
        .prepare(sql::TABLES)?
        .query_map([], |row| row.get::<_, i64>(1))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    if tables.iter().any(|strict| *strict != 1) {
        return Err(rusqlite::Error::InvalidQuery);
    }
    for row in connection.prepare(sql::SCHEMA)?.query_map([], |row| {
        Ok((row.get::<_, String>(1)?, row.get::<_, String>(2)?))
    })? {
        let (ddl, kind) = row?;
        match kind.as_str() {
            "table" => crate::schema::validate(&ddl)?,
            "index" | "view" | "trigger" => {}
            _ => return Err(rusqlite::Error::InvalidQuery),
        }
    }
    Ok(())
}

#[derive(PartialEq, Eq)]
struct LedgerEntry {
    number: i64,
    name: String,
    sha256: String,
    applied_at: i64,
}

fn ledger_rows(connection: &Connection) -> rusqlite::Result<Vec<LedgerEntry>> {
    connection
        .prepare(sql::LEDGER)?
        .query_map([], |row| {
            Ok(LedgerEntry {
                number: row.get(0)?,
                name: row.get(1)?,
                sha256: row.get(2)?,
                applied_at: row.get(3)?,
            })
        })?
        .collect()
}

fn verify_rows(rows: &[LedgerEntry], migrations: &[(&str, &str)]) -> rusqlite::Result<()> {
    for (index, row) in rows.iter().enumerate() {
        let Some((expected_name, source)) = migrations.get(index) else {
            return Err(rusqlite::Error::InvalidQuery);
        };
        if usize::try_from(row.number).ok()
            != Some(index.checked_add(1).ok_or(rusqlite::Error::InvalidQuery)?)
            || row.name.as_str() != *expected_name
            || row.sha256 != digest(source)
        {
            return Err(rusqlite::Error::InvalidQuery);
        }
    }
    Ok(())
}

fn verify_applied(
    connection: &Connection,
    at: Timestamp,
    before: &[LedgerEntry],
    migrations: &[(&str, &str)],
) -> rusqlite::Result<()> {
    let expected = migrations
        .iter()
        .enumerate()
        .map(|(index, (name, source))| {
            Ok(LedgerEntry {
                number: i64::try_from(index.checked_add(1).ok_or(rusqlite::Error::InvalidQuery)?)
                    .map_err(|_error| rusqlite::Error::InvalidQuery)?,
                name: String::from(*name),
                sha256: digest(source),
                applied_at: before
                    .get(index)
                    .map_or_else(|| *at.utc_seconds().as_i64(), |row| row.applied_at),
            })
        })
        .collect::<rusqlite::Result<Vec<_>>>()?;
    if ledger_rows(connection)? == expected {
        Ok(())
    } else {
        Err(rusqlite::Error::InvalidQuery)
    }
}

fn authorize(
    action: rusqlite::hooks::AuthAction<'_>,
    guarded: &AtomicBool,
) -> rusqlite::hooks::Authorization {
    use rusqlite::hooks::{AuthAction, Authorization};
    match action {
        AuthAction::Transaction { .. }
        | AuthAction::Savepoint { .. }
        | AuthAction::Pragma { .. } => {
            if guarded.load(Ordering::Relaxed) {
                Authorization::Deny
            } else {
                Authorization::Allow
            }
        }
        AuthAction::Unknown { .. }
        | AuthAction::CreateIndex { .. }
        | AuthAction::CreateTable { .. }
        | AuthAction::CreateTempIndex { .. }
        | AuthAction::CreateTempTable { .. }
        | AuthAction::CreateTempTrigger { .. }
        | AuthAction::CreateTempView { .. }
        | AuthAction::CreateTrigger { .. }
        | AuthAction::CreateView { .. }
        | AuthAction::Delete { .. }
        | AuthAction::DropIndex { .. }
        | AuthAction::DropTable { .. }
        | AuthAction::DropTempIndex { .. }
        | AuthAction::DropTempTable { .. }
        | AuthAction::DropTempTrigger { .. }
        | AuthAction::DropTempView { .. }
        | AuthAction::DropTrigger { .. }
        | AuthAction::DropView { .. }
        | AuthAction::Insert { .. }
        | AuthAction::Read { .. }
        | AuthAction::Select
        | AuthAction::Update { .. }
        | AuthAction::Attach { .. }
        | AuthAction::Detach { .. }
        | AuthAction::AlterTable { .. }
        | AuthAction::Reindex { .. }
        | AuthAction::Analyze { .. }
        | AuthAction::CreateVtable { .. }
        | AuthAction::DropVtable { .. }
        | AuthAction::Function { .. }
        | AuthAction::Recursive => Authorization::Allow,
        _ => Authorization::Deny,
    }
}
