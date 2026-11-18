use crate::tests::{Fixture, key};
use crate::{Database, DatabasePath};

#[test]
fn fresh_open_records_clock_and_reopen_preserves_ledger() {
    let fixture = Fixture::new();
    let path = DatabasePath::new(fixture.path());
    for _ in 0_u8..2 {
        let db = Database::open(&path, &key(), &crate::tests::clock()).expect("migrated");
        crate::tests::runtime()
            .block_on(db.read(|connection| {
                let entry = connection.query_row(crate::sql::migration::LEDGER, [], |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, i64>(3)?,
                    ))
                })?;
                assert_eq!(entry.0, 1);
                assert_eq!(entry.1, "0001_migration_ledger.sql");
                assert_eq!(
                    entry.2,
                    crate::migration::digest(crate::sql::migration::CREATE_LEDGER)
                );
                assert_eq!(entry.3, 1_723_456_789);
                assert_eq!(
                    connection.query_row(crate::sql::tests::LEDGER_COUNT, [], |row| row
                        .get::<_, i64>(0))?,
                    5
                );
                Ok(())
            }))
            .expect("ledger");
    }
}

#[test]
fn corrupt_ledger_and_future_database_fail_open() {
    for statement in [
        crate::sql::tests::BAD_HASH,
        crate::sql::holdings_tests::BAD_NAME,
        crate::sql::tests::MISSING,
        crate::sql::holdings_tests::GAP,
        crate::sql::holdings_tests::FUTURE,
        crate::sql::tests::DROP_LEDGER,
        crate::sql::tests::NONSTRICT,
        crate::sql::tests::PREFIX_NONSTRICT,
        crate::sql::tests::BAD_MONEY,
    ] {
        let fixture = Fixture::new();
        let path = DatabasePath::new(fixture.path());
        {
            let db = Database::open(&path, &key(), &crate::tests::clock()).expect("open");
            crate::tests::runtime()
                .block_on(db.write(move |transaction| transaction.execute_batch(statement)))
                .expect("corrupt fixture");
        };
        assert_eq!(
            Database::open(&path, &key(), &crate::tests::clock())
                .expect_err("closed")
                .failure(),
            application::SystemFailure::RepositoryIntegrity
        );
    }
}

#[test]
fn failed_pending_migration_rolls_back_ddl_and_ledger() {
    let mut connection = rusqlite::Connection::open_in_memory().expect("memory");
    let at = crate::tests::timestamp();
    crate::migration::apply(
        &mut connection,
        at,
        &[(
            "0001_migration_ledger.sql",
            crate::sql::migration::CREATE_LEDGER,
        )],
    )
    .expect("first");
    assert!(
        crate::migration::apply(
            &mut connection,
            at,
            &[
                (
                    "0001_migration_ledger.sql",
                    crate::sql::migration::CREATE_LEDGER
                ),
                ("0002_broken.sql", crate::sql::tests::BROKEN)
            ]
        )
        .is_err()
    );
    assert_eq!(
        connection
            .query_row(crate::sql::tests::PARTIAL, [], |row| row.get::<_, i64>(0))
            .expect("rolled back"),
        0
    );
    assert_eq!(
        connection
            .query_row(crate::sql::migration::VERSION, [], |row| row
                .get::<_, i64>(0))
            .expect("version"),
        1
    );
    assert_eq!(
        connection
            .query_row(crate::sql::tests::LEDGER_COUNT, [], |row| row
                .get::<_, i64>(0))
            .expect("rows"),
        1
    );
}

#[test]
fn embedded_migration_and_saved_schema_match_files() {
    let root = std::path::Path::new(".");
    assert_eq!(
        std::fs::read_to_string(root.join("migrations/0001_migration_ledger.sql")).expect("file"),
        crate::sql::migration::CREATE_LEDGER
    );
    let mut connection = rusqlite::Connection::open_in_memory().expect("memory");
    crate::migration::run(&mut connection, crate::tests::timestamp()).expect("migration");
    let mut schema = connection
        .prepare(crate::sql::migration::SCHEMA)
        .expect("prepare")
        .query_map([], |row| row.get::<_, String>(1))
        .expect("schema")
        .collect::<rusqlite::Result<Vec<_>>>()
        .expect("rows")
        .join(";\n");
    schema.push_str(";\n");
    assert_eq!(
        schema,
        std::fs::read_to_string(root.join("schema.snapshot.sql")).expect("snapshot")
    );
    for statement in [
        crate::sql::migration::LEDGER,
        crate::sql::migration::INSERT_LEDGER,
        crate::sql::migration::TABLES,
        crate::sql::migration::SCHEMA,
        crate::sql::migration::VERSION,
        crate::sql::tests::LEDGER_COUNT,
    ] {
        connection
            .prepare(statement)
            .expect("each runtime SQL prepares");
    }
}

