use crate::{Database, sql::instrument_mapping as sql};
use application::{
    InstrumentIdIssuer, InstrumentMappingRepository, MappingStoreError, SystemPortError,
};
use domain::{
    IndependentInstrumentIdIssuer, InstrumentId, InstrumentMappingError, InstrumentMappingRecord,
    InstrumentMappingStatus, InstrumentMappings, InstrumentSource, IsinCode, IssuedInstrumentId,
    IssuerSequence, KnownMarket, MappingAssignment, PersistedIssuerCursor, Symbol,
};
use rusqlite::{Connection, OptionalExtension, Row, Transaction, params};
use std::collections::BTreeSet;

const STATUS_ACTIVE: &str = "active";
const STATUS_SUPERSEDED: &str = "superseded";
const STATUS_RETIRED: &str = "retired";

#[derive(Debug)]
pub struct InstrumentMappingStore<'database> {
    database: &'database Database,
}

impl<'database> InstrumentMappingStore<'database> {
    #[must_use]
    pub const fn new(database: &'database Database) -> Self {
        Self { database }
    }
}

#[derive(Debug)]
pub struct InstrumentIdIssuerStore<'database> {
    database: &'database Database,
}

impl<'database> InstrumentIdIssuerStore<'database> {
    #[must_use]
    pub const fn new(database: &'database Database) -> Self {
        Self { database }
    }

    pub async fn restore_issuer(&self) -> Result<IndependentInstrumentIdIssuer, SystemPortError> {
        self.database.read(restore_domain_issuer).await
    }
}

enum IdPresence {
    Present,
    Absent,
}

fn invalid<E>(_error: E) -> rusqlite::Error {
    rusqlite::Error::InvalidQuery
}

const fn market_tag(market: KnownMarket) -> &'static str {
    match market {
        KnownMarket::Kr => "KR",
        KnownMarket::Us => "US",
    }
}

fn decode_market(raw: &str) -> rusqlite::Result<KnownMarket> {
    match raw {
        "KR" => Ok(KnownMarket::Kr),
        "US" => Ok(KnownMarket::Us),
        _ => Err(rusqlite::Error::InvalidQuery),
    }
}

fn decode_status(raw: &str) -> rusqlite::Result<InstrumentMappingStatus> {
    match raw {
        STATUS_ACTIVE => Ok(InstrumentMappingStatus::Active),
        STATUS_SUPERSEDED => Ok(InstrumentMappingStatus::Superseded),
        STATUS_RETIRED => Ok(InstrumentMappingStatus::Retired),
        _ => Err(rusqlite::Error::InvalidQuery),
    }
}

fn decode_row(row: &Row<'_>) -> rusqlite::Result<InstrumentMappingRecord> {
    let raw_id: String = row.get(0)?;
    let raw_market: String = row.get(1)?;
    let raw_symbol: String = row.get(2)?;
    let raw_isin: String = row.get(3)?;
    let raw_status: String = row.get(4)?;

    let id = InstrumentId::try_new(&raw_id).map_err(invalid)?;
    let market = decode_market(&raw_market)?;
    let symbol = Symbol::try_new(&raw_symbol).map_err(invalid)?;
    let isin = IsinCode::try_new(&raw_isin).map_err(invalid)?;
    let status = decode_status(&raw_status)?;

    Ok(InstrumentMappingRecord::new(
        InstrumentSource::new(market, symbol, isin),
        id,
        status,
    ))
}

fn encode_sequence_hex(sequence: IssuerSequence) -> String {
    let mut output = String::with_capacity(16);
    for byte in sequence.as_u64().to_be_bytes() {
        for nibble in [byte >> 4_u8, byte & 15_u8] {
            if let Some(digit) = char::from_digit(u32::from(nibble), 16) {
                output.push(digit);
            }
        }
    }
    output
}

