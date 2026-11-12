use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::arch_check::rules::parse_rules;
use crate::arch_check::{
    RULES_PATH, Rules, Workspace, check_workspace, load_rules, load_workspace,
};
use crate::harness_workspace_tests::{copy_tree, fresh_directory, generate_lockfile, repository_root};

type Expected = BTreeSet<(String, String)>;
type Cases = BTreeMap<String, (Workspace, Expected)>;

const REAL_WORKSPACE: &str = "(real workspace)";

fn expected_cases(root: &Path) -> BTreeMap<String, Expected> {
    let table: toml::Table = fs::read_to_string(root.join("harness/arch-check/cases.toml"))
        .expect("read cases.toml")
        .parse()
        .expect("parse cases.toml");
    table
        .get("case")
        .and_then(toml::Value::as_array)
        .expect("case array")
        .iter()
        .map(|case| {
            let name = case
                .get("name")
                .and_then(toml::Value::as_str)
                .expect("case name");
            let findings = case
                .get("findings")
                .and_then(toml::Value::as_array)
                .expect("findings array")
                .iter()
                .map(|finding| {
                    let field = |key: &str| {
                        String::from(
                            finding
                                .get(key)
                                .and_then(toml::Value::as_str)
                                .expect("finding field"),
                        )
                    };
                    (field("rule"), field("subject"))
                })
                .collect();
            (String::from(name), findings)
        })
        .collect()
}

fn vendor_options(directory: &Path) -> Vec<String> {
    vec![
        String::from("--config"),
        String::from("source.crates-io.replace-with=\"arch-check-vendor\""),
        String::from("--config"),
        format!(
            "source.arch-check-vendor.directory=\"{}\"",
            directory.join("vendor").display()
        ),
    ]
}

fn generate_vendored_lockfile(manifest: &Path, options: &[String]) {
    let status = Command::new("cargo")
        .args([
            "generate-lockfile",
            "--offline",
            "--quiet",
            "--manifest-path",
        ])
        .arg(manifest)
        .args(options)
        .status()
        .expect("cargo generate-lockfile");
    assert!(status.success(), "lockfile for {}", manifest.display());
}

fn copied_harness(directory_name: &str) -> (PathBuf, PathBuf) {
    let root = repository_root();
    let directory = fresh_directory(&root, directory_name);
    copy_tree(&root.join("harness").join("arch-check"), &directory);
    (root, directory)
}

fn loaded_cases(directory_name: &str) -> (Rules, Cases) {
    let (root, directory) = copied_harness(directory_name);
    let rules = load_rules(&root.join(RULES_PATH)).expect("load rules");
    let options = vendor_options(&directory);
    let mut cases: Cases = expected_cases(&root)
        .into_iter()
        .map(|(name, expected)| {
            let manifest = directory.join("cases").join(&name).join("Cargo.toml");
            generate_vendored_lockfile(&manifest, &options);
            let workspace = load_workspace(&manifest, &options).expect("load case workspace");
            (name, (workspace, expected))
        })
        .collect();
    let real = load_workspace(&root.join("Cargo.toml"), &[]).expect("load real workspace");
    cases.insert(String::from(REAL_WORKSPACE), (real, Expected::new()));
    (rules, cases)
}

fn reported(workspace: &Workspace, rules: &Rules) -> Expected {
    check_workspace(workspace, rules)
        .expect("run arch-check")
        .into_iter()
        .map(|finding| (String::from(finding.rule.name()), finding.subject))
        .collect()
}

fn mismatches(rules: &Rules, cases: &Cases) -> Vec<String> {
    cases
        .iter()
        .filter_map(|(name, (workspace, expected))| {
            let actual = reported(workspace, rules);
            (&actual != expected)
                .then(|| format!("{name}\n  expected {expected:?}\n  actual   {actual:?}"))
        })
        .collect()
}

#[test]
fn case_directories_match_case_list() {
    let root = repository_root();
    let directories: BTreeSet<String> = fs::read_dir(root.join("harness/arch-check/cases"))
        .expect("read cases directory")
        .map(|entry| {
            entry
                .expect("case entry")
                .file_name()
                .into_string()
                .expect("utf-8 case name")
        })
        .collect();
    let listed: BTreeSet<String> = expected_cases(&root).into_keys().collect();
    assert_eq!(directories, listed);
}

