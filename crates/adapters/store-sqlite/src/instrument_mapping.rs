use crate::{Database, sql::instrument_mapping as sql};
use application::{InstrumentMappingRepository, MappingStoreError, SystemPortError};
use domain::{
    InstrumentId, InstrumentMappingError, InstrumentMappingRecord, InstrumentMappingStatus,
    InstrumentMappings, InstrumentSource, InstrumentSourceConflict, IsinCode, KnownMarket,
    MappingAssignment, Symbol, instrument_source_conflicts,
};
use rusqlite::{Connection, Row, Transaction, params};
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
        STATUS_RETIRED => Ok(InstrumentMappingStatus::Retired),
        _ => Err(rusqlite::Error::InvalidQuery),
    }
}

fn decode_row(row: &Row<'_>) -> rusqlite::Result<InstrumentMappingRecord> {
    let id_raw: String = row.get(0)?;
    let market_raw: String = row.get(1)?;
    let symbol_raw: String = row.get(2)?;
    let isin_raw: String = row.get(3)?;
    let status_raw: String = row.get(4)?;
    let id = InstrumentId::try_new(&id_raw).map_err(invalid)?;
    let market = decode_market(&market_raw)?;
    let symbol = Symbol::try_new(&symbol_raw).map_err(invalid)?;
    let isin = IsinCode::try_new(&isin_raw).map_err(invalid)?;
    let status = decode_status(&status_raw)?;
    Ok(InstrumentMappingRecord::new(
        InstrumentSource::new(market, symbol, isin),
        id,
        status,
    ))
}

fn expected_schema_sql() -> BTreeSet<String> {
    sql::CREATE
        .split(";\n")
        .map(str::trim)
        .filter(|statement| !statement.is_empty())
        .map(|statement| statement.strip_suffix(';').unwrap_or(statement).to_owned())
        .collect()
}

pub(crate) fn validate_schema(connection: &Connection) -> rusqlite::Result<()> {
    let mut statement = connection.prepare(sql::SCHEMA_OBJECTS)?;
    let actual_rows = statement
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let expected = expected_schema_sql();
    let actual: BTreeSet<String> = actual_rows
        .iter()
        .map(|entry| entry.trim().to_owned())
        .collect();
    if actual_rows.len() != expected.len() || actual != expected {
        return Err(rusqlite::Error::InvalidQuery);
    }
    Ok(())
}

pub(crate) fn validate_ledger(connection: &Connection) -> rusqlite::Result<()> {
    let mut statement = connection.prepare(sql::ALL)?;
    let records = statement
        .query_map([], decode_row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut seen_ids = BTreeSet::new();
    let mut seen_sources = BTreeSet::new();
    let mut sources = Vec::with_capacity(records.len());
    for record in records {
        if !seen_ids.insert(record.id().clone()) {
            return Err(rusqlite::Error::InvalidQuery);
        }
        let exact_source = (
            record.source().market(),
            record.source().symbol().clone(),
            record.source().isin().clone(),
        );
        if !seen_sources.insert(exact_source) {
            return Err(rusqlite::Error::InvalidQuery);
        }
        sources.push(record.source().clone());
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

pub(crate) fn validate_on_open(connection: &Connection) -> rusqlite::Result<()> {
    validate_schema(connection)?;
    validate_ledger(connection)
}

fn restore_records(records: Vec<InstrumentMappingRecord>) -> rusqlite::Result<InstrumentMappings> {
    let mut ledger = InstrumentMappings::default();
    for record in records {
        match ledger.ensure(record.source(), record.id()) {
            Ok(MappingAssignment::Issued(_)) => {}
            Ok(MappingAssignment::Existing(_))
            | Err(
                InstrumentMappingError::Missing
                | InstrumentMappingError::IdentityUncertain
                | InstrumentMappingError::Retired
                | InstrumentMappingError::IdAlreadyUsed,
            ) => {
                return Err(rusqlite::Error::InvalidQuery);
            }
        }
        match record.status() {
            InstrumentMappingStatus::Active => {}
            InstrumentMappingStatus::Retired => {
                let _retired: InstrumentId = ledger.retire(record.source()).map_err(invalid)?;
            }
        }
    }
    Ok(ledger)
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

fn ensure_mapping(
    transaction: &Transaction<'_>,
    source: &InstrumentSource,
    candidate: &InstrumentId,
) -> rusqlite::Result<Result<MappingAssignment, InstrumentMappingError>> {
    let mut ledger = load_for_source_or_id(transaction, source, candidate)?;
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
        candidate: &InstrumentId,
    ) -> Result<MappingAssignment, MappingStoreError> {
        let owned_source = source.clone();
        let owned_candidate = candidate.clone();
        map_result(
            self.database
                .write(move |transaction| {
                    ensure_mapping(transaction, &owned_source, &owned_candidate)
                })
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
