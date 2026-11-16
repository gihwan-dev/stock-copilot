use crate::sql::{migration as sql, review_tests as fixtures};

#[test]
fn decimal_check_rejects_embedded_nul_bytes() {
    let connection = rusqlite::Connection::open_in_memory().expect("memory");
    connection
        .execute_batch(crate::sql::tests::GOOD_MONEY)
        .expect("fixture");
    for value in ["1\0abc", "1\0", "5\0.-.-e9", "\0"] {
        connection
            .execute(crate::sql::tests::INSERT_MONEY, [value])
            .expect_err("not a decimal string");
    }
}

#[test]
fn camel_case_column_must_check_its_own_identifier() {
    crate::schema::validate(fixtures::ALIASED_CHECK).expect_err("unitPrice does not check itself");
    let good = crate::sql::tests::GOOD_MONEY.replace("amount", "unitPrice");
    crate::schema::validate(&good).expect("actual camelCase identifier");
}

#[test]
fn quantity_and_ratio_checks_do_not_require_currency() {
    for name in [
        "quantity",
        "shares",
        "probability",
        "ratio",
        "tax_rate",
        "weight",
    ] {
        let good = crate::sql::tests::GOOD_MONEY
            .replace("amount", name)
            .replace("currency TEXT NOT NULL, ", "");
        crate::schema::validate(&good).expect("dimensionless decimal");
    }
}

#[test]
fn snapshot_contains_index_view_trigger_and_tables() {
    let mut connection = rusqlite::Connection::open_in_memory().expect("memory");
    crate::migration::run(&mut connection, crate::tests::timestamp()).expect("migrate");
    for statement in [
        fixtures::SNAPSHOT_TABLE,
        fixtures::SNAPSHOT_INDEX,
        fixtures::SNAPSHOT_VIEW,
        fixtures::SNAPSHOT_TRIGGER,
    ] {
        connection.execute_batch(statement).expect("schema object");
    }
    crate::migration::validate_schema(&connection).expect("non-table objects remain valid");
    let names = connection
        .prepare(sql::SCHEMA)
        .expect("prepare")
        .query_map([], |row| row.get::<_, String>(0))
        .expect("query")
        .collect::<rusqlite::Result<std::collections::BTreeSet<_>>>()
        .expect("names");
    assert_eq!(
        names,
        [
            "ledger_probe",
            "migration_ledger",
            "probe_index",
            "probe_trigger",
            "probe_view"
        ]
        .into_iter()
        .map(String::from)
        .collect()
    );
}

#[test]
fn resetting_both_ledger_and_version_is_not_a_fresh_database() {
    let fixture = crate::tests::Fixture::new();
    let path = crate::DatabasePath::new(fixture.path());
    {
        let database = crate::Database::open(&path, &crate::tests::key(), &crate::tests::clock())
            .expect("open");
        crate::tests::runtime()
            .block_on(
                database.write(|transaction| transaction.execute_batch(fixtures::RESET_LEDGER)),
            )
            .expect("tamper");
    };
    assert_eq!(
        crate::Database::open(&path, &crate::tests::key(), &crate::tests::clock())
            .expect_err("missing ledger")
            .failure(),
        application::SystemFailure::RepositoryIntegrity
    );
}

#[test]
fn embedded_sequence_gap_rolls_back_pending_schema() {
    let mut connection = rusqlite::Connection::open_in_memory().expect("memory");
    let at = crate::tests::timestamp();
    crate::migration::run(&mut connection, at).expect("initial");
    crate::migration::apply(
        &mut connection,
        at,
        &[
            ("0001_migration_ledger.sql", sql::CREATE_LEDGER),
            ("0003_wrong.sql", fixtures::SECOND_VALID),
        ],
    )
    .expect_err("embedded sequence gap");
    assert_eq!(
        connection
            .query_row(crate::sql::tests::PARTIAL, [], |row| row.get::<_, i64>(0))
            .expect("count"),
        0
    );
}

