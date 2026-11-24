mod codec;

use crate::{Database, migration::digest, sql::audit as sql};
use application::{AuditEntry, AuditIntegrity, AuditLog, RecordOutcome, SystemPortError};
use codec::Encoder;
use rusqlite::{Connection, OptionalExtension, Transaction};

pub(crate) const GENESIS_HASH: &str =
    "0000000000000000000000000000000000000000000000000000000000000000";

#[derive(Debug)]
pub struct AuditStore<'database> {
    database: &'database Database,
}

impl<'database> AuditStore<'database> {
    #[must_use]
    pub const fn new(database: &'database Database) -> Self {
        Self { database }
    }
}

pub(crate) struct EncodedEntry {
    at: String,
    kind: &'static str,
    payload: String,
}

pub(crate) fn encode(entry: &AuditEntry) -> EncodedEntry {
    let (kind, payload) = codec::encode_kind(entry.kind());
    EncodedEntry {
        at: codec::encode_timestamp(entry.at()),
        kind,
        payload,
    }
}

pub(crate) fn chain_hash(seq: i64, at: &str, kind: &str, payload: &str, prev: &str) -> String {
    let mut input = Encoder::new();
    input.push(&seq.to_string());
    input.push(at);
    input.push(kind);
    input.push(payload);
    input.push(prev);
    digest(&input.finish())
}

fn append(transaction: &Transaction<'_>, entry: &EncodedEntry) -> rusqlite::Result<RecordOutcome> {
    let existing: Option<i64> = transaction
        .query_row(
            sql::FIND_ENTRY,
            rusqlite::params![entry.at, entry.kind, entry.payload],
            |row| row.get(0),
        )
        .optional()?;
    if existing.is_some() {
        return Ok(RecordOutcome::AlreadyRecorded);
    }
    let last: Option<(i64, String)> = transaction
        .query_row(sql::LAST, [], |row| Ok((row.get(0)?, row.get(1)?)))
        .optional()?;
    let (seq, prev) = match last {
        Some((seq, hash)) => (
            seq.checked_add(1).ok_or(rusqlite::Error::InvalidQuery)?,
            hash,
        ),
        None => (1, String::from(GENESIS_HASH)),
    };
    let hash = chain_hash(seq, &entry.at, entry.kind, &entry.payload, &prev);
    match transaction.execute(
        sql::INSERT,
        rusqlite::params![seq, entry.at, entry.kind, entry.payload, prev, hash],
    )? {
        1 => Ok(RecordOutcome::Recorded),
        _ => Err(rusqlite::Error::InvalidQuery),
    }
}

pub(crate) fn verify_chain(connection: &Connection) -> rusqlite::Result<AuditIntegrity> {
    let mut statement = connection.prepare(sql::ALL)?;
    let mut rows = statement.query([])?;
    let mut expected: i64 = 1;
    let mut prev = String::from(GENESIS_HASH);
    while let Some(row) = rows.next()? {
        let seq: i64 = row.get(0)?;
        let at: String = row.get(1)?;
        let kind: String = row.get(2)?;
        let payload: String = row.get(3)?;
        let stored_prev: String = row.get(4)?;
        let hash: String = row.get(5)?;
        if seq != expected
            || stored_prev != prev
            || chain_hash(seq, &at, &kind, &payload, &stored_prev) != hash
        {
            return Ok(AuditIntegrity::Tampered);
        }
        expected = expected
            .checked_add(1)
            .ok_or(rusqlite::Error::InvalidQuery)?;
        prev = hash;
    }
    Ok(AuditIntegrity::Verified)
}

impl AuditLog for AuditStore<'_> {
    async fn append(&self, entry: &AuditEntry) -> Result<RecordOutcome, SystemPortError> {
        let encoded = encode(entry);
        self.database
            .write(move |transaction| append(transaction, &encoded))
            .await
    }

    async fn verify(&self) -> Result<AuditIntegrity, SystemPortError> {
        self.database.read(verify_chain).await
    }
}
