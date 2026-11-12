use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::check_support::{Finding, read_text};
use crate::git_base::{HEAD, git};
use crate::harness_workspace_tests::{
    copy_tree, fresh_directory, generate_lockfile, place_git_directory_outside, repository_root,
};

use super::{Rule, Sources, check, compare_lists, compare_sources};

const CASES_FILE: &str = "harness/test-diff/cases.toml";
const REPOSITORY_EXAMPLE: &str = "harness/test-diff/repository";
const LIBRARY: &str = "crates/sample/src/lib.rs";
const EXTRA_TESTS: &str = "crates/sample/src/extra_tests.rs";
const EXTRA_DECLARATION: &str = "#[cfg(test)]\nmod extra_tests;\n";

fn cases_table(root: &Path) -> toml::Table {
    read_text(&root.join(CASES_FILE))
        .expect("read test-diff cases")
        .parse()
        .expect("parse test-diff cases")
}

fn text_list(entry: &toml::Value, key: &str) -> BTreeSet<String> {
    entry
        .get(key)
        .and_then(toml::Value::as_array)
        .expect("text list")
        .iter()
        .map(|item| item.as_str().map(String::from).expect("text item"))
        .collect()
}

fn source_map(entry: &toml::Value, key: &str) -> Sources {
    entry
        .get(key)
        .and_then(toml::Value::as_table)
        .expect("source table")
        .iter()
        .filter(|(path, _)| super::in_scope(path))
        .map(|(path, text)| {
            (
                path.clone(),
                text.as_str().map(String::from).expect("source text"),
            )
        })
        .collect()
}

fn shown(findings: &BTreeSet<Finding<Rule>>) -> BTreeSet<String> {
    findings
        .iter()
        .map(|finding| finding.rule.to_string())
        .collect()
}

fn entries<'table>(table: &'table toml::Table, key: &str) -> &'table [toml::Value] {
    table
        .get(key)
        .and_then(toml::Value::as_array)
        .map(Vec::as_slice)
        .expect("case list")
}

fn name_of(entry: &toml::Value) -> String {
    entry
        .get("name")
        .and_then(toml::Value::as_str)
        .map(String::from)
        .expect("case name")
}

#[test]
fn source_cases_report_exactly_their_rules() {
    let table = cases_table(&repository_root());
    let mismatches: Vec<String> = entries(&table, "case")
        .iter()
        .filter_map(|entry| {
            let reported = shown(&compare_sources(
                &source_map(entry, "base"),
                &source_map(entry, "current"),
            ));
            let expected = text_list(entry, "expect");
            (reported != expected).then(|| {
                format!(
                    "{}: expected {expected:?}, reported {reported:?}",
                    name_of(entry)
                )
            })
        })
        .collect();
    assert!(mismatches.is_empty(), "{mismatches:#?}");
}

#[test]
fn list_cases_report_exactly_their_rules() {
    let table = cases_table(&repository_root());
    let mismatches: Vec<String> = entries(&table, "list-case")
        .iter()
        .filter_map(|entry| {
            let reported = shown(&compare_lists(
                &text_list(entry, "base"),
                &text_list(entry, "current"),
            ));
            let expected = text_list(entry, "expect");
            (reported != expected).then(|| {
                format!(
                    "{}: expected {expected:?}, reported {reported:?}",
                    name_of(entry)
                )
            })
        })
        .collect();
    assert!(mismatches.is_empty(), "{mismatches:#?}");
}

#[test]
fn every_test_diff_rule_has_an_example() {
    let table = cases_table(&repository_root());
    let reported: Vec<String> = entries(&table, "case")
        .iter()
        .chain(entries(&table, "list-case"))
        .flat_map(|entry| text_list(entry, "expect"))
        .collect();
    let missing: Vec<&str> = [
        "test-diff:removed:",
        "test-diff:modified:",
        "test-diff:not-listed:",
        "test-diff:count-decreased:",
    ]
    .into_iter()
    .filter(|prefix| !reported.iter().any(|line| line.starts_with(prefix)))
    .collect();
    assert!(missing.is_empty(), "rules without an example: {missing:?}");
    let prefixes: BTreeSet<String> = [
        Rule::Removed(String::new()),
        Rule::Modified(String::new()),
        Rule::NotListed(String::new()),
    ]
    .iter()
    .map(ToString::to_string)
    .collect();
    assert_eq!(
        prefixes,
        BTreeSet::from([
            String::from("test-diff:modified:"),
            String::from("test-diff:not-listed:"),
            String::from("test-diff:removed:"),
        ])
    );
}

