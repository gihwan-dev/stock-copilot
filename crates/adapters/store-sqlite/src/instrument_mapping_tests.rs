use crate::{
    Database, DatabasePath, InstrumentMappingStore,
    sql::{instrument_mapping as sql, instrument_mapping_tests as fixtures},
    tests::{Fixture, clock, key, runtime},
};
use application::{
    InstrumentMappingRepository, MappingStoreError, RetryDelay, SystemFailure, SystemPort,
    SystemPortError,
};
use domain::{
    InstrumentId, InstrumentMappingError, InstrumentMappingRecord, InstrumentMappingStatus,
    InstrumentSource, IsinCode, KnownMarket, MappingAssignment, Symbol,
};
use rusqlite::params;
use std::{sync::mpsc, time::Duration};

type RawRow = (
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    &'static str,
);

fn source(market: KnownMarket, symbol: &str, isin: &str) -> InstrumentSource {
    InstrumentSource::new(
        market,
        Symbol::try_new(symbol).expect("symbol"),
        IsinCode::try_new(isin).expect("isin"),
    )
}

fn id(value: &str) -> InstrumentId {
    InstrumentId::try_new(value).expect("instrument id")
}

fn open_fixture(fixture: &Fixture) -> Database {
    Database::open(&DatabasePath::new(fixture.path()), &key(), &clock()).expect("open database")
}

fn integrity_error() -> MappingStoreError {
    MappingStoreError::Storage(SystemPortError::new(
        SystemPort::Repository,
        SystemFailure::RepositoryIntegrity,
    ))
}

fn market_from_tag(tag: &str) -> KnownMarket {
    match tag {
        "KR" => KnownMarket::Kr,
        "US" => KnownMarket::Us,
        other => panic!("unexpected market tag: {other}"),
    }
}

fn insert_row_expect_err(
    rt: &tokio::runtime::Runtime,
    db: &Database,
    statement: &'static str,
    row: (&str, &str, &str, &str, &str),
    reason: &str,
) {
    let owned_id = row.0.to_owned();
    let owned_market = row.1.to_owned();
    let owned_sym = row.2.to_owned();
    let owned_isin = row.3.to_owned();
    let owned_status = row.4.to_owned();
    rt.block_on(db.write(move |tx| {
        tx.execute(
            statement,
            params![owned_id, owned_market, owned_sym, owned_isin, owned_status],
        )?;
        Ok(())
    }))
    .expect_err(reason);
}

fn assert_all_ops_fail_integrity(
    rt: &tokio::runtime::Runtime,
    db: &Database,
    probe: (&InstrumentSource, &InstrumentId),
    fresh: (&InstrumentSource, &InstrumentId),
) {
    let store = InstrumentMappingStore::new(db);
    assert_eq!(rt.block_on(store.resolve(probe.0)), Err(integrity_error()));
    assert_eq!(rt.block_on(store.lookup(probe.0)), Err(integrity_error()));
    assert_eq!(rt.block_on(store.for_id(probe.1)), Err(integrity_error()));
    assert_eq!(
        rt.block_on(store.ensure(fresh.0, fresh.1)),
        Err(integrity_error())
    );
    assert_eq!(rt.block_on(store.retire(probe.0)), Err(integrity_error()));
}

fn corrupt_row_sets() -> [&'static [RawRow]; 13] {
    [
        &[
            ("id-1", "KR", "005930", "KR7005930003", "active"),
            ("id-1", "KR", "005930", "KR7005930003", "active"),
        ],
        &[
            ("id-1", "KR", "005930", "KR7005930003", "active"),
            ("id-2", "KR", "005930", "KR7005930003", "active"),
        ],
        &[
            ("id-1", "KR", "005930", "KR7005930003", "active"),
            ("id-1", "KR", "005930", "KR7005930003", "retired"),
        ],
        &[
            ("id-1", "KR", "005930", "KR7005930003", "retired"),
            ("id-1", "KR", "005930", "KR7005930003", "retired"),
        ],
        &[
            ("id-1", "US", "AAPL", "US0378331005", "active"),
            ("id-2", "US", "AAPL", "US0000000002", "active"),
        ],
        &[
            ("id-1", "US", "AAPL", "US0378331005", "active"),
            ("id-2", "US", "aapl", "US0378331005", "active"),
        ],
        &[
            ("id-1", "US", "AAPL", "US0378331005", "retired"),
            ("id-2", "US", "aapl", "US0000000002", "active"),
        ],
        &[
            ("id-1", "US", "AAPL", "US0378331005", "active"),
            ("id-2", "US", "MSFT", "US0378331005", "active"),
        ],
        &[
            ("id-1", "US", "AAPL", "US0378331005", "active"),
            ("id-2", "KR", "005930", "US0378331005", "active"),
        ],
        &[
            ("id-1", "US", "AAPL", "US0378331005", "retired"),
            ("id-2", "US", "MSFT", "us0378331005", "active"),
        ],
        &[
            ("id-shared", "KR", "005930", "KR7005930003", "active"),
            ("id-shared", "US", "AAPL", "US0378331005", "active"),
        ],
        &[
            ("id-shared", "KR", "005930", "KR7005930003", "retired"),
            ("id-shared", "US", "AAPL", "US0378331005", "active"),
        ],
        &[
            ("id-shared", "KR", "005930", "KR7005930003", "retired"),
            ("id-shared", "US", "AAPL", "US0378331005", "retired"),
        ],
    ]
}

