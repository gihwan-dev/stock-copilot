use crate::{Database, sql::instrument_mapping as sql};
use application::{InstrumentMappingRepository, MappingStoreError, SystemPortError};
use domain::{
    InstrumentId, InstrumentMappingError, InstrumentMappingRecord, InstrumentMappingStatus,
    InstrumentMappings, InstrumentSource, IsinCode, KnownMarket, MappingAssignment, Symbol,
};
use rusqlite::{Connection, Row, Transaction, params};

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

pub(crate) fn restore(connection: &Connection) -> rusqlite::Result<InstrumentMappings> {
    let mut statement = connection.prepare(sql::ALL)?;
    let rows = statement
        .query_map([], decode_row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut ledger = InstrumentMappings::default();
    for record in rows {
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

fn ensure_mapping(
    transaction: &Transaction<'_>,
    source: &InstrumentSource,
    candidate: &InstrumentId,
) -> rusqlite::Result<Result<MappingAssignment, InstrumentMappingError>> {
    let mut ledger = restore(transaction)?;
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
    let ledger = restore(transaction)?;
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
                .read(move |connection| Ok(restore(connection)?.resolve(&owned_source)))
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
                .read(move |connection| Ok(restore(connection)?.lookup(&owned_source)))
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
                .read(move |connection| Ok(restore(connection)?.for_id(&owned_id)))
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
