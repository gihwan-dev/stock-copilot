use crate::{
    Database, DatabasePath, InstrumentIdIssuerStore, InstrumentMappingStore,
    sql::{
        instrument_mapping as sql, instrument_mapping_history_tests as history_sql,
        instrument_mapping_tests as mapping_fixtures,
    },
    tests::{Fixture, clock, key, runtime},
};
use application::{
    InstrumentIdIssuer, InstrumentMappingHistoryRepository, InstrumentMappingRepository,
    MappingStoreError, SystemFailure, SystemPort, SystemPortError,
};
use domain::{
    CorporateAction, CorporateActionKind, DartReceiptNo, DisclosureKey, InstrumentId,
    InstrumentMappingError, InstrumentMappingRecord, InstrumentMappingStatus, InstrumentSource,
    IsinCode, IssuedInstrumentId, KnownMarket, MappingAssignment, MappingHistoryEvent, Nanosecond,
    OffsetSeconds, ReceiptNo, Symbol, Timestamp, UnixSeconds, UntrustedText, UserReason,
};
use rusqlite::{Connection, Transaction, params};
use std::collections::BTreeSet;
use testkit_ports::contracts::instrument_mapping::{
    HistoryContractDataset, empty_history_contract, history_contract_dataset,
    populated_history_contract,
};

fn open_fixture(fixture: &Fixture) -> Database {
    Database::open(&DatabasePath::new(fixture.path()), &key(), &clock()).expect("open database")
}

fn source(market: KnownMarket, symbol: &str, isin: &str) -> InstrumentSource {
    InstrumentSource::new(
        market,
        Symbol::try_new(symbol).expect("symbol"),
        IsinCode::try_new(isin).expect("isin"),
    )
}

fn id(raw: &str) -> InstrumentId {
    InstrumentId::try_new(raw).expect("id")
}

fn issued_slot(slot: u16) -> IssuedInstrumentId {
    let mut issuer = domain::IndependentInstrumentIdIssuer::new();
    for _ in 0..slot {
        let _skip = issuer.issue().expect("skip");
    }
    issuer.issue().expect("issue")
}

fn integrity_system_error() -> SystemPortError {
    SystemPortError::new(SystemPort::Repository, SystemFailure::RepositoryIntegrity)
}

const fn market_str(market: KnownMarket) -> &'static str {
    match market {
        KnownMarket::Kr => "KR",
        KnownMarket::Us => "US",
    }
}

fn disclosure_parts(key: &DisclosureKey) -> (&'static str, &str) {
    match key.receipt_no() {
        ReceiptNo::Dart(value) => ("DART", value.as_str()),
        ReceiptNo::Kind(value) => ("KIND", value.as_str()),
        ReceiptNo::SecEdgar(value) => ("SEC_EDGAR", value.as_str()),
    }
}

struct ActionRowParts {
    kind: &'static str,
    from_sym: Option<String>,
    to_sym: Option<String>,
    from_cnt: Option<String>,
    to_cnt: Option<String>,
}

fn action_row_parts(action: &CorporateAction) -> rusqlite::Result<ActionRowParts> {
    match action.kind() {
        CorporateActionKind::SymbolChange { from, to } => Ok(ActionRowParts {
            kind: "symbol_change",
            from_sym: Some(from.as_str().to_owned()),
            to_sym: Some(to.as_str().to_owned()),
            from_cnt: None,
            to_cnt: None,
        }),
        CorporateActionKind::Split(ratio) => Ok(ActionRowParts {
            kind: "split",
            from_sym: None,
            to_sym: None,
            from_cnt: Some(ratio.old_shares().as_u64().to_string()),
            to_cnt: Some(ratio.new_shares().as_u64().to_string()),
        }),
        CorporateActionKind::ReverseSplit(ratio) => Ok(ActionRowParts {
            kind: "reverse_split",
            from_sym: None,
            to_sym: None,
            from_cnt: Some(ratio.old_shares().as_u64().to_string()),
            to_cnt: Some(ratio.new_shares().as_u64().to_string()),
        }),
        CorporateActionKind::BonusIssue { .. }
        | CorporateActionKind::StockDividend { .. }
        | CorporateActionKind::Delisting
        | CorporateActionKind::Merger
        | CorporateActionKind::Demerger
        | CorporateActionKind::Exchange => Err(rusqlite::Error::InvalidQuery),
    }
}

struct EventInsert<'a> {
    seq: i64,
    kind: &'static str,
    instrument_id: &'a InstrumentId,
    previous: &'a InstrumentSource,
    next: Option<&'a InstrumentSource>,
    disclosure: &'a DisclosureKey,
    action: Option<&'a CorporateAction>,
    note: &'a UserReason,
    at: Timestamp,
}

impl<'a> EventInsert<'a> {
    fn from_event(seq: i64, event: &'a MappingHistoryEvent) -> Self {
        match event {
            MappingHistoryEvent::IdentifierChanged {
                instrument_id,
                previous,
                next,
                disclosure,
                action,
                note,
                confirmed_at,
            } => Self {
                seq,
                kind: "identifier_changed",
                instrument_id,
                previous,
                next: Some(next),
                disclosure,
                action: Some(action),
                note,
                at: *confirmed_at,
            },
            MappingHistoryEvent::RetirementCorrected {
                instrument_id,
                source,
                disclosure,
                note,
                corrected_at,
            } => Self {
                seq,
                kind: "retirement_corrected",
                instrument_id,
                previous: source,
                next: None,
                disclosure,
                action: None,
                note,
                at: *corrected_at,
            },
            MappingHistoryEvent::LinkCorrected {
                instrument_id,
                restored_previous,
                unlinked_next,
                disclosure,
                note,
                corrected_at,
            } => Self {
                seq,
                kind: "link_corrected",
                instrument_id,
                previous: restored_previous,
                next: Some(unlinked_next),
                disclosure,
                action: None,
                note,
                at: *corrected_at,
            },
        }
    }
}

fn execute_event_insert(tx: &Transaction<'_>, row: &EventInsert<'_>) -> rusqlite::Result<()> {
    let (disc_src, disc_no) = disclosure_parts(row.disclosure);
    let action_parts = row.action.map(action_row_parts).transpose()?;
    let date = row.action.map(CorporateAction::date);
    tx.execute(
        history_sql::INSERT_EVENT,
        params![
            row.seq,
            row.kind,
            row.instrument_id.as_str(),
            market_str(row.previous.market()),
            row.previous.symbol().as_str(),
            row.previous.isin().as_str(),
            row.next.map(|value| value.symbol().as_str()),
            row.next.map(|value| value.isin().as_str()),
            disc_src,
            disc_no,
            action_parts.as_ref().map(|parts| parts.kind),
            date.map(|value| i64::from(*value.year().as_i32())),
            date.map(|value| i64::from(*value.month().as_u8())),
            date.map(|value| i64::from(*value.day().as_u8())),
            action_parts
                .as_ref()
                .and_then(|parts| parts.from_sym.as_deref()),
            action_parts
                .as_ref()
                .and_then(|parts| parts.to_sym.as_deref()),
            action_parts
                .as_ref()
                .and_then(|parts| parts.from_cnt.as_deref()),
            action_parts
                .as_ref()
                .and_then(|parts| parts.to_cnt.as_deref()),
            row.note.as_untrusted_text().as_untrusted_str(),
            *row.at.utc_seconds().as_i64(),
            i64::from(*row.at.nanosecond().as_u32()),
            i64::from(*row.at.offset_seconds().as_i32()),
        ],
    )?;
    Ok(())
}

