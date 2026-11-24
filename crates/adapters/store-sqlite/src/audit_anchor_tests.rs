use crate::sql::{audit as audit_sql, audit_tests as sql};
use crate::tests::{Fixture, clock, key, runtime, timestamp};
use crate::{AnchoredAuditStore, AuditStore, Database, DatabasePath};
use application::{
    AuditChainHash, AuditEntry, AuditHead, AuditIntegrity, AuditKind, AuditLog, AuditSequence,
    RecordOutcome, SystemFailure, SystemPort, SystemPortError,
};
use domain::InstrumentId;
use testkit_ports::system_fakes::MemoryAuditHeadAnchor;

fn entry(index: u8) -> AuditEntry {
    AuditEntry::new(
        timestamp(),
        AuditKind::SignalRaised(
            InstrumentId::try_new(["KR-0", "KR-1", "KR-2", "KR-3", "KR-4"][usize::from(index)])
                .expect("id"),
        ),
    )
}

fn forged_head(sequence: u64) -> AuditHead {
    AuditHead::new(
        AuditSequence::try_new(sequence).expect("sequence"),
        AuditChainHash::try_new(&"f".repeat(64)).expect("hash"),
    )
}

fn seeded(anchor: &MemoryAuditHeadAnchor) -> (Fixture, DatabasePath) {
    let fixture = Fixture::new();
    let path = DatabasePath::new(fixture.path());
    let db = Database::open(&path, &key(), &clock()).expect("open");
    let rt = runtime();
    let store = AnchoredAuditStore::new(&db, anchor);
    for index in 1..=4 {
        assert_eq!(
            rt.block_on(store.append(&entry(index))).expect("append"),
            RecordOutcome::Recorded
        );
    }
    rt.block_on(db.close()).expect("close");
    (fixture, path)
}

fn tamper(path: &DatabasePath, statements: &[&str]) {
    let connection = rusqlite::Connection::open(path.as_path()).expect("raw");
    crate::connection::apply_key(&connection, &key()).expect("key");
    connection.execute_batch(sql::DROP_GUARDS).expect("drop");
    for statement in statements {
        connection.execute_batch(statement).expect("tamper");
    }
    connection
        .execute_batch(sql::RESTORE_GUARDS)
        .expect("restore");
    connection.close().expect("close raw");
}

fn rewrite_chain(path: &DatabasePath) {
    let connection = rusqlite::Connection::open(path.as_path()).expect("raw");
    crate::connection::apply_key(&connection, &key()).expect("key");
    connection.execute_batch(sql::DROP_GUARDS).expect("drop");
    connection.execute_batch(sql::DELETE_ALL).expect("delete");
    let mut prev = String::from(crate::audit::GENESIS_HASH);
    for (seq, payload) in [(1_i64, "p1"), (2, "p2"), (3, "p3"), (4, "p4")] {
        let hash = crate::audit::chain_hash(seq, "at", "forged", payload, &prev);
        connection
            .execute(
                audit_sql::INSERT,
                rusqlite::params![seq, "at", "forged", payload, prev, hash],
            )
            .expect("insert");
        prev = hash;
    }
    connection
        .execute_batch(sql::RESTORE_GUARDS)
        .expect("restore");
    connection.close().expect("close raw");
}

fn verdicts(
    path: &DatabasePath,
    anchor: &MemoryAuditHeadAnchor,
) -> (AuditIntegrity, AuditIntegrity) {
    let db = Database::open(path, &key(), &clock()).expect("open");
    let rt = runtime();
    let plain = rt.block_on(AuditStore::new(&db).verify()).expect("plain");
    let anchored = rt
        .block_on(AnchoredAuditStore::new(&db, anchor).verify())
        .expect("anchored");
    rt.block_on(db.close()).expect("close");
    (plain, anchored)
}

