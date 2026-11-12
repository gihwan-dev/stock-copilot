use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use crate::harness_workspace_tests::repository_root;

use super::cargo_config::{ConfigFile, MUTANTS_FILE, NEXTEST_FILE, ProblemKind, config_problems};

const CASES_FILE: &str = "harness/source-checks/cargo-config-cases.toml";

const DOCUMENTED_FORBIDDEN_KEYS: [&str; 6] = [
    "profile.ci",
    "profile.default.overrides",
    "profile.default.default-filter",
    "profile.default.retries",
    "scripts.setup",
    "test-groups",
];

const DOCUMENTED_FORBIDDEN_MUTANTS_KEYS: [&str; 6] = [
    "exclude_globs",
    "exclude_re",
    "examine_re",
    "skip_calls",
    "error_values",
    "additional_cargo_args",
];

const MUTATION_TARGET_GLOBS: [&str; 4] = [
    "crates/domain/src/**/*.rs",
    "crates/order-execution/src/**/*.rs",
    "crates/application/src/order_gate.rs",
    "crates/application/src/order_gate/**/*.rs",
];

#[derive(Debug)]
struct Case {
    name: String,
    file: ConfigFile,
    text: String,
    expect: BTreeSet<String>,
}

fn text_field(entry: &toml::Value, key: &str) -> String {
    entry
        .get(key)
        .and_then(toml::Value::as_str)
        .map(String::from)
        .expect("case text field")
}

fn cases(root: &Path) -> Vec<Case> {
    let table: toml::Table = fs::read_to_string(root.join(CASES_FILE))
        .expect("read cargo config cases")
        .parse()
        .expect("parse cargo config cases");
    table
        .get("case")
        .and_then(toml::Value::as_array)
        .expect("case list")
        .iter()
        .map(|entry| Case {
            name: text_field(entry, "name"),
            file: ConfigFile::named(&text_field(entry, "file")).expect("known config file"),
            text: text_field(entry, "text"),
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

fn reported(case: &Case) -> BTreeSet<String> {
    config_problems(case.file, &case.text)
        .iter()
        .map(ToString::to_string)
        .collect()
}

fn kind_label(kind: &ProblemKind) -> &'static str {
    match kind {
        ProblemKind::Unparsable => "unparsable",
        ProblemKind::Key(_) => "key",
        ProblemKind::Missing(_) => "missing",
        ProblemKind::Value(_) => "value",
    }
}

fn kind_catalogue() -> BTreeSet<String> {
    let kinds = [
        ProblemKind::Unparsable,
        ProblemKind::Key(String::new()),
        ProblemKind::Missing(String::new()),
        ProblemKind::Value(String::new()),
    ];
    [ConfigFile::Nextest, ConfigFile::Mutants]
        .into_iter()
        .flat_map(|file| {
            kinds
                .iter()
                .map(move |kind| format!("{}:{}", file.label(), kind_label(kind)))
        })
        .collect()
}

fn kind_of(rule: &str) -> String {
    rule.splitn(3, ':').take(2).collect::<Vec<_>>().join(":")
}

#[test]
fn cargo_config_cases_report_exactly_their_problems() {
    let root = repository_root();
    let mismatches: Vec<String> = cases(&root)
        .iter()
        .filter_map(|case| {
            let found = reported(case);
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
fn every_cargo_config_problem_kind_has_an_example() {
    let root = repository_root();
    let exemplified: BTreeSet<String> = cases(&root)
        .iter()
        .flat_map(|case| case.expect.iter().map(|rule| kind_of(rule)))
        .collect();
    let missing: Vec<String> = kind_catalogue()
        .into_iter()
        .filter(|kind| !exemplified.contains(kind))
        .collect();
    assert!(
        missing.is_empty(),
        "problem kinds without an example: {missing:?}"
    );
    let clean: BTreeSet<ConfigFile> = cases(&root)
        .iter()
        .filter(|case| case.expect.is_empty())
        .map(|case| case.file)
        .collect();
    assert_eq!(
        clean,
        BTreeSet::from([ConfigFile::Nextest, ConfigFile::Mutants]),
        "each file needs an allowed example"
    );
}

#[test]
fn documented_forbidden_cargo_keys_have_examples() {
    let root = repository_root();
    let exemplified: BTreeSet<String> = cases(&root)
        .into_iter()
        .flat_map(|case| case.expect)
        .collect();
    let missing: Vec<String> = DOCUMENTED_FORBIDDEN_KEYS
        .into_iter()
        .map(|key| format!("nextest-config:key:{key}"))
        .chain(
            DOCUMENTED_FORBIDDEN_MUTANTS_KEYS
                .into_iter()
                .map(|key| format!("cargo-mutants:key:{key}")),
        )
        .filter(|rule| !exemplified.contains(rule))
        .collect();
    assert!(missing.is_empty(), "keys without an example: {missing:?}");
}

#[test]
fn repository_cargo_settings_are_the_allowed_ones() {
    let root = repository_root();
    let directory = root.join(".cargo");
    let names: BTreeSet<String> = fs::read_dir(&directory)
        .expect("read .cargo")
        .map(|entry| {
            entry
                .expect(".cargo entry")
                .file_name()
                .to_str()
                .map(String::from)
                .expect("utf-8 name")
        })
        .collect();
    assert_eq!(names, BTreeSet::from([String::from(MUTANTS_FILE)]));
    for (path, file) in [
        (directory.join(MUTANTS_FILE), ConfigFile::Mutants),
        (root.join(NEXTEST_FILE), ConfigFile::Nextest),
    ] {
        let text = fs::read_to_string(&path).expect("read setting");
        let problems: Vec<String> = config_problems(file, &text)
            .iter()
            .map(ToString::to_string)
            .collect();
        assert!(problems.is_empty(), "{}: {problems:?}", path.display());
    }
}

#[test]
fn mutation_targets_are_the_documented_modules() {
    let root = repository_root();
    let table: toml::Table = fs::read_to_string(root.join(".cargo").join(MUTANTS_FILE))
        .expect("read mutants settings")
        .parse()
        .expect("parse mutants settings");
    let globs: Vec<&str> = table
        .get("examine_globs")
        .and_then(toml::Value::as_array)
        .expect("examine_globs")
        .iter()
        .map(|glob| glob.as_str().expect("glob text"))
        .collect();
    assert_eq!(globs, MUTATION_TARGET_GLOBS);
}
