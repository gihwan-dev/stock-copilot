use crate::sql::{audit as audit_sql, audit_tests as sql};
use crate::tests::{Fixture, clock, key, runtime, timestamp};
use crate::{AuditStore, Database, DatabasePath};
use application::{
    AuditEntry, AuditIntegrity, AuditKind, AuditLog, OrderConfirmation, RecordOutcome,
    RiskChangeConfirmation, WebSearchAuditRecord,
};
use domain::{ClientOrderId, InstrumentId, ProposalId, RiskChange, WebSearchQueryDigest};

fn proposal() -> ProposalId {
    ProposalId::new(ClientOrderId::try_new("proposal-1").expect("id"))
}

fn entries() -> Vec<AuditEntry> {
    let digest = WebSearchQueryDigest::try_new(&"a".repeat(64)).expect("digest");
    vec![
        AuditEntry::new(
            timestamp(),
            AuditKind::EstimateSubmitted(InstrumentId::try_new("KR-005930").expect("id")),
        ),
        AuditEntry::new(
            timestamp(),
            AuditKind::PresenceRecorded {
                proposal: proposal(),
                outcome: OrderConfirmation::Confirmed,
            },
        ),
        AuditEntry::new(
            timestamp(),
            AuditKind::RiskChangeConfirmationRecorded {
                change: RiskChange::EnableOrders,
                outcome: RiskChangeConfirmation::Denied,
            },
        ),
        AuditEntry::new(
            timestamp(),
            AuditKind::WebSearchChecked(WebSearchAuditRecord::code_approved(digest)),
        ),
    ]
}

fn seeded() -> (Fixture, DatabasePath) {
    let fixture = Fixture::new();
    let path = DatabasePath::new(fixture.path());
    let db = Database::open(&path, &key(), &clock()).expect("open");
    let rt = runtime();
    let store = AuditStore::new(&db);
    for entry in entries() {
        assert_eq!(
            rt.block_on(store.append(&entry)).expect("append"),
            RecordOutcome::Recorded
        );
    }
    rt.block_on(db.close()).expect("close");
    (fixture, path)
}

fn unguarded(path: &DatabasePath) -> rusqlite::Connection {
    let connection = rusqlite::Connection::open(path.as_path()).expect("raw");
    crate::connection::apply_key(&connection, &key()).expect("key");
    connection
}

fn verify(path: &DatabasePath) -> AuditIntegrity {
    let db = Database::open(path, &key(), &clock()).expect("open");
    let rt = runtime();
    let result = rt.block_on(AuditStore::new(&db).verify()).expect("verify");
    rt.block_on(db.close()).expect("close");
    result
}

#[test]
fn appended_entries_form_a_contiguous_verified_chain_and_duplicates_are_not_rewritten() {
    let (_fixture, path) = seeded();
    let db = Database::open(&path, &key(), &clock()).expect("open");
    let rt = runtime();
    let store = AuditStore::new(&db);
    assert_eq!(
        rt.block_on(store.append(&entries()[1])).expect("again"),
        RecordOutcome::AlreadyRecorded
    );
    assert_eq!(
        rt.block_on(store.verify()).expect("verify"),
        AuditIntegrity::Verified
    );
    let rows = rt
        .block_on(db.read(|connection| {
            let mut statement = connection.prepare(sql::ROWS)?;
            statement
                .query_map([], |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                })?
                .collect::<rusqlite::Result<Vec<_>>>()
        }))
        .expect("rows");
    assert_eq!(
        rows.iter().map(|row| row.0).collect::<Vec<_>>(),
        vec![1, 2, 3, 4]
    );
    assert_eq!(rows[0].1, crate::audit::GENESIS_HASH);
    for (previous, next) in rows.iter().zip(rows.iter().skip(1)) {
        assert_eq!(next.1, previous.2);
    }
    rt.block_on(db.close()).expect("close");
}