fn verify_corrupt_set(rt: &tokio::runtime::Runtime, rows: &[RawRow]) {
    let fixture = Fixture::new();
    let path = DatabasePath::new(fixture.path());
    let db = Database::open(&path, &key(), &clock()).expect("open");
    let entries = rows.to_vec();
    rt.block_on(db.write(move |tx| {
        tx.execute_batch(fixtures::RECREATE_UNCONSTRAINED_STRICT)?;
        for (inst_id, market, sym, isin, status) in entries {
            tx.execute(sql::INSERT, params![inst_id, market, sym, isin, status])?;
        }
        Ok(())
    }))
    .expect("seed corrupt rows");
    rt.block_on(db.read(|conn| {
        assert!(crate::instrument_mapping::validate_ledger(conn).is_err());
        Ok(())
    }))
    .expect("validate_ledger direct check");

    let first_row = rows[0];
    let probe = source(market_from_tag(first_row.1), first_row.2, first_row.3);
    let probe_id = id(first_row.0);
    let fresh = source(KnownMarket::Us, "NVDA", "US67066G1040");
    let store = InstrumentMappingStore::new(&db);
    if rows[0].0 == rows[1].0 {
        assert_eq!(rt.block_on(store.for_id(&probe_id)), Err(integrity_error()));
        assert_eq!(
            rt.block_on(store.ensure(&fresh, &probe_id)),
            Err(integrity_error())
        );
    } else {
        assert_eq!(rt.block_on(store.resolve(&probe)), Err(integrity_error()));
        assert_eq!(rt.block_on(store.lookup(&probe)), Err(integrity_error()));
        assert_eq!(rt.block_on(store.retire(&probe)), Err(integrity_error()));
        assert_eq!(
            rt.block_on(store.ensure(&probe, &id("id-fresh"))),
            Err(integrity_error())
        );
    }
    rt.block_on(db.close()).expect("close");
    assert_eq!(
        Database::open(&path, &key(), &clock())
            .expect_err("reopen must fail closed on corrupt mapping table")
            .failure(),
        SystemFailure::RepositoryIntegrity
    );
}

fn verify_corrupt_column(
    rt: &tokio::runtime::Runtime,
    statement: &'static str,
    bad_value: &'static str,
) {
    let fixture = Fixture::new();
    let path = DatabasePath::new(fixture.path());
    let db = Database::open(&path, &key(), &clock()).expect("open");
    let store = InstrumentMappingStore::new(&db);
    let valid = source(KnownMarket::Kr, "005930", "KR7005930003");
    let valid_id = id("opaque-1");
    rt.block_on(store.ensure(&valid, &valid_id)).expect("seed");
    let bad = bad_value.to_owned();
    rt.block_on(db.write(move |tx| {
        tx.execute_batch(fixtures::DROP_GUARD_TRIGGERS)?;
        tx.execute_batch(fixtures::IGNORE_CHECKS_ON)?;
        tx.execute(statement, params!["opaque-1", bad])?;
        tx.execute_batch(fixtures::IGNORE_CHECKS_OFF)?;
        tx.execute_batch(fixtures::RESTORE_GUARD_TRIGGERS)?;
        Ok(())
    }))
    .expect("corrupt column with intact schema");
    rt.block_on(db.read(|conn| {
        crate::instrument_mapping::validate_schema(conn)?;
        assert!(crate::instrument_mapping::validate_ledger(conn).is_err());
        assert!(crate::instrument_mapping::validate_on_open(conn).is_err());
        Ok(())
    }))
    .expect("intact schema passes while validate_ledger rejects corrupt value");
    let fresh_id = id("opaque-2");
    if statement == fixtures::UPDATE_CORRUPT_ID {
        assert_eq!(rt.block_on(store.resolve(&valid)), Err(integrity_error()));
        assert_eq!(rt.block_on(store.lookup(&valid)), Err(integrity_error()));
        assert_eq!(
            rt.block_on(store.ensure(&valid, &fresh_id)),
            Err(integrity_error())
        );
        assert_eq!(rt.block_on(store.retire(&valid)), Err(integrity_error()));
    } else {
        assert_all_ops_fail_integrity(rt, &db, (&valid, &valid_id), (&valid, &fresh_id));
    }
    rt.block_on(db.close()).expect("close");
    assert_eq!(
        Database::open(&path, &key(), &clock())
            .expect_err("reopen must fail closed on broken field with intact schema")
            .failure(),
        SystemFailure::RepositoryIntegrity
    );
}

