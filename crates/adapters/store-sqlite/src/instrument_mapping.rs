use crate::{Database, sql::instrument_mapping as sql};
use application::{
    InstrumentIdIssuer, InstrumentMappingRepository, MappingStoreError, SystemPortError,
};
use domain::{
    IndependentInstrumentIdIssuer, InstrumentId, InstrumentMappingError, InstrumentMappingRecord,
    InstrumentMappingStatus, InstrumentSource, InstrumentSourceConflict, IsinCode,
    IssuedInstrumentId, IssuerSequence, KnownMarket, MappingAssignment, PersistedIssuerCursor,
    Symbol, instrument_source_conflicts,
};
use rusqlite::{Connection, OptionalExtension, Row, Transaction, params};
use std::collections::BTreeSet;

const STATUS_ACTIVE: &str = "active";
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
}

enum IdPresence {
    Present,
    Absent,
}

#[derive(Clone, Copy)]
enum StoredStatus {
    Active,
    Retired,
}

impl StoredStatus {
    const fn into_domain(self) -> InstrumentMappingStatus {
        match self {
            Self::Active => InstrumentMappingStatus::Active,
            Self::Retired => InstrumentMappingStatus::Retired,
        }
    }
}

struct StoredRecord {
    source: InstrumentSource,
    id: InstrumentId,
    status: StoredStatus,
}

impl StoredRecord {
    fn to_domain(&self) -> InstrumentMappingRecord {
        InstrumentMappingRecord::new(
            self.source.clone(),
            self.id.clone(),
            self.status.into_domain(),
        )
    }

