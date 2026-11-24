use crate::sql::{order_rate as rate_sql, order_rate_tests as sql};
use crate::tests::{Fixture, clock, key, runtime, timestamp};
use crate::{Database, DatabasePath, OrderRateStore};
use application::{CheckOrderRateLimit, OrderRateRepository};
use domain::{
    HourlyOrderLimit, Nanosecond, OffsetSeconds, OrderCount, OrderRateDecision, Timestamp,
    UnixSeconds,
};

fn at(seconds: i64, nanos: u32, offset: i32) -> Timestamp {
    Timestamp::from_parts(
        UnixSeconds::new(seconds),
        Nanosecond::new(nanos),
        OffsetSeconds::new(offset),
    )
    .expect("시각")
}

struct FixedClock(Timestamp);

impl application::Clock for FixedClock {
    fn now(&self) -> Timestamp {
        self.0
    }
}

fn limit(count: u32) -> HourlyOrderLimit {
    HourlyOrderLimit::try_new(OrderCount::new(count)).expect("상한")
}

#[test]
fn hourly_limit_round_trips_and_survives_reopen() {
    let fixture = Fixture::new();
    let path = DatabasePath::new(fixture.path());
    let rt = runtime();
    let db = Database::open(&path, &key(), &clock()).expect("open");
    let store = OrderRateStore::new(&db);
    assert_eq!(rt.block_on(store.hourly_limit()).expect("empty"), None);
    rt.block_on(store.store_hourly_limit(limit(3)))
        .expect("store");
    rt.block_on(store.store_hourly_limit(limit(5)))
        .expect("replace");
    assert_eq!(
        rt.block_on(store.hourly_limit()).expect("limit"),
        Some(limit(5))
    );
    rt.block_on(db.close()).expect("close");
    let reopened = Database::open(&path, &key(), &clock()).expect("reopen");
    assert_eq!(
        rt.block_on(OrderRateStore::new(&reopened).hourly_limit())
            .expect("limit"),
        Some(limit(5))
    );
    rt.block_on(reopened.close()).expect("close");
}

#[test]
fn recent_submissions_keep_offset_and_respect_the_since_boundary() {
    let fixture = Fixture::new();
    let db = Database::open(&DatabasePath::new(fixture.path()), &key(), &clock()).expect("open");
    let rt = runtime();
    let store = OrderRateStore::new(&db);
    let stamps = [
        at(1_000, 0, 0),
        at(2_000, 499, 32_400),
        at(2_000, 500, 32_400),
        at(2_001, 0, -3_600),
        at(1_500, 0, 0),
    ];
    for stamp in stamps {
        rt.block_on(store.record_submission(stamp)).expect("record");
    }
    let recent = rt
        .block_on(store.recent_submission_times(at(2_000, 500, 0)))
        .expect("recent");
    assert_eq!(recent, vec![at(2_000, 500, 32_400), at(2_001, 0, -3_600)]);
    assert_eq!(
        recent
            .iter()
            .map(|stamp| *stamp.offset_seconds().as_i32())
            .collect::<Vec<_>>(),
        vec![32_400_i32, -3_600_i32]
    );
    assert_eq!(
        rt.block_on(store.recent_submission_times(at(0, 0, 0)))
            .expect("all")
            .len(),
        5
    );
    rt.block_on(db.close()).expect("close");
}

#[test]
fn check_order_rate_limit_uses_stored_limit_and_submissions() {
    let fixture = Fixture::new();
    let db = Database::open(&DatabasePath::new(fixture.path()), &key(), &clock()).expect("open");
    let rt = runtime();
    let store = OrderRateStore::new(&db);
    let now = FixedClock(at(10_000, 0, 0));
    assert_eq!(
        rt.block_on(CheckOrderRateLimit::new(&store, &now).evaluate())
            .expect("empty"),
        OrderRateDecision::Allowed {
            remaining: OrderCount::new(10)
        }
    );
    rt.block_on(store.store_hourly_limit(limit(2)))
        .expect("limit");
    for stamp in [at(6_000, 0, 0), at(7_000, 0, 0), at(8_000, 0, 0)] {
        rt.block_on(store.record_submission(stamp)).expect("record");
    }
    assert_eq!(
        rt.block_on(CheckOrderRateLimit::new(&store, &now).evaluate())
            .expect("exceeded"),
        OrderRateDecision::Exceeded {
            limit: limit(2),
            recent_count: OrderCount::new(2),
            retry_at: at(10_600, 0, 0),
        }
    );
    rt.block_on(store.record_submission(at(10_500, 0, 0)))
        .expect("future");
    assert_eq!(
        rt.block_on(CheckOrderRateLimit::new(&store, &now).evaluate())
            .expect("rollback"),
        OrderRateDecision::ClockRollback
    );
    rt.block_on(db.close()).expect("close");
}

fn count(db: &Database) -> i64 {
    runtime()
        .block_on(db.read(|connection| {
            connection.query_row(sql::SUBMISSION_COUNT, [], |row| row.get::<_, i64>(0))
        }))
        .expect("count")
}

#[test]
fn order_rate_tables_reject_rewrites_deletes_and_invalid_rows() {
    let fixture = Fixture::new();
    let db = Database::open(&DatabasePath::new(fixture.path()), &key(), &clock()).expect("open");
    let rt = runtime();
    let store = OrderRateStore::new(&db);
    rt.block_on(store.store_hourly_limit(limit(4)))
        .expect("limit");
    rt.block_on(store.record_submission(at(1, 0, 0)))
        .expect("record");
    for statement in [
        sql::UPDATE_SUBMISSION,
        sql::DELETE_SUBMISSION,
        sql::DELETE_LIMIT,
        sql::SECOND_LIMIT_ROW,
        sql::OUT_OF_RANGE_LIMIT,
    ] {
        assert!(
            rt.block_on(db.write(move |transaction| transaction.execute_batch(statement)))
                .is_err(),
            "{statement}"
        );
    }
    assert_eq!(count(&db), 1);
    assert_eq!(
        rt.block_on(store.hourly_limit()).expect("limit"),
        Some(limit(4))
    );
    rt.block_on(db.close()).expect("close");
}

#[test]
fn order_rate_sql_constants_match_the_migration_and_prepare() {
    assert_eq!(
        std::fs::read_to_string("migrations/0015_order_rate_limit.sql").expect("file"),
        rate_sql::CREATE
    );
    let mut connection = rusqlite::Connection::open_in_memory().expect("memory");
    crate::migration::run(&mut connection, timestamp()).expect("migrate");
    for statement in [
        rate_sql::LIMIT,
        rate_sql::UPSERT_LIMIT,
        rate_sql::INSERT_SUBMISSION,
        rate_sql::SUBMISSIONS_SINCE,
        sql::SUBMISSION_COUNT,
    ] {
        connection.prepare(statement).expect(statement);
    }
}
