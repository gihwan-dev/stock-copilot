use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use crate::harness_workspace_tests::{fresh_directory, generate_lockfile, repository_root};

use super::allowlist::{ALLOWLIST_FILE, Allowlists};
use super::finding::{ExpectProblem, Rule};
use super::manifest::BANNED_TABLES;
use super::tokens::{
    BANNED_ATTRIBUTES, BANNED_MACROS, CLOCK_AND_ENVIRONMENT_FUNCTIONS, CLOCK_METHODS, CLOCK_PREFIX,
    DOMAIN_FLOATS, ENVIRONMENT_GLOB, ENVIRONMENT_MACRO, OPTIONAL_ENVIRONMENT_MACRO,
    TOOL_ATTRIBUTES,
};
use super::{NESTED_CONFIG_FILES, check_repository, check_source};

const CASES_FILE: &str = "harness/source-checks/cases.toml";
const SOURCES_DIRECTORY: &str = "harness/source-checks/sources";
const WORKSPACES_DIRECTORY: &str = "harness/source-checks/workspaces";
const BASE_WORKSPACE: &str = "base";

const EXPECT_PROBLEMS: [ExpectProblem; 4] = [
    ExpectProblem::Inner,
    ExpectProblem::NoReason,
    ExpectProblem::MultipleLints,
    ExpectProblem::Malformed,
];

const UNIT_RULES: [Rule; 12] = [
    Rule::Comment,
    Rule::Unparsable,
    Rule::CfgNotTest,
    Rule::CfgTestOutsideModule,
    Rule::CfgTestModuleName,
    Rule::CfgMacro,
    Rule::LintsNotInherited,
    Rule::BuildScript,
    Rule::ProcMacro,
    Rule::CargoConfig,
    Rule::Symlink,
    Rule::TargetOutsideMember,
];

const REQUIRED_CLEAN_EXAMPLES: [&str; 5] = [
    "clean-slashes-in-strings.rs",
    "clean-cfg-test-module.rs",
    "clean-expect.rs",
    "clean-clock-port.rs",
    "clean-clock-env-allowlisted.rs",
];

#[derive(Debug)]
struct SourceCase {
    example: String,
    path: String,
    expect: BTreeSet<String>,
}

#[derive(Debug)]
struct WorkspaceCase {
    name: String,
    overlay: Option<String>,
    symlink: Option<(String, String)>,
    expect: BTreeSet<String>,
}

fn text_field(entry: &toml::Value, key: &str) -> String {
    entry
        .get(key)
        .and_then(toml::Value::as_str)
        .map(String::from)
        .expect("case text field")
}

fn expected_rules(entry: &toml::Value) -> BTreeSet<String> {
    entry
        .get("expect")
        .and_then(toml::Value::as_array)
        .expect("case expect list")
        .iter()
        .map(|rule| rule.as_str().map(String::from).expect("rule name"))
        .collect()
}

fn case_entries(root: &Path, key: &str) -> Vec<toml::Value> {
    let table: toml::Table = fs::read_to_string(root.join(CASES_FILE))
        .expect("read cases")
        .parse()
        .expect("parse cases");
    table
        .get(key)
        .and_then(toml::Value::as_array)
        .cloned()
        .expect("case list")
}

fn source_cases(root: &Path) -> Vec<SourceCase> {
    case_entries(root, "source")
        .iter()
        .map(|entry| SourceCase {
            example: text_field(entry, "example"),
            path: text_field(entry, "path"),
            expect: expected_rules(entry),
        })
        .collect()
}

fn workspace_cases(root: &Path) -> Vec<WorkspaceCase> {
    case_entries(root, "workspace")
        .iter()
        .map(|entry| WorkspaceCase {
            name: text_field(entry, "name"),
            overlay: entry.get("overlay").map(|_| text_field(entry, "overlay")),
            symlink: entry
                .get("symlink")
                .map(|link| (text_field(link, "link"), text_field(link, "target"))),
            expect: expected_rules(entry),
        })
        .collect()
}

fn reported_rules(findings: &[super::Finding]) -> BTreeSet<String> {
    findings
        .iter()
        .map(|finding| finding.rule.to_string())
        .collect()
}