fn wait_contender_blocked_on_lock(started_rx: &mpsc::Receiver<()>, done_rx: &mpsc::Receiver<()>) {
    started_rx
        .recv()
        .expect("contender thread ready to call ensure");
    assert_eq!(
        done_rx.recv_timeout(Duration::from_millis(80)),
        Err(mpsc::RecvTimeoutError::Timeout),
        "contender must stay blocked on BEGIN IMMEDIATE lock while holder has not finished"
    );
}

#[test]
fn instrument_mapping_sql_prepares_and_matches_migration_file() {
    assert_eq!(
        std::fs::read_to_string("migrations/0008_instrument_mappings.sql").expect("migration file"),
        sql::CREATE
    );
    let mut connection = rusqlite::Connection::open_in_memory().expect("memory");
    crate::migration::run(&mut connection, crate::tests::timestamp()).expect("migrate");
    for statement in [
        sql::SCHEMA_OBJECTS,
        sql::ALL_TRIGGERS,
        sql::ALL,
        sql::BY_SOURCE,
        sql::BY_SOURCE_OR_ID,
        sql::BY_ID,
        sql::INSERT,
        sql::RETIRE,
    ] {
        connection
            .prepare(statement)
            .expect("runtime SQL prepares cleanly");
    }
}

#[test]
fn indexed_point_queries_use_indexes_without_table_scan() {
    let fixture = Fixture::new();
    let rt = runtime();
    let db = open_fixture(&fixture);

    for (query, expected_index) in [
        (
            fixtures::EXPLAIN_BY_SOURCE,
            "instrument_mappings_symbol_key",
        ),
        (fixtures::EXPLAIN_BY_SOURCE, "instrument_mappings_isin_key"),
        (
            fixtures::EXPLAIN_BY_SOURCE_OR_ID,
            "sqlite_autoindex_instrument_mappings_1",
        ),
        (
            fixtures::EXPLAIN_BY_SOURCE_OR_ID,
            "instrument_mappings_symbol_key",
        ),
        (
            fixtures::EXPLAIN_BY_SOURCE_OR_ID,
            "instrument_mappings_isin_key",
        ),
        (
            fixtures::EXPLAIN_BY_ID,
            "sqlite_autoindex_instrument_mappings_1",
        ),
        (
            fixtures::EXPLAIN_NO_REPLACE_SUBQUERY,
            "sqlite_autoindex_instrument_mappings_1",
        ),
        (
            fixtures::EXPLAIN_NO_REPLACE_SUBQUERY,
            "instrument_mappings_symbol_key",
        ),
        (
            fixtures::EXPLAIN_NO_REPLACE_SUBQUERY,
            "instrument_mappings_isin_key",
        ),
    ] {
        let details = rt
            .block_on(db.read(move |conn| {
                let mut stmt = conn.prepare(query)?;
                stmt.query_map([], |row| row.get::<_, String>(3))?
                    .collect::<rusqlite::Result<Vec<_>>>()
            }))
            .expect("query plan");
        let joined = details.join("\n");
        assert!(
            !joined.contains("SCAN instrument_mappings"),
            "point query must not full-scan instrument_mappings: {joined}"
        );
        assert!(
            joined.contains(expected_index),
            "expected index {expected_index} in query plan: {joined}"
        );
    }
    rt.block_on(db.close()).expect("close");
}

