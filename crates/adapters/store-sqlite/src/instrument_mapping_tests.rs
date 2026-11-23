use crate::{
    Database, DatabasePath, InstrumentIdIssuerStore, InstrumentMappingStore,
    sql::{instrument_mapping as sql, instrument_mapping_tests as fixtures},
    tests::{Fixture, clock, key, runtime},
};
use application::{
    InstrumentIdIssuer, InstrumentMappingRepository, MappingStoreError, RetryDelay, SystemFailure,
    SystemPort, SystemPortError,
};
use domain::{
    IndependentInstrumentIdIssuer, InstrumentId, InstrumentMappingError, InstrumentMappingRecord,
    InstrumentMappingStatus, InstrumentSource, IsinCode, IssuedInstrumentId, IssuerSequence,
    KnownMarket, MappingAssignment, PersistedIssuerCursor, Symbol,
};
use rusqlite::params;
use std::{collections::BTreeSet, sync::mpsc, time::Duration};

type RawRow = (
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    &'static str,
);

const SLOT_0_ID: &str = "inst-0000000000000001";
const SLOT_1_ID: &str = "inst-0000000000000002";

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

fn issued_slot(slot: u16) -> IssuedInstrumentId {
    let mut issuer = IndependentInstrumentIdIssuer::new();
    for _ in 0..slot {
        let _skip = issuer.issue().expect("skip slot");
    }
    issuer.issue().expect("issue slot")
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
    fresh: (&InstrumentSource, IssuedInstrumentId),
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
            (SLOT_0_ID, "KR", "005930", "KR7005930003", "active"),
            (SLOT_0_ID, "KR", "005930", "KR7005930003", "active"),
        ],
        &[
            (SLOT_0_ID, "KR", "005930", "KR7005930003", "active"),
            (SLOT_1_ID, "KR", "005930", "KR7005930003", "active"),
        ],
        &[
            (SLOT_0_ID, "KR", "005930", "KR7005930003", "active"),
            (SLOT_0_ID, "KR", "005930", "KR7005930003", "retired"),
        ],
        &[
            (SLOT_0_ID, "KR", "005930", "KR7005930003", "retired"),
            (SLOT_0_ID, "KR", "005930", "KR7005930003", "retired"),
        ],
        &[
            (SLOT_0_ID, "US", "AAPL", "US0378331005", "active"),
            (SLOT_1_ID, "US", "AAPL", "US0000000002", "active"),
        ],
        &[
            (SLOT_0_ID, "US", "AAPL", "US0378331005", "active"),
            (SLOT_1_ID, "US", "aapl", "US0378331005", "active"),
        ],
        &[
            (SLOT_0_ID, "US", "AAPL", "US0378331005", "retired"),
            (SLOT_1_ID, "US", "aapl", "US0000000002", "active"),
        ],
        &[
            (SLOT_0_ID, "US", "AAPL", "US0378331005", "active"),
            (SLOT_1_ID, "US", "MSFT", "US0378331005", "active"),
        ],
        &[
            (SLOT_0_ID, "US", "AAPL", "US0378331005", "active"),
            (SLOT_1_ID, "KR", "005930", "US0378331005", "active"),
        ],
        &[
            (SLOT_0_ID, "US", "AAPL", "US0378331005", "retired"),
            (SLOT_1_ID, "US", "MSFT", "us0378331005", "active"),
        ],
        &[
            (SLOT_0_ID, "KR", "005930", "KR7005930003", "active"),
            (SLOT_0_ID, "US", "AAPL", "US0378331005", "active"),
        ],
        &[
            (SLOT_0_ID, "KR", "005930", "KR7005930003", "retired"),
            (SLOT_0_ID, "US", "AAPL", "US0378331005", "active"),
        ],
        &[
            (SLOT_0_ID, "KR", "005930", "KR7005930003", "retired"),
            (SLOT_0_ID, "US", "AAPL", "US0378331005", "retired"),
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
            rt.block_on(store.ensure(&fresh, issued_slot(0))),
            Err(integrity_error())
        );
    } else {
        assert_eq!(rt.block_on(store.resolve(&probe)), Err(integrity_error()));
        assert_eq!(rt.block_on(store.lookup(&probe)), Err(integrity_error()));
        assert_eq!(rt.block_on(store.retire(&probe)), Err(integrity_error()));
        assert_eq!(
            rt.block_on(store.ensure(&probe, issued_slot(5))),
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
    let valid_cand = issued_slot(0);
    let valid_id = valid_cand.as_instrument_id().clone();
    rt.block_on(store.ensure(&valid, valid_cand)).expect("seed");
    let bad = bad_value.to_owned();
    rt.block_on(db.write(move |tx| {
        tx.execute_batch(fixtures::DROP_GUARD_TRIGGERS)?;
        tx.execute_batch(fixtures::IGNORE_CHECKS_ON)?;
        tx.execute(statement, params![SLOT_0_ID, bad])?;
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
    let fresh_cand = issued_slot(1);
    if statement == fixtures::UPDATE_CORRUPT_ID {
        assert_eq!(rt.block_on(store.resolve(&valid)), Err(integrity_error()));
        assert_eq!(rt.block_on(store.lookup(&valid)), Err(integrity_error()));
        assert_eq!(
            rt.block_on(store.ensure(&valid, fresh_cand)),
            Err(integrity_error())
        );
        assert_eq!(rt.block_on(store.retire(&valid)), Err(integrity_error()));
    } else {
        assert_all_ops_fail_integrity(rt, &db, (&valid, &valid_id), (&valid, fresh_cand));
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
        .expect("contender thread ready to call write");
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
        sql::ID_EXISTS,
        sql::BY_SOURCE,
        sql::BY_SOURCE_OR_ID,
        sql::BY_ID,
        sql::INSERT,
        sql::RETIRE,
        sql::ISSUER_CURSOR,
        sql::INSERT_ISSUER_CURSOR,
        sql::UPDATE_ISSUER_CURSOR,
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
            fixtures::EXPLAIN_ID_EXISTS,
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

    for flag_query in [
        fixtures::TABLE_STRICT_FLAG,
        fixtures::ISSUER_TABLE_STRICT_FLAG,
    ] {
        assert_eq!(
            rt.block_on(
                db.read(move |conn| { conn.query_row(flag_query, [], |row| row.get::<_, i64>(0)) })
            )
            .expect("strict flag"),
            1
        );
    }
    rt.block_on(db.write(|tx| tx.execute_batch(fixtures::INSERT_STRICT_BLOB_ID)))
        .expect_err("STRICT rejects blob instrument_id");
    rt.block_on(db.write(|tx| tx.execute_batch(fixtures::INSERT_STRICT_BLOB_SYMBOL)))
        .expect_err("STRICT rejects blob symbol");
    rt.block_on(db.write(|tx| tx.execute_batch(fixtures::INSERT_ISSUER_BLOB)))
        .expect_err("STRICT rejects blob issuer cursor");

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
    rt.block_on(store.ensure(&apple, issued_slot(0)))
        .expect("issue");

    for non_active_status in ["retired", "superseded"] {
        let non_active = (
            "opaque-non-active-direct",
            "KR",
            "005930",
            "KR7005930003",
            non_active_status,
        );
        insert_row_expect_err(
            &rt,
            &db,
            sql::INSERT,
            non_active,
            "direct insert with non-active status must be blocked",
        );
    }

    let duplicate_rows: [RawRow; 5] = [
        (SLOT_0_ID, "KR", "005930", "KR7005930003", "active"),
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
    rt.block_on(store.ensure(&apple, issued_slot(0)))
        .expect("issue");

    let replace_rows: [RawRow; 3] = [
        (SLOT_0_ID, "KR", "005930", "KR7005930003", "active"),
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
    rt.block_on(store.ensure(&apple, issued_slot(0)))
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
    let apple_cand = issued_slot(0);
    let apple_id = apple_cand.as_instrument_id().clone();
    rt.block_on(store.ensure(&apple, apple_cand))
        .expect("issue");

    rt.block_on(db.write(|tx| {
        tx.execute(fixtures::DELETE_BY_ID, [SLOT_0_ID])?;
        Ok(())
    }))
    .expect_err("DELETE active mapping must be blocked");

    for (statement, value) in [
        (fixtures::CHANGE_ID, SLOT_1_ID),
        (fixtures::CHANGE_MARKET, "KR"),
        (fixtures::CHANGE_SYMBOL, "MSFT"),
        (fixtures::CHANGE_ISIN, "US0000000002"),
    ] {
        let owned = value.to_owned();
        rt.block_on(db.write(move |tx| {
            tx.execute(statement, params![SLOT_0_ID, owned])?;
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
            tx.execute(statement, [SLOT_0_ID])?;
            Ok(())
        }))
        .expect_err("retired row must be immutable and non-deletable");
    }
    insert_row_expect_err(
        &rt,
        &db,
        fixtures::UPSERT_ROW,
        (SLOT_0_ID, "US", "AAPL", "US0378331005", "active"),
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
        fixtures::RECREATE_UNCONSTRAINED_ISSUER,
        fixtures::DROP_SYMBOL_INDEX,
        fixtures::DROP_ISIN_INDEX,
        fixtures::DROP_NO_DELETE_TRIGGER,
        fixtures::DROP_RETIRE_ONLY_TRIGGER,
        fixtures::DROP_NO_REPLACE_TRIGGER,
        fixtures::DROP_INSERT_ACTIVE_TRIGGER,
        fixtures::DROP_ISSUER_NO_DELETE_TRIGGER,
        fixtures::DROP_ISSUER_MONOTONIC_TRIGGER,
        fixtures::DROP_ISSUER_SINGLE_INSERT_TRIGGER,
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
fn open_and_operations_reject_orphan_superseded_status_in_s21_schema() {
    let rt = runtime();
    let fixture = Fixture::new();
    let path = DatabasePath::new(fixture.path());
    let db = Database::open(&path, &key(), &clock()).expect("open");
    let store = InstrumentMappingStore::new(&db);
    let fb = source(KnownMarket::Us, "FB", "US30303M1027");
    let fb_cand = issued_slot(0);
    let fb_id = fb_cand.as_instrument_id().clone();
    rt.block_on(store.ensure(&fb, fb_cand)).expect("seed fb");

    rt.block_on(db.write(|tx| {
        tx.execute_batch(fixtures::DROP_GUARD_TRIGGERS)?;
        let res = tx.execute(
            fixtures::UPDATE_CORRUPT_STATUS,
            params![SLOT_0_ID, "superseded"],
        );
        tx.execute_batch(fixtures::RESTORE_GUARD_TRIGGERS)?;
        res?;
        Ok(())
    }))
    .expect_err("CHECK must reject superseded status in S-21 1:1 schema");

    rt.block_on(db.write(|tx| {
        tx.execute_batch(fixtures::DROP_GUARD_TRIGGERS)?;
        tx.execute_batch(fixtures::IGNORE_CHECKS_ON)?;
        tx.execute(
            fixtures::UPDATE_CORRUPT_STATUS,
            params![SLOT_0_ID, "superseded"],
        )?;
        tx.execute_batch(fixtures::IGNORE_CHECKS_OFF)?;
        tx.execute_batch(fixtures::RESTORE_GUARD_TRIGGERS)?;
        Ok(())
    }))
    .expect("inject orphan superseded row with intact schema");

    assert_eq!(rt.block_on(store.resolve(&fb)), Err(integrity_error()));
    assert_eq!(rt.block_on(store.lookup(&fb)), Err(integrity_error()));
    assert_eq!(rt.block_on(store.for_id(&fb_id)), Err(integrity_error()));
    assert_eq!(
        rt.block_on(store.ensure(&fb, issued_slot(1))),
        Err(integrity_error())
    );
    assert_eq!(rt.block_on(store.retire(&fb)), Err(integrity_error()));
    rt.block_on(db.close()).expect("close");

    assert_eq!(
        Database::open(&path, &key(), &clock())
            .expect_err("reopen must reject orphan superseded row")
            .failure(),
        SystemFailure::RepositoryIntegrity
    );
}

#[test]
fn unpolled_issuer_future_drop_leaves_cursor_table_empty() {
    let rt = runtime();
    let fixture = Fixture::new();
    let path = DatabasePath::new(fixture.path());
    let db = Database::open(&path, &key(), &clock()).expect("open");
    let issuer = InstrumentIdIssuerStore::new(&db);

    {
        let _unpolled = issuer.issue();
    }
    assert_eq!(
        rt.block_on(db.read(crate::instrument_mapping::read_issuer_cursor))
            .expect("read cursor"),
        PersistedIssuerCursor::Initial
    );
    let first = rt.block_on(issuer.issue()).expect("first polled issue");
    assert_eq!(first.as_instrument_id().as_str(), "inst-0000000000000001");
    rt.block_on(db.close()).expect("close");
}

#[test]
fn issuer_persists_monotonic_cursor_and_skips_retired_and_dropped_across_restarts() {
    let rt = runtime();
    let fixture = Fixture::new();
    let path = DatabasePath::new(fixture.path());
    let kr = source(KnownMarket::Kr, "005930", "KR7005930003");
    let us = source(KnownMarket::Us, "AAPL", "US0378331005");

    let db = Database::open(&path, &key(), &clock()).expect("open");
    let store = InstrumentMappingStore::new(&db);
    let issuer = InstrumentIdIssuerStore::new(&db);

    let first = rt.block_on(issuer.issue()).expect("issue 1");
    let first_id = first.as_instrument_id().clone();
    assert_eq!(first_id.as_str(), "inst-0000000000000001");
    let second_id = rt
        .block_on(issuer.issue())
        .expect("issue 2")
        .as_instrument_id()
        .clone();
    assert_eq!(second_id.as_str(), "inst-0000000000000002");

    assert_eq!(
        rt.block_on(store.ensure(&kr, first)),
        Ok(MappingAssignment::Issued(first_id.clone()))
    );
    assert_eq!(rt.block_on(store.retire(&kr)), Ok(first_id));
    rt.block_on(db.close()).expect("close");

    let reopened = Database::open(&path, &key(), &clock()).expect("reopen");
    let reopened_store = InstrumentMappingStore::new(&reopened);
    let reopened_issuer = InstrumentIdIssuerStore::new(&reopened);

    assert_eq!(
        rt.block_on(reopened.read(crate::instrument_mapping::read_issuer_cursor))
            .expect("read persisted cursor"),
        PersistedIssuerCursor::LastIssued(IssuerSequence::try_new(2).expect("seq 2"))
    );

    let third = rt.block_on(reopened_issuer.issue()).expect("issue 3");
    let third_id = third.as_instrument_id().clone();
    assert_eq!(third_id.as_str(), "inst-0000000000000003");
    assert_eq!(
        rt.block_on(reopened_store.ensure(&us, third)),
        Ok(MappingAssignment::Issued(third_id))
    );
    assert_eq!(
        rt.block_on(reopened.read(crate::instrument_mapping::read_issuer_cursor))
            .expect("read advanced cursor"),
        PersistedIssuerCursor::LastIssued(IssuerSequence::try_new(3).expect("seq 3"))
    );
    rt.block_on(reopened.close()).expect("close");
}

#[test]
fn issuer_skips_preexisting_external_and_retired_ids_and_exhausts_at_u64_max() {
    let rt = runtime();
    let fixture = Fixture::new();
    let path = DatabasePath::new(fixture.path());
    let db = Database::open(&path, &key(), &clock()).expect("open");
    let store = InstrumentMappingStore::new(&db);
    let issuer = InstrumentIdIssuerStore::new(&db);
    let kr = source(KnownMarket::Kr, "005930", "KR7005930003");
    let us = source(KnownMarket::Us, "AAPL", "US0378331005");

    rt.block_on(store.ensure(&kr, issued_slot(0)))
        .expect("external slot 0");
    rt.block_on(store.ensure(&us, issued_slot(1)))
        .expect("external slot 1");
    rt.block_on(store.retire(&us)).expect("retire slot 1");

    let skipped = rt.block_on(issuer.issue()).expect("skip slots 0 and 1");
    assert_eq!(skipped.as_instrument_id().as_str(), "inst-0000000000000003");
    assert_eq!(
        rt.block_on(db.read(crate::instrument_mapping::read_issuer_cursor))
            .expect("cursor after skip"),
        PersistedIssuerCursor::LastIssued(IssuerSequence::try_new(3).expect("seq 3"))
    );

    rt.block_on(db.write(|tx| {
        tx.execute(fixtures::UPDATE_ISSUER_VALUE, ["00000000000000af"])?;
        Ok(())
    }))
    .expect("advance cursor to hex boundary 0xaf");
    let hex_next = rt.block_on(issuer.issue()).expect("issue 0xb0");
    assert_eq!(
        hex_next.as_instrument_id().as_str(),
        "inst-00000000000000b0"
    );

    rt.block_on(db.write(|tx| {
        tx.execute(fixtures::UPDATE_ISSUER_VALUE, ["ffffffffffffffff"])?;
        Ok(())
    }))
    .expect("advance cursor to u64::MAX");
    rt.block_on(db.close()).expect("close");

    let reopened = Database::open(&path, &key(), &clock()).expect("reopen at u64::MAX");
    let exhausted_issuer = InstrumentIdIssuerStore::new(&reopened);
    assert_eq!(
        rt.block_on(exhausted_issuer.issue())
            .expect_err("u64::MAX must exhaust closed")
            .failure(),
        SystemFailure::RepositoryIntegrity
    );
    rt.block_on(reopened.close()).expect("close");
}

#[test]
fn issuer_table_constraints_and_triggers_block_tampering() {
    let rt = runtime();
    let fixture = Fixture::new();
    let path = DatabasePath::new(fixture.path());
    let db = Database::open(&path, &key(), &clock()).expect("open");
    let issuer = InstrumentIdIssuerStore::new(&db);

    for (slot, bad_hex) in [
        (2_i64, "0000000000000001"),
        (1, "0000000000000000"),
        (1, "000000000000000A"),
        (1, "000000000000001"),
        (1, "00000000000000001"),
        (1, "00000000000000\x001"),
    ] {
        let owned = bad_hex.to_owned();
        rt.block_on(db.write(move |tx| {
            tx.execute(fixtures::INSERT_ISSUER_RAW, params![slot, owned])?;
            Ok(())
        }))
        .expect_err("issuer CHECK must reject invalid slot or hex");
    }

    let _first = rt.block_on(issuer.issue()).expect("issue 1");
    let _second = rt.block_on(issuer.issue()).expect("issue 2");

    rt.block_on(db.write(|tx| {
        tx.execute(fixtures::DELETE_ISSUER_ROW, [])?;
        Ok(())
    }))
    .expect_err("issuer cursor deletion must be blocked");
    for non_advancing in ["0000000000000001", "0000000000000002"] {
        let owned = non_advancing.to_owned();
        rt.block_on(db.write(move |tx| {
            tx.execute(fixtures::UPDATE_ISSUER_VALUE, [owned])?;
            Ok(())
        }))
        .expect_err("issuer cursor must not decrement or stay equal");
    }
    rt.block_on(db.write(|tx| {
        tx.execute(fixtures::REPLACE_ISSUER_ROW, ["0000000000000001"])?;
        Ok(())
    }))
    .expect_err("issuer cursor conflict replacement must be blocked");
    rt.block_on(db.close()).expect("close");
}

#[test]
fn open_rejects_corrupted_issuer_cursor_with_intact_schema() {
    let rt = runtime();
    for bad_cursor in [
        "0000000000000000",
        "000000000000000A",
        "000000000000000g",
        "000000000000001",
        "short",
    ] {
        let bad_fixture = Fixture::new();
        let bad_path = DatabasePath::new(bad_fixture.path());
        let bad_db = Database::open(&bad_path, &key(), &clock()).expect("open");
        let _seed = rt
            .block_on(InstrumentIdIssuerStore::new(&bad_db).issue())
            .expect("seed cursor");
        let owned = bad_cursor.to_owned();
        rt.block_on(bad_db.write(move |tx| {
            tx.execute_batch(fixtures::DROP_ISSUER_TRIGGERS)?;
            tx.execute_batch(fixtures::IGNORE_CHECKS_ON)?;
            tx.execute(fixtures::UPDATE_ISSUER_VALUE, [owned])?;
            tx.execute_batch(fixtures::IGNORE_CHECKS_OFF)?;
            tx.execute_batch(fixtures::RESTORE_ISSUER_TRIGGERS)?;
            Ok(())
        }))
        .expect("corrupt issuer cursor with intact schema");
        rt.block_on(bad_db.read(|conn| {
            crate::instrument_mapping::validate_schema(conn)?;
            assert!(crate::instrument_mapping::validate_issuer_cursor(conn).is_err());
            assert!(crate::instrument_mapping::validate_on_open(conn).is_err());
            Ok(())
        }))
        .expect("validate_issuer_cursor and validate_on_open reject corrupt cursor");
        rt.block_on(bad_db.close()).expect("close");
        assert_eq!(
            Database::open(&bad_path, &key(), &clock())
                .expect_err("reopen must reject corrupt issuer cursor")
                .failure(),
            SystemFailure::RepositoryIntegrity
        );
    }
}

#[test]
fn open_rejects_non_singleton_issuer_cursor_slot() {
    let rt = runtime();
    let fixture = Fixture::new();
    let path = DatabasePath::new(fixture.path());
    let db = Database::open(&path, &key(), &clock()).expect("open");
    rt.block_on(db.write(|tx| {
        tx.execute_batch(fixtures::DROP_ISSUER_TRIGGERS)?;
        tx.execute_batch(fixtures::IGNORE_CHECKS_ON)?;
        tx.execute(
            fixtures::INSERT_ISSUER_RAW,
            params![2_i64, "0000000000000001"],
        )?;
        tx.execute_batch(fixtures::IGNORE_CHECKS_OFF)?;
        tx.execute_batch(fixtures::RESTORE_ISSUER_TRIGGERS)?;
        Ok(())
    }))
    .expect("inject slot 2 cursor");
    rt.block_on(db.read(|conn| {
        crate::instrument_mapping::validate_schema(conn)?;
        assert!(crate::instrument_mapping::validate_issuer_cursor(conn).is_err());
        Ok(())
    }))
    .expect("validate_issuer_cursor rejects slot 2");
    rt.block_on(db.close()).expect("close");
    assert_eq!(
        Database::open(&path, &key(), &clock())
            .expect_err("reopen must reject slot 2 issuer cursor")
            .failure(),
        SystemFailure::RepositoryIntegrity
    );
}

#[test]
fn injected_write_failures_and_zero_row_effects_roll_back_cleanly() {
    let rt = runtime();
    let sample = source(KnownMarket::Kr, "005930", "KR7005930003");

    for trigger in [fixtures::FAIL_INSERT_ABORT, fixtures::FAIL_INSERT_IGNORE] {
        let fixture = Fixture::new();
        let db = open_fixture(&fixture);
        let store = InstrumentMappingStore::new(&db);
        rt.block_on(db.write(move |tx| tx.execute_batch(trigger)))
            .expect("install insert trigger");
        assert_eq!(
            rt.block_on(store.ensure(&sample, issued_slot(0))),
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
        let cand = issued_slot(0);
        let sample_id = cand.as_instrument_id().clone();
        rt.block_on(store.ensure(&sample, cand))
            .expect("seed active");
        rt.block_on(db.write(move |tx| tx.execute_batch(trigger)))
            .expect("install retire trigger");
        assert_eq!(rt.block_on(store.retire(&sample)), Err(integrity_error()));
        assert_eq!(rt.block_on(store.resolve(&sample)), Ok(sample_id.clone()));
        assert_eq!(
            rt.block_on(store.lookup(&sample)),
            Ok(InstrumentMappingRecord::new(
                sample.clone(),
                sample_id,
                InstrumentMappingStatus::Active
            ))
        );
        rt.block_on(db.close()).expect("close");
    }
}

#[test]
fn injected_issuer_zero_row_effects_fail_closed() {
    let rt = runtime();
    let issuer_insert_fixture = Fixture::new();
    let issuer_insert_db = open_fixture(&issuer_insert_fixture);
    rt.block_on(issuer_insert_db.write(|tx| tx.execute_batch(fixtures::FAIL_ISSUER_INSERT_IGNORE)))
        .expect("install ignore issuer insert trigger");
    assert_eq!(
        rt.block_on(InstrumentIdIssuerStore::new(&issuer_insert_db).issue())
            .expect_err("zero-row issuer insert must fail closed")
            .failure(),
        SystemFailure::RepositoryIntegrity
    );
    rt.block_on(issuer_insert_db.close()).expect("close");

    let issuer_update_fixture = Fixture::new();
    let issuer_update_db = open_fixture(&issuer_update_fixture);
    let _seed = rt
        .block_on(InstrumentIdIssuerStore::new(&issuer_update_db).issue())
        .expect("seed issuer cursor");
    rt.block_on(issuer_update_db.write(|tx| tx.execute_batch(fixtures::FAIL_ISSUER_UPDATE_IGNORE)))
        .expect("install ignore issuer update trigger");
    assert_eq!(
        rt.block_on(InstrumentIdIssuerStore::new(&issuer_update_db).issue())
            .expect_err("zero-row issuer update must fail closed")
            .failure(),
        SystemFailure::RepositoryIntegrity
    );
    rt.block_on(issuer_update_db.close()).expect("close");
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
                params![SLOT_0_ID, "KR", "005930", "KR7005930003", "active"],
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
            let outcome = contender_rt.block_on(store.ensure(&contender_source, issued_slot(1)));
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
        Ok(MappingAssignment::Existing(id(SLOT_0_ID)))
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
                params![SLOT_0_ID, "US", "AAPL", "US0378331005", "active"],
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
            let outcome = contender_rt.block_on(store.ensure(&contender_source, issued_slot(1)));
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
    assert_eq!(second_result, Ok(MappingAssignment::Issued(id(SLOT_1_ID))));
    rt.block_on(first_db.close()).expect("close first");
    rt.block_on(second_db.close()).expect("close second");
}

#[test]
fn two_connections_serialize_issuer_immediate_lock() {
    let rt = runtime();
    let fixture = Fixture::new();
    let first_db = open_fixture(&fixture);
    let second_db = open_fixture(&fixture);
    let (locked_tx, locked_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let (started_tx, started_rx) = mpsc::channel();
    let (done_tx, done_rx) = mpsc::channel();
    let holder = first_db
        .writer
        .submit(move |tx| {
            tx.execute(sql::INSERT_ISSUER_CURSOR, ["0000000000000001"])?;
            locked_tx.send(()).expect("signal issuer lock held");
            release_rx.recv().expect("wait release");
            Ok(())
        })
        .expect("submit issuer holder");
    let contender_id = std::thread::scope(|scope| {
        let contender_db = &second_db;
        locked_rx.recv().expect("holder holds immediate lock");
        let handle = scope.spawn(move || {
            let contender_rt = runtime();
            let issuer = InstrumentIdIssuerStore::new(contender_db);
            started_tx.send(()).expect("signal issuer contender start");
            let outcome = contender_rt
                .block_on(issuer.issue())
                .expect("contender issue succeeds after commit")
                .as_instrument_id()
                .clone();
            done_tx.send(()).expect("signal issuer contender done");
            outcome
        });
        wait_contender_blocked_on_lock(&started_rx, &done_rx);
        release_tx.send(()).expect("commit issuer holder");
        rt.block_on(holder)
            .expect("holder recv")
            .expect("holder commit");
        done_rx.recv().expect("contender finishes after commit");
        handle.join().expect("contender thread")
    });
    assert_eq!(contender_id.as_str(), SLOT_1_ID);
    rt.block_on(first_db.close()).expect("close first");
    rt.block_on(second_db.close()).expect("close second");
}

#[test]
fn two_connections_issue_concurrent_batches_without_duplicates_across_reopen() {
    let rt = runtime();
    let fixture = Fixture::new();
    let first_db = open_fixture(&fixture);
    let second_db = open_fixture(&fixture);
    let (batch_a, batch_b) = std::thread::scope(|scope| {
        let db_a = &first_db;
        let db_b = &second_db;
        let left = scope.spawn(move || {
            let local_rt = runtime();
            let issuer = InstrumentIdIssuerStore::new(db_a);
            (0_u8..8_u8)
                .map(|_| {
                    local_rt
                        .block_on(issuer.issue())
                        .expect("issue a")
                        .as_instrument_id()
                        .clone()
                })
                .collect::<Vec<_>>()
        });
        let right = scope.spawn(move || {
            let local_rt = runtime();
            let issuer = InstrumentIdIssuerStore::new(db_b);
            (0_u8..8_u8)
                .map(|_| {
                    local_rt
                        .block_on(issuer.issue())
                        .expect("issue b")
                        .as_instrument_id()
                        .clone()
                })
                .collect::<Vec<_>>()
        });
        (left.join().expect("join a"), right.join().expect("join b"))
    });
    let mut seen = BTreeSet::new();
    for issued in batch_a.into_iter().chain(batch_b) {
        assert!(
            seen.insert(issued),
            "two-connection issuer must not duplicate"
        );
    }
    assert_eq!(seen.len(), 16);
    rt.block_on(first_db.close()).expect("close first");
    rt.block_on(second_db.close()).expect("close second");

    let reopened = open_fixture(&fixture);
    let next = rt
        .block_on(InstrumentIdIssuerStore::new(&reopened).issue())
        .expect("issue after reopen");
    assert_eq!(next.as_instrument_id().as_str(), "inst-0000000000000011");
    rt.block_on(reopened.close()).expect("close reopened");
}

#[test]
fn two_connections_report_busy_unavailable_when_lock_times_out() {
    let rt = runtime();
    let fixture = Fixture::new();
    let first_db = open_fixture(&fixture);
    let second_db = open_fixture(&fixture);
    let sample = source(KnownMarket::Kr, "005930", "KR7005930003");
    let first_store = InstrumentMappingStore::new(&first_db);
    rt.block_on(first_store.ensure(&sample, issued_slot(0)))
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
                params![SLOT_0_ID, "KR", "005930", "KR7005930003"],
            )?;
            locked_tx.send(()).expect("signal lock held");
            release_rx.recv().expect("wait release");
            Ok(())
        })
        .expect("submit retire holder");

    locked_rx.recv().expect("holder holds immediate lock");
    let second_store = InstrumentMappingStore::new(&second_db);
    assert_eq!(
        rt.block_on(second_store.ensure(&sample, issued_slot(1))),
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
