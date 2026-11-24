use crate::estimates_tests::eligible_task;
use crate::sql::{watchlist as watch_sql, watchlist_tests as sql};
use crate::tests::{Fixture, clock, key, runtime, timestamp};
use crate::{Database, DatabasePath, EstimateStore, SignalSkipStore, WatchlistStore};
use application::{
    AiRunId, EstimateAssignmentRepository, EstimateCommitOutcome, EstimateSubmissionRepository,
    SignalSkip, SignalSkipRepository, SkipReason, SystemFailure, Watchlist, WatchlistEntry,
    WatchlistRepository, WatchlistStoreOutcome,
};
use domain::{
    CalibrationEstimateId, EstimateAssignments, EstimateConsumption, InstrumentId, Nanosecond,
    OffsetSeconds, Timestamp, UnixSeconds,
};

fn at(seconds: i64) -> Timestamp {
    Timestamp::from_parts(
        UnixSeconds::new(seconds),
        Nanosecond::new(3),
        OffsetSeconds::new(32_400),
    )
    .expect("시각")
}

fn id(raw: &str) -> InstrumentId {
    InstrumentId::try_new(raw).expect("종목")
}

fn watchlist(names: &[&str]) -> Watchlist {
    Watchlist::try_from_entries(
        names
            .iter()
            .zip(1_i64..)
            .map(|(name, seconds)| WatchlistEntry::new(id(name), at(seconds)))
            .collect(),
    )
    .expect("목록")
}

#[test]
fn watchlist_replaces_with_compare_and_survives_reopen() {
    let fixture = Fixture::new();
    let path = DatabasePath::new(fixture.path());
    let rt = runtime();
    let db = Database::open(&path, &key(), &clock()).expect("open");
    let store = WatchlistStore::new(&db);
    assert_eq!(
        rt.block_on(store.load()).expect("empty"),
        Watchlist::empty()
    );
    let first = watchlist(&["instrument-b", "instrument-a"]);
    assert_eq!(
        rt.block_on(store.store(&Watchlist::empty(), &first))
            .expect("store"),
        WatchlistStoreOutcome::Stored
    );
    assert_eq!(
        rt.block_on(store.store(&Watchlist::empty(), &watchlist(&["instrument-c"])))
            .expect("stale"),
        WatchlistStoreOutcome::Conflict
    );
    let second = watchlist(&["instrument-a"]);
    assert_eq!(
        rt.block_on(store.store(&first, &second)).expect("shrink"),
        WatchlistStoreOutcome::Stored
    );
    rt.block_on(db.close()).expect("close");
    let reopened = Database::open(&path, &key(), &clock()).expect("reopen");
    assert_eq!(
        rt.block_on(WatchlistStore::new(&reopened).load())
            .expect("load"),
        second
    );
    rt.block_on(reopened.close()).expect("close");
}

#[test]
fn signal_skips_append_and_keep_reasons() {
    let fixture = Fixture::new();
    let path = DatabasePath::new(fixture.path());
    let rt = runtime();
    let db = Database::open(&path, &key(), &clock()).expect("open");
    let store = SignalSkipStore::new(&db);
    let skips = vec![
        SignalSkip {
            instrument: id("instrument-a"),
            reason: Some(SkipReason::try_new("실적 발표 뒤에 다시 본다").expect("사유")),
            skipped_at: at(10),
        },
        SignalSkip {
            instrument: id("instrument-a"),
            reason: None,
            skipped_at: at(11),
        },
    ];
    for skip in &skips {
        rt.block_on(store.record_skip(skip)).expect("skip");
    }
    rt.block_on(db.close()).expect("close");
    let reopened = Database::open(&path, &key(), &clock()).expect("reopen");
    assert_eq!(
        rt.block_on(SignalSkipStore::new(&reopened).skips())
            .expect("skips"),
        skips
    );
    rt.block_on(reopened.close()).expect("close");
}

fn issue_two(
    empty: &EstimateAssignments<AiRunId>,
    run: &AiRunId,
    first: &CalibrationEstimateId,
) -> EstimateAssignments<AiRunId> {
    empty
        .issue(
            first.clone(),
            run.clone(),
            eligible_task("instrument-a", "005930", "102"),
        )
        .expect("issue")
        .issue(
            CalibrationEstimateId::try_new("task-2").expect("과제"),
            run.clone(),
            eligible_task("instrument-b", "000660", "103"),
        )
        .expect("issue")
}

