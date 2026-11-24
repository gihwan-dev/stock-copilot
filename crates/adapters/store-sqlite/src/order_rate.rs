use crate::estimates::codec::invalid;
use crate::{Database, sql::order_rate as sql};
use application::{OrderRateRepository, SystemPortError, WriteCompleted};
use domain::{HourlyOrderLimit, Nanosecond, OffsetSeconds, OrderCount, Timestamp, UnixSeconds};
use rusqlite::OptionalExtension;
use std::future::Future;

#[derive(Debug)]
pub struct OrderRateStore<'database> {
    database: &'database Database,
}

impl<'database> OrderRateStore<'database> {
    #[must_use]
    pub const fn new(database: &'database Database) -> Self {
        Self { database }
    }

    pub async fn store_hourly_limit(
        &self,
        limit: HourlyOrderLimit,
    ) -> Result<WriteCompleted, SystemPortError> {
        let raw = i64::from(*limit.limit().as_u32());
        self.database
            .write(
                move |transaction| match transaction.execute(sql::UPSERT_LIMIT, [raw])? {
                    1 => Ok(WriteCompleted),
                    _ => Err(invalid(())),
                },
            )
            .await
    }

    pub async fn record_submission(
        &self,
        at: Timestamp,
    ) -> Result<WriteCompleted, SystemPortError> {
        let seconds = *at.utc_seconds().as_i64();
        let nanos = i64::from(*at.nanosecond().as_u32());
        let offset = i64::from(*at.offset_seconds().as_i32());
        self.database
            .write(move |transaction| {
                match transaction.execute(
                    sql::INSERT_SUBMISSION,
                    rusqlite::params![seconds, nanos, offset],
                )? {
                    1 => Ok(WriteCompleted),
                    _ => Err(invalid(())),
                }
            })
            .await
    }
}

fn decode_limit(raw: i64) -> rusqlite::Result<HourlyOrderLimit> {
    let count = u32::try_from(raw).map_err(invalid)?;
    HourlyOrderLimit::try_new(OrderCount::new(count)).map_err(invalid)
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

impl OrderRateRepository for OrderRateStore<'_> {
    fn hourly_limit(
        &self,
    ) -> impl Future<Output = Result<Option<HourlyOrderLimit>, SystemPortError>> + Send {
        self.database.read(|connection| {
            connection
                .query_row(sql::LIMIT, [], |row| row.get::<_, i64>(0))
                .optional()?
                .map(decode_limit)
                .transpose()
        })
    }

    fn recent_submission_times(
        &self,
        since: Timestamp,
    ) -> impl Future<Output = Result<Vec<Timestamp>, SystemPortError>> + Send {
        let seconds = *since.utc_seconds().as_i64();
        let nanos = i64::from(*since.nanosecond().as_u32());
        self.database.read(move |connection| {
            let mut statement = connection.prepare(sql::SUBMISSIONS_SINCE)?;
            let rows = statement.query_map(rusqlite::params![seconds, nanos], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            })?;
            let mut times = Vec::new();
            for row in rows {
                let (stored_seconds, stored_nanos, offset) = row?;
                times.push(decode_time(stored_seconds, stored_nanos, offset)?);
            }
            Ok(times)
        })
    }
}