#[test]
fn anchored_appends_pin_the_latest_sequence_and_hash() {
    let anchor = MemoryAuditHeadAnchor::default();
    let (_fixture, path) = seeded(&anchor);
    let head = anchor.head().expect("head");
    assert_eq!(*head.sequence().as_u64(), 4);
    assert_eq!(
        verdicts(&path, &anchor),
        (AuditIntegrity::Verified, AuditIntegrity::Verified)
    );
}

#[test]
fn tail_truncation_is_detected_only_with_the_pinned_head() {
    let anchor = MemoryAuditHeadAnchor::default();
    let (_fixture, path) = seeded(&anchor);
    tamper(&path, &[sql::DELETE_TAIL]);
    assert_eq!(
        verdicts(&path, &anchor),
        (AuditIntegrity::Verified, AuditIntegrity::Tampered)
    );
}

#[test]
fn rewriting_the_whole_chain_is_detected_only_with_the_pinned_head() {
    let anchor = MemoryAuditHeadAnchor::default();
    let (_fixture, path) = seeded(&anchor);
    rewrite_chain(&path);
    assert_eq!(
        verdicts(&path, &anchor),
        (AuditIntegrity::Verified, AuditIntegrity::Tampered)
    );
}

#[test]
fn middle_edits_deletes_and_removed_or_rolled_back_heads_are_detected() {
    for statement in [sql::UPDATE_PAYLOAD, sql::DELETE_MIDDLE] {
        let anchor = MemoryAuditHeadAnchor::default();
        let (_fixture, path) = seeded(&anchor);
        tamper(&path, &[statement]);
        assert_eq!(verdicts(&path, &anchor).1, AuditIntegrity::Tampered);
    }
    let anchor = MemoryAuditHeadAnchor::default();
    let (_fixture, path) = seeded(&anchor);
    anchor.replace_head(None);
    assert_eq!(verdicts(&path, &anchor).1, AuditIntegrity::Tampered);
    anchor.replace_head(Some(forged_head(4)));
    assert_eq!(verdicts(&path, &anchor).1, AuditIntegrity::Tampered);
    anchor.replace_head(Some(forged_head(9)));
    assert_eq!(verdicts(&path, &anchor).1, AuditIntegrity::Tampered);
}

#[test]
fn failed_head_write_fails_the_append_until_a_retry_pins_it() {
    let anchor = MemoryAuditHeadAnchor::default();
    let fixture = Fixture::new();
    let path = DatabasePath::new(fixture.path());
    let db = Database::open(&path, &key(), &clock()).expect("open");
    let rt = runtime();
    let store = AnchoredAuditStore::new(&db, &anchor);
    assert_eq!(
        rt.block_on(store.verify()).expect("empty"),
        AuditIntegrity::Verified
    );
    anchor.fail_next_store(SystemPortError::new(
        SystemPort::SecretStore,
        SystemFailure::PermissionDenied,
    ));
    assert_eq!(
        rt.block_on(store.append(&entry(1)))
            .expect_err("anchor failed")
            .failure(),
        SystemFailure::PermissionDenied
    );
    assert_eq!(
        rt.block_on(store.verify()).expect("unpinned"),
        AuditIntegrity::Tampered
    );
    assert_eq!(
        rt.block_on(store.append(&entry(1))).expect("retry"),
        RecordOutcome::AlreadyRecorded
    );
    assert_eq!(
        rt.block_on(store.verify()).expect("pinned"),
        AuditIntegrity::Verified
    );
    rt.block_on(db.close()).expect("close");
}

#[test]
fn audit_head_rejects_zero_sequence_and_malformed_hashes() {
    assert_eq!(
        AuditSequence::try_new(0),
        Err(application::AuditHeadError::ZeroSequence)
    );
    for bad in ["A".repeat(64), "a".repeat(63)] {
        assert_eq!(
            AuditChainHash::try_new(&bad),
            Err(application::AuditHeadError::InvalidHash)
        );
    }
    let mut connection = rusqlite::Connection::open_in_memory().expect("memory");
    crate::migration::run(&mut connection, timestamp()).expect("migrate");
    for statement in [audit_sql::HASH_AT, audit_sql::LAST] {
        connection.prepare(statement).expect(statement);
    }
}