#[test]
fn estimate_assignments_commit_issue_and_consume_with_conflict_check() {
    let fixture = Fixture::new();
    let db = Database::open(&DatabasePath::new(fixture.path()), &key(), &clock()).expect("open");
    let rt = runtime();
    let store = EstimateStore::new(&db);
    let run = AiRunId::try_new("alpha").expect("실행");
    let first = CalibrationEstimateId::try_new("task-1").expect("과제");
    let empty = rt
        .block_on(EstimateAssignmentRepository::load_assignments(&store))
        .expect("empty");
    assert_eq!(empty, EstimateAssignments::default());
    let issued = issue_two(&empty, &run, &first);
    assert_eq!(
        rt.block_on(store.commit_assignments(&empty, &issued))
            .expect("commit"),
        EstimateCommitOutcome::Committed
    );
    assert_eq!(
        rt.block_on(store.commit_assignments(&empty, &issued))
            .expect("stale"),
        EstimateCommitOutcome::Conflict
    );
    let consumed = issued.consume(&run, &[first]).expect("consume");
    assert_eq!(
        rt.block_on(store.commit_assignments(&issued, &consumed))
            .expect("consume"),
        EstimateCommitOutcome::Committed
    );
    let loaded = rt.block_on(store.assignments()).expect("load");
    assert_eq!(loaded, consumed);
    assert_eq!(
        loaded
            .entries()
            .iter()
            .map(domain::IssuedEstimateTask::consumption)
            .collect::<Vec<_>>(),
        vec![
            EstimateConsumption::Consumed,
            EstimateConsumption::Available
        ]
    );
    assert_eq!(
        rt.block_on(store.commit_assignments(&consumed, &issued))
            .expect_err("revive")
            .failure(),
        SystemFailure::RepositoryIntegrity
    );
    assert_eq!(rt.block_on(store.assignments()).expect("load"), consumed);
    rt.block_on(db.close()).expect("close");
}

#[test]
fn watchlist_and_skip_tables_reject_invalid_rows() {
    let fixture = Fixture::new();
    let db = Database::open(&DatabasePath::new(fixture.path()), &key(), &clock()).expect("open");
    let rt = runtime();
    rt.block_on(WatchlistStore::new(&db).store(&Watchlist::empty(), &watchlist(&["instrument-a"])))
        .expect("watch");
    rt.block_on(SignalSkipStore::new(&db).record_skip(&SignalSkip {
        instrument: id("instrument-a"),
        reason: None,
        skipped_at: at(1),
    }))
    .expect("skip");
    for statement in [
        sql::UPDATE_SKIP,
        sql::DELETE_SKIP,
        sql::MULTILINE_SKIP,
        sql::DUPLICATE_WATCH,
        sql::OVER_CAPACITY,
    ] {
        assert!(
            rt.block_on(db.write(move |transaction| transaction.execute_batch(statement)))
                .is_err(),
            "{statement}"
        );
    }
    rt.block_on(db.write(|transaction| transaction.execute_batch(sql::CORRUPT_WATCH)))
        .expect("corrupt");
    assert_eq!(
        rt.block_on(WatchlistStore::new(&db).load())
            .expect_err("corrupt")
            .failure(),
        SystemFailure::RepositoryIntegrity
    );
    rt.block_on(db.close()).expect("close");
}

#[test]
fn watchlist_sql_constants_match_the_migration_and_prepare() {
    assert_eq!(
        std::fs::read_to_string("migrations/0018_watchlist_and_signal_skips.sql").expect("file"),
        watch_sql::CREATE
    );
    let mut connection = rusqlite::Connection::open_in_memory().expect("memory");
    crate::migration::run(&mut connection, timestamp()).expect("migrate");
    for statement in [
        watch_sql::WATCHLIST,
        watch_sql::CLEAR_WATCHLIST,
        watch_sql::INSERT_WATCH,
        watch_sql::INSERT_SKIP,
        watch_sql::SKIPS,
    ] {
        connection.prepare(statement).expect(statement);
    }
}
