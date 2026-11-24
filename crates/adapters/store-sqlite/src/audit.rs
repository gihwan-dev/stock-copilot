mod codec;

use crate::estimates::codec::invalid;
use crate::{Database, migration::digest, sql::audit as sql};
use application::{
    AuditChainHash, AuditEntry, AuditHead, AuditHeadAnchor, AuditIntegrity, AuditLog,
    AuditSequence, RecordOutcome, SystemPortError,
};
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

fn head_of(seq: i64, hash: &str) -> rusqlite::Result<AuditHead> {
    Ok(AuditHead::new(
        AuditSequence::try_new(u64::try_from(seq).map_err(invalid)?).map_err(invalid)?,
        AuditChainHash::try_new(hash).map_err(invalid)?,
    ))
}

fn last(connection: &Connection) -> rusqlite::Result<Option<(i64, String)>> {
    connection
        .query_row(sql::LAST, [], |row| Ok((row.get(0)?, row.get(1)?)))
        .optional()
}

fn append(
    transaction: &Transaction<'_>,
    entry: &EncodedEntry,
) -> rusqlite::Result<(RecordOutcome, AuditHead)> {
    let existing: Option<i64> = transaction
        .query_row(
            sql::FIND_ENTRY,
            rusqlite::params![entry.at, entry.kind, entry.payload],
            |row| row.get(0),
        )
        .optional()?;
    if existing.is_some() {
        let (seq, hash) = last(transaction)?.ok_or_else(|| invalid(()))?;
        return Ok((RecordOutcome::AlreadyRecorded, head_of(seq, &hash)?));
    }
    let (seq, prev) = match last(transaction)? {
        Some((seq, hash)) => (seq.checked_add(1).ok_or_else(|| invalid(()))?, hash),
        None => (1, String::from(GENESIS_HASH)),
    };
    let hash = chain_hash(seq, &entry.at, entry.kind, &entry.payload, &prev);
    match transaction.execute(
        sql::INSERT,
        rusqlite::params![seq, entry.at, entry.kind, entry.payload, prev, hash],
    )? {
        1 => Ok((RecordOutcome::Recorded, head_of(seq, &hash)?)),
        _ => Err(invalid(())),
    }
}

fn matches_head(
    connection: &Connection,
    head: Option<&AuditHead>,
) -> rusqlite::Result<AuditIntegrity> {
    let Some(anchored) = head else {
        return Ok(match last(connection)? {
            Some(_) => AuditIntegrity::Tampered,
            None => AuditIntegrity::Verified,
        });
    };
    let seq = i64::try_from(*anchored.sequence().as_u64()).map_err(invalid)?;
    let stored: Option<String> = connection
        .query_row(sql::HASH_AT, [seq], |row| row.get(0))
        .optional()?;
    Ok(match stored {
        Some(hash) if hash == anchored.hash().as_str() => AuditIntegrity::Verified,
        Some(_) | None => AuditIntegrity::Tampered,
    })
}

fn verify_anchored(
    connection: &Connection,
    head: Option<&AuditHead>,
) -> rusqlite::Result<AuditIntegrity> {
    match verify_chain(connection)? {
        AuditIntegrity::Verified => matches_head(connection, head),
        AuditIntegrity::Tampered => Ok(AuditIntegrity::Tampered),
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
            .map(|(outcome, _head)| outcome)
    }

    async fn verify(&self) -> Result<AuditIntegrity, SystemPortError> {
        self.database.read(verify_chain).await
    }
}

pub struct AnchoredAuditStore<'database, 'anchor, A> {
    database: &'database Database,
    anchor: &'anchor A,
}

impl<A> std::fmt::Debug for AnchoredAuditStore<'_, '_, A> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("AnchoredAuditStore")
    }
}

impl<'database, 'anchor, A: AuditHeadAnchor> AnchoredAuditStore<'database, 'anchor, A> {
    #[must_use]
    pub const fn new(database: &'database Database, anchor: &'anchor A) -> Self {
        Self { database, anchor }
    }
}

impl<A: AuditHeadAnchor> AuditLog for AnchoredAuditStore<'_, '_, A> {
    async fn append(&self, entry: &AuditEntry) -> Result<RecordOutcome, SystemPortError> {
        let encoded = encode(entry);
        let (outcome, head) = self
            .database
            .write(move |transaction| append(transaction, &encoded))
            .await?;
        self.anchor.store_head(&head).await?;
        Ok(outcome)
    }

    async fn verify(&self) -> Result<AuditIntegrity, SystemPortError> {
        let head = self.anchor.load_head().await?;
        self.database
            .read(move |connection| verify_anchored(connection, head.as_ref()))
            .await
    }
}