fn run_git(directory: &Path, arguments: &[&str]) {
    place_git_directory_outside(directory, arguments);
    let mut command = git(directory);
    command.args([
        "-c",
        "user.name=harness",
        "-c",
        "user.email=harness@example.invalid",
        "-c",
        "commit.gpgsign=false",
    ]);
    command.args(arguments);
    let finished = command.output().expect("run git");
    assert!(
        finished.status.success(),
        "git {arguments:?}: {}",
        String::from_utf8_lossy(&finished.stderr)
    );
}

fn committed_repository(name: &str) -> (PathBuf, String) {
    let root = repository_root();
    let directory = fresh_directory(&root, name);
    copy_tree(&root.join(REPOSITORY_EXAMPLE), &directory);
    generate_lockfile(&directory.join("Cargo.toml"));
    fs::write(directory.join(".gitignore"), "/target/\n").expect("write gitignore");
    run_git(&directory, &["init", "--quiet"]);
    run_git(&directory, &["add", "--all"]);
    run_git(&directory, &["commit", "--quiet", "-m", "base"]);
    let output = Command::new("git")
        .arg("-C")
        .arg(&directory)
        .args(["rev-parse", "HEAD"])
        .output()
        .expect("rev-parse");
    let commit = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    (directory, commit)
}

fn edit(directory: &Path, path: &str, from: &str, to: &str) {
    let file = directory.join(path);
    let text = read_text(&file).expect("read example file");
    assert!(text.contains(from), "{path} lacks {from:?}");
    fs::write(&file, text.replacen(from, to, 1)).expect("write example file");
}

#[test]
fn unchanged_repository_passes_against_head_and_commit() {
    let (directory, commit) = committed_repository("test-diff-unchanged");
    for base in [HEAD, commit.as_str()] {
        let findings = check(&directory, base).expect("check");
        assert!(findings.is_empty(), "{base}: {findings:?}");
    }
}

#[test]
fn dropping_a_test_module_declaration_fails_through_the_test_list() {
    let (directory, commit) = committed_repository("test-diff-dropped-module");
    edit(&directory, LIBRARY, EXTRA_DECLARATION, "");
    let findings = check(&directory, &commit).expect("check");
    assert_eq!(
        shown(&findings),
        BTreeSet::from([
            String::from("test-diff:not-listed:sample extra_tests::adds_large"),
            String::from("test-diff:not-listed:sample extra_tests::adds_zero"),
            String::from("test-diff:count-decreased:3->1"),
            String::from("test-diff:removed:sample extra_tests::adds_large"),
            String::from("test-diff:removed:sample extra_tests::adds_zero"),
            String::from("test-diff:unreachable:crates/sample/src/extra_tests.rs"),
        ])
    );
}

#[test]
fn replacing_a_disabled_test_with_a_new_one_still_fails() {
    let (directory, commit) = committed_repository("test-diff-replaced");
    edit(&directory, LIBRARY, EXTRA_DECLARATION, "");
    edit(
        &directory,
        LIBRARY,
        "    #[test]\n    fn adds() {",
        "    #[test]\n    fn trivial_one() {}\n\n    #[test]\n    fn trivial_two() {}\n\n    #[test]\n    fn adds() {",
    );
    let findings = check(&directory, &commit).expect("check");
    assert_eq!(
        shown(&findings),
        BTreeSet::from([
            String::from("test-diff:not-listed:sample extra_tests::adds_large"),
            String::from("test-diff:not-listed:sample extra_tests::adds_zero"),
            String::from("test-diff:removed:sample extra_tests::adds_large"),
            String::from("test-diff:removed:sample extra_tests::adds_zero"),
            String::from("test-diff:unreachable:crates/sample/src/extra_tests.rs"),
        ])
    );
}