#[test]
fn strict_table_and_valid_domain_boundaries_roundtrip() {
    let fixture = Fixture::new();
    let rt = runtime();
    let db = open_fixture(&fixture);

    assert_eq!(
        rt.block_on(db.read(|conn| {
            conn.query_row(fixtures::TABLE_STRICT_FLAG, [], |row| row.get::<_, i64>(0))
        }))
        .expect("strict flag"),
        1
    );
    rt.block_on(db.write(|tx| tx.execute_batch(fixtures::INSERT_STRICT_BLOB_ID)))
        .expect_err("STRICT rejects blob instrument_id");
    rt.block_on(db.write(|tx| tx.execute_batch(fixtures::INSERT_STRICT_BLOB_SYMBOL)))
        .expect_err("STRICT rejects blob symbol");

    let valid_symbol_32 = "A".repeat(32);
    let valid_isin_64 = "I".repeat(64);
    let valid_id_64 = "Z".repeat(64);
    let graphic_id = "!#$%&'()*+,-./09:;<=>?@AZ[\\]^_`az{|}~";
    let graphic_isin = "~}|{za`_^]\\[ZA@?>=<;:90/.-,+*)('&%$#!";

    rt.block_on(db.write(move |tx| {
        tx.execute(
            sql::INSERT,
            params![valid_id_64, "KR", valid_symbol_32, valid_isin_64, "active"],
        )?;
        tx.execute(
            sql::INSERT,
            params![graphic_id, "US", "0aZ.-9", graphic_isin, "active"],
        )?;
        Ok(())
    }))
    .expect("valid boundary and ASCII graphic values succeed");

    let store = InstrumentMappingStore::new(&db);
    let graphic_source = source(KnownMarket::Us, "0aZ.-9", graphic_isin);
    assert_eq!(
        rt.block_on(store.resolve(&graphic_source)),
        Ok(id(graphic_id))
    );
    rt.block_on(db.close()).expect("close");
}

#[test]
fn check_constraints_reject_invalid_fields_and_nul() {
    let fixture = Fixture::new();
    let rt = runtime();
    let db = open_fixture(&fixture);

    let symbol_33 = "A".repeat(33);
    let id_65 = "X".repeat(65);
    let isin_65 = "Y".repeat(65);
    let invalid_rows: [(&str, &str, &str, &str, &str); 24] = [
        ("", "KR", "005930", "KR7005930003", "active"),
        (&id_65, "KR", "005930", "KR7005930003", "active"),
        ("bad id", "KR", "005930", "KR7005930003", "active"),
        ("bad\tid", "KR", "005930", "KR7005930003", "active"),
        ("bad\nid", "KR", "005930", "KR7005930003", "active"),
        ("bad\x7fid", "KR", "005930", "KR7005930003", "active"),
        ("종목아이디", "KR", "005930", "KR7005930003", "active"),
        ("id-kr-bad", "kr", "005930", "KR7005930003", "active"),
        ("id-jp-bad", "JP", "005930", "KR7005930003", "active"),
        ("id-sym-empty", "KR", "", "KR7005930003", "active"),
        ("id-sym-33", "KR", &symbol_33, "KR7005930003", "active"),
        ("id-sym-dot", "KR", ".AAPL", "KR7005930003", "active"),
        ("id-sym-dash", "KR", "-AAPL", "KR7005930003", "active"),
        ("id-sym-under", "KR", "AA_PL", "KR7005930003", "active"),
        ("id-sym-space", "KR", "AA PL", "KR7005930003", "active"),
        ("id-isin-empty", "KR", "005931", "", "active"),
        ("id-isin-65", "KR", "005931", &isin_65, "active"),
        ("id-isin-space", "KR", "005931", "KR 7005930003", "active"),
        ("id-isin-unicode", "KR", "005931", "KR종목", "active"),
        ("id-status-bad", "KR", "005931", "KR7005930004", "ACTIVE"),
        ("id-status-del", "KR", "005931", "KR7005930004", "deleted"),
        ("id\x00nul", "KR", "005932", "KR7005930005", "active"),
        ("id-sym-nul", "KR", "A\x00B", "KR7005930005", "active"),
        ("id-isin-nul", "KR", "005932", "KR\x007005930005", "active"),
    ];
    for row in invalid_rows {
        insert_row_expect_err(&rt, &db, sql::INSERT, row, "CHECK rejects invalid field");
    }
    rt.block_on(db.close()).expect("close");
}

#[test]
fn sqlite_constraints_protect_uniqueness_and_block_direct_retired_insert() {
    let fixture = Fixture::new();
    let rt = runtime();
    let db = open_fixture(&fixture);
    let store = InstrumentMappingStore::new(&db);

    let apple = source(KnownMarket::Us, "AAPL", "US0378331005");
    rt.block_on(store.ensure(&apple, &id("opaque-apple")))
        .expect("issue");

    let retired_direct = (
        "opaque-retired-direct",
        "KR",
        "005930",
        "KR7005930003",
        "retired",
    );
    insert_row_expect_err(
        &rt,
        &db,
        sql::INSERT,
        retired_direct,
        "direct insert with retired status must be blocked",
    );

    let duplicate_rows: [RawRow; 5] = [
        ("opaque-apple", "KR", "005930", "KR7005930003", "active"),
        ("opaque-dup-exact", "US", "AAPL", "US0378331005", "active"),
        ("opaque-dup-sym", "US", "aapl", "US0000000002", "active"),
        ("opaque-dup-isin", "US", "MSFT", "US0378331005", "active"),
        ("opaque-dup-case", "KR", "005930", "us0378331005", "active"),
    ];
    for row in duplicate_rows {
        insert_row_expect_err(
            &rt,
            &db,
            sql::INSERT,
            row,
            "unique constraints reject duplicate or overlap",
        );
    }
    rt.block_on(db.close()).expect("close");
}

