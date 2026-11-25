use std::collections::BTreeSet;
use std::fs;

use crate::harness_workspace_tests::repository_root;

use super::statement::{self, Source};
use super::structure::{self, Context, Structure};

const CASES_FILE: &str = "harness/sql-check/structure-cases.toml";
const PREFIX: &str = "sql-structure:";

fn table() -> toml::Table {
    fs::read_to_string(repository_root().join(CASES_FILE))
        .expect("read structure cases")
        .parse()
        .expect("parse structure cases")
}

fn known_migration() -> String {
    table()
        .get("known_migration")
        .and_then(toml::Value::as_str)
        .map(String::from)
        .expect("known migration")
}

fn cases(key: &str) -> Vec<toml::Value> {
    let table: toml::Table = fs::read_to_string(repository_root().join(CASES_FILE))
        .expect("read structure cases")
        .parse()
        .expect("parse structure cases");
    table
        .get(key)
        .and_then(toml::Value::as_array)
        .cloned()
        .expect("structure case list")
}

fn field(entry: &toml::Value, key: &str) -> String {
    entry
        .get(key)
        .and_then(toml::Value::as_str)
        .map(String::from)
        .expect("structure case field")
}

fn expected(entry: &toml::Value) -> Vec<String> {
    entry
        .get("expect")
        .and_then(toml::Value::as_array)
        .expect("structure case expect")
        .iter()
        .map(|value| format!("{PREFIX}{}", value.as_str().expect("expect text")))
        .collect()
}

fn money() -> BTreeSet<String> {
    ["amount", "price"].into_iter().map(String::from).collect()
}

fn statement_rules(sql: &str, source: Source) -> Vec<String> {
    statement::check(sql, source, &money())
        .into_iter()
        .map(|problem| Structure::Statement(problem).to_string())
        .collect()
}

fn context() -> Context {
    Context {
        migrations: BTreeSet::from([known_migration()]),
        money: money(),
        forwarders: BTreeSet::from([(
            String::from("crates/adapters/store-sqlite/src/runner.rs"),
            String::from("run"),
        )]),
    }
}

#[test]
fn query_cases_match() {
    let entries = cases("query");
    assert!(entries.len() >= 30);
    for entry in entries {
        let sql = field(&entry, "sql");
        assert_eq!(
            statement_rules(&sql, Source::Query),
            expected(&entry),
            "{sql}"
        );
    }
}

#[test]
fn migration_cases_match() {
    let entries = cases("migration");
    assert!(entries.len() >= 6);
    for entry in entries {
        let sql = field(&entry, "sql");
        assert_eq!(
            statement_rules(&sql, Source::Migration),
            expected(&entry),
            "{sql}"
        );
    }
}

#[test]
fn money_columns_come_from_decimal_checks() {
    for entry in cases("money") {
        let found = statement::money_columns(&[(String::from("case.sql"), field(&entry, "sql"))])
            .expect("money columns");
        let wanted: BTreeSet<String> = entry
            .get("expect")
            .and_then(toml::Value::as_array)
            .expect("money expect")
            .iter()
            .map(|value| String::from(value.as_str().expect("column")))
            .collect();
        assert_eq!(found, wanted);
    }
}

#[test]
fn source_cases_match() {
    let entries = cases("source");
    assert!(entries.len() >= 18);
    let context = context();
    for entry in entries {
        let path = field(&entry, "path");
        let text = field(&entry, "text");
        let found: Vec<String> = structure::check_file(&path, &text, &context)
            .iter()
            .map(|finding| finding.rule.to_string())
            .collect();
        assert_eq!(found, expected(&entry), "{path}: {text}");
    }
}

#[test]
fn repository_context_loads_money_columns_and_forwarders() {
    let context = Context::load(&repository_root()).expect("context");
    for column in ["quantity", "acquisition_price", "valuation", "profit"] {
        assert!(context.money.contains(column), "{column}");
    }
    assert!(context.forwarders.len() >= 5);
    assert!(context.migrations.len() >= 20);
}

#[test]
fn test_paths_are_recognized() {
    assert!(structure::is_test_path(
        "crates/adapters/store-sqlite/src/sql/tests.rs"
    ));
    assert!(structure::is_test_path(
        "crates/adapters/store-sqlite/src/signals_tests/a.rs"
    ));
    assert!(structure::is_test_path(
        "crates/adapters/store-sqlite/tests/a.rs"
    ));
    assert!(!structure::is_test_path(
        "crates/adapters/store-sqlite/src/sql/holdings.rs"
    ));
}