fn insert_event_row(
    tx: &Transaction<'_>,
    seq: i64,
    event: &MappingHistoryEvent,
) -> rusqlite::Result<()> {
    execute_event_insert(tx, &EventInsert::from_event(seq, event))
}

fn require_one(affected: usize) -> rusqlite::Result<()> {
    if affected == 1 {
        Ok(())
    } else {
        Err(rusqlite::Error::InvalidQuery)
    }
}

fn insert_active_mapping(
    tx: &Transaction<'_>,
    instrument_id: &InstrumentId,
    source: &InstrumentSource,
) -> rusqlite::Result<()> {
    let inserted = tx.execute(
        sql::INSERT,
        params![
            instrument_id.as_str(),
            market_str(source.market()),
            source.symbol().as_str(),
            source.isin().as_str(),
            "active",
        ],
    )?;
    require_one(inserted)
}

fn seed_initial_roots(
    tx: &Transaction<'_>,
    dataset: &HistoryContractDataset,
) -> rusqlite::Result<()> {
    let mut seeded_ids = BTreeSet::new();
    for event in &dataset.events {
        let inst_id = event.instrument_id();
        if seeded_ids.insert(inst_id.clone()) {
            let root = match event {
                MappingHistoryEvent::IdentifierChanged { previous, .. } => previous,
                MappingHistoryEvent::RetirementCorrected { source, .. } => source,
                MappingHistoryEvent::LinkCorrected { .. } => {
                    return Err(rusqlite::Error::InvalidQuery);
                }
            };
            insert_active_mapping(tx, inst_id, root)?;
        }
    }
    for record in &dataset.records {
        if seeded_ids.insert(record.id().clone()) {
            insert_active_mapping(tx, record.id(), record.source())?;
        }
    }
    Ok(())
}

fn apply_seed_identifier_changed(
    tx: &Transaction<'_>,
    seq: i64,
    instrument_id: &InstrumentId,
    previous: &InstrumentSource,
    next: &InstrumentSource,
) -> rusqlite::Result<()> {
    require_one(tx.execute(
        history_sql::MARK_SUPERSEDED,
        params![
            instrument_id.as_str(),
            market_str(previous.market()),
            previous.symbol().as_str(),
            previous.isin().as_str(),
            seq,
        ],
    )?)?;
    insert_active_mapping(tx, instrument_id, next)
}

fn apply_seed_retirement_corrected(
    tx: &Transaction<'_>,
    seq: i64,
    event: &MappingHistoryEvent,
    instrument_id: &InstrumentId,
    source: &InstrumentSource,
) -> rusqlite::Result<()> {
    require_one(tx.execute(
        sql::RETIRE,
        params![
            instrument_id.as_str(),
            market_str(source.market()),
            source.symbol().as_str(),
            source.isin().as_str(),
        ],
    )?)?;
    insert_event_row(tx, seq, event)?;
    require_one(tx.execute(
        history_sql::RESTORE_RETIRED_TO_ACTIVE,
        params![
            instrument_id.as_str(),
            market_str(source.market()),
            source.symbol().as_str(),
            source.isin().as_str(),
        ],
    )?)
}

fn apply_seed_link_corrected(
    tx: &Transaction<'_>,
    instrument_id: &InstrumentId,
    restored_previous: &InstrumentSource,
    unlinked_next: &InstrumentSource,
) -> rusqlite::Result<()> {
    require_one(tx.execute(
        history_sql::DELETE_UNLINKED_ACTIVE,
        params![
            instrument_id.as_str(),
            market_str(unlinked_next.market()),
            unlinked_next.symbol().as_str(),
            unlinked_next.isin().as_str(),
        ],
    )?)?;
    require_one(tx.execute(
        history_sql::RESTORE_SUPERSEDED_TO_ACTIVE,
        params![
            instrument_id.as_str(),
            market_str(restored_previous.market()),
            restored_previous.symbol().as_str(),
            restored_previous.isin().as_str(),
        ],
    )?)
}

fn apply_seed_event(
    tx: &Transaction<'_>,
    seq: i64,
    event: &MappingHistoryEvent,
) -> rusqlite::Result<()> {
    match event {
        MappingHistoryEvent::IdentifierChanged {
            instrument_id,
            previous,
            next,
            ..
        } => {
            insert_event_row(tx, seq, event)?;
            apply_seed_identifier_changed(tx, seq, instrument_id, previous, next)
        }
        MappingHistoryEvent::RetirementCorrected {
            instrument_id,
            source,
            ..
        } => apply_seed_retirement_corrected(tx, seq, event, instrument_id, source),
        MappingHistoryEvent::LinkCorrected {
            instrument_id,
            restored_previous,
            unlinked_next,
            ..
        } => {
            insert_event_row(tx, seq, event)?;
            apply_seed_link_corrected(tx, instrument_id, restored_previous, unlinked_next)
        }
    }
}

fn seed_contract_dataset(db: &Database, dataset: &HistoryContractDataset) {
    let rt = runtime();
    let cloned = dataset.clone();
    rt.block_on(db.write(move |tx| {
        seed_initial_roots(tx, &cloned)?;
        for (index, event) in cloned.events.iter().enumerate() {
            let seq = i64::try_from(index.checked_add(1).expect("seq overflow")).expect("seq");
            apply_seed_event(tx, seq, event)?;
        }
        Ok(())
    }))
    .expect("seed history dataset through real 0009 triggers");
}

#[test]
fn migration_0009_sql_matches_file_prepares_queries_and_enforces_strict() {
    assert_eq!(
        std::fs::read_to_string("migrations/0009_instrument_mapping_history.sql")
            .expect("0009 migration file"),
        sql::MIGRATE_0009
    );

    let fixture = Fixture::new();
    let rt = runtime();
    let db = open_fixture(&fixture);

    rt.block_on(db.read(|conn| {
        for statement in [
            sql::ALL_EVENTS,
            sql::EVENTS_FOR_ID,
            history_sql::INSERT_EVENT,
            history_sql::MARK_SUPERSEDED,
            history_sql::RESTORE_RETIRED_TO_ACTIVE,
            history_sql::DELETE_UNLINKED_ACTIVE,
            history_sql::RESTORE_SUPERSEDED_TO_ACTIVE,
        ] {
            conn.prepare(statement)?;
        }
        let strict: i64 =
            conn.query_row(history_sql::EVENTS_TABLE_STRICT_FLAG, [], |row| row.get(0))?;
        assert_eq!(strict, 1);
        Ok(())
    }))
    .expect("prepare and strict check");

    rt.block_on(db.write(|tx| tx.execute_batch(history_sql::INSERT_EVENT_STRICT_BLOB)))
        .expect_err("STRICT rejects blob instrument_id in events table");
    rt.block_on(db.close()).expect("close");
}

#[test]
fn migration_0009_queries_and_triggers_use_indexes_without_table_scans() {
    let fixture = Fixture::new();
    let rt = runtime();
    let db = open_fixture(&fixture);

    for (query, expected_index) in [
        (
            history_sql::EXPLAIN_EVENTS_FOR_ID,
            "instrument_mapping_events_instrument_seq",
        ),
        (
            history_sql::EXPLAIN_MONOTONIC_MAX_SEQ,
            "instrument_mapping_events",
        ),
        (
            history_sql::EXPLAIN_TRIGGER_SYMBOL_CONFLICT,
            "instrument_mappings_symbol_key",
        ),
        (
            history_sql::EXPLAIN_TRIGGER_ISIN_CONFLICT,
            "instrument_mappings_isin_key",
        ),
        (
            history_sql::EXPLAIN_TRIGGER_SAME_ID,
            "instrument_mappings_active_retired_id_key",
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
            !joined.contains("SCAN instrument_mappings")
                && !joined.contains("SCAN instrument_mapping_events"),
            "query must not full-scan tables: {joined}"
        );
        assert!(
            joined.contains(expected_index),
            "expected {expected_index} in plan: {joined}"
        );
    }
    rt.block_on(db.close()).expect("close");
}