#[test]
fn sqlite_no_replace_trigger_blocks_conflict_replacement() {
    let fixture = Fixture::new();
    let rt = runtime();
    let db = open_fixture(&fixture);
    let store = InstrumentMappingStore::new(&db);

    let apple = source(KnownMarket::Us, "AAPL", "US0378331005");
    rt.block_on(store.ensure(&apple, &id("opaque-apple")))
        .expect("issue");

    let replace_rows: [RawRow; 3] = [
        ("opaque-apple", "KR", "005930", "KR7005930003", "active"),
        ("opaque-replace-sym", "US", "aapl", "US0000000002", "active"),
        (
            "opaque-replace-isin",
            "KR",
            "005930",
            "us0378331005",
            "active",
        ),
    ];
    for row in replace_rows {
        insert_row_expect_err(
            &rt,
            &db,
            fixtures::REPLACE_ROW,
            row,
            "conflict replacement must be blocked",
        );
    }
    rt.block_on(db.close()).expect("close");
}

#[test]
fn sqlite_unique_indexes_enforce_case_insensitive_keys_without_triggers() {
    let fixture = Fixture::new();
    let rt = runtime();
    let db = open_fixture(&fixture);
    let store = InstrumentMappingStore::new(&db);

    let apple = source(KnownMarket::Us, "AAPL", "US0378331005");
    rt.block_on(store.ensure(&apple, &id("opaque-apple")))
        .expect("issue");
    rt.block_on(db.write(|tx| tx.execute_batch(fixtures::DROP_GUARD_TRIGGERS)))
        .expect("drop guard triggers");

    for row in [
        ("opaque-sym-idx", "US", "aapl", "US0000000002", "active"),
        ("opaque-isin-idx", "KR", "005930", "us0378331005", "active"),
    ] {
        insert_row_expect_err(
            &rt,
            &db,
            sql::INSERT,
            row,
            "unique index alone must reject case-variant symbol or isin",
        );
    }
    rt.block_on(db.close()).expect("close");
}

#[test]
fn sqlite_triggers_protect_identity_and_retired_history() {
    let fixture = Fixture::new();
    let rt = runtime();
    let db = open_fixture(&fixture);
    let store = InstrumentMappingStore::new(&db);

    let apple = source(KnownMarket::Us, "AAPL", "US0378331005");
    let apple_id = id("opaque-apple");
    rt.block_on(store.ensure(&apple, &apple_id)).expect("issue");

    rt.block_on(db.write(|tx| {
        tx.execute(fixtures::DELETE_BY_ID, ["opaque-apple"])?;
        Ok(())
    }))
    .expect_err("DELETE active mapping must be blocked");

    for (statement, value) in [
        (fixtures::CHANGE_ID, "opaque-other"),
        (fixtures::CHANGE_MARKET, "KR"),
        (fixtures::CHANGE_SYMBOL, "MSFT"),
        (fixtures::CHANGE_ISIN, "US0000000002"),
    ] {
        let owned = value.to_owned();
        rt.block_on(db.write(move |tx| {
            tx.execute(statement, params!["opaque-apple", owned])?;
            Ok(())
        }))
        .expect_err("mutating identity columns must be blocked");
    }

    assert_eq!(rt.block_on(store.retire(&apple)), Ok(apple_id.clone()));

    for statement in [
        fixtures::DELETE_BY_ID,
        fixtures::REACTIVATE_BY_ID,
        fixtures::RETIRE_AGAIN_BY_ID,
    ] {
        rt.block_on(db.write(move |tx| {
            tx.execute(statement, ["opaque-apple"])?;
            Ok(())
        }))
        .expect_err("retired row must be immutable and non-deletable");
    }
    insert_row_expect_err(
        &rt,
        &db,
        fixtures::UPSERT_ROW,
        ("opaque-apple", "US", "AAPL", "US0378331005", "active"),
        "UPSERT reactivation must be blocked",
    );

    assert_eq!(
        rt.block_on(store.lookup(&apple)),
        Ok(InstrumentMappingRecord::new(
            apple,
            apple_id,
            InstrumentMappingStatus::Retired
        ))
    );
    rt.block_on(db.close()).expect("close");
}