#[test]
fn digit_to_uppercase_boundary_is_a_numeric_name_segment() {
    let bad = crate::sql::tests::BAD_MONEY.replace("amount TEXT", "x1Price REAL");
    crate::schema::validate(&bad).expect_err("price after digit boundary");
}

#[test]
fn table_rename_quotes_do_not_change_column_identity() {
    let mut connection = rusqlite::Connection::open_in_memory().expect("memory");
    crate::migration::run(&mut connection, crate::tests::timestamp()).expect("initial");
    connection
        .execute_batch(fixtures::SNAPSHOT_TABLE)
        .expect("table");
    connection
        .execute_batch(fixtures::RENAME_TABLE)
        .expect("rename");
    crate::migration::validate_schema(&connection).expect("table quotes are not column quotes");
}

#[test]
fn third_migration_failure_rolls_back_second_migration_too() {
    let mut connection = rusqlite::Connection::open_in_memory().expect("memory");
    let at = crate::tests::timestamp();
    crate::migration::run(&mut connection, at).expect("initial");
    crate::migration::apply(
        &mut connection,
        at,
        &[
            ("0001_migration_ledger.sql", sql::CREATE_LEDGER),
            ("0002_second.sql", fixtures::SECOND_VALID),
            ("0003_broken.sql", fixtures::THIRD_BROKEN),
        ],
    )
    .expect_err("rollback all pending");
    assert_eq!(
        connection
            .query_row(crate::sql::tests::PARTIAL, [], |row| row.get::<_, i64>(0))
            .expect("count"),
        0
    );
    assert_eq!(
        connection
            .query_row(sql::VERSION, [], |row| row.get::<_, i64>(0))
            .expect("version"),
        1
    );
    assert_eq!(
        connection
            .query_row(crate::sql::tests::LEDGER_COUNT, [], |row| row
                .get::<_, i64>(0))
            .expect("ledger"),
        1
    );
}

#[test]
fn migration_directory_and_embedded_inventory_match_exactly() {
    let mut files = std::fs::read_dir("migrations")
        .expect("migration directory")
        .map(|listed| {
            let file = listed.expect("entry");
            let name = file.file_name().into_string().expect("UTF-8 name");
            (name, std::fs::read_to_string(file.path()).expect("source"))
        })
        .collect::<Vec<_>>();
    files.sort();
    let embedded = crate::migration::MIGRATIONS
        .iter()
        .map(|(name, source)| (String::from(*name), String::from(*source)))
        .collect::<Vec<_>>();
    assert_eq!(files, embedded);
}

#[test]
fn ledger_hash_matches_frozen_migration_bytes() {
    assert_eq!(
        crate::migration::digest(sql::CREATE_LEDGER),
        "f059233e48dd3cd2f492b96f916d760c19a109861b52707d6901f2bc0c6b2501"
    );
}

#[test]
fn monetary_suffix_still_requires_currency_after_scalar_qualifier() {
    for name in ["probability_amount", "quantity_price", "tax_rate_amount"] {
        let bad = crate::sql::tests::GOOD_MONEY
            .replace("amount", name)
            .replace("currency TEXT NOT NULL, ", "");
        crate::schema::validate(&bad).expect_err("monetary final unit");
    }
}

#[test]
fn unsupported_schema_forms_fail_after_sqlite_accepts_them() {
    for statement in [
        fixtures::EMPTY_TABLE_NAME,
        fixtures::UNICODE_TABLE_NAME,
        fixtures::BLOCK_COMMENT,
        fixtures::LINE_COMMENT,
    ] {
        let mut connection = rusqlite::Connection::open_in_memory().expect("memory");
        crate::migration::run(&mut connection, crate::tests::timestamp()).expect("initial");
        connection
            .execute_batch(statement)
            .expect("SQLite accepts fixture");
        crate::migration::validate_schema(&connection)
            .expect_err("outside supported schema dialect");
    }
}
