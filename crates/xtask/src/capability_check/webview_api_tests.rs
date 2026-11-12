use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use crate::harness_workspace_tests::{fresh_directory, repository_root};

use super::webview_api::{WEBVIEW_API_FILE, WebviewApis};
use super::{TAURI_DIRECTORY, check_webview_apis};

const CASES_FILE: &str = "harness/capability-check/webview-api/cases.toml";

#[derive(Debug)]
struct Case {
    name: String,
    files: Vec<(String, String)>,
    expect: BTreeSet<String>,
}

fn text(entry: &toml::Value, key: &str) -> String {
    entry
        .get(key)
        .and_then(toml::Value::as_str)
        .map(String::from)
        .expect("case text")
}

fn cases(root: &Path) -> Vec<Case> {
    let table: toml::Table = fs::read_to_string(root.join(CASES_FILE))
        .expect("read webview api cases")
        .parse()
        .expect("parse webview api cases");
    table
        .get("case")
        .and_then(toml::Value::as_array)
        .expect("case list")
        .iter()
        .map(|entry| Case {
            name: text(entry, "name"),
            files: entry
                .get("file")
                .and_then(toml::Value::as_array)
                .expect("case files")
                .iter()
                .map(|file| (text(file, "path"), text(file, "source")))
                .collect(),
            expect: entry
                .get("expect")
                .and_then(toml::Value::as_array)
                .expect("expect list")
                .iter()
                .map(|rule| rule.as_str().map(String::from).expect("expected finding"))
                .collect(),
        })
        .collect()
}

fn apis(root: &Path) -> WebviewApis {
    WebviewApis::load(root).expect("load webview apis")
}

fn reported(root: &Path, prefix: &str, case: &Case, table: &WebviewApis) -> BTreeSet<String> {
    let directory = fresh_directory(root, &format!("{prefix}/{}", case.name));
    for (path, source) in &case.files {
        let file = directory.join(path);
        fs::create_dir_all(file.parent().expect("parent")).expect("create folder");
        fs::write(file, source).expect("write case file");
    }
    check_webview_apis(&directory, table)
        .into_iter()
        .map(|finding| finding.to_string())
        .map(|line| line.replace(&format!("{TAURI_DIRECTORY}/"), ""))
        .collect()
}

#[test]
fn webview_api_cases_report_exactly_their_findings() {
    let root = repository_root();
    let table = apis(&root);
    let mismatches: Vec<String> = cases(&root)
        .iter()
        .filter_map(|case| {
            let found = reported(&root, "capability-webview-cases", case, &table);
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
fn repository_sources_open_no_webview_channel_outside_the_listed_files() {
    let root = repository_root();
    let found = check_webview_apis(&root.join(TAURI_DIRECTORY), &apis(&root));
    assert!(found.is_empty(), "{found:#?}");
}

#[test]
fn every_api_group_has_a_failing_case_and_the_list_holds_the_researched_names() {
    let root = repository_root();
    let table: toml::Table = fs::read_to_string(root.join(WEBVIEW_API_FILE))
        .expect("read api table")
        .parse()
        .expect("parse api table");
    let groups: Vec<Vec<String>> = table
        .get("api")
        .and_then(toml::Value::as_array)
        .expect("api groups")
        .iter()
        .map(|group| {
            group
                .get("names")
                .and_then(toml::Value::as_array)
                .expect("names")
                .iter()
                .map(|name| name.as_str().map(String::from).expect("name"))
                .collect()
        })
        .collect();
    let failing: BTreeSet<String> = cases(&root)
        .into_iter()
        .flat_map(|case| case.expect)
        .filter_map(|finding| finding.rsplit(':').next().map(String::from))
        .collect();
    let uncovered: Vec<&Vec<String>> = groups
        .iter()
        .filter(|names| !names.iter().any(|name| failing.contains(name)))
        .collect();
    assert!(
        uncovered.is_empty(),
        "groups without a failing case: {uncovered:?}"
    );
    let all: BTreeSet<&str> = groups.iter().flatten().map(String::as_str).collect();
    for required in [
        "generate_handler",
        "invoke_handler",
        "register_uri_scheme_protocol",
        "register_asynchronous_uri_scheme_protocol",
        "Channel",
        "plugin",
        "on_webview_event",
        "on_page_load",
        "eval",
        "WebviewWindowBuilder",
        "emit",
        "listen",
    ] {
        assert!(all.contains(required), "{required} is not listed");
    }
}

fn groups_of(table: &toml::Table) -> Vec<toml::Value> {
    table
        .get("api")
        .and_then(toml::Value::as_array)
        .expect("api groups")
        .clone()
}

fn allowed_files_of(group: &toml::Value) -> Vec<toml::Value> {
    group
        .get("allowed-files")
        .and_then(toml::Value::as_array)
        .expect("allowed files")
        .clone()
}

fn allowed_file_positions(table: &toml::Table) -> Vec<(usize, usize)> {
    groups_of(table)
        .iter()
        .enumerate()
        .flat_map(|(group, entry)| {
            (0..allowed_files_of(entry).len()).map(move |index| (group, index))
        })
        .collect()
}

fn with_fewer_files(entry: toml::Value, index: usize) -> toml::Value {
    let fewer: Vec<toml::Value> = allowed_files_of(&entry)
        .into_iter()
        .enumerate()
        .filter(|&(listed, _)| listed != index)
        .map(|(_, file)| file)
        .collect();
    let mut changed = entry;
    changed
        .as_table_mut()
        .expect("group table")
        .insert(String::from("allowed-files"), toml::Value::Array(fewer));
    changed
}

fn without_allowed_file(table: &toml::Table, group: usize, index: usize) -> toml::Table {
    let groups: Vec<toml::Value> = groups_of(table)
        .into_iter()
        .enumerate()
        .map(|(position, entry)| {
            if position == group {
                with_fewer_files(entry, index)
            } else {
                entry
            }
        })
        .collect();
    let mut candidate = table.clone();
    candidate.insert(String::from("api"), toml::Value::Array(groups));
    candidate
}

#[test]
fn removing_any_allowed_file_breaks_some_case() {
    let root = repository_root();
    let table: toml::Table = fs::read_to_string(root.join(WEBVIEW_API_FILE))
        .expect("read api table")
        .parse()
        .expect("parse api table");
    let all_cases = cases(&root);
    let passes = |candidate: &toml::Table| {
        WebviewApis::parse(&toml::to_string(candidate).expect("render table")).is_ok_and(|parsed| {
            all_cases.iter().all(|case| {
                reported(&root, "capability-webview-variants", case, &parsed) == case.expect
            }) && check_webview_apis(&root.join(TAURI_DIRECTORY), &parsed).is_empty()
        })
    };
    assert!(passes(&table), "the unmodified table must pass");
    let survivors: Vec<String> = allowed_file_positions(&table)
        .into_iter()
        .filter(|&(group, index)| passes(&without_allowed_file(&table, group, index)))
        .map(|(group, index)| format!("api[{group}].allowed-files[{index}]"))
        .collect();
    assert!(
        survivors.is_empty(),
        "elements without a test: {survivors:?}"
    );
}