#[test]
fn open_rejects_tampered_mapping_schema_objects() {
    let rt = runtime();
    for tamper in [
        fixtures::RECREATE_UNCONSTRAINED_STRICT,
        fixtures::DROP_SYMBOL_INDEX,
        fixtures::DROP_ISIN_INDEX,
        fixtures::DROP_NO_DELETE_TRIGGER,
        fixtures::DROP_RETIRE_ONLY_TRIGGER,
        fixtures::DROP_NO_REPLACE_TRIGGER,
        fixtures::DROP_INSERT_ACTIVE_TRIGGER,
        fixtures::EXTRA_ROGUE_TRIGGER,
        fixtures::ROGUE_UPPERCASE_TRIGGER,
        fixtures::ROGUE_CROSS_TABLE_TRIGGER,
        fixtures::REPLACE_WEAK_NO_DELETE_TRIGGER,
    ] {
        let fixture = Fixture::new();
        let path = DatabasePath::new(fixture.path());
        let db = Database::open(&path, &key(), &clock()).expect("open");
        rt.block_on(db.write(move |tx| tx.execute_batch(tamper)))
            .expect("apply tamper");
        if tamper == fixtures::ROGUE_UPPERCASE_TRIGGER {
            rt.block_on(db.read(|conn| {
                assert!(crate::instrument_mapping::validate_mapping_objects(conn).is_err());
                Ok(())
            }))
            .expect("uppercase table trigger must fail validate_mapping_objects directly");
        }
        if tamper == fixtures::ROGUE_CROSS_TABLE_TRIGGER {
            rt.block_on(db.read(|conn| {
                assert!(crate::instrument_mapping::validate_all_triggers(conn).is_err());
                Ok(())
            }))
            .expect("cross-table rogue trigger must fail validate_all_triggers directly");
        }
        rt.block_on(db.close()).expect("close");
        assert_eq!(
            Database::open(&path, &key(), &clock())
                .expect_err("reopen must reject tampered mapping schema")
                .failure(),
            SystemFailure::RepositoryIntegrity
        );
    }
}

#[test]
fn open_rejects_writable_schema_disguised_triggers() {
    let rt = runtime();
    for tamper in [
        fixtures::WRITABLE_SCHEMA_MIXED_CASE_TRIGGER,
        fixtures::WRITABLE_SCHEMA_NUL_TEXT_TRIGGER,
        fixtures::WRITABLE_SCHEMA_NUL_BLOB_TRIGGER,
    ] {
        let fixture = Fixture::new();
        let path = DatabasePath::new(fixture.path());
        let db = Database::open(&path, &key(), &clock()).expect("open");
        rt.block_on(db.write(move |tx| tx.execute_batch(tamper)))
            .expect("inject disguised trigger row");
        rt.block_on(db.read(|conn| {
            assert!(crate::instrument_mapping::validate_all_triggers(conn).is_err());
            Ok(())
        }))
        .expect("disguised trigger row must fail validate_all_triggers");
        rt.block_on(db.close()).expect("close");
        assert_eq!(
            Database::open(&path, &key(), &clock())
                .expect_err("reopen must reject disguised writable_schema trigger")
                .failure(),
            SystemFailure::RepositoryIntegrity
        );
    }
}

#[test]
fn validate_ledger_accepts_valid_unindexed_rows_and_rejects_duplicate_overlap_or_shared_id() {
    let rt = runtime();
    let valid_fixture = Fixture::new();
    let valid_db = open_fixture(&valid_fixture);
    rt.block_on(valid_db.write(|tx| {
        tx.execute_batch(fixtures::RECREATE_UNCONSTRAINED_STRICT)?;
        tx.execute(
            sql::INSERT,
            params!["id-kr", "KR", "005930", "KR7005930003", "active"],
        )?;
        tx.execute(
            sql::INSERT,
            params!["id-us", "US", "AAPL", "US0378331005", "retired"],
        )?;
        Ok(())
    }))
    .expect("seed positive control");
    rt.block_on(valid_db.read(crate::instrument_mapping::validate_ledger))
        .expect("positive control passes validate_ledger");
    rt.block_on(valid_db.close()).expect("close");

    for rows in corrupt_row_sets() {
        verify_corrupt_set(&rt, rows);
    }
}

#[test]
fn open_and_operations_reject_corrupted_column_values() {
    let rt = runtime();
    for (statement, bad_value) in [
        (fixtures::UPDATE_CORRUPT_ID, "bad id"),
        (fixtures::UPDATE_CORRUPT_MARKET, "JP"),
        (fixtures::UPDATE_CORRUPT_SYMBOL, ".BAD"),
        (fixtures::UPDATE_CORRUPT_ISIN, "BAD ISIN"),
        (fixtures::UPDATE_CORRUPT_STATUS, "deleted"),
    ] {
        verify_corrupt_column(&rt, statement, bad_value);
    }
}