    fn into_domain(self) -> InstrumentMappingRecord {
        InstrumentMappingRecord::new(self.source, self.id, self.status.into_domain())
    }
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

fn decode_status(raw: &str) -> rusqlite::Result<StoredStatus> {
    match raw {
        STATUS_ACTIVE => Ok(StoredStatus::Active),
        STATUS_RETIRED => Ok(StoredStatus::Retired),
        _ => Err(rusqlite::Error::InvalidQuery),
    }
}

fn decode_row(row: &Row<'_>) -> rusqlite::Result<StoredRecord> {
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

    Ok(StoredRecord {
        source: InstrumentSource::new(market, symbol, isin),
        id,
        status,
    })
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

pub(crate) fn read_issuer_cursor(
    connection: &Connection,
) -> rusqlite::Result<PersistedIssuerCursor> {
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

fn validate_records(records: &[StoredRecord]) -> rusqlite::Result<()> {
    let mut seen_ids = BTreeSet::new();
    let mut seen_sources = BTreeSet::new();
    let mut sources = Vec::with_capacity(records.len());
    for record in records {
        if !seen_ids.insert(record.id.clone()) {
            return Err(rusqlite::Error::InvalidQuery);
        }
        let exact_source = (
            record.source.market(),
            record.source.symbol().clone(),
            record.source.isin().clone(),
        );
        if !seen_sources.insert(exact_source) {
            return Err(rusqlite::Error::InvalidQuery);
        }
        sources.push(record.source.clone());
    }
    for conflict in instrument_source_conflicts(&sources) {
        match conflict {
            InstrumentSourceConflict::Distinct => {}
            InstrumentSourceConflict::IdentityUncertain => {
                return Err(rusqlite::Error::InvalidQuery);
            }
        }
    }
    Ok(())
}

pub(crate) fn validate_ledger(connection: &Connection) -> rusqlite::Result<()> {
    let mut statement = connection.prepare(sql::ALL)?;
    let records = statement
        .query_map([], decode_row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    validate_records(&records)
}

pub(crate) fn validate_on_open(connection: &Connection) -> rusqlite::Result<()> {
    validate_schema(connection)?;
    validate_ledger(connection)?;
    validate_issuer_cursor(connection)
}

fn load_for_source(
    connection: &Connection,
    source: &InstrumentSource,
) -> rusqlite::Result<Vec<StoredRecord>> {
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
    validate_records(&records)?;
    Ok(records)
}

fn load_for_source_or_id(
    connection: &Connection,
    source: &InstrumentSource,
    candidate: &InstrumentId,
) -> rusqlite::Result<Vec<StoredRecord>> {
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
    validate_records(&records)?;
    Ok(records)
}

fn load_for_id(connection: &Connection, id: &InstrumentId) -> rusqlite::Result<Vec<StoredRecord>> {
    let mut statement = connection.prepare(sql::BY_ID)?;
    let records = statement
        .query_map(params![id.as_str()], decode_row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    validate_records(&records)?;
    Ok(records)
}

fn lookup_in_candidates<'a>(
    records: &'a [StoredRecord],
    source: &InstrumentSource,
) -> Result<&'a StoredRecord, InstrumentMappingError> {
    if let Some(entry) = records.iter().find(|entry| entry.source == *source) {
        return Ok(entry);
    }
    let mut sources: Vec<InstrumentSource> =
        records.iter().map(|entry| entry.source.clone()).collect();
    sources.push(source.clone());
    match instrument_source_conflicts(&sources).last() {
        Some(InstrumentSourceConflict::IdentityUncertain) => {
            Err(InstrumentMappingError::IdentityUncertain)
        }
        Some(InstrumentSourceConflict::Distinct) | None => Err(InstrumentMappingError::Missing),
    }
}

fn resolve_in_candidates(
    records: &[StoredRecord],
    source: &InstrumentSource,
) -> Result<InstrumentId, InstrumentMappingError> {
    let entry = lookup_in_candidates(records, source)?;
    match entry.status {
        StoredStatus::Active => Ok(entry.id.clone()),
        StoredStatus::Retired => Err(InstrumentMappingError::Retired),
    }
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

fn ensure_mapping(
    transaction: &Transaction<'_>,
    source: &InstrumentSource,
    candidate: &IssuedInstrumentId,
) -> rusqlite::Result<Result<MappingAssignment, InstrumentMappingError>> {
    let records = load_for_source_or_id(transaction, source, candidate.as_instrument_id())?;
    match resolve_in_candidates(&records, source) {
        Ok(existing_id) => return Ok(Ok(MappingAssignment::Existing(existing_id))),
        Err(InstrumentMappingError::Missing) => {}
        Err(error) => return Ok(Err(error)),
    }
    if records
        .iter()
        .any(|entry| entry.id == *candidate.as_instrument_id())
    {
        return Ok(Err(InstrumentMappingError::IdAlreadyUsed));
    }
    let issued_id = candidate.as_instrument_id().clone();
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
    Ok(Ok(MappingAssignment::Issued(issued_id)))
}

fn retire_mapping(
    transaction: &Transaction<'_>,
    source: &InstrumentSource,
) -> rusqlite::Result<Result<InstrumentId, InstrumentMappingError>> {
    let records = load_for_source(transaction, source)?;
    let existing = match lookup_in_candidates(&records, source) {
        Ok(record) => record,
        Err(error) => return Ok(Err(error)),
    };
    match existing.status {
        StoredStatus::Retired => Ok(Ok(existing.id.clone())),
        StoredStatus::Active => {
            let updated = transaction.execute(
                sql::RETIRE,
                params![
                    existing.id.as_str(),
                    market_tag(source.market()),
                    source.symbol().as_str(),
                    source.isin().as_str(),
                ],
            )?;
            if updated != 1 {
                return Err(rusqlite::Error::InvalidQuery);
            }
            Ok(Ok(existing.id.clone()))
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
                    let records = load_for_source(connection, &owned_source)?;
                    Ok(resolve_in_candidates(&records, &owned_source))
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
                    let records = load_for_source(connection, &owned_source)?;
                    Ok(lookup_in_candidates(&records, &owned_source).map(StoredRecord::to_domain))
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
                .read(move |connection| {
                    let records = load_for_id(connection, &owned_id)?;
                    if records.is_empty() {
                        return Ok(Err(InstrumentMappingError::Missing));
                    }
                    Ok(Ok(records
                        .into_iter()
                        .map(StoredRecord::into_domain)
                        .collect()))
                })
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
                .write(move |transaction| ensure_mapping(transaction, &owned_source, &candidate))
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