fn seed_0008_database(fixture: &Fixture) {
    let at = crate::tests::timestamp();
    let mut conn = Connection::open(fixture.path()).expect("raw open");
    conn.execute_batch(crate::sql::tests::RAW_KEY)
        .expect("raw key");
    conn.execute_batch(crate::sql::SET_FOREIGN_KEYS)
        .expect("fk on");
    crate::migration::apply(&mut conn, at, &crate::migration::MIGRATIONS[..8])
        .expect("apply through 0008");
    conn.execute(
        sql::INSERT,
        params![
            "inst-0000000000000001",
            "KR",
            "005930",
            "KR7005930003",
            "active"
        ],
    )
    .expect("insert kr active");
    conn.execute(
        sql::INSERT,
        params![
            "inst-0000000000000002",
            "US",
            "brk.B",
            "us0846707026",
            "active"
        ],
    )
    .expect("insert us mixed active");
    conn.execute(
        sql::RETIRE,
        params!["inst-0000000000000002", "US", "brk.B", "us0846707026"],
    )
    .expect("retire us mixed");
    conn.execute(sql::INSERT_ISSUER_CURSOR, ["0000000000000005"])
        .expect("seed issuer cursor");
}

fn assert_migrated_rows_null_seq_and_trigger_active(db: &Database) {
    let rt = runtime();
    rt.block_on(db.read(|conn| {
        let kr_seq: Option<i64> = conn.query_row(
            history_sql::SELECT_SUPERSEDED_SEQ,
            params!["inst-0000000000000001", "KR", "005930", "KR7005930003"],
            |row| row.get(0),
        )?;
        let us_seq: Option<i64> = conn.query_row(
            history_sql::SELECT_SUPERSEDED_SEQ,
            params!["inst-0000000000000002", "US", "brk.B", "us0846707026"],
            |row| row.get(0),
        )?;
        assert_eq!(kr_seq, None);
        assert_eq!(us_seq, None);
        Ok(())
    }))
    .expect("migrated rows keep null superseded_event_seq");

    rt.block_on(db.write(|tx| {
        tx.execute(
            history_sql::UPSERT_COMPOSITE_PK,
            params![
                "inst-0000000000000002",
                "US",
                "brk.B",
                "us0846707026",
                "active"
            ],
        )?;
        Ok(())
    }))
    .expect_err("0009 triggers block reactivation of migrated retired row");
}

#[test]
fn migration_0008_to_0009_preserves_existing_active_retired_rows_and_issuer_cursor() {
    let fixture = Fixture::new();
    let path = DatabasePath::new(fixture.path());
    seed_0008_database(&fixture);

    let rt = runtime();
    let db = Database::open(&path, &key(), &clock()).expect("migrate 0008 -> 0009 on open");
    let store = InstrumentMappingStore::new(&db);
    let issuer = InstrumentIdIssuerStore::new(&db);
    let kr = source(KnownMarket::Kr, "005930", "KR7005930003");
    let us = source(KnownMarket::Us, "brk.B", "us0846707026");
    let kr_id = id("inst-0000000000000001");
    let us_id = id("inst-0000000000000002");

    assert_eq!(rt.block_on(store.resolve(&kr)), Ok(kr_id.clone()));
    assert_eq!(
        rt.block_on(store.lookup(&kr)),
        Ok(InstrumentMappingRecord::new(
            kr.clone(),
            kr_id.clone(),
            InstrumentMappingStatus::Active
        ))
    );
    assert_eq!(
        rt.block_on(store.resolve(&us)),
        Err(MappingStoreError::Mapping(InstrumentMappingError::Retired))
    );
    assert_eq!(
        rt.block_on(store.lookup(&us)),
        Ok(InstrumentMappingRecord::new(
            us.clone(),
            us_id,
            InstrumentMappingStatus::Retired
        ))
    );
    assert_eq!(rt.block_on(store.history()), Ok(Vec::new()));
    assert_eq!(rt.block_on(store.history_for_id(&kr_id)), Ok(Vec::new()));
    assert_migrated_rows_null_seq_and_trigger_active(&db);

    let next_id = rt.block_on(issuer.issue()).expect("issue after migration");
    assert_eq!(next_id.as_instrument_id().as_str(), "inst-0000000000000006");
    rt.block_on(db.close()).expect("close migrated");

    let reopened = Database::open(&path, &key(), &clock()).expect("reopen after 0009 migration");
    let reopened_store = InstrumentMappingStore::new(&reopened);
    assert_eq!(rt.block_on(reopened_store.resolve(&kr)), Ok(kr_id));
    assert_eq!(
        rt.block_on(reopened_store.resolve(&us)),
        Err(MappingStoreError::Mapping(InstrumentMappingError::Retired))
    );
    rt.block_on(reopened.close()).expect("close reopened");
}

#[test]
fn sqlite_store_passes_shared_history_contracts_across_restart() {
    let rt = runtime();
    let empty_fixture = Fixture::new();
    let empty_db = open_fixture(&empty_fixture);
    let empty_store = InstrumentMappingStore::new(&empty_db);
    rt.block_on(empty_history_contract(&empty_store, &empty_store));
    rt.block_on(empty_db.close()).expect("close empty");

    let dataset = history_contract_dataset().expect("shared dataset");
    let fixture = Fixture::new();
    let path = DatabasePath::new(fixture.path());
    let db = Database::open(&path, &key(), &clock()).expect("open");
    seed_contract_dataset(&db, &dataset);
    let store = InstrumentMappingStore::new(&db);
    rt.block_on(populated_history_contract(&store, &dataset));
    rt.block_on(db.close()).expect("close seeded");

    let reopened = Database::open(&path, &key(), &clock()).expect("reopen seeded history db");
    let reopened_store = InstrumentMappingStore::new(&reopened);
    rt.block_on(populated_history_contract(&reopened_store, &dataset));
    rt.block_on(reopened.close()).expect("close reopened");
}