#[test]
fn injected_write_failures_and_zero_row_effects_roll_back_cleanly() {
    let rt = runtime();
    let sample = source(KnownMarket::Kr, "005930", "KR7005930003");
    let sample_id = id("opaque-1");

    for trigger in [fixtures::FAIL_INSERT_ABORT, fixtures::FAIL_INSERT_IGNORE] {
        let fixture = Fixture::new();
        let db = open_fixture(&fixture);
        let store = InstrumentMappingStore::new(&db);
        rt.block_on(db.write(move |tx| tx.execute_batch(trigger)))
            .expect("install insert trigger");
        assert_eq!(
            rt.block_on(store.ensure(&sample, &sample_id)),
            Err(integrity_error())
        );
        assert_eq!(
            rt.block_on(store.resolve(&sample)),
            Err(MappingStoreError::Mapping(InstrumentMappingError::Missing))
        );
        assert_eq!(
            rt.block_on(db.read(|conn| {
                conn.query_row(fixtures::COUNT_ROWS, [], |row| row.get::<_, i64>(0))
            }))
            .expect("count"),
            0
        );
        rt.block_on(db.close()).expect("close");
    }

    for trigger in [fixtures::FAIL_RETIRE_ABORT, fixtures::FAIL_RETIRE_IGNORE] {
        let fixture = Fixture::new();
        let db = open_fixture(&fixture);
        let store = InstrumentMappingStore::new(&db);
        rt.block_on(store.ensure(&sample, &sample_id))
            .expect("seed active");
        rt.block_on(db.write(move |tx| tx.execute_batch(trigger)))
            .expect("install retire trigger");
        assert_eq!(rt.block_on(store.retire(&sample)), Err(integrity_error()));
        assert_eq!(rt.block_on(store.resolve(&sample)), Ok(sample_id.clone()));
        assert_eq!(
            rt.block_on(store.lookup(&sample)),
            Ok(InstrumentMappingRecord::new(
                sample.clone(),
                sample_id.clone(),
                InstrumentMappingStatus::Active
            ))
        );
        rt.block_on(db.close()).expect("close");
    }
}