#[test]
fn committed_weakening_fails_against_the_base_commit_but_not_against_head() {
    let (directory, commit) = committed_repository("test-diff-committed");
    edit(
        &directory,
        EXTRA_TESTS,
        "assert_eq!(super::add(4, 0), 4, \"zero\");",
        "assert!(true, \"zero\");",
    );
    run_git(&directory, &["commit", "--quiet", "--all", "-m", "weaken"]);
    run_git(&directory, &["branch", "--quiet", "main"]);
    assert!(check(&directory, HEAD).expect("check HEAD").is_empty());
    assert!(check(&directory, "").is_err(), "an empty base is rejected");
    assert!(
        check(&directory, "main").is_err(),
        "a branch name is rejected"
    );
    assert_eq!(
        shown(&check(&directory, &commit).expect("check base")),
        BTreeSet::from([String::from(
            "test-diff:modified:sample extra_tests::adds_zero"
        )])
    );
}

#[test]
fn base_must_be_head_or_a_full_commit_id() {
    let (directory, commit) = committed_repository("test-diff-base-names");
    run_git(&directory, &["branch", "--quiet", "main"]);
    run_git(&directory, &["tag", "base-tag"]);
    let short = commit.get(..7).expect("short id");
    for base in ["main", "base-tag", short, "HEAD~0", "@", ""] {
        let outcome = check(&directory, base);
        assert!(outcome.is_err(), "{base:?} accepted: {outcome:?}");
    }
    let unknown = "0".repeat(40);
    assert!(check(&directory, &unknown).is_err(), "unknown commit");
}

#[test]
fn replace_refs_do_not_change_the_base() {
    let (directory, commit) = committed_repository("test-diff-replace-refs");
    edit(
        &directory,
        EXTRA_TESTS,
        "assert_eq!(super::add(4, 0), 4, \"zero\");",
        "assert!(true, \"zero\");",
    );
    run_git(&directory, &["commit", "--quiet", "--all", "-m", "weaken"]);
    let weakened = String::from_utf8_lossy(
        &Command::new("git")
            .arg("-C")
            .arg(&directory)
            .args(["rev-parse", "HEAD"])
            .output()
            .expect("rev-parse")
            .stdout,
    )
    .trim()
    .to_owned();
    run_git(&directory, &["replace", &commit, &weakened]);
    assert_eq!(
        shown(&check(&directory, &commit).expect("check base")),
        BTreeSet::from([String::from(
            "test-diff:modified:sample extra_tests::adds_zero"
        )])
    );
}

#[test]
fn base_must_be_an_ancestor_of_head() {
    let (directory, commit) = committed_repository("test-diff-ancestor");
    run_git(&directory, &["switch", "--quiet", "--orphan", "unrelated"]);
    run_git(
        &directory,
        &["commit", "--quiet", "--allow-empty", "-m", "unrelated"],
    );
    let outcome = check(&directory, &commit);
    assert!(
        outcome
            .as_ref()
            .is_err_and(|failure| failure.contains("조상")),
        "{outcome:?}"
    );
}

#[test]
fn inline_module_replacing_a_file_module_leaves_the_file_unreachable() {
    let (directory, commit) = committed_repository("test-diff-inline-replacement");
    edit(
        &directory,
        LIBRARY,
        EXTRA_DECLARATION,
        "#[cfg(test)]\nmod extra_tests {\n    #[test]\n    fn adds_zero() {\n        assert!(true, \"zero\");\n    }\n\n    #[test]\n    fn adds_large() {\n        assert!(true, \"large\");\n    }\n}\n",
    );
    assert_eq!(
        shown(&check(&directory, &commit).expect("check")),
        BTreeSet::from([
            String::from("test-diff:modified:sample extra_tests::adds_large"),
            String::from("test-diff:modified:sample extra_tests::adds_zero"),
            String::from("test-diff:unreachable:crates/sample/src/extra_tests.rs"),
        ])
    );
}

#[test]
fn stale_build_directories_do_not_affect_the_verdict() {
    let (directory, commit) = committed_repository("test-diff-stale-cache");
    edit(&directory, LIBRARY, EXTRA_DECLARATION, "");
    let expected = shown(&check(&directory, &commit).expect("check"));
    let poisoned = directory.join("target/test-diff/tree");
    fs::create_dir_all(&poisoned).expect("create stale tree");
    fs::write(poisoned.join("Cargo.toml"), "[workspace]\nmembers = []\n")
        .expect("write stale manifest");
    assert_eq!(shown(&check(&directory, &commit).expect("check")), expected);
    assert!(
        expected.contains("test-diff:removed:sample extra_tests::adds_zero"),
        "{expected:?}"
    );
}