fn decode_sequence_hex(raw: &str) -> rusqlite::Result<IssuerSequence> {
    if raw.len() != 16
        || !raw
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(rusqlite::Error::InvalidQuery);
    }
    let parsed = u64::from_str_radix(raw, 16).map_err(invalid)?;
    IssuerSequence::try_new(parsed).map_err(invalid)
}

fn expected_schema_sql() -> BTreeSet<String> {
    sql::CREATE
        .split(";\n")
        .map(str::trim)
        .filter(|statement| !statement.is_empty())
        .map(|statement| statement.strip_suffix(';').unwrap_or(statement).to_owned())
        .collect()
}

fn expected_migration_triggers() -> BTreeSet<String> {
    crate::migration::MIGRATIONS
        .iter()
        .flat_map(|(_, source)| source.split(";\n"))
        .map(str::trim)
        .filter(|statement| statement.starts_with(sql::TRIGGER_PREFIX))
        .map(|statement| statement.strip_suffix(';').unwrap_or(statement).to_owned())
        .collect()
}

fn validate_sql_set(
    connection: &Connection,
    query: &str,
    expected: &BTreeSet<String>,
) -> rusqlite::Result<()> {
    let mut statement = connection.prepare(query)?;
    let actual_rows = statement
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let actual: BTreeSet<String> = actual_rows
        .iter()
        .map(|entry| entry.trim().to_owned())
        .collect();
    if actual_rows.len() != expected.len() || actual != *expected {
        return Err(rusqlite::Error::InvalidQuery);
    }
    Ok(())
}

pub(crate) fn validate_mapping_objects(connection: &Connection) -> rusqlite::Result<()> {
    validate_sql_set(connection, sql::SCHEMA_OBJECTS, &expected_schema_sql())
}

pub(crate) fn validate_all_triggers(connection: &Connection) -> rusqlite::Result<()> {
    validate_sql_set(
        connection,
        sql::ALL_TRIGGERS,
        &expected_migration_triggers(),
    )
}

pub(crate) fn validate_schema(connection: &Connection) -> rusqlite::Result<()> {
    validate_mapping_objects(connection)?;
    validate_all_triggers(connection)
}

fn read_issuer_cursor(connection: &Connection) -> rusqlite::Result<PersistedIssuerCursor> {
    let mut statement = connection.prepare(sql::ISSUER_CURSOR)?;
    let rows = statement
        .query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    match rows.as_slice() {
        [] => Ok(PersistedIssuerCursor::Initial),
        [(1, raw)] => Ok(PersistedIssuerCursor::LastIssued(decode_sequence_hex(raw)?)),
        _ => Err(rusqlite::Error::InvalidQuery),
    }
}

pub(crate) fn validate_issuer_cursor(connection: &Connection) -> rusqlite::Result<()> {
    let _cursor = read_issuer_cursor(connection)?;
    Ok(())
}