#[test]
fn sqlite_store_enforces_superseded_and_unlinked_mapping_operations_across_restart() {
    let rt = runtime();
    let dataset = history_contract_dataset().expect("shared dataset");
    let fixture = Fixture::new();
    let path = DatabasePath::new(fixture.path());
    let db = Database::open(&path, &key(), &clock()).expect("open");
    seed_contract_dataset(&db, &dataset);
    rt.block_on(db.close()).expect("close seeded");

    let reopened = Database::open(&path, &key(), &clock()).expect("reopen seeded history db");
    let store = InstrumentMappingStore::new(&reopened);
    let us_prev = source(KnownMarket::Us, "fb.A", "us30303m1027");
    let us_next = source(KnownMarket::Us, "META", "us30303m1027");
    let us_mistake = source(KnownMarket::Us, "MIST", "US30303M9999");
    let sup = MappingStoreError::Mapping(InstrumentMappingError::Superseded);

    assert_eq!(rt.block_on(store.resolve(&us_prev)), Err(sup.clone()));
    assert_eq!(rt.block_on(store.retire(&us_prev)), Err(sup.clone()));
    assert_eq!(
        rt.block_on(store.ensure(&us_prev, issued_slot(20))),
        Err(sup)
    );
    let prev_rec = InstrumentMappingRecord::new(
        us_prev.clone(),
        dataset.primary_id.clone(),
        InstrumentMappingStatus::Superseded,
    );
    let next_rec = InstrumentMappingRecord::new(
        us_next.clone(),
        dataset.primary_id.clone(),
        InstrumentMappingStatus::Active,
    );
    assert_eq!(rt.block_on(store.lookup(&us_prev)), Ok(prev_rec.clone()));
    assert_eq!(
        rt.block_on(store.resolve(&us_next)),
        Ok(dataset.primary_id.clone())
    );
    let primary_records = rt
        .block_on(store.for_id(&dataset.primary_id))
        .expect("for_id primary");
    assert_eq!(primary_records.len(), 2);
    assert!(primary_records.contains(&prev_rec) && primary_records.contains(&next_rec));
    assert_eq!(
        rt.block_on(store.lookup(&us_mistake)),
        Err(MappingStoreError::Mapping(InstrumentMappingError::Missing))
    );
    let freed_cand = issued_slot(21);
    let freed_id = freed_cand.as_instrument_id().clone();
    assert_eq!(
        rt.block_on(store.ensure(&us_mistake, freed_cand)),
        Ok(MappingAssignment::Issued(freed_id))
    );
    let steal_superseded_sym = source(KnownMarket::Us, "FB.A", "US9999999999");
    assert_eq!(
        rt.block_on(store.ensure(&steal_superseded_sym, issued_slot(22))),
        Err(MappingStoreError::Mapping(
            InstrumentMappingError::IdentityUncertain
        ))
    );
    rt.block_on(reopened.close()).expect("close reopened");
}

#[test]
fn event_table_triggers_enforce_append_only_and_monotonic_seq() {
    let rt = runtime();
    let dataset = history_contract_dataset().expect("shared dataset");
    let fixture = Fixture::new();
    let db = open_fixture(&fixture);
    seed_contract_dataset(&db, &dataset);

    rt.block_on(db.write(|tx| {
        tx.execute(history_sql::UPDATE_EVENT_NOTE, params![1_i64, "변조 사유"])?;
        Ok(())
    }))
    .expect_err("events table rejects UPDATE");
    rt.block_on(db.write(|tx| {
        tx.execute(history_sql::DELETE_EVENT_BY_SEQ, [1_i64])?;
        Ok(())
    }))
    .expect_err("events table rejects DELETE");

    for bad_seq in [0_i64, -1, 1, 5, 7] {
        let ev = dataset.events.last().expect("seeded event").clone();
        rt.block_on(db.write(move |tx| insert_event_row(tx, bad_seq, &ev)))
            .expect_err("events table rejects non-consecutive or invalid seq");
    }

    rt.block_on(db.write(|tx| {
        tx.execute(
            history_sql::REPLACE_EVENT,
            params![
                1_i64,
                "retirement_corrected",
                "inst-0000000000000001",
                "KR",
                "005930",
                "KR7005930003",
                Option::<&str>::None,
                Option::<&str>::None,
                "DART",
                "20261124000001",
                Option::<&str>::None,
                Option::<i64>::None,
                Option::<i64>::None,
                Option::<i64>::None,
                Option::<&str>::None,
                Option::<&str>::None,
                Option::<&str>::None,
                Option::<&str>::None,
                "덮어쓰기 시도",
                1_763_942_400_i64,
                0_i64,
                32_400_i64,
            ],
        )?;
        Ok(())
    }))
    .expect_err("events table rejects INSERT OR REPLACE");
    rt.block_on(db.close()).expect("close");
}

#[test]
fn mapping_triggers_block_stale_superseded_seq_cross_id_overlap_and_active_with_seq() {
    let rt = runtime();
    let dataset = history_contract_dataset().expect("shared dataset");
    let fixture = Fixture::new();
    let db = open_fixture(&fixture);
    seed_contract_dataset(&db, &dataset);

    let primary_id = dataset.primary_id.as_str().to_owned();
    rt.block_on(db.write(move |tx| {
        tx.execute(
            history_sql::MARK_SUPERSEDED,
            params![primary_id, "US", "META", "us30303m1027", 1_i64],
        )?;
        Ok(())
    }))
    .expect_err("retire_only trigger blocks stale event seq reuse");
    rt.block_on(db.write(|tx| {
        tx.execute(
            sql::INSERT,
            params![
                "inst-0000000000000099",
                "US",
                "FB.A",
                "US9999999999",
                "active"
            ],
        )?;
        Ok(())
    }))
    .expect_err("no_replace trigger blocks cross-id conflict with superseded symbol");
    rt.block_on(db.write(|tx| {
        tx.execute(
            history_sql::INSERT_WITH_SUPERSEDED_SEQ,
            params![
                "inst-0000000000000099",
                "KR",
                "000660",
                "KR7000660001",
                "active",
                1_i64
            ],
        )?;
        Ok(())
    }))
    .expect_err("CHECK and trigger block active row with non-null superseded_event_seq");
    rt.block_on(db.close()).expect("close");
}

struct BadEventCase {
    event_kind: &'static str,
    disc_src: &'static str,
    disc_no: &'static str,
    note: String,
    nanos: i64,
    offset: i64,
    next_sym: Option<&'static str>,
    next_isin: Option<&'static str>,
    action_kind: Option<&'static str>,
    from_cnt: Option<&'static str>,
    to_cnt: Option<&'static str>,
}

impl BadEventCase {
    fn base_retirement() -> Self {
        Self {
            event_kind: "retirement_corrected",
            disc_src: "DART",
            disc_no: "20261124000001",
            note: String::from("ok"),
            nanos: 0,
            offset: 0,
            next_sym: None,
            next_isin: None,
            action_kind: None,
            from_cnt: None,
            to_cnt: None,
        }
    }
}

fn malformed_event_core_cases() -> Vec<BadEventCase> {
    vec![
        BadEventCase {
            event_kind: "unknown_kind",
            ..BadEventCase::base_retirement()
        },
        BadEventCase {
            disc_no: "2026112400001",
            ..BadEventCase::base_retirement()
        },
        BadEventCase {
            disc_src: "SEC_EDGAR",
            ..BadEventCase::base_retirement()
        },
        BadEventCase {
            note: String::from("   \t\n "),
            ..BadEventCase::base_retirement()
        },
        BadEventCase {
            note: "a".repeat(513),
            ..BadEventCase::base_retirement()
        },
        BadEventCase {
            nanos: 1_000_000_000,
            ..BadEventCase::base_retirement()
        },
        BadEventCase {
            offset: 61,
            ..BadEventCase::base_retirement()
        },
        BadEventCase {
            next_sym: Some("EXTRA"),
            next_isin: Some("KR7005930999"),
            ..BadEventCase::base_retirement()
        },
    ]
}

fn malformed_event_action_cases() -> Vec<BadEventCase> {
    vec![
        BadEventCase {
            event_kind: "link_corrected",
            next_sym: Some("005930"),
            next_isin: Some("kr7005930003"),
            ..BadEventCase::base_retirement()
        },
        BadEventCase {
            event_kind: "identifier_changed",
            next_sym: Some("005930"),
            next_isin: Some("kr7005930003"),
            action_kind: Some("split"),
            from_cnt: Some("1"),
            to_cnt: Some("2"),
            ..BadEventCase::base_retirement()
        },
        BadEventCase {
            event_kind: "identifier_changed",
            next_sym: Some("005931"),
            next_isin: Some("KR7005930003"),
            action_kind: Some("split"),
            from_cnt: Some("01"),
            to_cnt: Some("2"),
            ..BadEventCase::base_retirement()
        },
        BadEventCase {
            event_kind: "identifier_changed",
            next_sym: Some("005931"),
            next_isin: Some("KR7005930003"),
            action_kind: Some("split"),
            from_cnt: Some("10"),
            to_cnt: Some("2"),
            ..BadEventCase::base_retirement()
        },
        BadEventCase {
            event_kind: "identifier_changed",
            next_sym: Some("005931"),
            next_isin: Some("KR7005930003"),
            action_kind: Some("reverse_split"),
            from_cnt: Some("2"),
            to_cnt: Some("10"),
            ..BadEventCase::base_retirement()
        },
    ]
}

