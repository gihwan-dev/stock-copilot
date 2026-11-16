use crate::sql::{migration as sql, third_tests as fixtures};

#[test]
fn quoted_body_tokens_cannot_hide_numeric_columns() {
    for source in [
        fixtures::DOUBLE_REF,
        fixtures::BACKTICK_REF,
        fixtures::BRACKET_REF,
        fixtures::PAREN_REF,
        fixtures::DOUBLE_DEFAULT,
        fixtures::QUOTED_COLUMN,
        fixtures::BACKTICK_COLUMN,
        fixtures::BRACKET_COLUMN,
        fixtures::PLAIN_QUOTED_REF,
        fixtures::PLAIN_BACKTICK_REF,
        fixtures::PLAIN_BRACKET_REF,
    ] {
        let mut connection = rusqlite::Connection::open_in_memory().expect("memory");
        crate::migration::run(&mut connection, crate::tests::timestamp()).expect("initial");
        connection
            .execute_batch(source)
            .expect("SQLite accepts quoted fixture");
        crate::migration::validate_schema(&connection)
            .expect_err("unsupported quoting cannot hide a column");
    }
}

#[test]
fn hidden_numeric_column_fails_apply_and_preserves_ledger() {
    for source in [
        fixtures::DOUBLE_REF,
        fixtures::BACKTICK_REF,
        fixtures::BRACKET_REF,
        fixtures::PAREN_REF,
        fixtures::DOUBLE_DEFAULT,
    ] {
        let mut connection = rusqlite::Connection::open_in_memory().expect("memory");
        crate::migration::run(&mut connection, crate::tests::timestamp()).expect("initial");
        crate::migration::apply(
            &mut connection,
            crate::tests::timestamp(),
            &[
                ("0001_migration_ledger.sql", sql::CREATE_LEDGER),
                ("0002_hidden.sql", source),
            ],
        )
        .expect_err("bad pending schema");
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
                .expect("rows"),
            1
        );
    }
}

#[test]
fn single_quoted_text_markers_do_not_change_column_boundaries() {
    let mut connection = rusqlite::Connection::open_in_memory().expect("memory");
    crate::migration::run(&mut connection, crate::tests::timestamp()).expect("initial");
    connection
        .execute_batch(fixtures::TEXT_MARKERS)
        .expect("literal markers");
    crate::migration::validate_schema(&connection).expect("markers are text, not syntax");
    connection
        .execute_batch(fixtures::TEXT_HIDDEN_PRICE)
        .expect("later numeric column");
    crate::migration::validate_schema(&connection)
        .expect_err("real numeric column remains visible");
}

#[test]
fn alter_after_quoted_reference_is_closed() {
    let mut connection = rusqlite::Connection::open_in_memory().expect("memory");
    crate::migration::run(&mut connection, crate::tests::timestamp()).expect("initial");
    connection
        .execute_batch(fixtures::QUOTED_TABLE_BASE)
        .expect("reference");
    connection
        .execute_batch(fixtures::ADD_PRICE)
        .expect("add price");
    crate::migration::validate_schema(&connection).expect_err("closed after ALTER");
}

#[test]
fn exposure_requires_currency_but_exposure_ratio_does_not() {
    for name in ["sector_exposure", "instrument_exposure", "realized_return"] {
        let monetary = crate::sql::tests::GOOD_MONEY.replace("amount", name);
        crate::schema::validate(&monetary).expect("explicit monetary unit");
        let without_currency = monetary.replace("currency TEXT NOT NULL, ", "");
        crate::schema::validate(&without_currency).expect_err("Money requires currency");
    }
    let ratio = crate::sql::tests::GOOD_MONEY
        .replace("amount", "sector_exposure_ratio")
        .replace("currency TEXT NOT NULL, ", "");
    crate::schema::validate(&ratio).expect("explicit dimensionless suffix");
}

#[test]
fn successful_sql_cannot_commit_a_damaged_ledger() {
    let mut accepted = Vec::new();
    for source in [
        fixtures::TAMPER_DELETE,
        fixtures::TAMPER_HASH,
        fixtures::TAMPER_NAME,
        fixtures::TAMPER_NUMBER,
        fixtures::TAMPER_TIME,
        fixtures::SUPPRESS_NEW,
        fixtures::CHANGE_NEW_TIME,
    ] {
        let mut connection = rusqlite::Connection::open_in_memory().expect("memory");
        let at = crate::tests::timestamp();
        crate::migration::run(&mut connection, at).expect("initial");
        let outcome = crate::migration::apply(
            &mut connection,
            at,
            &[
                ("0001_migration_ledger.sql", sql::CREATE_LEDGER),
                ("0002_tamper.sql", source),
            ],
        );
        if outcome.is_ok() {
            accepted.push(source);
            continue;
        }
        assert_initial_state(&connection);
    }
    assert!(
        accepted.is_empty(),
        "accepted damaged ledger scripts: {accepted:?}"
    );
}

