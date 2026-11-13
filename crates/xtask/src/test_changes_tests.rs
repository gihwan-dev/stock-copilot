use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::check_support::read_text;
use crate::git_base::git;
use crate::harness_workspace_tests::{
    copy_tree, fresh_directory, generate_lockfile, place_git_directory_outside, repository_root,
};
use crate::test_diff::commit_changes;

const REPOSITORY_EXAMPLE: &str = "harness/test-diff/repository";
const EXTRA_TESTS: &str = "crates/sample/src/extra_tests.rs";
const WEAKENED_FROM: &str = "assert_eq!(super::add(4, 0), 4, \"zero\");";
const WEAKENED_TO: &str = "assert!(true, \"zero\");";

fn run_git(directory: &Path, arguments: &[&str]) -> String {
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
    String::from_utf8_lossy(&finished.stdout).trim().to_owned()
}

fn commit_all(directory: &Path, message: &str) -> String {
    run_git(directory, &["add", "--all"]);
    run_git(
        directory,
        &["commit", "--quiet", "--allow-empty", "-m", message],
    );
    run_git(directory, &["rev-parse", "HEAD"])
}

fn repository(name: &str) -> (PathBuf, String) {
    let root = repository_root();
    let directory = fresh_directory(&root, name);
    copy_tree(&root.join(REPOSITORY_EXAMPLE), &directory);
    generate_lockfile(&directory.join("Cargo.toml"));
    fs::write(directory.join(".gitignore"), "/target/\n").expect("write gitignore");
    run_git(&directory, &["init", "--quiet"]);
    let base = commit_all(&directory, "base");
    (directory, base)
}

fn edit(directory: &Path, path: &str, from: &str, to: &str) {
    let file = directory.join(path);
    let text = read_text(&file).expect("read example file");
    assert!(text.contains(from), "{path} lacks {from:?}");
    fs::write(&file, text.replacen(from, to, 1)).expect("write example file");
}

fn shown(directory: &Path, base: &str, parent: &str, commit: &str) -> BTreeSet<String> {
    commit_changes(directory, base, parent, commit)
        .expect("test changes")
        .iter()
        .map(|finding| finding.rule.to_string())
        .collect()
}

#[test]
fn a_commit_weakening_a_base_test_is_reported_against_its_parent() {
    let (directory, base) = repository("test-changes-weakened");
    edit(&directory, EXTRA_TESTS, WEAKENED_FROM, WEAKENED_TO);
    let weakened = commit_all(&directory, "weaken");
    edit(&directory, EXTRA_TESTS, WEAKENED_TO, WEAKENED_FROM);
    let restored = commit_all(&directory, "restore");
    assert_eq!(
        shown(&directory, &base, &base, &weakened),
        BTreeSet::from([String::from(
            "test-diff:modified:sample extra_tests::adds_zero"
        )])
    );
    assert_eq!(
        shown(&directory, &base, &weakened, &restored),
        BTreeSet::from([String::from(
            "test-diff:modified:sample extra_tests::adds_zero"
        )])
    );
    assert!(shown(&directory, &base, &base, &restored).is_empty());
}

#[test]
fn tests_added_after_the_base_may_change_without_a_report() {
    let (directory, base) = repository("test-changes-own-tests");
    edit(
        &directory,
        EXTRA_TESTS,
        "#[test]\nfn adds_zero() {",
        "#[test]\nfn adds_new() {\n    assert_eq!(super::add(1, 1), 2, \"new\");\n}\n\n#[test]\nfn adds_zero() {",
    );
    let added = commit_all(&directory, "add");
    edit(
        &directory,
        EXTRA_TESTS,
        "assert_eq!(super::add(1, 1), 2, \"new\");",
        "assert!(true, \"new\");",
    );
    let changed = commit_all(&directory, "change own test");
    assert!(shown(&directory, &base, &base, &added).is_empty());
    assert!(shown(&directory, &base, &added, &changed).is_empty());
    assert_eq!(
        shown(&directory, &added, &added, &changed),
        BTreeSet::from([String::from(
            "test-diff:modified:sample extra_tests::adds_new"
        )])
    );
}

#[test]
fn removing_a_base_test_file_reports_removed_and_unreachable_tests() {
    let (directory, base) = repository("test-changes-removed");
    edit(
        &directory,
        "crates/sample/src/lib.rs",
        "#[cfg(test)]\nmod extra_tests;\n",
        "",
    );
    let dropped = commit_all(&directory, "drop");
    assert_eq!(
        shown(&directory, &base, &base, &dropped),
        BTreeSet::from([
            String::from("test-diff:removed:sample extra_tests::adds_large"),
            String::from("test-diff:removed:sample extra_tests::adds_zero"),
            String::from("test-diff:unreachable:crates/sample/src/extra_tests.rs"),
        ])
    );
}

#[test]
fn test_changes_accept_only_full_commit_ids() {
    let (directory, base) = repository("test-changes-names");
    for (first, second, third) in [
        ("HEAD", base.as_str(), base.as_str()),
        (base.as_str(), "main", base.as_str()),
        (base.as_str(), base.as_str(), ""),
    ] {
        assert!(
            commit_changes(&directory, first, second, third).is_err(),
            "{first} {second} {third}"
        );
    }
}
