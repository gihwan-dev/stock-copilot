use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use crate::harness_workspace_tests::repository_root;

use super::money::{MONEY_COLUMNS_FILE, MoneyNames, money_rules};
use super::rules::{
    ARITHMETIC_OPERATORS, ASSEMBLY_MACROS, BETWEEN, COMPARISON_OPERATORS, Follow, MONEY_FUNCTIONS,
    Recognition, Rule, STATEMENT_STARTS, recognize,
};
use super::{check_file, check_repository};

const CASES_FILE: &str = "harness/sql-check/cases.toml";
const SOURCES_DIRECTORY: &str = "harness/sql-check/sources";

const UNIT_RULES: [Rule; 5] = [
    Rule::MoneyOrderBy,
    Rule::OrderByPosition,
    Rule::NotConstant,
    Rule::OutsideStore,
    Rule::Unparsable,
];

const REQUIRED_CLEAN_STATEMENTS: [&str; 6] = [
    "clean-plain-columns",
    "clean-length",
    "clean-typeof",
    "clean-count",
    "clean-limit-ends-order-by",
    "clean-sequence-arithmetic",
];

const REQUIRED_CLEAN_SOURCES: [&str; 3] = [
    "store-clean.rs",
    "outside-store-text.rs",
    "migration-clean.sql",
];

fn cases(root: &Path, key: &str) -> Vec<toml::Value> {
    let table: toml::Table = fs::read_to_string(root.join(CASES_FILE))
        .expect("read sql cases")
        .parse()
        .expect("parse sql cases");
    table
        .get(key)
        .and_then(toml::Value::as_array)
        .cloned()
        .expect("sql case list")
}

fn text_field(entry: &toml::Value, key: &str) -> String {
    entry
        .get(key)
        .and_then(toml::Value::as_str)
        .map(String::from)
        .expect("sql case text field")
}

fn expected(entry: &toml::Value) -> BTreeSet<String> {
    entry
        .get("expect")
        .and_then(toml::Value::as_array)
        .expect("sql case expect list")
        .iter()
        .map(|rule| rule.as_str().map(String::from).expect("rule name"))
        .collect()
}

fn names(root: &Path) -> MoneyNames {
    MoneyNames::load(root).expect("load money columns")
}

fn rule_names(rules: impl IntoIterator<Item = Rule>) -> BTreeSet<String> {
    rules.into_iter().map(|rule| rule.to_string()).collect()
}

fn source_reported(root: &Path, entry: &toml::Value, money: &MoneyNames) -> BTreeSet<String> {
    let example = text_field(entry, "example");
    let text = fs::read_to_string(root.join(SOURCES_DIRECTORY).join(example))
        .expect("read sql source example");
    rule_names(
        check_file(&text_field(entry, "path"), &text, money)
            .into_iter()
            .map(|finding| finding.rule),
    )
}

#[test]
fn statement_cases_report_exactly_their_rules() {
    let root = repository_root();
    let money = names(&root);
    let mismatches: Vec<String> = cases(&root, "statement")
        .iter()
        .filter_map(|entry| {
            let reported = rule_names(money_rules(&text_field(entry, "sql"), &money));
            let wanted = expected(entry);
            (reported != wanted).then(|| {
                format!(
                    "{}: expected {wanted:?}, reported {reported:?}",
                    text_field(entry, "name")
                )
            })
        })
        .collect();
    assert!(mismatches.is_empty(), "{mismatches:#?}");
}

#[test]
fn source_cases_report_exactly_their_rules() {
    let root = repository_root();
    let money = names(&root);
    let mismatches: Vec<String> = cases(&root, "source")
        .iter()
        .filter_map(|entry| {
            let reported = source_reported(&root, entry, &money);
            let wanted = expected(entry);
            (reported != wanted).then(|| {
                format!(
                    "{}: expected {wanted:?}, reported {reported:?}",
                    text_field(entry, "example")
                )
            })
        })
        .collect();
    assert!(mismatches.is_empty(), "{mismatches:#?}");
}

#[test]
fn every_source_example_is_listed() {
    let root = repository_root();
    let listed: BTreeSet<String> = cases(&root, "source")
        .iter()
        .map(|entry| text_field(entry, "example"))
        .collect();
    let present: BTreeSet<String> = fs::read_dir(root.join(SOURCES_DIRECTORY))
        .expect("read sql sources")
        .map(|entry| {
            entry
                .expect("sql source entry")
                .file_name()
                .to_str()
                .map(String::from)
                .expect("utf-8 name")
        })
        .collect();
    assert_eq!(listed, present);
}

#[test]
fn recognition_cases_match() {
    let root = repository_root();
    let mismatches: Vec<String> = cases(&root, "recognition")
        .iter()
        .filter_map(|entry| {
            let text = text_field(entry, "text");
            let wanted = match text_field(entry, "kind").as_str() {
                "sql" => Recognition::Sql,
                "text" => Recognition::Text,
                other => panic!("unknown recognition kind {other}"),
            };
            let reported = recognize(&text);
            (reported != wanted).then(|| format!("{text:?}: expected {wanted:?}, got {reported:?}"))
        })
        .collect();
    assert!(mismatches.is_empty(), "{mismatches:#?}");
}