#[test]
fn valid_second_migration_stays_reopenable() {
    let mut connection = rusqlite::Connection::open_in_memory().expect("memory");
    let migrations = [
        ("0001_migration_ledger.sql", sql::CREATE_LEDGER),
        ("0002_valid.sql", crate::sql::review_tests::SECOND_VALID),
    ];
    for _ in 0_u8..2 {
        crate::migration::apply(&mut connection, crate::tests::timestamp(), &migrations)
            .expect("complete ledger");
        assert_eq!(
            connection
                .query_row(sql::VERSION, [], |row| row.get::<_, i64>(0))
                .expect("version"),
            2
        );
        assert_eq!(
            connection
                .query_row(crate::sql::tests::LEDGER_COUNT, [], |row| row
                    .get::<_, i64>(0))
                .expect("rows"),
            2
        );
    }
}

fn assert_initial_state(connection: &rusqlite::Connection) {
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
            .expect("rows"),
        1
    );
    assert_eq!(
        connection
            .query_row(crate::sql::tests::PARTIAL, [], |row| row.get::<_, i64>(0))
            .expect("partial table"),
        0
    );
    let restored = connection
        .query_row(sql::LEDGER, [], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
            ))
        })
        .expect("original ledger");
    assert_eq!(
        restored,
        (
            1,
            String::from("0001_migration_ledger.sql"),
            String::from("f059233e48dd3cd2f492b96f916d760c19a109861b52707d6901f2bc0c6b2501"),
            1_723_456_789
        )
    );
}

#[test]
fn valid_data_migration_preserves_distinct_application_times() {
    let mut connection = rusqlite::Connection::open_in_memory().expect("memory");
    let initial = crate::tests::timestamp();
    let later = domain::Timestamp::from_parts(
        domain::UnixSeconds::new(1_734_567_891),
        domain::Nanosecond::new(0),
        domain::OffsetSeconds::new(0),
    )
    .expect("later timestamp");
    crate::migration::run(&mut connection, initial).expect("initial");
    let migrations = [
        ("0001_migration_ledger.sql", sql::CREATE_LEDGER),
        ("0002_data.sql", fixtures::VALID_DML),
    ];
    crate::migration::apply(&mut connection, later, &migrations).expect("data migration");
    crate::migration::apply(&mut connection, initial, &migrations).expect("already applied");
    assert_eq!(
        connection
            .query_row(fixtures::PARTIAL_ID, [], |row| row.get::<_, i64>(0))
            .expect("data"),
        29
    );
    let rows = connection
        .prepare(sql::LEDGER)
        .expect("prepare")
        .query_map([], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(3)?)))
        .expect("query")
        .collect::<rusqlite::Result<Vec<_>>>()
        .expect("rows");
    assert_eq!(rows, vec![(1, 1_723_456_789), (2, 1_734_567_891)]);
}

#[test]
fn supported_negative_and_division_defaults_are_not_comments() {
    for source in [fixtures::NEGATIVE_DEFAULT, fixtures::DIVISION_DEFAULT] {
        let mut connection = rusqlite::Connection::open_in_memory().expect("memory");
        crate::migration::run(&mut connection, crate::tests::timestamp()).expect("initial");
        connection.execute_batch(source).expect("valid default");
        crate::migration::validate_schema(&connection).expect("supported numeric syntax");
    }
}

#[test]
fn damaged_initial_ledger_cannot_be_repaired_by_pending_sql() {
    for damage in [
        crate::sql::tests::BAD_HASH,
        crate::sql::tests::BAD_NAME,
        crate::sql::tests::GAP,
    ] {
        let mut connection = rusqlite::Connection::open_in_memory().expect("memory");
        crate::migration::run(&mut connection, crate::tests::timestamp()).expect("initial");
        connection.execute_batch(damage).expect("damage");
        crate::migration::apply(
            &mut connection,
            crate::tests::timestamp(),
            &[
                ("0001_migration_ledger.sql", sql::CREATE_LEDGER),
                ("0002_repair.sql", fixtures::REPAIR_LEDGER),
            ],
        )
        .expect_err("must reject before executing repair SQL");
        assert_eq!(
            connection
                .query_row(crate::sql::tests::PARTIAL, [], |row| row.get::<_, i64>(0))
                .expect("repair was not executed"),
            0
        );
    }
}
