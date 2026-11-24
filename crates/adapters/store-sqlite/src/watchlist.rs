use crate::estimates::codec::invalid;
use crate::{Database, sql::watchlist as sql};
use application::{
    SignalSkip, SignalSkipRepository, SkipReason, SystemPortError, Watchlist, WatchlistEntry,
    WatchlistRepository, WatchlistStoreOutcome, WriteCompleted,
};
use domain::{InstrumentId, Nanosecond, OffsetSeconds, Timestamp, UnixSeconds};
use rusqlite::Connection;
use std::future::Future;

fn time_parts(at: Timestamp) -> (i64, i64, i64) {
    (
        *at.utc_seconds().as_i64(),
        i64::from(*at.nanosecond().as_u32()),
        i64::from(*at.offset_seconds().as_i32()),
    )
}

fn decode_time(seconds: i64, raw_nanos: i64, raw_offset: i64) -> rusqlite::Result<Timestamp> {
    let nanos = u32::try_from(raw_nanos).map_err(invalid)?;
    let offset = i32::try_from(raw_offset).map_err(invalid)?;
    Timestamp::from_parts(
        UnixSeconds::new(seconds),
        Nanosecond::new(nanos),
        OffsetSeconds::new(offset),
    )
    .map_err(invalid)
}

fn load_watchlist(connection: &Connection) -> rusqlite::Result<Watchlist> {
    let mut statement = connection.prepare(sql::WATCHLIST)?;
    let entries = statement
        .query_map([], |row| {
            let instrument = InstrumentId::try_new(&row.get::<_, String>(0)?).map_err(invalid)?;
            let added_at = decode_time(row.get(1)?, row.get(2)?, row.get(3)?)?;
            Ok(WatchlistEntry::new(instrument, added_at))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Watchlist::try_from_entries(entries).map_err(invalid)
}

#[derive(Debug)]
pub struct WatchlistStore<'database> {
    database: &'database Database,
}

impl<'database> WatchlistStore<'database> {
    #[must_use]
    pub const fn new(database: &'database Database) -> Self {
        Self { database }
    }
}

impl WatchlistRepository for WatchlistStore<'_> {
    fn load(&self) -> impl Future<Output = Result<Watchlist, SystemPortError>> + Send {
        self.database.read(load_watchlist)
    }

    fn store(
        &self,
        previous: &Watchlist,
        next: &Watchlist,
    ) -> impl Future<Output = Result<WatchlistStoreOutcome, SystemPortError>> + Send {
        let expected = previous.clone();
        let replacement = next.clone();
        self.database.write(move |transaction| {
            if load_watchlist(transaction)? != expected {
                return Ok(WatchlistStoreOutcome::Conflict);
            }
            transaction.execute(sql::CLEAR_WATCHLIST, [])?;
            for (position, entry) in replacement.entries().iter().enumerate() {
                let (seconds, nanos, offset) = time_parts(entry.added_at());
                transaction.execute(
                    sql::INSERT_WATCH,
                    rusqlite::params![
                        i64::try_from(position).map_err(invalid)?,
                        entry.instrument().as_str(),
                        seconds,
                        nanos,
                        offset
                    ],
                )?;
            }
            Ok(WatchlistStoreOutcome::Stored)
        })
    }
}

#[derive(Debug)]
pub struct SignalSkipStore<'database> {
    database: &'database Database,
}

impl<'database> SignalSkipStore<'database> {
    #[must_use]
    pub const fn new(database: &'database Database) -> Self {
        Self { database }
    }

    pub async fn skips(&self) -> Result<Vec<SignalSkip>, SystemPortError> {
        self.database
            .read(|connection| {
                let mut statement = connection.prepare(sql::SKIPS)?;
                statement
                    .query_map([], |row| {
                        let instrument =
                            InstrumentId::try_new(&row.get::<_, String>(0)?).map_err(invalid)?;
                        let reason = match row.get::<_, Option<String>>(1)? {
                            Some(raw) => Some(
                                SkipReason::try_new(&raw)
                                    .map_err(invalid)?
                                    .ok_or_else(|| invalid(()))?,
                            ),
                            None => None,
                        };
                        let skipped_at = decode_time(row.get(2)?, row.get(3)?, row.get(4)?)?;
                        Ok(SignalSkip {
                            instrument,
                            reason,
                            skipped_at,
                        })
                    })?
                    .collect()
            })
            .await
    }
}

impl SignalSkipRepository for SignalSkipStore<'_> {
    fn record_skip(
        &self,
        skip: &SignalSkip,
    ) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send {
        let instrument = skip.instrument.as_str().to_owned();
        let reason = skip.reason.as_ref().map(|value| value.as_str().to_owned());
        let (seconds, nanos, offset) = time_parts(skip.skipped_at);
        self.database.write(move |transaction| {
            match transaction.execute(
                sql::INSERT_SKIP,
                rusqlite::params![instrument, reason, seconds, nanos, offset],
            )? {
                1 => Ok(WriteCompleted),
                _ => Err(invalid(())),
            }
        })
    }
}
