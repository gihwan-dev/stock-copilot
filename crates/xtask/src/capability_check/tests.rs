use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use crate::harness_workspace_tests::{copy_tree, fresh_directory, repository_root};

use super::{
    ALLOWED_PERMISSIONS_FILE, AllowedPermissions, FORBIDDEN_PERMISSIONS, Rule, TAURI_DIRECTORY,
    check_tauri_directory,
};

const CASES_FILE: &str = "harness/capability-check/cases.toml";
const BASE_DIRECTORY: &str = "harness/capability-check/base";
const OVERLAYS_DIRECTORY: &str = "harness/capability-check/overlays";

const UNIT_RULES: [Rule; 18] = [
    Rule::NotAllowed,
    Rule::NotExplicit,
    Rule::ListNotMain,
    Rule::MainMissing,
    Rule::ExtraCapabilityFile,
    Rule::Identifier,
    Rule::Windows,
    Rule::Remote,
    Rule::HandWrittenCommand,
    Rule::CommandSetNotReferenced,
    Rule::CommandSetShape,
    Rule::CommandMismatch,
    Rule::CommandListFormat,
    Rule::ExtraPermissionFile,
    Rule::BuildHandWrittenCommands,
    Rule::BuildNotReadingList,
    Rule::ExtraConfig,
    Rule::Unparsable,
];

const ARCHITECTURE_FORBIDDEN: [&str; 10] = [
    "core:default",
    "core:event:allow-emit",
    "shell:*",
    "dialog:*",
    "opener:*",
    "fs:*",
    "http:*",
    "biometric:*",
    "notification:*",
    "autostart:*",
];

const REQUIRED_CLEAN_CASES: [&str; 3] = [
    "base",
    "clean-with-commands",
    "clean-narrow-core-permissions",
];

#[derive(Debug)]
struct Case {
    name: String,
    overlay: Option<String>,
    remove: Vec<String>,
    allowed: Option<Vec<String>>,
    expect: BTreeSet<String>,
}

fn strings(entry: &toml::Value, key: &str) -> Vec<String> {
    entry
        .get(key)
        .and_then(toml::Value::as_array)
        .map_or_else(Vec::new, |list| {
            list.iter()
                .map(|item| item.as_str().map(String::from).expect("string item"))
                .collect()
        })
}

fn cases(root: &Path) -> Vec<Case> {
    let table: toml::Table = fs::read_to_string(root.join(CASES_FILE))
        .expect("read capability cases")
        .parse()
        .expect("parse capability cases");
    table
        .get("case")
        .and_then(toml::Value::as_array)
        .expect("capability case list")
        .iter()
        .map(|entry| Case {
            name: entry
                .get("name")
                .and_then(toml::Value::as_str)
                .map(String::from)
                .expect("case name"),
            overlay: entry
                .get("overlay")
                .and_then(toml::Value::as_str)
                .map(String::from),
            remove: strings(entry, "remove"),
            allowed: entry.get("allowed").map(|_| strings(entry, "allowed")),
            expect: strings(entry, "expect").into_iter().collect(),
        })
        .collect()
}

fn reported(root: &Path, case: &Case) -> BTreeSet<String> {
    let directory = fresh_directory(root, &format!("capability-check/{}", case.name));
    copy_tree(&root.join(BASE_DIRECTORY), &directory);
    if let Some(overlay) = &case.overlay {
        copy_tree(&root.join(OVERLAYS_DIRECTORY).join(overlay), &directory);
    }
    for removed in &case.remove {
        fs::remove_file(directory.join(removed)).expect("remove fixture file");
    }
    let allowed = case.allowed.as_ref().map_or_else(
        || AllowedPermissions::load(root).expect("load allowed permissions"),
        |listed| AllowedPermissions::new(ALLOWED_PERMISSIONS_FILE, listed.iter().cloned()),
    );
    check_tauri_directory(&directory, &allowed)
        .expect("check fixture")
        .into_iter()
        .map(|finding| finding.rule.to_string())
        .collect()
}