pub(crate) fn validate_ledger(connection: &Connection) -> rusqlite::Result<()> {
    let mut statement = connection.prepare(sql::ALL)?;
    let records = statement
        .query_map([], decode_row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let _ledger = InstrumentMappings::restore(records).map_err(invalid)?;
    Ok(())
}

pub(crate) fn validate_on_open(connection: &Connection) -> rusqlite::Result<()> {
    validate_schema(connection)?;
    validate_ledger(connection)?;
    validate_issuer_cursor(connection)
}

fn restore_records(records: Vec<InstrumentMappingRecord>) -> rusqlite::Result<InstrumentMappings> {
    InstrumentMappings::restore(records).map_err(invalid)
}

fn load_for_source(
    connection: &Connection,
    source: &InstrumentSource,
) -> rusqlite::Result<InstrumentMappings> {
    let mut statement = connection.prepare(sql::BY_SOURCE)?;
    let records = statement
        .query_map(
            params![
                market_tag(source.market()),
                source.symbol().as_str(),
                source.isin().as_str(),
            ],
            decode_row,
        )?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    restore_records(records)
}

fn load_for_source_or_id(
    connection: &Connection,
    source: &InstrumentSource,
    candidate: &InstrumentId,
) -> rusqlite::Result<InstrumentMappings> {
    let mut statement = connection.prepare(sql::BY_SOURCE_OR_ID)?;
    let records = statement
        .query_map(
            params![
                market_tag(source.market()),
                source.symbol().as_str(),
                source.isin().as_str(),
                candidate.as_str(),
            ],
            decode_row,
        )?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    restore_records(records)
}

fn load_for_id(connection: &Connection, id: &InstrumentId) -> rusqlite::Result<InstrumentMappings> {
    let mut statement = connection.prepare(sql::BY_ID)?;
    let records = statement
        .query_map(params![id.as_str()], decode_row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    restore_records(records)
}

fn mapping_id_presence(connection: &Connection, id: &InstrumentId) -> rusqlite::Result<IdPresence> {
    let mut statement = connection.prepare(sql::ID_EXISTS)?;
    let found: Option<i64> = statement
        .query_row([id.as_str()], |row| row.get(0))
        .optional()?;
    match found {
        Some(_) => Ok(IdPresence::Present),
        None => Ok(IdPresence::Absent),
    }
}

fn persist_issuer_cursor(
    transaction: &Transaction<'_>,
    previous: PersistedIssuerCursor,
    next: IssuerSequence,
) -> rusqlite::Result<()> {
    let encoded_next = encode_sequence_hex(next);
    let affected = match previous {
        PersistedIssuerCursor::Initial => {
            transaction.execute(sql::INSERT_ISSUER_CURSOR, [encoded_next.as_str()])?
        }
        PersistedIssuerCursor::LastIssued(prev_seq) => {
            let encoded_prev = encode_sequence_hex(prev_seq);
            transaction.execute(
                sql::UPDATE_ISSUER_CURSOR,
                params![encoded_next.as_str(), encoded_prev.as_str()],
            )?
        }
    };
    if affected != 1 {
        return Err(rusqlite::Error::InvalidQuery);
    }
    Ok(())
}

fn issue_next(transaction: &Transaction<'_>) -> rusqlite::Result<IssuedInstrumentId> {
    let previous = read_issuer_cursor(transaction)?;
    let mut issuer = IndependentInstrumentIdIssuer::restore(previous, &[]);
    loop {
        let candidate = issuer.issue().map_err(invalid)?;
        match mapping_id_presence(transaction, candidate.as_instrument_id())? {
            IdPresence::Present => {}
            IdPresence::Absent => {
                match issuer.last_issued() {
                    PersistedIssuerCursor::LastIssued(next_seq) => {
                        persist_issuer_cursor(transaction, previous, next_seq)?;
                    }
                    PersistedIssuerCursor::Initial => return Err(rusqlite::Error::InvalidQuery),
                }
                return Ok(candidate);
            }
        }
    }
}

fn restore_domain_issuer(
    connection: &Connection,
) -> rusqlite::Result<IndependentInstrumentIdIssuer> {
    let cursor = read_issuer_cursor(connection)?;
    let mut statement = connection.prepare(sql::ALL_IDS)?;
    let existing = statement
        .query_map([], |row| {
            let raw: String = row.get(0)?;
            InstrumentId::try_new(&raw).map_err(invalid)
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(IndependentInstrumentIdIssuer::restore(cursor, &existing))
}

fn ensure_mapping(
    transaction: &Transaction<'_>,
    source: &InstrumentSource,
    candidate: IssuedInstrumentId,
) -> rusqlite::Result<Result<MappingAssignment, InstrumentMappingError>> {
    let mut ledger = load_for_source_or_id(transaction, source, candidate.as_instrument_id())?;
    let assignment = match ledger.ensure(source, candidate) {
        Ok(value) => value,
        Err(error) => return Ok(Err(error)),
    };
    match &assignment {
        MappingAssignment::Existing(_) => Ok(Ok(assignment)),
        MappingAssignment::Issued(issued_id) => {
            let inserted = transaction.execute(
                sql::INSERT,
                params![
                    issued_id.as_str(),
                    market_tag(source.market()),
                    source.symbol().as_str(),
                    source.isin().as_str(),
                    STATUS_ACTIVE,
                ],
            )?;
            if inserted != 1 {
                return Err(rusqlite::Error::InvalidQuery);
            }
            Ok(Ok(assignment))
        }
    }
}

fn retire_mapping(
    transaction: &Transaction<'_>,
    source: &InstrumentSource,
) -> rusqlite::Result<Result<InstrumentId, InstrumentMappingError>> {
    let ledger = load_for_source(transaction, source)?;
    let existing = match ledger.lookup(source) {
        Ok(record) => record,
        Err(error) => return Ok(Err(error)),
    };
    match existing.status() {
        InstrumentMappingStatus::Retired => Ok(Ok(existing.id().clone())),
        InstrumentMappingStatus::Superseded => Ok(Err(InstrumentMappingError::Superseded)),
        InstrumentMappingStatus::Active => {
            let updated = transaction.execute(
                sql::RETIRE,
                params![
                    existing.id().as_str(),
                    market_tag(source.market()),
                    source.symbol().as_str(),
                    source.isin().as_str(),
                ],
            )?;
            if updated != 1 {
                return Err(rusqlite::Error::InvalidQuery);
            }
            Ok(Ok(existing.id().clone()))
        }
    }
}

fn map_result<T>(
    outcome: Result<Result<T, InstrumentMappingError>, SystemPortError>,
) -> Result<T, MappingStoreError> {
    match outcome {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(error)) => Err(MappingStoreError::Mapping(error)),
        Err(error) => Err(MappingStoreError::Storage(error)),
    }
}

impl InstrumentIdIssuer for InstrumentIdIssuerStore<'_> {
    async fn issue(&self) -> Result<IssuedInstrumentId, SystemPortError> {
        self.database.write(issue_next).await
    }
}

impl InstrumentMappingRepository for InstrumentMappingStore<'_> {
    async fn resolve(&self, source: &InstrumentSource) -> Result<InstrumentId, MappingStoreError> {
        let owned_source = source.clone();
        map_result(
            self.database
                .read(move |connection| {
                    Ok(load_for_source(connection, &owned_source)?.resolve(&owned_source))
                })
                .await,
        )
    }

    async fn lookup(
        &self,
        source: &InstrumentSource,
    ) -> Result<InstrumentMappingRecord, MappingStoreError> {
        let owned_source = source.clone();
        map_result(
            self.database
                .read(move |connection| {
                    Ok(load_for_source(connection, &owned_source)?.lookup(&owned_source))
                })
                .await,
        )
    }

    async fn for_id(
        &self,
        id: &InstrumentId,
    ) -> Result<Vec<InstrumentMappingRecord>, MappingStoreError> {
        let owned_id = id.clone();
        map_result(
            self.database
                .read(move |connection| Ok(load_for_id(connection, &owned_id)?.for_id(&owned_id)))
                .await,
        )
    }

    async fn ensure(
        &self,
        source: &InstrumentSource,
        candidate: IssuedInstrumentId,
    ) -> Result<MappingAssignment, MappingStoreError> {
        let owned_source = source.clone();
        map_result(
            self.database
                .write(move |transaction| ensure_mapping(transaction, &owned_source, candidate))
                .await,
        )
    }

    async fn retire(&self, source: &InstrumentSource) -> Result<InstrumentId, MappingStoreError> {
        let owned_source = source.clone();
        map_result(
            self.database
                .write(move |transaction| retire_mapping(transaction, &owned_source))
                .await,
        )
    }
}