fn rule_catalogue() -> BTreeSet<String> {
    let mut rules: Vec<Rule> = UNIT_RULES.to_vec();
    rules.extend(EXPECT_PROBLEMS.map(Rule::Expect));
    rules.extend(
        BANNED_ATTRIBUTES
            .into_iter()
            .chain(TOOL_ATTRIBUTES)
            .map(|name| Rule::Attribute(String::from(name))),
    );
    rules.extend(BANNED_MACROS.map(|name| Rule::Macro(String::from(name))));
    rules.extend(DOMAIN_FLOATS.map(|name| Rule::DomainFloat(String::from(name))));
    rules.extend(
        [
            CLOCK_PREFIX,
            ENVIRONMENT_MACRO,
            OPTIONAL_ENVIRONMENT_MACRO,
            ENVIRONMENT_GLOB,
        ]
        .into_iter()
        .chain(CLOCK_AND_ENVIRONMENT_FUNCTIONS)
        .chain(CLOCK_METHODS)
        .map(|name| Rule::ClockOrEnvironment(String::from(name))),
    );
    rules.extend(BANNED_TABLES.map(|name| Rule::ManifestTable(String::from(name))));
    rules.extend(NESTED_CONFIG_FILES.map(|name| Rule::NestedConfig(String::from(name))));
    rules.iter().map(ToString::to_string).collect()
}

fn materialized_name(name: &str) -> String {
    let base = name.strip_suffix(".example").unwrap_or(name);
    base.strip_prefix("dot-")
        .map_or_else(|| String::from(base), |rest| format!(".{rest}"))
}

fn copy_fixture(source: &Path, destination: &Path) {
    fs::create_dir_all(destination).expect("create fixture directory");
    for listed in fs::read_dir(source).expect("read fixture directory") {
        let entry = listed.expect("fixture entry");
        let name = entry
            .file_name()
            .to_str()
            .map(String::from)
            .expect("utf-8 name");
        let target = destination.join(materialized_name(&name));
        if entry.file_type().expect("fixture file type").is_dir() {
            copy_fixture(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), &target).expect("copy fixture file");
        }
    }
}

fn workspace_mismatch(
    root: &Path,
    allowlists: &Allowlists,
    case: &WorkspaceCase,
) -> Option<String> {
    let fixtures = root.join(WORKSPACES_DIRECTORY);
    let directory = fresh_directory(root, &format!("source-checks/{}", case.name));
    copy_fixture(&fixtures.join(BASE_WORKSPACE), &directory);
    if let Some(overlay) = &case.overlay {
        copy_fixture(&fixtures.join(overlay), &directory);
    }
    if let Some((link, target)) = &case.symlink {
        std::os::unix::fs::symlink(target, directory.join(link)).expect("create symlink");
    }
    generate_lockfile(&directory.join("Cargo.toml"));
    let reported = match check_repository(&directory, allowlists) {
        Ok(findings) => reported_rules(&findings.into_iter().collect::<Vec<_>>()),
        Err(failure) => BTreeSet::from([format!("failure: {failure}")]),
    };
    (reported != case.expect).then(|| {
        format!(
            "{}: expected {:?}, reported {reported:?}",
            case.name, case.expect
        )
    })
}

#[test]
fn every_source_example_is_listed_in_cases() {
    let root = repository_root();
    let listed: BTreeSet<String> = source_cases(&root)
        .into_iter()
        .map(|case| case.example)
        .collect();
    let present: BTreeSet<String> = fs::read_dir(root.join(SOURCES_DIRECTORY))
        .expect("read source examples")
        .map(|listed_entry| {
            listed_entry
                .expect("source example entry")
                .file_name()
                .to_str()
                .map(String::from)
                .expect("utf-8 example name")
        })
        .collect();
    assert_eq!(
        listed, present,
        "examples on disk and in {CASES_FILE} differ"
    );
}

#[test]
fn source_examples_report_exactly_their_rules() {
    let root = repository_root();
    let allowlists = Allowlists::load(&root).expect("load allowlists");
    let mismatches: Vec<String> = source_cases(&root)
        .into_iter()
        .filter_map(|case| {
            let source = fs::read_to_string(root.join(SOURCES_DIRECTORY).join(&case.example))
                .expect("read source example");
            let reported = reported_rules(&check_source(&case.path, &source, &allowlists));
            (reported != case.expect).then(|| {
                format!(
                    "{} as {}: expected {:?}, reported {reported:?}",
                    case.example, case.path, case.expect
                )
            })
        })
        .collect();
    assert!(mismatches.is_empty(), "{mismatches:#?}");
}