fn malformed_event_cases() -> Vec<BadEventCase> {
    let mut cases = malformed_event_core_cases();
    cases.extend(malformed_event_action_cases());
    cases
}

#[test]
fn event_table_checks_reject_malformed_event_rows() {
    let rt = runtime();
    let dataset = history_contract_dataset().expect("shared dataset");
    let fixture = Fixture::new();
    let db = open_fixture(&fixture);
    seed_contract_dataset(&db, &dataset);

    for case in malformed_event_cases() {
        rt.block_on(db.write(move |tx| {
            tx.execute(
                history_sql::INSERT_EVENT,
                params![
                    6_i64,
                    case.event_kind,
                    "inst-0000000000000001",
                    "KR",
                    "005930",
                    "KR7005930003",
                    case.next_sym,
                    case.next_isin,
                    case.disc_src,
                    case.disc_no,
                    case.action_kind,
                    case.action_kind.map(|_| 2026_i64),
                    case.action_kind.map(|_| 11_i64),
                    case.action_kind.map(|_| 24_i64),
                    Option::<&str>::None,
                    Option::<&str>::None,
                    case.from_cnt,
                    case.to_cnt,
                    case.note,
                    1_763_942_400_i64,
                    case.nanos,
                    case.offset,
                ],
            )?;
            Ok(())
        }))
        .expect_err("CHECK must reject malformed event row");
    }
    rt.block_on(db.close()).expect("close");
}