#[test]
fn capability_cases_report_exactly_their_rules() {
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
fn every_overlay_is_listed() {
    let root = repository_root();
    let used: BTreeSet<String> = cases(&root)
        .into_iter()
        .filter_map(|case| case.overlay)
        .collect();
    let present: BTreeSet<String> = fs::read_dir(root.join(OVERLAYS_DIRECTORY))
        .expect("read overlays")
        .map(|entry| {
            entry
                .expect("overlay entry")
                .file_name()
                .to_str()
                .map(String::from)
                .expect("utf-8 name")
        })
        .collect();
    assert_eq!(used, present);
}

#[test]
fn every_capability_rule_has_a_violation_example() {
    let root = repository_root();
    let exemplified: BTreeSet<String> = cases(&root)
        .into_iter()
        .flat_map(|case| case.expect)
        .collect();
    let mut catalogue: Vec<Rule> = UNIT_RULES.to_vec();
    catalogue.extend(FORBIDDEN_PERMISSIONS.map(|pattern| Rule::Forbidden(String::from(pattern))));
    let missing: Vec<String> = catalogue
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
fn forbidden_permissions_cover_the_architecture_list() {
    let missing: Vec<&str> = ARCHITECTURE_FORBIDDEN
        .into_iter()
        .filter(|pattern| !FORBIDDEN_PERMISSIONS.contains(pattern))
        .collect();
    assert!(
        missing.is_empty(),
        "architecture 3.6 permissions not forbidden: {missing:?}"
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
fn repository_passes_capability_check() {
    let root = repository_root();
    let allowed = AllowedPermissions::load(&root).expect("load allowed permissions");
    let findings =
        check_tauri_directory(&root.join(TAURI_DIRECTORY), &allowed).expect("check repository");
    let lines: Vec<String> = findings.iter().map(ToString::to_string).collect();
    assert!(lines.is_empty(), "{lines:#?}");
}

#[test]
fn json_reader_accepts_json_and_rejects_the_rest() {
    let parsed = super::json::parse(
        "{ \"a\": [1, -2.5e3, true, false, null], \"b\": \"x\\u00e9\\ud83d\\ude00\\n\" }",
    )
    .expect("valid json");
    assert_eq!(
        parsed.get("b").and_then(super::json::Json::as_str),
        Some("x\u{e9}\u{1f600}\n")
    );
    assert_eq!(
        parsed
            .get("a")
            .and_then(super::json::Json::as_array)
            .map(<[_]>::len),
        Some(5)
    );
    for invalid in [
        "{\"a\": 1,}",
        "[1, 2,]",
        "{\"a\": 1, \"a\": 2}",
        "{\"a\": 1} trailing",
        "{'a': 1}",
        "\"line\nbreak\"",
        "{\"a\": tru}",
        "\"\\ud83d\"",
        "",
    ] {
        assert_eq!(super::json::parse(invalid), None, "{invalid:?}");
    }
}

fn probe_output(justfile: &Path) -> String {
    let output = std::process::Command::new("just")
        .arg("--justfile")
        .arg(justfile)
        .arg("probe")
        .env("TAURI_CONFIG", HOSTILE_TAURI_CONFIG)
        .output()
        .expect("run just");
    assert!(output.status.success(), "just probe failed");
    String::from_utf8_lossy(&output.stdout).into_owned()
}

const HOSTILE_TAURI_CONFIG: &str = "{\"app\":{\"security\":{\"capabilities\":[]}}}";

const PROBE_RECIPE: &str = "probe:\n    printf '%s' \"${TAURI_CONFIG:-unset}\"\n";

#[test]
fn tauri_config_is_neutralized() {
    let root = repository_root();
    let directory = fresh_directory(&root, "capability-check-tauri-config");
    let fixed = directory.join("fixed.just");
    fs::write(
        &fixed,
        format!(
            "import '{}'\n\n{PROBE_RECIPE}",
            root.join("justfile").display()
        ),
    )
    .expect("write importing justfile");
    let plain = directory.join("plain.just");
    fs::write(&plain, PROBE_RECIPE).expect("write plain justfile");
    assert_eq!(
        probe_output(&fixed),
        "unset",
        "TAURI_CONFIG reached just recipes"
    );
    assert_eq!(
        probe_output(&plain),
        HOSTILE_TAURI_CONFIG,
        "TAURI_CONFIG did not reach a recipe even without the justfile"
    );
}