#[test]
fn every_case_reports_exactly_its_findings() {
    let (rules, cases) = loaded_cases("arch-check-cases");
    let failures = mismatches(&rules, &cases);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn real_workspace_passes_arch_check() {
    let root = repository_root();
    let rules = load_rules(&root.join(RULES_PATH)).expect("load rules");
    let workspace = load_workspace(&root.join("Cargo.toml"), &[]).expect("load workspace");
    let findings = reported(&workspace, &rules);
    assert!(findings.is_empty(), "{findings:?}");
}

#[test]
fn unknown_rule_keys_are_rejected() {
    let root = repository_root();
    let content = fs::read_to_string(root.join(RULES_PATH)).expect("read rules");
    assert!(parse_rules(&content).is_ok(), "real rules must parse");
    let misspelled = content.replacen("transitive-forbidden =", "transitive-forbiden =", 1);
    assert_ne!(
        misspelled, content,
        "rules must contain transitive-forbidden"
    );
    assert!(
        parse_rules(&misspelled).is_err(),
        "a misspelled key must not silently drop a rule"
    );
    let extra = format!("{content}\n[surprise]\nkey = 1\n");
    assert!(parse_rules(&extra).is_err(), "unknown tables must fail");
}

fn without_element(items: &[toml::Value], position: usize) -> toml::Value {
    toml::Value::Array(
        items
            .iter()
            .enumerate()
            .filter(|(index, _)| *index != position)
            .map(|(_, item)| item.clone())
            .collect(),
    )
}

fn value_variants(value: &toml::Value, label: &str) -> Vec<(String, toml::Value)> {
    match value {
        toml::Value::Table(table) => table
            .iter()
            .flat_map(|(key, child)| {
                value_variants(child, &format!("{label}.{key}"))
                    .into_iter()
                    .map(|(child_label, mutated)| {
                        let mut copy = table.clone();
                        copy.insert(key.clone(), mutated);
                        (child_label, toml::Value::Table(copy))
                    })
                    .collect::<Vec<_>>()
            })
            .collect(),
        toml::Value::Array(items) => items
            .iter()
            .enumerate()
            .flat_map(|(position, item)| {
                let removed = (
                    format!("{label}[{position}] removed"),
                    without_element(items, position),
                );
                let nested = value_variants(item, &format!("{label}[{position}]"))
                    .into_iter()
                    .map(|(item_label, mutated)| {
                        let mut copy = items.clone();
                        if let Some(slot) = copy.get_mut(position) {
                            *slot = mutated;
                        }
                        (item_label, toml::Value::Array(copy))
                    })
                    .collect::<Vec<_>>();
                std::iter::once(removed).chain(nested).collect::<Vec<_>>()
            })
            .collect(),
        toml::Value::String(_)
        | toml::Value::Integer(_)
        | toml::Value::Float(_)
        | toml::Value::Boolean(_)
        | toml::Value::Datetime(_) => Vec::new(),
    }
}

fn unnoticed_variant(variant: &toml::Value, cases: &Cases) -> bool {
    let table = variant.as_table().expect("rules document table");
    let text = toml::to_string(table).expect("serialize variant");
    parse_rules(&text).is_ok_and(|mutated| mismatches(&mutated, cases).is_empty())
}

#[test]
fn removing_any_rule_element_breaks_some_case() {
    let (rules, cases) = loaded_cases("arch-check-mutations");
    assert!(
        mismatches(&rules, &cases).is_empty(),
        "cases must pass before mutation"
    );
    let document = toml::Value::Table(
        fs::read_to_string(repository_root().join(RULES_PATH))
            .expect("read rules")
            .parse()
            .expect("parse rules document"),
    );
    let variants = value_variants(&document, "rules");
    assert!(variants.len() > 200, "too few variants: {}", variants.len());
    let workers = std::thread::available_parallelism().map_or(1, std::num::NonZero::get);
    let unnoticed: BTreeSet<String> = std::thread::scope(|scope| {
        let mut handles = Vec::new();
        for worker in 0..workers {
            let shared_variants = &variants;
            let shared_cases = &cases;
            handles.push(scope.spawn(move || {
                shared_variants
                    .iter()
                    .enumerate()
                    .filter(|(index, _)| index.checked_rem(workers) == Some(worker))
                    .filter(|(_, (_, variant))| unnoticed_variant(variant, shared_cases))
                    .map(|(_, (label, _))| label.clone())
                    .collect::<Vec<_>>()
            }));
        }
        handles
            .into_iter()
            .flat_map(|handle| handle.join().expect("variant worker"))
            .collect()
    });
    assert!(
        unnoticed.is_empty(),
        "rule elements without a case: {unnoticed:#?}"
    );
}

fn command_exit(case: &str, directory_name: &str) -> std::process::ExitCode {
    let (root, directory) = copied_harness(directory_name);
    let case_root = directory.join("cases").join(case);
    let rules = case_root.join(RULES_PATH);
    fs::create_dir_all(rules.parent().expect("rules directory")).expect("create rules directory");
    fs::copy(root.join(RULES_PATH), &rules).expect("copy rules");
    generate_lockfile(&case_root.join("Cargo.toml"));
    crate::arch_check::command(&case_root.join("Cargo.toml"))
}

#[test]
fn command_fails_on_violation_and_passes_clean_workspace() {
    assert_eq!(
        command_exit(
            "testkit-depends-on-order-execution",
            "arch-check-command-violation"
        ),
        std::process::ExitCode::FAILURE
    );
    assert_eq!(
        command_exit(
            "user-presence-impl-in-allowed-crates-pass",
            "arch-check-command-clean"
        ),
        std::process::ExitCode::SUCCESS
    );
}