#[test]
fn guarded_connections_reject_every_audit_mutation_and_schema_rewrite() {
    let (_fixture, path) = seeded();
    let db = Database::open(&path, &key(), &clock()).expect("open");
    let rt = runtime();
    for statement in [
        sql::UPDATE_PAYLOAD,
        sql::DELETE_MIDDLE,
        sql::DROP_UPDATE_TRIGGER,
        sql::DROP_TABLE,
        sql::ALTER_TABLE,
        sql::CREATE_TEMP_TRIGGER,
        sql::DROP_INDEX,
        sql::INSERT_GAP,
    ] {
        assert!(
            rt.block_on(db.write(move |transaction| transaction.execute_batch(statement)))
                .is_err(),
            "{statement}"
        );
        assert!(
            rt.block_on(db.read(move |connection| connection.execute_batch(statement)))
                .is_err(),
            "{statement}"
        );
    }
    assert_eq!(
        rt.block_on(
            db.read(|connection| connection.query_row(sql::COUNT, [], |row| row.get::<_, i64>(0)))
        )
        .expect("count"),
        4
    );
    assert_eq!(
        rt.block_on(AuditStore::new(&db).verify()).expect("verify"),
        AuditIntegrity::Verified
    );
    rt.block_on(db.close()).expect("close");
}

#[test]
fn triggers_reject_update_delete_and_gaps_even_without_the_authorizer() {
    let (_fixture, path) = seeded();
    let connection = unguarded(&path);
    for statement in [sql::UPDATE_PAYLOAD, sql::DELETE_MIDDLE, sql::INSERT_GAP] {
        assert!(connection.execute_batch(statement).is_err(), "{statement}");
    }
    connection.close().expect("close raw");
    assert_eq!(verify(&path), AuditIntegrity::Verified);
}

#[test]
fn chain_detects_middle_edit_middle_delete_and_hash_rewrite() {
    for tamper in [sql::UPDATE_PAYLOAD, sql::DELETE_MIDDLE, sql::UPDATE_HASH] {
        let (_fixture, path) = seeded();
        let connection = unguarded(&path);
        connection.execute_batch(sql::DROP_GUARDS).expect("drop");
        connection.execute_batch(tamper).expect("tamper");
        connection
            .execute_batch(sql::RESTORE_GUARDS)
            .expect("restore");
        connection.close().expect("close raw");
        assert_eq!(verify(&path), AuditIntegrity::Tampered, "{tamper}");
    }
}

#[test]
fn empty_log_verifies_and_failed_append_reports_repository_error() {
    let fixture = Fixture::new();
    let path = DatabasePath::new(fixture.path());
    let db = Database::open(&path, &key(), &clock()).expect("open");
    let rt = runtime();
    assert_eq!(
        rt.block_on(AuditStore::new(&db).verify()).expect("verify"),
        AuditIntegrity::Verified
    );
    rt.block_on(db.write(|transaction| transaction.execute_batch(sql::INSERT_FIRST_FORGED)))
        .expect("forged first row");
    assert_eq!(
        rt.block_on(AuditStore::new(&db).verify()).expect("verify"),
        AuditIntegrity::Tampered
    );
    rt.block_on(db.close()).expect("close");
}

#[test]
fn audit_sql_constants_match_the_migration_and_prepare() {
    assert_eq!(
        std::fs::read_to_string("migrations/0012_audit_log.sql").expect("file"),
        audit_sql::CREATE
    );
    let mut connection = rusqlite::Connection::open_in_memory().expect("memory");
    crate::migration::run(&mut connection, timestamp()).expect("migrate");
    for statement in [
        audit_sql::LAST,
        audit_sql::FIND_ENTRY,
        audit_sql::INSERT,
        audit_sql::ALL,
        sql::COUNT,
        sql::ROWS,
    ] {
        connection.prepare(statement).expect(statement);
    }
}

#[test]
fn reopening_rejects_a_database_whose_audit_triggers_were_dropped() {
    let (_fixture, path) = seeded();
    let connection = unguarded(&path);
    connection.execute_batch(sql::DROP_GUARDS).expect("drop");
    connection.close().expect("close raw");
    assert_eq!(
        Database::open(&path, &key(), &clock())
            .expect_err("missing audit triggers")
            .failure(),
        application::SystemFailure::RepositoryIntegrity
    );
}

#[test]
fn reopening_rejects_a_rewritten_audit_trigger() {
    let (_fixture, path) = seeded();
    let db = Database::open(&path, &key(), &clock()).expect("open");
    let rt = runtime();
    rt.block_on(db.write(|transaction| transaction.execute_batch(sql::WRITABLE_SCHEMA)))
        .expect("rewrite trigger text");
    rt.block_on(db.close()).expect("close");
    assert_eq!(
        Database::open(&path, &key(), &clock())
            .expect_err("rewritten audit trigger")
            .failure(),
        application::SystemFailure::RepositoryIntegrity
    );
}
