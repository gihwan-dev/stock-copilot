use crate::{migration, sql, tests::timestamp};

fn apply_second(source: &'static str) -> rusqlite::Result<()> {
    let mut connection = rusqlite::Connection::open_in_memory()?;
    migration::apply(
        &mut connection,
        timestamp(),
        &[
            ("0001_migration_ledger.sql", sql::migration::CREATE_LEDGER),
            ("0002_fts_test.sql", source),
        ],
    )
}

#[test]
fn declared_fts5_and_actual_shadow_tables_are_accepted() {
    apply_second(sql::fts_guard_tests::FTS).expect("fts5 schema");
}

#[test]
fn ordinary_nonstrict_table_is_still_rejected() {
    apply_second(sql::fts_guard_tests::NONSTRICT).expect_err("strict");
}

#[test]
fn non_fts5_virtual_table_is_still_rejected() {
    let connection = rusqlite::Connection::open_in_memory().expect("memory");
    connection
        .execute_batch(sql::fts_guard_tests::FTS4)
        .expect("fts4 available");
    apply_second(sql::fts_guard_tests::FTS4).expect_err("not fts5");
}

#[test]
fn shadow_name_imitation_is_rejected_even_if_sqlite_calls_it_shadow() {
    let connection = rusqlite::Connection::open_in_memory().expect("memory");
    connection
        .execute_batch(sql::fts_guard_tests::SPOOF)
        .expect("sqlite accepts spoof");
    let kind: String = connection
        .query_row(sql::fts_guard_tests::SPOOF_KIND, [], |row| row.get(0))
        .expect("kind");
    assert_eq!(kind, "shadow");
    apply_second(sql::fts_guard_tests::SPOOF).expect_err("fake shadow ddl");
}

#[test]
fn a_shadow_like_name_without_an_fts_parent_is_rejected() {
    apply_second(sql::fts_guard_tests::ORPHAN).expect_err("ordinary table");
}

#[test]
fn fts_created_outside_the_registered_migration_is_rejected() {
    let mut connection = rusqlite::Connection::open_in_memory().expect("memory");
    migration::apply(
        &mut connection,
        timestamp(),
        &[("0001_migration_ledger.sql", sql::migration::CREATE_LEDGER)],
    )
    .expect("ledger");
    connection
        .execute_batch(sql::fts_guard_tests::ROGUE_FTS)
        .expect("unregistered fts");
    migration::validate_schema(&connection).expect_err("unregistered virtual table");
}

#[test]
fn fts5_with_content_shadow_accepts_only_real_generated_shape() {
    apply_second(sql::fts_guard_tests::FTS_DEFAULT).expect("fts5 content shadow");
}

#[test]
fn fts_text_inside_a_string_is_not_a_migration_declaration() {
    let connection = rusqlite::Connection::open_in_memory().expect("memory");
    connection
        .execute_batch(sql::fts_guard_tests::FTS)
        .expect("fts");
    crate::fts_schema::validate(
        &connection,
        &[("0002_fake.sql", sql::fts_guard_tests::FAKE_DECLARATION)],
    )
    .expect_err("not a declaration");
}

#[test]
fn identical_ddl_shadow_replacement_must_pass_fts_integrity() {
    let connection = rusqlite::Connection::open_in_memory().expect("memory");
    connection
        .execute_batch(sql::fts_guard_tests::EXACT_SPOOF)
        .expect("spoof");
    let kind: String = connection
        .query_row(sql::fts_guard_tests::SPOOF_KIND, [], |row| row.get(0))
        .expect("kind");
    assert_eq!(kind, "shadow");
    crate::fts_schema::validate(&connection, &[("0002_fts.sql", sql::fts_guard_tests::FTS)])
        .expect_err("fts integrity detects damaged runtime shadow");
    apply_second(sql::fts_guard_tests::EXACT_SPOOF).expect_err("missing fts shadow contents");
}

#[test]
fn explicitly_created_shadow_lookalike_with_copied_contents_is_rejected() {
    let connection = rusqlite::Connection::open_in_memory().expect("memory");
    connection
        .execute_batch(sql::fts_guard_tests::COPIED_SPOOF)
        .expect("spoof");
    let result: String = connection
        .query_row(sql::migration::QUICK_CHECK, [], |row| row.get(0))
        .expect("fts data copied");
    assert_eq!(result, "ok");
    apply_second(sql::fts_guard_tests::COPIED_SPOOF).expect_err("ordinary shadow lookalike");
}

#[test]
fn declared_fts5_name_and_its_actual_shadow_tables_are_accepted() {
    apply_second(sql::fts_guard_tests::ROGUE_FTS).expect("declared fts5");
    apply_second(sql::fts_guard_tests::FTS_NO_DOCSIZE).expect("no docsize shadow");
}

#[test]
fn quick_check_rejects_damaged_shadow_for_any_declared_fts_name() {
    let connection = rusqlite::Connection::open_in_memory().expect("memory");
    connection
        .execute_batch(sql::fts_guard_tests::ROGUE_DAMAGED)
        .expect("spoof");
    crate::fts_schema::validate(
        &connection,
        &[("0002_fts.sql", sql::fts_guard_tests::ROGUE_FTS)],
    )
    .expect_err("fts integrity detects damaged runtime shadow");
}
