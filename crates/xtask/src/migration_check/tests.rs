use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use crate::harness_workspace_tests::{copy_tree, fresh_directory, repository_root};

use super::{MIGRATIONS_DIRECTORY, Rule, check_repository, git, migration_number};

const CASES_FILE: &str = "harness/migration-check/cases.toml";
const CASES_DIRECTORY: &str = "harness/migration-check/cases";
const DEFAULT_BRANCH: &str = "main";
const FAILURE: &str = "failure";

const RULES: [Rule; 6] = [
    Rule::Name,
    Rule::DuplicateNumber,
    Rule::Gap,
    Rule::Modified,
    Rule::Removed,
    Rule::ModifiedInHistory,
];

const REQUIRED_CLEAN_CASES: [&str; 4] = [
    "no-migrations",
    "applied-without-changes",
    "clean-sequence",
    "base-branch-master",
];

#[derive(Debug)]
struct Case {
    name: String,
    branch: String,
    work_committed: Option<String>,
    expect: BTreeSet<String>,
}

fn cases(root: &Path) -> Vec<Case> {
    let table: toml::Table = fs::read_to_string(root.join(CASES_FILE))
        .expect("read migration cases")
        .parse()
        .expect("parse migration cases");
    let text = |entry: &toml::Value, key: &str| {
        entry
            .get(key)
            .and_then(toml::Value::as_str)
            .map(String::from)
    };
    table
        .get("case")
        .and_then(toml::Value::as_array)
        .expect("migration case list")
        .iter()
        .map(|entry| Case {
            name: text(entry, "name").expect("case name"),
            branch: text(entry, "branch").unwrap_or_else(|| String::from(DEFAULT_BRANCH)),
            work_committed: text(entry, "work-committed"),
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

fn run_git(directory: &Path, arguments: &[&str]) {
    git(directory, arguments).expect("git in harness repository");
}

fn commit(directory: &Path, message: &str) {
    run_git(directory, &["add", "--all"]);
    run_git(
        directory,
        &[
            "-c",
            "user.name=harness",
            "-c",
            "user.email=harness@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "--no-verify",
            "--quiet",
            "--allow-empty",
            "--message",
            message,
        ],
    );
}

fn replace_migrations(directory: &Path, stage: &Path) {
    let migrations = directory.join(MIGRATIONS_DIRECTORY);
    if migrations.exists() {
        fs::remove_dir_all(&migrations).expect("clear migrations");
    }
    copy_tree(stage, &migrations);
}

fn reported(root: &Path, case: &Case) -> BTreeSet<String> {
    let fixtures = root.join(CASES_DIRECTORY).join(&case.name);
    let directory = fresh_directory(root, &format!("migration-check/{}", case.name));
    run_git(
        &directory,
        &["init", "--quiet", "--initial-branch", &case.branch],
    );
    fs::write(directory.join("placeholder.txt"), "harness\n").expect("write placeholder");
    if fixtures.join("base").is_dir() {
        replace_migrations(&directory, &fixtures.join("base"));
    }
    commit(&directory, "base");
    if fixtures.join("main-later").is_dir() {
        replace_migrations(&directory, &fixtures.join("main-later"));
        commit(&directory, "later on the base branch");
    }
    run_git(&directory, &["checkout", "--quiet", "-b", "work"]);
    if fixtures.join("work").is_dir() {
        replace_migrations(&directory, &fixtures.join("work"));
    }
    if case.work_committed.is_some() {
        commit(&directory, "work");
    }
    match check_repository(&directory) {
        Ok(findings) => findings
            .into_iter()
            .map(|finding| finding.rule.to_string())
            .collect(),
        Err(_) => BTreeSet::from([String::from(FAILURE)]),
    }
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
    let unlisted: Vec<String> = fs::read_dir(root.join(CASES_DIRECTORY))
        .expect("read migration case directories")
        .map(|entry| {
            entry
                .expect("case entry")
                .file_name()
                .to_str()
                .map(String::from)
                .expect("utf-8 name")
        })
        .filter(|name| !listed.contains(name))
        .collect();
    assert!(
        unlisted.is_empty(),
        "case directories not in {CASES_FILE}: {unlisted:?}"
    );
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
fn repository_passes_migration_check() {
    let root = repository_root();
    let findings = check_repository(&root).expect("check repository");
    let lines: Vec<String> = findings.iter().map(ToString::to_string).collect();
    assert!(lines.is_empty(), "{lines:#?}");
}
