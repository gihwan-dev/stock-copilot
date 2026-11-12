use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use crate::harness_workspace_tests::{
    copy_tree, fresh_directory, generate_lockfile, repository_root,
};

use super::allowlist::Allowlists;
use super::check_repository;
use super::finding::Rule;
use super::globs::glob_import_lines;

const CASES_FILE: &str = "harness/source-checks/glob-cases.toml";
const SOURCES_DIRECTORY: &str = "harness/source-checks/glob-sources";
const BASE_WORKSPACE: &str = "harness/source-checks/workspaces/base";
const GLOB_WORKSPACE: &str = "harness/source-checks/glob-workspace";

fn cases(root: &Path) -> Vec<(String, Vec<usize>)> {
    let table: toml::Table = fs::read_to_string(root.join(CASES_FILE))
        .expect("read glob cases")
        .parse()
        .expect("parse glob cases");
    table
        .get("case")
        .and_then(toml::Value::as_array)
        .expect("glob case list")
        .iter()
        .map(|entry| {
            let example = entry
                .get("example")
                .and_then(toml::Value::as_str)
                .map(String::from)
                .expect("example name");
            let lines = entry
                .get("lines")
                .and_then(toml::Value::as_array)
                .expect("line list")
                .iter()
                .map(|line| {
                    line.as_integer()
                        .and_then(|number| usize::try_from(number).ok())
                        .expect("line number")
                })
                .collect();
            (example, lines)
        })
        .collect()
}

#[test]
fn glob_examples_report_exactly_their_lines() {
    let root = repository_root();
    let mismatches: Vec<String> = cases(&root)
        .into_iter()
        .filter_map(|(example, expected)| {
            let source = fs::read_to_string(root.join(SOURCES_DIRECTORY).join(&example))
                .expect("read example");
            let found = glob_import_lines(&source);
            (found != expected)
                .then(|| format!("{example}: expected {expected:?}, found {found:?}"))
        })
        .collect();
    assert!(mismatches.is_empty(), "{mismatches:#?}");
}

#[test]
fn every_glob_example_is_listed_with_clean_and_failing_cases() {
    let root = repository_root();
    let listed: BTreeSet<String> = cases(&root)
        .into_iter()
        .map(|(example, _)| example)
        .collect();
    let present: BTreeSet<String> = fs::read_dir(root.join(SOURCES_DIRECTORY))
        .expect("read glob examples")
        .map(|entry| {
            entry
                .expect("example entry")
                .file_name()
                .to_str()
                .map(String::from)
                .expect("utf-8 name")
        })
        .collect();
    assert_eq!(
        listed, present,
        "glob examples on disk and in {CASES_FILE} differ"
    );
    let all = cases(&root);
    assert!(
        all.iter().any(|(_, lines)| lines.is_empty()),
        "no clean glob example"
    );
    assert!(
        all.iter().any(|(_, lines)| !lines.is_empty()),
        "no failing glob example"
    );
}

#[test]
fn source_check_reports_glob_imports_in_workspace_members() {
    let root = repository_root();
    let directory = fresh_directory(&root, "source-checks-glob");
    copy_tree(&root.join(BASE_WORKSPACE), &directory);
    copy_tree(&root.join(GLOB_WORKSPACE), &directory);
    generate_lockfile(&directory.join("Cargo.toml"));
    let allowlists = Allowlists::load(&root).expect("load allowlists");
    let reported: BTreeSet<String> = check_repository(&directory, &allowlists)
        .expect("check workspace")
        .into_iter()
        .map(|finding| finding.rule.to_string())
        .collect();
    assert_eq!(reported, BTreeSet::from([Rule::GlobImport.to_string()]));
}