fn assert_event_tamper_rejected<F>(dataset: &HistoryContractDataset, label: &str, mutate: F)
where
    F: FnOnce(&Transaction<'_>) -> rusqlite::Result<()> + Send + 'static,
{
    let rt = runtime();
    let fixture = Fixture::new();
    let path = DatabasePath::new(fixture.path());
    let db = Database::open(&path, &key(), &clock()).expect("open");
    seed_contract_dataset(&db, dataset);
    rt.block_on(db.write(move |tx| {
        tx.execute_batch(history_sql::DROP_EVENT_TRIGGERS)?;
        tx.execute_batch(mapping_fixtures::IGNORE_CHECKS_ON)?;
        mutate(tx)?;
        tx.execute_batch(mapping_fixtures::IGNORE_CHECKS_OFF)?;
        tx.execute_batch(history_sql::RESTORE_EVENT_TRIGGERS)?;
        Ok(())
    }))
    .expect(label);

    let store = InstrumentMappingStore::new(&db);
    assert_eq!(
        rt.block_on(store.history()),
        Err(integrity_system_error()),
        "history() must reject {label}"
    );
    rt.block_on(db.close()).expect("close");
    assert_eq!(
        Database::open(&path, &key(), &clock())
            .expect_err(label)
            .failure(),
        SystemFailure::RepositoryIntegrity,
        "open must reject {label}"
    );
}

#[test]
fn open_and_history_reads_reject_tampered_event_seq_kind_and_action_columns() {
    let dataset = history_contract_dataset().expect("shared dataset");
    assert_event_tamper_rejected(&dataset, "seq gap 5 -> 6", |tx| {
        tx.execute(history_sql::UPDATE_EVENT_SEQ, params![5_i64, 6_i64])?;
        Ok(())
    });
    assert_event_tamper_rejected(&dataset, "bad event kind", |tx| {
        tx.execute(history_sql::UPDATE_EVENT_KIND, params![4_i64, "forged"])?;
        Ok(())
    });
    assert_event_tamper_rejected(&dataset, "extra next_symbol on retirement", |tx| {
        tx.execute(
            history_sql::UPDATE_EVENT_NEXT_SYMBOL,
            params![4_i64, "EXTRA"],
        )?;
        Ok(())
    });
    assert_event_tamper_rejected(&dataset, "extra action_kind on link_corrected", |tx| {
        tx.execute(
            history_sql::UPDATE_EVENT_ACTION_KIND,
            params![5_i64, "split"],
        )?;
        Ok(())
    });
    assert_event_tamper_rejected(&dataset, "mismatched symbol_change from_symbol", |tx| {
        tx.execute(
            history_sql::UPDATE_EVENT_ACTION_FROM_SYMBOL,
            params![1_i64, "WRONG"],
        )?;
        Ok(())
    });
    assert_event_tamper_rejected(&dataset, "leading zero share count", |tx| {
        tx.execute(
            history_sql::UPDATE_EVENT_ACTION_FROM_COUNT,
            params![3_i64, "01"],
        )?;
        Ok(())
    });
}

#[test]
fn open_and_history_reads_reject_tampered_event_overflow_date_disclosure_note_and_time() {
    let dataset = history_contract_dataset().expect("shared dataset");
    assert_event_tamper_rejected(&dataset, "overflow share count", |tx| {
        tx.execute(
            history_sql::UPDATE_EVENT_ACTION_FROM_COUNT,
            params![3_i64, "18446744073709551616"],
        )?;
        Ok(())
    });
    assert_event_tamper_rejected(&dataset, "invalid calendar date 2026-11-31", |tx| {
        tx.execute(history_sql::UPDATE_EVENT_ACTION_DAY, params![3_i64, 31_i64])?;
        Ok(())
    });
    assert_event_tamper_rejected(&dataset, "bad disclosure source", |tx| {
        tx.execute(
            history_sql::UPDATE_EVENT_DISCLOSURE_SOURCE,
            params![4_i64, "UNKNOWN"],
        )?;
        Ok(())
    });
    assert_event_tamper_rejected(&dataset, "whitespace-only unicode note", |tx| {
        tx.execute(history_sql::UPDATE_EVENT_NOTE, params![4_i64, "\u{3000}"])?;
        Ok(())
    });
    assert_event_tamper_rejected(&dataset, "nanos out of range", |tx| {
        tx.execute(
            history_sql::UPDATE_EVENT_RECORDED_NANOS,
            params![4_i64, 1_000_000_000_i64],
        )?;
        Ok(())
    });
    assert_event_tamper_rejected(&dataset, "offset non-minute multiple", |tx| {
        tx.execute(
            history_sql::UPDATE_EVENT_RECORDED_OFFSET,
            params![4_i64, 61_i64],
        )?;
        Ok(())
    });
}

#[test]
fn open_rejects_mismatched_superseded_event_seq_and_unbacked_superseded_rows() {
    let rt = runtime();
    let dataset = history_contract_dataset().expect("shared dataset");

    let mismatch_fixture = Fixture::new();
    let mismatch_path = DatabasePath::new(mismatch_fixture.path());
    let mismatch_db = Database::open(&mismatch_path, &key(), &clock()).expect("open");
    seed_contract_dataset(&mismatch_db, &dataset);
    let primary_id = dataset.primary_id.as_str().to_owned();
    rt.block_on(mismatch_db.write(move |tx| {
        tx.execute_batch(mapping_fixtures::DROP_GUARD_TRIGGERS)?;
        tx.execute(
            history_sql::UPDATE_SUPERSEDED_SEQ_DIRECT,
            params![primary_id, "US", "fb.A", "us30303m1027", 3_i64],
        )?;
        tx.execute_batch(mapping_fixtures::RESTORE_GUARD_TRIGGERS)?;
        Ok(())
    }))
    .expect("point superseded_event_seq to cancelled event 3");
    rt.block_on(mismatch_db.close()).expect("close");
    assert_eq!(
        Database::open(&mismatch_path, &key(), &clock())
            .expect_err("open must reject mismatched superseded_event_seq")
            .failure(),
        SystemFailure::RepositoryIntegrity
    );

    let extra_row_fixture = Fixture::new();
    let extra_row_path = DatabasePath::new(extra_row_fixture.path());
    let extra_row_db = Database::open(&extra_row_path, &key(), &clock()).expect("open");
    seed_contract_dataset(&extra_row_db, &dataset);
    let secondary_id = dataset.secondary_id.as_str().to_owned();
    rt.block_on(extra_row_db.write(move |tx| {
        tx.execute_batch(mapping_fixtures::DROP_GUARD_TRIGGERS)?;
        tx.execute(
            history_sql::INSERT_WITH_SUPERSEDED_SEQ,
            params![
                secondary_id,
                "KR",
                "005939",
                "KR7005930777",
                "superseded",
                3_i64
            ],
        )?;
        tx.execute_batch(mapping_fixtures::RESTORE_GUARD_TRIGGERS)?;
        Ok(())
    }))
    .expect("inject extra superseded row pointing to cancelled seq 3");
    rt.block_on(extra_row_db.close()).expect("close");
    assert_eq!(
        Database::open(&extra_row_path, &key(), &clock())
            .expect_err("open must reject extra superseded row not backed by replay")
            .failure(),
        SystemFailure::RepositoryIntegrity
    );
}

#[test]
fn open_and_history_reads_reject_extra_action_and_next_columns_on_events() {
    let dataset = history_contract_dataset().expect("shared dataset");
    assert_event_tamper_rejected(&dataset, "extra next_isin on retirement", |tx| {
        tx.execute(
            history_sql::UPDATE_EVENT_NEXT_ISIN,
            params![4_i64, "KR7005930999"],
        )?;
        Ok(())
    });
    assert_event_tamper_rejected(&dataset, "extra action_year on retirement", |tx| {
        tx.execute(
            history_sql::UPDATE_EVENT_ACTION_YEAR,
            params![4_i64, 2026_i64],
        )?;
        Ok(())
    });
    assert_event_tamper_rejected(&dataset, "extra action_month on retirement", |tx| {
        tx.execute(
            history_sql::UPDATE_EVENT_ACTION_MONTH,
            params![4_i64, 11_i64],
        )?;
        Ok(())
    });
    assert_event_tamper_rejected(&dataset, "extra action_day on retirement", |tx| {
        tx.execute(history_sql::UPDATE_EVENT_ACTION_DAY, params![4_i64, 24_i64])?;
        Ok(())
    });
    assert_event_tamper_rejected(&dataset, "extra action_from_symbol on retirement", |tx| {
        tx.execute(
            history_sql::UPDATE_EVENT_ACTION_FROM_SYMBOL,
            params![4_i64, "EXTRA"],
        )?;
        Ok(())
    });
    assert_event_tamper_rejected(&dataset, "extra action_to_symbol on retirement", |tx| {
        tx.execute(
            history_sql::UPDATE_EVENT_ACTION_TO_SYMBOL,
            params![4_i64, "EXTRA"],
        )?;
        Ok(())
    });
    assert_event_tamper_rejected(&dataset, "extra action_from_count on retirement", |tx| {
        tx.execute(
            history_sql::UPDATE_EVENT_ACTION_FROM_COUNT,
            params![4_i64, "1"],
        )?;
        Ok(())
    });
    assert_event_tamper_rejected(&dataset, "extra action_to_count on retirement", |tx| {
        tx.execute(
            history_sql::UPDATE_EVENT_ACTION_TO_COUNT,
            params![4_i64, "2"],
        )?;
        Ok(())
    });
}

#[test]
fn open_and_history_reads_reject_cross_kind_action_columns_and_plus_share_count() {
    let dataset = history_contract_dataset().expect("shared dataset");
    assert_event_tamper_rejected(&dataset, "plus sign in share count", |tx| {
        tx.execute(
            history_sql::UPDATE_EVENT_ACTION_FROM_COUNT,
            params![3_i64, "+1"],
        )?;
        Ok(())
    });
    assert_event_tamper_rejected(&dataset, "symbol_change with from_count", |tx| {
        tx.execute(
            history_sql::UPDATE_EVENT_ACTION_FROM_COUNT,
            params![1_i64, "1"],
        )?;
        Ok(())
    });
    assert_event_tamper_rejected(&dataset, "symbol_change with to_count", |tx| {
        tx.execute(
            history_sql::UPDATE_EVENT_ACTION_TO_COUNT,
            params![1_i64, "2"],
        )?;
        Ok(())
    });
    assert_event_tamper_rejected(&dataset, "symbol_change mismatched to_symbol", |tx| {
        tx.execute(
            history_sql::UPDATE_EVENT_ACTION_TO_SYMBOL,
            params![1_i64, "WRONG"],
        )?;
        Ok(())
    });
    assert_event_tamper_rejected(&dataset, "split with from_symbol", |tx| {
        tx.execute(
            history_sql::UPDATE_EVENT_ACTION_FROM_SYMBOL,
            params![3_i64, "META"],
        )?;
        Ok(())
    });
    assert_event_tamper_rejected(&dataset, "split with to_symbol", |tx| {
        tx.execute(
            history_sql::UPDATE_EVENT_ACTION_TO_SYMBOL,
            params![3_i64, "MIST"],
        )?;
        Ok(())
    });
    assert_event_tamper_rejected(&dataset, "identifier_changed same normalized pair", |tx| {
        tx.execute(
            history_sql::UPDATE_EVENT_NEXT_PAIR,
            params![1_i64, "FB.A", "US30303M1027"],
        )?;
        Ok(())
    });
    assert_event_tamper_rejected(&dataset, "link_corrected identical source pair", |tx| {
        tx.execute(
            history_sql::UPDATE_EVENT_NEXT_PAIR,
            params![5_i64, "META", "us30303m1027"],
        )?;
        Ok(())
    });
}

fn write_zero_superseded_seq(path: &std::path::Path, id: &InstrumentId) {
    let raw = Connection::open(path).expect("raw open");
    raw.execute_batch(crate::sql::tests::RAW_KEY)
        .expect("raw key");
    raw.execute_batch(history_sql::FOREIGN_KEYS_OFF)
        .expect("fk off");
    raw.execute_batch(mapping_fixtures::DROP_GUARD_TRIGGERS)
        .expect("drop triggers");
    raw.execute_batch(mapping_fixtures::IGNORE_CHECKS_ON)
        .expect("checks off");
    raw.execute(
        history_sql::UPDATE_SUPERSEDED_SEQ_DIRECT,
        params![id.as_str(), "US", "fb.A", "us30303m1027", 0_i64],
    )
    .expect("set zero superseded_event_seq");
    raw.execute_batch(mapping_fixtures::IGNORE_CHECKS_OFF)
        .expect("checks on");
    raw.execute_batch(mapping_fixtures::RESTORE_GUARD_TRIGGERS)
        .expect("restore triggers");
    raw.execute_batch(history_sql::FOREIGN_KEYS_ON)
        .expect("fk on");
}

#[test]
fn decoder_rejects_zero_superseded_seq() {
    let rt = runtime();
    let dataset = history_contract_dataset().expect("shared dataset");
    let zero_seq_fixture = Fixture::new();
    let zero_seq_path = DatabasePath::new(zero_seq_fixture.path());
    let zero_seq_db = Database::open(&zero_seq_path, &key(), &clock()).expect("open");
    seed_contract_dataset(&zero_seq_db, &dataset);
    rt.block_on(zero_seq_db.close()).expect("close");
    write_zero_superseded_seq(&zero_seq_fixture.path(), &dataset.primary_id);
    assert_eq!(
        Database::open(&zero_seq_path, &key(), &clock())
            .expect_err("zero superseded_event_seq must fail closed")
            .failure(),
        SystemFailure::RepositoryIntegrity
    );

    let raw_lookup_fixture = Fixture::new();
    let raw_lookup_db = open_fixture(&raw_lookup_fixture);
    rt.block_on(raw_lookup_db.write(|tx| {
        tx.execute_batch(mapping_fixtures::RECREATE_UNCONSTRAINED_STRICT)?;
        tx.execute(
            history_sql::INSERT_WITH_SUPERSEDED_SEQ,
            params![
                "inst-0000000000000001",
                "US",
                "fb.A",
                "us30303m1027",
                "superseded",
                0_i64
            ],
        )?;
        Ok(())
    }))
    .expect("seed zero superseded_event_seq row in unconstrained table");
    let store = InstrumentMappingStore::new(&raw_lookup_db);
    let us_prev = source(KnownMarket::Us, "fb.A", "us30303m1027");
    assert_eq!(
        rt.block_on(store.lookup(&us_prev)),
        Err(MappingStoreError::Storage(integrity_system_error()))
    );
    rt.block_on(raw_lookup_db.close()).expect("close");
}

#[test]
fn open_rejects_active_tip_mismatch_against_retirement_corrected_event() {
    let rt = runtime();
    let fixture = Fixture::new();
    let path = DatabasePath::new(fixture.path());
    let db = Database::open(&path, &key(), &clock()).expect("open");
    let target_id = id("inst-0000000000000001");
    let original = source(KnownMarket::Kr, "005930", "KR7005930003");
    let event = MappingHistoryEvent::RetirementCorrected {
        instrument_id: target_id.clone(),
        source: original.clone(),
        disclosure: DisclosureKey::from_receipt(ReceiptNo::Dart(
            DartReceiptNo::try_new("20261124000088").expect("dart"),
        )),
        note: UserReason::try_new(UntrustedText::try_new("팁 불일치 시험").expect("text"))
            .expect("note"),
        corrected_at: Timestamp::from_parts(
            UnixSeconds::new(1_763_942_400),
            Nanosecond::new(0),
            OffsetSeconds::new(32_400),
        )
        .expect("ts"),
    };
    rt.block_on(db.write(move |tx| {
        insert_active_mapping(tx, &target_id, &original)?;
        apply_seed_retirement_corrected(tx, 1, &event, &target_id, &original)?;
        tx.execute_batch(mapping_fixtures::DROP_GUARD_TRIGGERS)?;
        tx.execute(
            mapping_fixtures::UPDATE_CORRUPT_SYMBOL,
            params![target_id.as_str(), "005931"],
        )?;
        tx.execute_batch(mapping_fixtures::RESTORE_GUARD_TRIGGERS)?;
        Ok(())
    }))
    .expect("corrupt active tip after retirement_corrected");
    rt.block_on(db.close()).expect("close");
    assert_eq!(
        Database::open(&path, &key(), &clock())
            .expect_err("open must reject active tip mismatch via verify_persisted_mapping_ledger")
            .failure(),
        SystemFailure::RepositoryIntegrity
    );
}

#[test]
fn history_for_id_rejects_duplicate_seq_in_unconstrained_events() {
    let rt = runtime();
    let dataset = history_contract_dataset().expect("shared dataset");
    let dup_seq_fixture = Fixture::new();
    let dup_seq_db = open_fixture(&dup_seq_fixture);
    let first_ev = dataset.events.first().expect("first event").clone();
    let dup_ev = first_ev.clone();
    rt.block_on(dup_seq_db.write(move |tx| {
        tx.execute_batch(history_sql::RECREATE_UNCONSTRAINED_EVENTS)?;
        insert_event_row(tx, 1, &first_ev)?;
        insert_event_row(tx, 1, &dup_ev)?;
        Ok(())
    }))
    .expect("seed duplicate seq in unconstrained events table");
    let dup_store = InstrumentMappingStore::new(&dup_seq_db);
    assert_eq!(
        rt.block_on(dup_store.history_for_id(&dataset.primary_id)),
        Err(integrity_system_error())
    );
    rt.block_on(dup_seq_db.close()).expect("close");
}

#[test]
fn verify_superseded_event_links_rejects_unmatched_correction_and_duplicate_links() {
    let dataset = history_contract_dataset().expect("shared dataset");
    let unmatched_link = crate::instrument_mapping::StoredEvent {
        seq: 1,
        event: dataset.events.get(4).expect("link_corrected event").clone(),
    };
    assert!(
        crate::instrument_mapping::verify_superseded_event_links(&[], &[unmatched_link]).is_err()
    );

    let dup_superseded_rec = crate::instrument_mapping::StoredRecord {
        source: source(KnownMarket::Us, "fb.A", "us30303m1027"),
        id: dataset.primary_id.clone(),
        status: crate::instrument_mapping::StoredStatus::Superseded(1),
    };
    let dup_superseded_rec_2 = crate::instrument_mapping::StoredRecord {
        source: source(KnownMarket::Us, "fb.A", "us30303m1027"),
        id: dataset.primary_id,
        status: crate::instrument_mapping::StoredStatus::Superseded(1),
    };
    let valid_change = crate::instrument_mapping::StoredEvent {
        seq: 1,
        event: dataset.events.first().expect("first event").clone(),
    };
    assert!(
        crate::instrument_mapping::verify_superseded_event_links(
            &[dup_superseded_rec, dup_superseded_rec_2],
            &[valid_change],
        )
        .is_err()
    );
}

#[test]
fn open_and_history_reads_reject_missing_action_and_next_fields_and_non_positive_seq() {
    let rt = runtime();
    let dataset = history_contract_dataset().expect("shared dataset");
    assert_event_tamper_rejected(
        &dataset,
        "missing next_symbol on identifier_changed",
        |tx| {
            tx.execute(history_sql::CLEAR_EVENT_NEXT_SYMBOL, [1_i64])?;
            Ok(())
        },
    );
    assert_event_tamper_rejected(
        &dataset,
        "missing action_kind on identifier_changed",
        |tx| {
            tx.execute(history_sql::CLEAR_EVENT_ACTION_KIND, [1_i64])?;
            Ok(())
        },
    );
    assert_event_tamper_rejected(
        &dataset,
        "unknown action_kind on identifier_changed",
        |tx| {
            tx.execute(
                history_sql::UPDATE_EVENT_ACTION_KIND,
                params![1_i64, "dividend"],
            )?;
            Ok(())
        },
    );
    assert_event_tamper_rejected(&dataset, "missing from_symbol on symbol_change", |tx| {
        tx.execute(history_sql::CLEAR_EVENT_ACTION_FROM_SYMBOL, [1_i64])?;
        Ok(())
    });
    assert_event_tamper_rejected(&dataset, "missing from_count on split", |tx| {
        tx.execute(history_sql::CLEAR_EVENT_ACTION_FROM_COUNT, [3_i64])?;
        Ok(())
    });

    let zero_ev_fixture = Fixture::new();
    let zero_ev_db = open_fixture(&zero_ev_fixture);
    let first_ev = dataset.events.first().expect("first event").clone();
    rt.block_on(zero_ev_db.write(move |tx| {
        tx.execute_batch(history_sql::RECREATE_UNCONSTRAINED_EVENTS)?;
        insert_event_row(tx, 0, &first_ev)?;
        Ok(())
    }))
    .expect("seed seq 0 event");
    let store = InstrumentMappingStore::new(&zero_ev_db);
    assert_eq!(
        rt.block_on(store.history_for_id(&dataset.primary_id)),
        Err(integrity_system_error())
    );
    rt.block_on(zero_ev_db.close()).expect("close");
}

#[derive(Clone, Copy)]
struct RawMappingRow {
    market: &'static str,
    symbol: &'static str,
    isin: &'static str,
    status: &'static str,
    seq: Option<i64>,
}

fn raw_row(
    symbol: &'static str,
    isin: &'static str,
    status: &'static str,
    seq: Option<i64>,
) -> RawMappingRow {
    RawMappingRow {
        market: "KR",
        symbol,
        isin,
        status,
        seq,
    }
}

#[test]
fn for_id_rejects_corrupt_unconstrained_records_for_same_id() {
    let rt = runtime();
    let target_id = id("inst-0000000000000001");
    let corrupt_sets = [
        vec![
            raw_row("005930", "KR7005930003", "superseded", Some(1)),
            raw_row("005930", "kr7005930003", "active", None),
        ],
        vec![
            raw_row("005930", "KR7005930003", "active", None),
            raw_row("005931", "KR7005930003", "retired", None),
        ],
        vec![raw_row("005930", "KR7005930003", "superseded", Some(1))],
    ];
    for rows in corrupt_sets {
        let fixture = Fixture::new();
        let db = open_fixture(&fixture);
        rt.block_on(db.write(move |tx| {
            tx.execute_batch(mapping_fixtures::RECREATE_UNCONSTRAINED_STRICT)?;
            for row in rows {
                tx.execute(
                    history_sql::INSERT_WITH_SUPERSEDED_SEQ,
                    params![
                        "inst-0000000000000001",
                        row.market,
                        row.symbol,
                        row.isin,
                        row.status,
                        row.seq
                    ],
                )?;
            }
            Ok(())
        }))
        .expect("seed corrupt unconstrained rows");
        let store = InstrumentMappingStore::new(&db);
        assert_eq!(
            rt.block_on(store.for_id(&target_id)),
            Err(MappingStoreError::Storage(integrity_system_error()))
        );
        rt.block_on(db.close()).expect("close");
    }
}

#[test]
fn lookup_rejects_multiple_terminal_records_for_same_id_in_partial_slice() {
    let rt = runtime();
    let fixture = Fixture::new();
    let db = open_fixture(&fixture);
    rt.block_on(db.write(|tx| {
        tx.execute_batch(mapping_fixtures::RECREATE_UNCONSTRAINED_STRICT)?;
        for row in [
            raw_row("005930", "KR7005930003", "active", None),
            raw_row("005930", "KR7005930999", "retired", None),
        ] {
            tx.execute(
                history_sql::INSERT_WITH_SUPERSEDED_SEQ,
                params![
                    "inst-0000000000000001",
                    row.market,
                    row.symbol,
                    row.isin,
                    row.status,
                    row.seq
                ],
            )?;
        }
        Ok(())
    }))
    .expect("seed duplicate terminal rows sharing symbol");
    let store = InstrumentMappingStore::new(&db);
    assert_eq!(
        rt.block_on(store.lookup(&source(KnownMarket::Kr, "005930", "KR7005930003"))),
        Err(MappingStoreError::Storage(integrity_system_error()))
    );
    rt.block_on(db.close()).expect("close");
}

#[test]
fn history_roundtrips_user_reason_containing_nul_byte() {
    let rt = runtime();
    let fixture = Fixture::new();
    let path = DatabasePath::new(fixture.path());
    let db = Database::open(&path, &key(), &clock()).expect("open");
    let target_id = id("inst-0000000000000001");
    let src = source(KnownMarket::Kr, "005930", "KR7005930003");
    let nul_note = UserReason::try_new(UntrustedText::try_new("정정\0사유").expect("untrusted"))
        .expect("note");
    let event = MappingHistoryEvent::RetirementCorrected {
        instrument_id: target_id.clone(),
        source: src.clone(),
        disclosure: DisclosureKey::from_receipt(ReceiptNo::Dart(
            DartReceiptNo::try_new("20261124000099").expect("dart"),
        )),
        note: nul_note,
        corrected_at: Timestamp::from_parts(
            UnixSeconds::new(1_763_942_400),
            Nanosecond::new(7),
            OffsetSeconds::new(32_400),
        )
        .expect("timestamp"),
    };
    let seeded = event.clone();
    let seeded_id = target_id.clone();
    rt.block_on(db.write(move |tx| {
        insert_active_mapping(tx, &seeded_id, &src)?;
        apply_seed_retirement_corrected(tx, 1, &seeded, &seeded_id, &src)
    }))
    .expect("seed event with embedded NUL note");
    rt.block_on(db.close()).expect("close");

    let reopened = Database::open(&path, &key(), &clock()).expect("reopen with NUL note");
    let store = InstrumentMappingStore::new(&reopened);
    assert_eq!(rt.block_on(store.history()), Ok(vec![event.clone()]));
    assert_eq!(
        rt.block_on(store.history_for_id(&target_id)),
        Ok(vec![event])
    );
    rt.block_on(reopened.close()).expect("close reopened");
}

#[test]
fn migration_schema_tracker_tracks_rename_and_drop_of_tables_indexes_and_triggers() {
    let mut tracker = crate::instrument_mapping::MigrationSchemaTracker::default();
    tracker.apply_script(history_sql::TRACKER_RENAME_SCRIPT);
    assert!(tracker.mapping_objects().is_empty());
    assert_eq!(tracker.all_triggers().len(), 1);

    tracker.apply_script(history_sql::TRACKER_DROP_SCRIPT);
    assert!(tracker.mapping_objects().is_empty());
    assert!(tracker.all_triggers().is_empty());
}

#[test]
fn open_rejects_tampered_0009_schema_objects() {
    let rt = runtime();
    for tamper in [
        history_sql::DROP_EVENTS_INDEX,
        history_sql::DROP_ACTIVE_RETIRED_ID_INDEX,
        history_sql::DROP_ACTIVE_RETIRED_SYMBOL_INDEX,
        history_sql::DROP_ACTIVE_RETIRED_ISIN_INDEX,
        history_sql::DROP_SUPERSEDED_EVENT_INDEX,
        history_sql::DROP_EVENTS_NO_UPDATE_TRIGGER,
        history_sql::DROP_EVENTS_NO_DELETE_TRIGGER,
        history_sql::DROP_EVENTS_MONOTONIC_TRIGGER,
    ] {
        let fixture = Fixture::new();
        let path = DatabasePath::new(fixture.path());
        let db = Database::open(&path, &key(), &clock()).expect("open");
        rt.block_on(db.write(move |tx| tx.execute_batch(tamper)))
            .expect("apply 0009 schema tamper");
        rt.block_on(db.close()).expect("close");
        assert_eq!(
            Database::open(&path, &key(), &clock())
                .expect_err("reopen must reject 0009 schema tamper")
                .failure(),
            SystemFailure::RepositoryIntegrity
        );
    }
}