#[test]
fn two_connections_serialize_immediate_transaction_commit() {
    let rt = runtime();
    let fixture = Fixture::new();
    let first_db = open_fixture(&fixture);
    let second_db = open_fixture(&fixture);
    let sample = source(KnownMarket::Kr, "005930", "KR7005930003");
    let (locked_tx, locked_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let (started_tx, started_rx) = mpsc::channel();
    let (done_tx, done_rx) = mpsc::channel();
    let holder = first_db
        .writer
        .submit(move |tx| {
            tx.execute(
                sql::INSERT,
                params!["opaque-holder", "KR", "005930", "KR7005930003", "active"],
            )?;
            locked_tx.send(()).expect("signal lock held");
            release_rx.recv().expect("wait release");
            Ok(())
        })
        .expect("submit holder");
    let second_result = std::thread::scope(|scope| {
        let contender_source = sample.clone();
        let contender_db = &second_db;
        locked_rx.recv().expect("holder holds immediate lock");
        let handle = scope.spawn(move || {
            let contender_rt = runtime();
            let store = InstrumentMappingStore::new(contender_db);
            started_tx.send(()).expect("signal contender start");
            let outcome =
                contender_rt.block_on(store.ensure(&contender_source, &id("opaque-contender")));
            done_tx.send(()).expect("signal contender done");
            outcome
        });
        wait_contender_blocked_on_lock(&started_rx, &done_rx);
        release_tx.send(()).expect("commit holder");
        rt.block_on(holder)
            .expect("holder recv")
            .expect("holder commit");
        done_rx.recv().expect("contender finishes after commit");
        handle.join().expect("contender thread")
    });
    assert_eq!(
        second_result,
        Ok(MappingAssignment::Existing(id("opaque-holder")))
    );
    rt.block_on(first_db.close()).expect("close first");
    rt.block_on(second_db.close()).expect("close second");
}

#[test]
fn two_connections_serialize_immediate_transaction_rollback() {
    let rt = runtime();
    let fixture = Fixture::new();
    let first_db = open_fixture(&fixture);
    let second_db = open_fixture(&fixture);
    let sample = source(KnownMarket::Us, "AAPL", "US0378331005");
    let (locked_tx, locked_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let (started_tx, started_rx) = mpsc::channel();
    let (done_tx, done_rx) = mpsc::channel();
    let holder = first_db
        .writer
        .submit(move |tx| {
            tx.execute(
                sql::INSERT,
                params!["opaque-aborted", "US", "AAPL", "US0378331005", "active"],
            )?;
            locked_tx.send(()).expect("signal lock held");
            release_rx.recv().expect("wait release");
            Err::<(), _>(rusqlite::Error::InvalidQuery)
        })
        .expect("submit rollback holder");
    let second_result = std::thread::scope(|scope| {
        let contender_source = sample.clone();
        let contender_db = &second_db;
        locked_rx.recv().expect("holder holds immediate lock");
        let handle = scope.spawn(move || {
            let contender_rt = runtime();
            let store = InstrumentMappingStore::new(contender_db);
            started_tx.send(()).expect("signal contender start");
            let outcome =
                contender_rt.block_on(store.ensure(&contender_source, &id("opaque-winner")));
            done_tx.send(()).expect("signal contender done");
            outcome
        });
        wait_contender_blocked_on_lock(&started_rx, &done_rx);
        release_tx.send(()).expect("abort holder");
        rt.block_on(holder)
            .expect("holder recv")
            .expect_err("holder rolled back");
        done_rx.recv().expect("contender finishes after rollback");
        handle.join().expect("contender thread")
    });
    assert_eq!(
        second_result,
        Ok(MappingAssignment::Issued(id("opaque-winner")))
    );
    rt.block_on(first_db.close()).expect("close first");
    rt.block_on(second_db.close()).expect("close second");
}

#[test]
fn two_connections_report_busy_unavailable_when_lock_times_out() {
    let rt = runtime();
    let fixture = Fixture::new();
    let first_db = open_fixture(&fixture);
    let second_db = open_fixture(&fixture);
    let sample = source(KnownMarket::Kr, "005930", "KR7005930003");
    let first_store = InstrumentMappingStore::new(&first_db);
    rt.block_on(first_store.ensure(&sample, &id("opaque-1")))
        .expect("seed");
    rt.block_on(second_db.write(|tx| tx.execute_batch(fixtures::ZERO_BUSY_TIMEOUT)))
        .expect("zero busy timeout on contender");

    let (locked_tx, locked_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let holder = first_db
        .writer
        .submit(move |tx| {
            tx.execute(
                sql::RETIRE,
                params!["opaque-1", "KR", "005930", "KR7005930003"],
            )?;
            locked_tx.send(()).expect("signal lock held");
            release_rx.recv().expect("wait release");
            Ok(())
        })
        .expect("submit retire holder");

    locked_rx.recv().expect("holder holds immediate lock");
    let second_store = InstrumentMappingStore::new(&second_db);
    assert_eq!(
        rt.block_on(second_store.ensure(&sample, &id("opaque-2"))),
        Err(MappingStoreError::Storage(SystemPortError::new(
            SystemPort::Repository,
            SystemFailure::Unavailable(RetryDelay::Unknown),
        )))
    );
    release_tx.send(()).expect("commit retire holder");
    rt.block_on(holder)
        .expect("holder recv")
        .expect("holder commit");
    assert_eq!(
        rt.block_on(second_store.resolve(&sample)),
        Err(MappingStoreError::Mapping(InstrumentMappingError::Retired))
    );
    rt.block_on(first_db.close()).expect("close first");
    rt.block_on(second_db.close()).expect("close second");
}

#[test]
fn large_ledger_reopens_and_resolves_indexed_entries_accurately() {
    let rt = runtime();
    let fixture = Fixture::new();
    let db = open_fixture(&fixture);
    rt.block_on(db.write(|tx| {
        for index in 0_u32..1000 {
            let market = if index & 1 == 0 { "KR" } else { "US" };
            let inst_id = ["inst-", &index.to_string()].join("");
            let sym = ["SYM", &index.to_string()].join("");
            let isin = ["ISIN", &index.to_string()].join("");
            tx.execute(sql::INSERT, params![inst_id, market, sym, isin, "active"])?;
        }
        Ok(())
    }))
    .expect("seed 1000 mappings");
    rt.block_on(db.close()).expect("close");

    let reopened = open_fixture(&fixture);
    let store = InstrumentMappingStore::new(&reopened);
    for index in [0_u32, 333, 500, 999] {
        let market = if index & 1 == 0 {
            KnownMarket::Kr
        } else {
            KnownMarket::Us
        };
        let sym = ["SYM", &index.to_string()].join("");
        let isin = ["ISIN", &index.to_string()].join("");
        let expected_id = id(&["inst-", &index.to_string()].join(""));
        let entry = source(market, &sym, &isin);
        assert_eq!(rt.block_on(store.resolve(&entry)), Ok(expected_id.clone()));
        assert_eq!(
            rt.block_on(store.for_id(&expected_id)),
            Ok(vec![InstrumentMappingRecord::new(
                entry,
                expected_id,
                InstrumentMappingStatus::Active
            )])
        );
    }
    let overlap = source(KnownMarket::Kr, "sym500", "ISIN-DIFFERENT");
    assert_eq!(
        rt.block_on(store.resolve(&overlap)),
        Err(MappingStoreError::Mapping(
            InstrumentMappingError::IdentityUncertain
        ))
    );
    rt.block_on(reopened.close()).expect("close");
}