#[test]
fn workspace_examples_report_exactly_their_rules() {
    let root = repository_root();
    let allowlists = Allowlists::load(&root).expect("load allowlists");
    let mismatches: Vec<String> = workspace_cases(&root)
        .iter()
        .filter_map(|case| workspace_mismatch(&root, &allowlists, case))
        .collect();
    assert!(mismatches.is_empty(), "{mismatches:#?}");
}

#[test]
fn every_harness_rule_has_a_violation_example() {
    let root = repository_root();
    let mut exemplified: BTreeSet<String> = source_cases(&root)
        .into_iter()
        .flat_map(|case| case.expect)
        .collect();
    exemplified.extend(
        workspace_cases(&root)
            .into_iter()
            .flat_map(|case| case.expect),
    );
    let missing: Vec<String> = rule_catalogue()
        .into_iter()
        .filter(|rule| !exemplified.contains(rule))
        .collect();
    assert!(
        missing.is_empty(),
        "rules without a violation example: {missing:?}"
    );
}

#[test]
fn clean_examples_expect_no_findings() {
    let root = repository_root();
    let cases = source_cases(&root);
    let missing: Vec<&str> = REQUIRED_CLEAN_EXAMPLES
        .into_iter()
        .filter(|example| {
            !cases
                .iter()
                .any(|case| case.example == *example && case.expect.is_empty())
        })
        .collect();
    assert!(missing.is_empty(), "clean examples missing: {missing:?}");
}

#[test]
fn allowlists_hold_only_the_documented_paths() {
    let root = repository_root();
    let table: toml::Table = fs::read_to_string(root.join(ALLOWLIST_FILE))
        .expect("read allowlists")
        .parse()
        .expect("parse allowlists");
    let paths = |key: &str| -> BTreeSet<String> {
        table
            .get(key)
            .and_then(toml::Value::as_array)
            .expect("allowlist")
            .iter()
            .map(|entry| text_field(entry, "path"))
            .collect()
    };
    assert_eq!(
        paths("generated"),
        BTreeSet::from([String::from("crates/adapters/broker-toss-schema")])
    );
    assert_eq!(
        paths("clock-environment"),
        BTreeSet::from([String::from("apps/desktop/src-tauri/src/main.rs")])
    );
    assert_eq!(
        paths("build-script"),
        BTreeSet::from([String::from("apps/desktop/src-tauri/build.rs")])
    );
}

#[test]
fn repository_passes_source_checks() {
    let root = repository_root();
    let allowlists = Allowlists::load(&root).expect("load allowlists");
    let findings = check_repository(&root, &allowlists).expect("check repository");
    let lines: Vec<String> = findings.iter().map(ToString::to_string).collect();
    assert!(lines.is_empty(), "{lines:#?}");
}

#[test]
fn allowlist_entry_for_a_missing_file_fails_to_load() {
    let root = repository_root();
    let directory = fresh_directory(&root, "source-checks-missing-allowlist");
    let listed = directory.join(ALLOWLIST_FILE);
    fs::create_dir_all(listed.parent().expect("allowlist directory")).expect("create directory");
    fs::create_dir_all(directory.join("crates/generated")).expect("create generated directory");
    fs::write(directory.join("build.rs"), "fn main() {}\n").expect("write build script");
    fs::write(
        &listed,
        "[[generated]]\npath = \"crates/generated\"\nreason = \"harness\"\n\n[[clock-environment]]\npath = \"src/clock.rs\"\nreason = \"harness\"\n\n[[build-script]]\npath = \"build.rs\"\nreason = \"harness\"\n",
    )
    .expect("write allowlist");
    let loaded = Allowlists::load(&directory);
    assert!(
        loaded
            .as_ref()
            .is_err_and(|failure| failure.to_string().contains("src/clock.rs")),
        "{loaded:?}"
    );
    fs::create_dir_all(directory.join("src")).expect("create source directory");
    fs::write(directory.join("src/clock.rs"), "pub fn clock() {}\n").expect("write clock");
    assert!(Allowlists::load(&directory).is_ok(), "existing paths load");
}
