use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use crate::harness_workspace_tests::repository_root;

use super::sha256::sha256_hex;
use super::{Rule, STORE_DIRECTORY, check_store, migration_number};

const CASES_FILE: &str = "harness/migration-check/cases.toml";
const CASES_DIRECTORY: &str = "harness/migration-check/cases";

const RULES: [Rule; 9] = [
    Rule::Name,
    Rule::DuplicateNumber,
    Rule::Gap,
    Rule::Empty,
    Rule::Modified,
    Rule::Removed,
    Rule::LockFormat,
    Rule::LockDuplicate,
    Rule::LockOrder,
];

const REQUIRED_CLEAN_CASES: [&str; 3] = [
    "empty-workspace",
    "pending-without-lock-entries",
    "locked-and-pending",
];

#[derive(Debug)]
struct Case {
    name: String,
    expect: BTreeSet<String>,
}

fn cases(root: &Path) -> Vec<Case> {
    let table: toml::Table = fs::read_to_string(root.join(CASES_FILE))
        .expect("read migration cases")
        .parse()
        .expect("parse migration cases");
    table
        .get("case")
        .and_then(toml::Value::as_array)
        .expect("migration case list")
        .iter()
        .map(|entry| Case {
            name: entry
                .get("name")
                .and_then(toml::Value::as_str)
                .map(String::from)
                .expect("case name"),
            expect: entry
                .get("expect")
                .and_then(toml::Value::as_array)
                .expect("expect list")
                .iter()
                .map(|rule| rule.as_str().map(String::from).expect("rule name"))
                .collect(),
        })
        .collect()
}

fn reported(root: &Path, case: &Case) -> BTreeSet<String> {
    check_store(&root.join(CASES_DIRECTORY).join(&case.name))
        .expect("check migration case")
        .into_iter()
        .map(|finding| finding.rule.to_string())
        .collect()
}

#[test]
fn migration_cases_report_exactly_their_rules() {
    let root = repository_root();
    let mismatches: Vec<String> = cases(&root)
        .iter()
        .filter_map(|case| {
            let found = reported(&root, case);
            (found != case.expect).then(|| {
                format!(
                    "{}: expected {:?}, reported {found:?}",
                    case.name, case.expect
                )
            })
        })
        .collect();
    assert!(mismatches.is_empty(), "{mismatches:#?}");
}

#[test]
fn every_case_directory_is_listed() {
    let root = repository_root();
    let listed: BTreeSet<String> = cases(&root).into_iter().map(|case| case.name).collect();
    let present: BTreeSet<String> = fs::read_dir(root.join(CASES_DIRECTORY))
        .expect("read migration case directories")
        .map(|entry| {
            entry
                .expect("case entry")
                .file_name()
                .to_str()
                .map(String::from)
                .expect("utf-8 name")
        })
        .collect();
    assert_eq!(listed, present);
}

#[test]
fn every_migration_rule_has_a_violation_example() {
    let root = repository_root();
    let exemplified: BTreeSet<String> = cases(&root)
        .into_iter()
        .flat_map(|case| case.expect)
        .collect();
    let missing: Vec<String> = RULES
        .iter()
        .map(ToString::to_string)
        .filter(|rule| !exemplified.contains(rule))
        .collect();
    assert!(
        missing.is_empty(),
        "rules without a violation example: {missing:?}"
    );
}

#[test]
fn clean_cases_expect_no_findings() {
    let root = repository_root();
    let clean: BTreeSet<String> = cases(&root)
        .into_iter()
        .filter(|case| case.expect.is_empty())
        .map(|case| case.name)
        .collect();
    let missing: Vec<&str> = REQUIRED_CLEAN_CASES
        .into_iter()
        .filter(|name| !clean.contains(*name))
        .collect();
    assert!(missing.is_empty(), "clean cases missing: {missing:?}");
}

#[test]
fn migration_names_follow_the_numbered_pattern() {
    assert_eq!(migration_number("0001_create_holdings.sql"), Some(1));
    assert_eq!(migration_number("0012_add_index_2.sql"), Some(12));
    for name in [
        "1_create.sql",
        "00001_create.sql",
        "0001_create.SQL",
        "0001-create.sql",
        "0001_.sql",
        "0001__create.sql",
        "0001_create_.sql",
        "0001_Create.sql",
        "abcd_create.sql",
        "0001_create.sql.bak",
    ] {
        assert_eq!(migration_number(name), None, "{name}");
    }
}

#[test]
fn sha256_matches_known_digests() {
    assert_eq!(
        sha256_hex(b""),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
    assert_eq!(
        sha256_hex(b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    assert_eq!(
        sha256_hex(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"),
        "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
    );
}

#[test]
fn repository_passes_migration_check() {
    let root = repository_root();
    let findings = check_store(&root.join(STORE_DIRECTORY)).expect("check repository");
    let lines: Vec<String> = findings.iter().map(ToString::to_string).collect();
    assert!(lines.is_empty(), "{lines:#?}");
}
