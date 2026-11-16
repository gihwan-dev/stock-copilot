use crate::sql::{migration as sql, third_tests as fixtures};

#[test]
fn unsupported_header_and_unicode_syntax_fail_apply_and_revalidation() {
    for source in [
        fixtures::HEADER_BLOCK,
        fixtures::HEADER_LINE,
        fixtures::HEADER_DEPTH,
        fixtures::HEADER_LITERAL,
        fixtures::HEADER_UNDERFLOW,
        fixtures::UNICODE_SPACE,
        fixtures::UNICODE_NBSP,
    ] {
        let mut connection = rusqlite::Connection::open_in_memory().expect("memory");
        crate::migration::run(&mut connection, crate::tests::timestamp()).expect("initial");
        connection
            .execute_batch(source)
            .expect("SQLite accepts fixture");
        crate::migration::validate_schema(&connection).expect_err(source);
        let mut fresh = rusqlite::Connection::open_in_memory().expect("memory");
        crate::migration::apply(
            &mut fresh,
            crate::tests::timestamp(),
            &[
                ("0001_migration_ledger.sql", sql::CREATE_LEDGER),
                ("0002_bad.sql", source),
            ],
        )
        .expect_err(source);
        assert_eq!(
            fresh
                .query_row(crate::sql::SCHEMA, [], |row| row.get::<_, i64>(0))
                .expect("rollback"),
            0
        );
    }
}

#[test]
fn unicode_literal_data_remains_supported() {
    let mut connection = rusqlite::Connection::open_in_memory().expect("memory");
    let migrations = [
        ("0001_migration_ledger.sql", sql::CREATE_LEDGER),
        ("0002_text.sql", fixtures::UNICODE_TEXT),
    ];
    crate::migration::apply(&mut connection, crate::tests::timestamp(), &migrations)
        .expect("Unicode data");
    crate::migration::apply(&mut connection, crate::tests::timestamp(), &migrations)
        .expect("reopen");
}

#[test]
fn migration_pragma_cannot_disable_checks_and_owned_pragmas_still_work() {
    let mut connection = rusqlite::Connection::open_in_memory().expect("memory");
    let initial = [
        ("0001_migration_ledger.sql", sql::CREATE_LEDGER),
        ("0002_money.sql", crate::sql::tests::GOOD_MONEY),
    ];
    crate::migration::apply(&mut connection, crate::tests::timestamp(), &initial)
        .expect("normal migration");
    let bad = [
        initial[0],
        initial[1],
        ("0003_pragma.sql", fixtures::IGNORE_CHECKS),
    ];
    crate::migration::apply(&mut connection, crate::tests::timestamp(), &bad)
        .expect_err("migration PRAGMA denied");
    assert_eq!(
        connection
            .query_row(fixtures::CHECK_STATE, [], |row| row.get::<_, i64>(0))
            .expect("checks"),
        0
    );
    assert_eq!(
        connection
            .query_row(sql::VERSION, [], |row| row.get::<_, i64>(0))
            .expect("version"),
        2
    );
    connection
        .execute(crate::sql::tests::INSERT_MONEY, ["1e9 abc"])
        .expect_err("CHECK still enforced");
    connection
        .execute(crate::sql::tests::INSERT_MONEY, ["123.45"])
        .expect("valid decimal");
    crate::migration::apply(&mut connection, crate::tests::timestamp(), &initial)
        .expect("authorizer reset");
}
