use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use crate::harness_workspace_tests::{copy_tree, fresh_directory, repository_root};

use super::registration::EXPECTED_BUILD_SCRIPT_FILE;
use super::{AllowedPermissions, Rule, TAURI_DIRECTORY, check_registered_tauri_directory};

const CASES_FILE: &str = "harness/capability-check/registration/cases.toml";
const BASE_DIRECTORY: &str = "harness/capability-check/registration/base";
const OVERLAYS_DIRECTORY: &str = "harness/capability-check/registration/overlays";

const REGISTRATION_RULES: [Rule; 4] = [
    Rule::RegistrationMismatch,
    Rule::RegistrationCount,
    Rule::HandWrittenHandler,
    Rule::BuildBody,
];

#[derive(Debug)]
struct Case {
    name: String,
    overlay: Option<String>,
    remove: Vec<String>,
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
        .expect("read registration cases")
        .parse()
        .expect("parse registration cases");
    table
        .get("case")
        .and_then(toml::Value::as_array)
        .expect("registration case list")
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
            expect: strings(entry, "expect").into_iter().collect(),
        })
        .collect()
}

fn expected_build_script(root: &Path) -> String {
    fs::read_to_string(root.join(EXPECTED_BUILD_SCRIPT_FILE)).expect("read expected build.rs")
}

fn reported(root: &Path, case: &Case) -> BTreeSet<String> {
    let directory = fresh_directory(root, &format!("capability-registration/{}", case.name));
    copy_tree(&root.join(BASE_DIRECTORY), &directory);
    if let Some(overlay) = &case.overlay {
        copy_tree(&root.join(OVERLAYS_DIRECTORY).join(overlay), &directory);
    }
    for removed in &case.remove {
        fs::remove_file(directory.join(removed)).expect("remove fixture file");
    }
    let allowed = AllowedPermissions::load(root).expect("load allowed permissions");
    check_registered_tauri_directory(&directory, &allowed, &expected_build_script(root))
        .expect("check fixture")
        .into_iter()
        .map(|finding| finding.rule.to_string())
        .collect()
}

#[test]
fn registration_cases_report_exactly_their_rules() {
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
fn every_registration_overlay_is_listed() {
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
fn every_registration_rule_has_a_violation_example() {
    let root = repository_root();
    let exemplified: BTreeSet<String> = cases(&root)
        .into_iter()
        .flat_map(|case| case.expect)
        .collect();
    let missing: Vec<String> = REGISTRATION_RULES
        .iter()
        .map(ToString::to_string)
        .filter(|rule| !exemplified.contains(rule))
        .collect();
    assert!(missing.is_empty(), "rules without example: {missing:?}");
}

#[test]
fn repository_build_script_matches_the_expected_body() {
    let root = repository_root();
    let actual =
        fs::read_to_string(root.join(TAURI_DIRECTORY).join("build.rs")).expect("read build.rs");
    assert_eq!(actual, expected_build_script(&root), "build.rs differs");
}