#[test]
fn every_statement_start_has_a_recognition_case() {
    let root = repository_root();
    let recognized: Vec<String> = cases(&root, "recognition")
        .iter()
        .filter(|entry| text_field(entry, "kind") == "sql")
        .map(|entry| {
            text_field(entry, "text")
                .split_whitespace()
                .collect::<Vec<&str>>()
                .join(" ")
                .to_lowercase()
        })
        .collect();
    let mut missing = Vec::new();
    for (keyword, follow) in STATEMENT_STARTS {
        let prefixes: Vec<String> = match follow {
            Follow::Words(words) => words
                .iter()
                .map(|word| format!("{keyword} {word}"))
                .collect(),
            Follow::Any | Follow::Contains(_) => vec![format!("{keyword} ")],
        };
        missing.extend(prefixes.into_iter().filter(|prefix| {
            !recognized
                .iter()
                .any(|text| text.starts_with(prefix.as_str()))
        }));
    }
    assert!(
        missing.is_empty(),
        "statement starts without a case: {missing:?}"
    );
}

#[test]
fn every_sql_rule_has_a_violation_example() {
    let root = repository_root();
    let mut exemplified: BTreeSet<String> = cases(&root, "statement")
        .iter()
        .flat_map(expected)
        .collect();
    exemplified.extend(cases(&root, "source").iter().flat_map(expected));
    let mut catalogue: Vec<Rule> = UNIT_RULES.to_vec();
    catalogue.extend(MONEY_FUNCTIONS.map(|name| Rule::MoneyFunction(String::from(name))));
    catalogue.extend(ARITHMETIC_OPERATORS.map(|name| Rule::MoneyArithmetic(String::from(name))));
    catalogue.extend(
        COMPARISON_OPERATORS
            .into_iter()
            .chain([BETWEEN])
            .map(|name| Rule::MoneyComparison(String::from(name))),
    );
    catalogue.extend(ASSEMBLY_MACROS.map(|name| Rule::Assembly(String::from(name))));
    let missing: Vec<String> = rule_names(catalogue)
        .into_iter()
        .filter(|rule| !exemplified.contains(rule))
        .collect();
    assert!(
        missing.is_empty(),
        "rules without a violation example: {missing:?}"
    );
}

#[test]
fn every_money_segment_has_a_violation_example() {
    let root = repository_root();
    let table: toml::Table = fs::read_to_string(root.join(MONEY_COLUMNS_FILE))
        .expect("read money columns")
        .parse()
        .expect("parse money columns");
    let statements: Vec<String> = cases(&root, "statement")
        .iter()
        .filter(|entry| !expected(entry).is_empty())
        .map(|entry| text_field(entry, "sql").to_lowercase())
        .collect();
    let missing: Vec<String> = table
        .get("segments")
        .and_then(toml::Value::as_array)
        .expect("segments")
        .iter()
        .map(|segment| segment.as_str().map(String::from).expect("segment"))
        .filter(|segment| !statements.iter().any(|sql| sql.contains(segment.as_str())))
        .collect();
    assert!(
        missing.is_empty(),
        "segments without a violation example: {missing:?}"
    );
}

#[test]
fn clean_examples_expect_no_findings() {
    let root = repository_root();
    let clean = |key: &str, field: &str| -> BTreeSet<String> {
        cases(&root, key)
            .iter()
            .filter(|entry| expected(entry).is_empty())
            .map(|entry| text_field(entry, field))
            .collect()
    };
    let statements = clean("statement", "name");
    let sources = clean("source", "example");
    let missing: Vec<&str> = REQUIRED_CLEAN_STATEMENTS
        .into_iter()
        .filter(|name| !statements.contains(*name))
        .chain(
            REQUIRED_CLEAN_SOURCES
                .into_iter()
                .filter(|name| !sources.contains(*name)),
        )
        .collect();
    assert!(missing.is_empty(), "clean examples missing: {missing:?}");
}

#[test]
fn money_columns_file_needs_segments_and_reason() {
    let root = repository_root();
    let directory = crate::harness_workspace_tests::fresh_directory(&root, "sql-check-money-file");
    let listed = directory.join(MONEY_COLUMNS_FILE);
    fs::create_dir_all(listed.parent().expect("money file directory")).expect("create directory");
    fs::write(&listed, "segments = []\nreason = \"harness\"\n").expect("write money file");
    assert!(MoneyNames::load(&directory).is_err(), "empty segments load");
    fs::write(&listed, "segments = [\"amount\"]\n").expect("write money file");
    assert!(
        MoneyNames::load(&directory).is_err(),
        "missing reason loads"
    );
    fs::write(&listed, "segments = [\"Amount\"]\nreason = \"harness\"\n")
        .expect("write money file");
    assert!(
        MoneyNames::load(&directory).is_err(),
        "upper case segment loads"
    );
    fs::write(&listed, "segments = [\"amount\"]\nreason = \"harness\"\n")
        .expect("write money file");
    assert!(
        MoneyNames::load(&directory).is_ok(),
        "valid file fails to load"
    );
}

#[test]
fn repository_passes_sql_check() {
    let root = repository_root();
    let findings = check_repository(&root, &names(&root)).expect("check repository");
    let lines: Vec<String> = findings.iter().map(ToString::to_string).collect();
    assert!(lines.is_empty(), "{lines:#?}");
}