#[test]
fn explicit_close_drains_accepted_writes() {
    let fixture = Fixture::new();
    let db = Database::open(
        &DatabasePath::new(fixture.path()),
        &key(),
        &crate::tests::clock(),
    )
    .expect("open");
    let pending = db.writer.submit(|_| Ok(37_u8)).expect("accepted");
    let rt = crate::tests::runtime();
    rt.block_on(db.close()).expect("close");
    assert_eq!(rt.block_on(pending).expect("response").expect("write"), 37);
}

#[test]
fn sha256_matches_independent_known_vectors() {
    assert_eq!(
        crate::migration::digest(""),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
    assert_eq!(
        crate::migration::digest("abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
}

#[test]
fn connection_sql_constants_prepare_on_migrated_database() {
    let mut connection = rusqlite::Connection::open_in_memory().expect("memory");
    crate::migration::run(&mut connection, crate::tests::timestamp()).expect("migrate");
    for statement in [
        crate::sql::PROVIDER,
        crate::sql::TEMP_STORE,
        crate::sql::SET_TEMP_STORE,
        crate::sql::JOURNAL,
        crate::sql::SET_JOURNAL,
        crate::sql::FOREIGN_KEYS,
        crate::sql::SET_FOREIGN_KEYS,
        crate::sql::SCHEMA,
        crate::sql::QUERY_ONLY,
        crate::sql::SET_QUERY_ONLY,
        crate::sql::CIPHER_MEMORY,
        crate::sql::SET_CIPHER_MEMORY,
        crate::sql::CIPHER_HMAC,
        crate::sql::PAGE_SIZE,
        crate::sql::SYNCHRONOUS,
        crate::sql::SET_SYNCHRONOUS,
    ] {
        connection
            .prepare(statement)
            .expect("connection SQL prepares");
    }
    let pristine = rusqlite::Connection::open_in_memory().expect("memory");
    pristine
        .prepare(crate::sql::migration::CREATE_LEDGER)
        .expect("migration prepares before application");
}

#[test]
fn explicit_close_reports_writer_failure() {
    let fixture = Fixture::new();
    let db = Database::open(
        &DatabasePath::new(fixture.path()),
        &key(),
        &crate::tests::clock(),
    )
    .expect("open");
    let rt = crate::tests::runtime();
    rt.block_on(db.write::<(), _>(|_| std::panic::resume_unwind(Box::new("failed writer"))))
        .expect_err("panic closes response");
    assert_eq!(
        rt.block_on(db.close())
            .expect_err("writer failure")
            .failure(),
        application::SystemFailure::RepositoryIntegrity
    );
}

#[test]
fn migration_cannot_escape_transaction() {
    for source in [
        crate::sql::tests::ESCAPE_COMMIT,
        crate::sql::tests::ESCAPE_ROLLBACK,
        crate::sql::tests::ESCAPE_SAVEPOINT,
    ] {
        let mut connection = rusqlite::Connection::open_in_memory().expect("memory");
        let at = crate::tests::timestamp();
        crate::migration::run(&mut connection, at).expect("first");
        assert_eq!(
            crate::migration::apply(
                &mut connection,
                at,
                &[
                    (
                        "0001_migration_ledger.sql",
                        crate::sql::migration::CREATE_LEDGER
                    ),
                    ("0002_escape.sql", source)
                ]
            )
            .expect_err("migration controls its own transaction"),
            rusqlite::Error::InvalidQuery
        );
        assert_eq!(
            connection
                .query_row(crate::sql::tests::PARTIAL, [], |row| row.get::<_, i64>(0))
                .expect("count"),
            0
        );
        assert!(connection.is_autocommit());
        crate::migration::run(&mut connection, at).expect("policy reset and reopen valid ledger");
    }
}
