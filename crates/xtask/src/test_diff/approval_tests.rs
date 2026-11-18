use std::collections::BTreeMap;
use std::fmt::Write;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use sha2::{Digest, Sha256};

use crate::harness_workspace_tests::{fresh_directory, repository_root};

use super::{Files, approval, compare_files};

const SOURCE: &str = "crates/sample/src/lib.rs";
const RECORD: &str = "harness/test-diff/approvals.toml";
const BEFORE: &str = r#"
#[cfg(test)] mod tests {
    #[test] fn first() { let _fixture = "old"; assert_eq!(1, 1); }
    #[test] fn second() { let _fixture = "old"; assert_eq!(2, 2); }
}
"#;
const AFTER_ONE: &str = r#"
#[cfg(test)] mod tests {
    #[test] fn first() { let _fixture = "new"; assert_eq!(1, 1); }
    #[test] fn second() { let _fixture = "old"; assert_eq!(2, 2); }
}
"#;
const AFTER_TWO: &str = r#"
#[cfg(test)] mod tests {
    #[test] fn first() { let _fixture = "new"; assert_eq!(1, 1); }
    #[test] fn second() { let _fixture = "new"; assert_eq!(2, 2); }
}
"#;
const ASSERTION_DELETED: &str = r#"
#[cfg(test)] mod tests {
    #[test] fn first() { let _fixture = "new"; }
    #[test] fn second() { let _fixture = "old"; assert_eq!(2, 2); }
}
"#;

fn files(source: &str) -> Files {
    BTreeMap::from([
        (
            String::from("Cargo.toml"),
            b"[workspace]\nmembers = [\"crates/sample\"]\n".to_vec(),
        ),
        (
            String::from("crates/sample/Cargo.toml"),
            b"[package]\nname = \"sample\"\n".to_vec(),
        ),
        (String::from(SOURCE), source.as_bytes().to_vec()),
    ])
}

fn digest(text: &str) -> String {
    format!("{:x}", Sha256::digest(text.as_bytes()))
}

fn record(after: &str, findings: &[&str]) -> String {
    record_for(SOURCE, &digest(BEFORE), &digest(after), findings)
}

fn record_for(path: &str, before: &str, after: &str, findings: &[&str]) -> String {
    let mut output = String::new();
    for finding in findings {
        write!(output, "[[approval]]\ntask = \"H-49\"\npath = \"{path}\"\nbefore_sha256 = \"{before}\"\nafter_sha256 = \"{after}\"\nfinding = \"{finding}\"\ndecision = \"approved\"\nreason = \"fixture preparation reviewed by the main agent\"\n\n").expect("write record");
    }
    output
}

fn git(root: &Path, arguments: &[&str]) -> String {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(arguments)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_AUTHOR_NAME", "Fixture")
        .env("GIT_AUTHOR_EMAIL", "fixture@example.invalid")
        .env("GIT_COMMITTER_NAME", "Fixture")
        .env("GIT_COMMITTER_EMAIL", "fixture@example.invalid")
        .output()
        .expect("run fixture git");
    assert!(
        output.status.success(),
        "git {arguments:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .expect("git output")
        .trim()
        .to_owned()
}

fn write(root: &Path, relative: &str, contents: &str) {
    let path = root.join(relative);
    fs::create_dir_all(path.parent().expect("parent")).expect("directory");
    fs::write(path, contents).expect("write fixture");
}

fn repository(name: &str) -> (PathBuf, String) {
    let root = fresh_directory(&repository_root(), name);
    git(&root, &["init", "--quiet", "-b", "master"]);
    write(&root, SOURCE, BEFORE);
    git(&root, &["add", SOURCE]);
    git(&root, &["commit", "--quiet", "-m", "base"]);
    let base = git(&root, &["rev-parse", "HEAD"]);
    (root, base)
}

fn commit_record(root: &Path, value: &str) -> String {
    write(root, RECORD, value);
    git(root, &["add", RECORD]);
    git(root, &["commit", "--quiet", "-m", "main approval"]);
    git(root, &["rev-parse", "HEAD"])
}

fn commit_change(root: &Path, after: &str) -> String {
    write(root, SOURCE, after);
    git(root, &["add", SOURCE]);
    git(root, &["commit", "--quiet", "-m", "change"]);
    git(root, &["rev-parse", "HEAD"])
}

fn remaining(root: &Path, authority: &str, head: &str, after: &str) -> Vec<String> {
    let before = files(BEFORE);
    let current = files(after);
    let (findings, _) = compare_files(&before, &current);
    approval::apply_with_authority(root, authority, head, (&before, &current), findings)
        .expect("approval evaluation")
        .into_iter()
        .map(|finding| finding.rule.to_string())
        .collect()
}

#[test]
fn owner_approved_exact_fixture_change_passes() {
    let (root, _) = repository("h52-approval-exact");
    let authority = commit_record(
        &root,
        &record(AFTER_ONE, &["test-diff:modified:sample tests::first"]),
    );
    git(
        &root,
        &["fetch", "--quiet", ".", "master:refs/remotes/origin/master"],
    );
    git(&root, &["checkout", "--quiet", "-b", "feature"]);
    let head = commit_change(&root, AFTER_ONE);
    assert_eq!(
        remaining(&root, &authority, &head, AFTER_ONE),
        Vec::<String>::new()
    );
    let before = files(BEFORE);
    let current = files(AFTER_ONE);
    let (findings, _) = compare_files(&before, &current);
    assert!(
        approval::apply(
            &root,
            &before,
            &current,
            findings,
            crate::git_base::BaseUse::Ancestor
        )
        .expect("production approval")
        .is_empty()
    );
}

#[test]
fn branch_written_approval_is_not_an_authority() {
    let (root, authority) = repository("h52-approval-branch");
    git(
        &root,
        &["fetch", "--quiet", ".", "master:refs/remotes/origin/master"],
    );
    git(&root, &["checkout", "--quiet", "-b", "feature"]);
    write(
        &root,
        RECORD,
        &record(AFTER_ONE, &["test-diff:modified:sample tests::first"]),
    );
    git(&root, &["add", RECORD]);
    let head = commit_change(&root, AFTER_ONE);
    assert_eq!(
        remaining(&root, &authority, &head, AFTER_ONE),
        ["test-diff:modified:sample tests::first"]
    );
    let before = files(BEFORE);
    let current = files(AFTER_ONE);
    let (findings, _) = compare_files(&before, &current);
    assert_eq!(
        approval::apply(
            &root,
            &before,
            &current,
            findings,
            crate::git_base::BaseUse::Ancestor
        )
        .expect("production branch check")
        .len(),
        1
    );
}

#[test]
fn a_master_commit_cannot_approve_its_own_test_changes() {
    let (root, _) = repository("h52-approval-master-self");
    write(
        &root,
        RECORD,
        &record(AFTER_ONE, &["test-diff:modified:sample tests::first"]),
    );
    write(&root, SOURCE, AFTER_ONE);
    git(&root, &["add", RECORD, SOURCE]);
    git(&root, &["commit", "--quiet", "-m", "self approval"]);
    git(
        &root,
        &["fetch", "--quiet", ".", "master:refs/remotes/origin/master"],
    );
    let before = files(BEFORE);
    let current = files(AFTER_ONE);
    let (findings, _) = compare_files(&before, &current);
    assert_eq!(
        approval::apply(
            &root,
            &before,
            &current,
            findings,
            crate::git_base::BaseUse::Ancestor
        )
        .expect("master self approval")
        .len(),
        1
    );
}

#[test]
fn approval_must_match_every_finding_and_the_exact_file_bytes() {
    let (root, _) = repository("h52-approval-exact-set");
    let authority = commit_record(
        &root,
        &record(AFTER_TWO, &["test-diff:modified:sample tests::first"]),
    );
    git(&root, &["checkout", "--quiet", "-b", "feature"]);
    let head = commit_change(&root, AFTER_TWO);
    assert_eq!(remaining(&root, &authority, &head, AFTER_TWO).len(), 2);
    assert_eq!(remaining(&root, &authority, &head, AFTER_ONE).len(), 1);
}

#[test]
fn approval_never_allows_deleting_an_assertion() {
    let (root, _) = repository("h52-approval-assertion");
    let authority = commit_record(
        &root,
        &record(
            ASSERTION_DELETED,
            &["test-diff:modified:sample tests::first"],
        ),
    );
    git(&root, &["checkout", "--quiet", "-b", "feature"]);
    let head = commit_change(&root, ASSERTION_DELETED);
    assert_eq!(
        remaining(&root, &authority, &head, ASSERTION_DELETED),
        ["test-diff:modified:sample tests::first"]
    );
}

#[test]
fn a_record_introduced_by_merging_a_branch_is_not_approved() {
    let (root, _) = repository("h52-approval-merge");
    git(&root, &["checkout", "--quiet", "-b", "side"]);
    commit_record(
        &root,
        &record(AFTER_ONE, &["test-diff:modified:sample tests::first"]),
    );
    git(&root, &["checkout", "--quiet", "master"]);
    git(&root, &["merge", "--quiet", "--no-ff", "--no-edit", "side"]);
    let authority = git(&root, &["rev-parse", "HEAD"]);
    git(&root, &["checkout", "--quiet", "-b", "feature"]);
    let head = commit_change(&root, AFTER_ONE);
    assert_eq!(
        remaining(&root, &authority, &head, AFTER_ONE),
        ["test-diff:modified:sample tests::first"]
    );
}

#[test]
fn authority_must_be_an_ancestor_of_the_target() {
    let (root, base) = repository("h52-approval-ancestor");
    git(&root, &["checkout", "--quiet", "-b", "feature"]);
    let head = commit_change(&root, AFTER_ONE);
    git(&root, &["checkout", "--quiet", "master"]);
    let authority = commit_record(
        &root,
        &record(AFTER_ONE, &["test-diff:modified:sample tests::first"]),
    );
    assert_ne!(base, authority);
    assert_eq!(
        remaining(&root, &authority, &head, AFTER_ONE),
        ["test-diff:modified:sample tests::first"]
    );
}

#[test]
fn removing_an_entire_test_is_never_approved() {
    let (root, _) = repository("h52-approval-removed");
    let removed = "#[cfg(test)] mod tests { #[test] fn second() { assert_eq!(2, 2); } }\n";
    let authority = commit_record(
        &root,
        &record(removed, &["test-diff:removed:sample tests::first"]),
    );
    git(&root, &["checkout", "--quiet", "-b", "feature"]);
    let head = commit_change(&root, removed);
    let actual = remaining(&root, &authority, &head, removed);
    assert!(
        actual
            .iter()
            .any(|entry| entry == "test-diff:removed:sample tests::first"),
        "{actual:?}"
    );
}

#[test]
fn h49_historical_fixture_changes_match_exactly_two_prior_findings() {
    let source_root = repository_root();
    let before = super::base_files(&source_root, "997389dbc68dde28daff8d8b268425491c26a7b0")
        .expect("H-49 base");
    let after = super::base_files(&source_root, "0e10f05f4fb1b915a245a32b17a68f7387477d5d")
        .expect("H-49 target");
    let path = "crates/xtask/src/mutants_check/tests.rs";
    let before_hash = format!(
        "{:x}",
        Sha256::digest(before.get(path).expect("before file"))
    );
    let after_hash = format!("{:x}", Sha256::digest(after.get(path).expect("after file")));
    let expected = [
        "test-diff:modified:xtask::bin/xtask mutants_check::tests::nightly_allows_an_empty_shard_only_after_a_nonempty_catalog",
        "test-diff:support-modified:xtask::bin/xtask mutants_check::tests::fn prepared_workspace",
    ];
    let record = record_for(path, &before_hash, &after_hash, &expected);
    let (root, _) = repository("h52-approval-h49");
    let authority = commit_record(&root, &record);
    git(&root, &["checkout", "--quiet", "-b", "feature"]);
    let head = commit_change(&root, AFTER_ONE);
    let (findings, _) = compare_files(&before, &after);
    let actual: std::collections::BTreeSet<String> = findings
        .iter()
        .map(|finding| finding.rule.to_string())
        .collect();
    assert_eq!(actual, expected.into_iter().map(String::from).collect());
    let remaining =
        approval::apply_with_authority(&root, &authority, &head, (&before, &after), findings)
            .expect("historical approval evaluation");
    assert!(remaining.is_empty(), "{remaining:?}");
    let (historical, _) = repository("h52-approval-h49-history");
    let historical_head = commit_change(&historical, AFTER_ONE);
    commit_record(&historical, &record);
    git(
        &historical,
        &["fetch", "--quiet", ".", "master:refs/remotes/origin/master"],
    );
    git(
        &historical,
        &["checkout", "--quiet", "--detach", &historical_head],
    );
    let (historical_findings, _) = compare_files(&before, &after);
    let historical_remaining = approval::apply(
        &historical,
        &before,
        &after,
        historical_findings,
        crate::git_base::BaseUse::Ancestor,
    )
    .expect("historical master evaluation");
    assert!(historical_remaining.is_empty(), "{historical_remaining:?}");
}

#[test]
fn unrelated_master_merges_do_not_invalidate_an_earlier_main_approval() {
    let (root, _) = repository("h52-approval-unrelated-merge");
    commit_record(
        &root,
        &record(AFTER_ONE, &["test-diff:modified:sample tests::first"]),
    );
    git(&root, &["checkout", "--quiet", "-b", "side"]);
    write(&root, "other.txt", "independent change\n");
    git(&root, &["add", "other.txt"]);
    git(&root, &["commit", "--quiet", "-m", "independent"]);
    git(&root, &["checkout", "--quiet", "master"]);
    git(&root, &["merge", "--quiet", "--no-ff", "--no-edit", "side"]);
    let authority = git(&root, &["rev-parse", "HEAD"]);
    git(&root, &["checkout", "--quiet", "-b", "feature"]);
    let head = commit_change(&root, AFTER_ONE);
    assert_eq!(
        remaining(&root, &authority, &head, AFTER_ONE),
        Vec::<String>::new()
    );
}

#[test]
fn later_main_approval_applies_to_a_historical_master_commit() {
    let (root, _) = repository("h52-approval-history");
    let head = commit_change(&root, AFTER_ONE);
    let authority = commit_record(
        &root,
        &record(AFTER_ONE, &["test-diff:modified:sample tests::first"]),
    );
    git(
        &root,
        &["fetch", "--quiet", ".", "master:refs/remotes/origin/master"],
    );
    git(&root, &["checkout", "--quiet", "--detach", &head]);
    let before = files(BEFORE);
    let current = files(AFTER_ONE);
    let (findings, _) = compare_files(&before, &current);
    let remaining = approval::apply(
        &root,
        &before,
        &current,
        findings,
        crate::git_base::BaseUse::Ancestor,
    )
    .expect("historical master approval");
    assert!(remaining.is_empty(), "authority {authority}: {remaining:?}");
}

#[test]
fn a_later_master_approval_does_not_make_a_side_branch_trusted() {
    let (root, _) = repository("h52-approval-side-history");
    git(&root, &["checkout", "--quiet", "-b", "side"]);
    let head = commit_change(&root, AFTER_ONE);
    git(&root, &["checkout", "--quiet", "master"]);
    git(&root, &["merge", "--quiet", "--no-ff", "--no-edit", "side"]);
    commit_record(
        &root,
        &record(AFTER_ONE, &["test-diff:modified:sample tests::first"]),
    );
    git(
        &root,
        &["fetch", "--quiet", ".", "master:refs/remotes/origin/master"],
    );
    git(&root, &["checkout", "--quiet", "--detach", &head]);
    let before = files(BEFORE);
    let current = files(AFTER_ONE);
    let (findings, _) = compare_files(&before, &current);
    let remaining = approval::apply(
        &root,
        &before,
        &current,
        findings,
        crate::git_base::BaseUse::Ancestor,
    )
    .expect("side branch evaluation");
    assert_eq!(remaining.len(), 1);
}

#[test]
fn approved_preparation_cannot_skip_an_unchanged_assertion() {
    let (root, _) = repository("h52-approval-early-return");
    let bypass = r#"
#[cfg(test)] mod tests {
    #[test] fn first() { let _fixture = "new"; return; assert_eq!(1, 1); }
    #[test] fn second() { let _fixture = "old"; assert_eq!(2, 2); }
}
"#;
    let authority = commit_record(
        &root,
        &record(bypass, &["test-diff:modified:sample tests::first"]),
    );
    git(&root, &["checkout", "--quiet", "-b", "feature"]);
    let head = commit_change(&root, bypass);
    assert_eq!(
        remaining(&root, &authority, &head, bypass),
        ["test-diff:modified:sample tests::first"]
    );
}

#[test]
fn a_shallow_repository_never_supplies_approval_authority() {
    let (root, _) = repository("h52-approval-shallow");
    let authority = commit_record(
        &root,
        &record(AFTER_ONE, &["test-diff:modified:sample tests::first"]),
    );
    git(&root, &["checkout", "--quiet", "-b", "feature"]);
    let head = commit_change(&root, AFTER_ONE);
    write(&root, ".git/shallow", &format!("{authority}\n"));
    assert_eq!(
        remaining(&root, &authority, &head, AFTER_ONE),
        ["test-diff:modified:sample tests::first"]
    );
}

#[test]
fn later_master_history_does_not_validate_a_same_commit_approval() {
    let (root, _) = repository("h52-approval-master-self-history");
    write(
        &root,
        RECORD,
        &record(AFTER_ONE, &["test-diff:modified:sample tests::first"]),
    );
    write(&root, SOURCE, AFTER_ONE);
    git(&root, &["add", RECORD, SOURCE]);
    git(&root, &["commit", "--quiet", "-m", "self approval"]);
    let head = git(&root, &["rev-parse", "HEAD"]);
    write(&root, "later.txt", "unrelated\n");
    git(&root, &["add", "later.txt"]);
    git(&root, &["commit", "--quiet", "-m", "later master"]);
    git(
        &root,
        &["fetch", "--quiet", ".", "master:refs/remotes/origin/master"],
    );
    git(&root, &["checkout", "--quiet", "--detach", &head]);
    let before = files(BEFORE);
    let current = files(AFTER_ONE);
    let (findings, _) = compare_files(&before, &current);
    assert_eq!(
        approval::apply(
            &root,
            &before,
            &current,
            findings,
            crate::git_base::BaseUse::Ancestor
        )
        .expect("historical self approval")
        .len(),
        1
    );
}

#[test]
fn approved_preparation_between_unchanged_assertions_can_pass() {
    let before_text = r#"#[cfg(test)] mod tests { #[test] fn first() { assert_eq!(1, 1); let _fixture = "old"; assert_eq!(2, 2); } }"#;
    let after_text = r#"#[cfg(test)] mod tests { #[test] fn first() { assert_eq!(1, 1); let _fixture = "new"; assert_eq!(2, 2); } }"#;
    let (root, _) = repository("h52-approval-between-assertions");
    let before_hash = digest(before_text);
    let after_hash = digest(after_text);
    let record = format!(
        "[[approval]]\ntask = \"H-49\"\npath = \"{SOURCE}\"\nbefore_sha256 = \"{before_hash}\"\nafter_sha256 = \"{after_hash}\"\nfinding = \"test-diff:modified:sample tests::first\"\ndecision = \"approved\"\nreason = \"reviewed fixture preparation\"\n"
    );
    let authority = commit_record(&root, &record);
    git(&root, &["checkout", "--quiet", "-b", "feature"]);
    let head = commit_change(&root, after_text);
    let before = files(before_text);
    let current = files(after_text);
    let (findings, _) = compare_files(&before, &current);
    let remaining =
        approval::apply_with_authority(&root, &authority, &head, (&before, &current), findings)
            .expect("fixture between assertions");
    assert!(remaining.is_empty(), "{remaining:?}");
}

#[test]
fn a_branch_can_receive_main_approval_in_a_verified_master_merge() {
    let (root, _) = repository("h52-approval-incoming-master");
    git(&root, &["checkout", "--quiet", "-b", "feature"]);
    commit_change(&root, AFTER_ONE);
    git(&root, &["checkout", "--quiet", "master"]);
    commit_record(
        &root,
        &record(AFTER_ONE, &["test-diff:modified:sample tests::first"]),
    );
    git(
        &root,
        &["fetch", "--quiet", ".", "master:refs/remotes/origin/master"],
    );
    git(&root, &["checkout", "--quiet", "feature"]);
    git(
        &root,
        &["merge", "--quiet", "--no-ff", "--no-commit", "master"],
    );
    let before = files(BEFORE);
    let current = files(AFTER_ONE);
    let (findings, _) = compare_files(&before, &current);
    assert!(
        approval::apply(
            &root,
            &before,
            &current,
            findings,
            crate::git_base::BaseUse::Commit
        )
        .expect("incoming approval")
        .is_empty()
    );
}

#[test]
fn a_branch_cannot_append_its_own_approval_to_an_existing_record() {
    let (root, _) = repository("h52-approval-append");
    commit_record(
        &root,
        &record(AFTER_ONE, &["test-diff:modified:sample tests::first"]),
    );
    git(&root, &["checkout", "--quiet", "-b", "side"]);
    write(
        &root,
        RECORD,
        &record(
            AFTER_TWO,
            &[
                "test-diff:modified:sample tests::first",
                "test-diff:modified:sample tests::second",
            ],
        ),
    );
    git(&root, &["add", RECORD]);
    git(&root, &["commit", "--quiet", "-m", "unauthorized record"]);
    git(&root, &["checkout", "--quiet", "master"]);
    git(&root, &["merge", "--quiet", "--no-ff", "--no-edit", "side"]);
    let authority = git(&root, &["rev-parse", "HEAD"]);
    git(&root, &["checkout", "--quiet", "-b", "feature"]);
    let head = commit_change(&root, AFTER_TWO);
    assert_eq!(remaining(&root, &authority, &head, AFTER_TWO).len(), 2);
}

#[test]
fn an_approved_finding_with_extra_unapproved_preparation_is_rejected() {
    let (root, _) = repository("h52-approval-extra-edit");
    let authority = commit_record(
        &root,
        &record(AFTER_ONE, &["test-diff:modified:sample tests::first"]),
    );
    git(&root, &["checkout", "--quiet", "-b", "feature"]);
    let extra = AFTER_ONE.replace(
        "let _fixture = \"new\";",
        "let _fixture = \"new\"; let _extra = 7;",
    );
    let head = commit_change(&root, &extra);
    assert_eq!(
        remaining(&root, &authority, &head, &extra),
        ["test-diff:modified:sample tests::first"]
    );
}

#[test]
fn approved_assertions_cannot_be_replaced_by_weaker_ones() {
    let (root, _) = repository("h52-approval-weakened-assertion");
    let weak = AFTER_ONE.replace("assert_eq!(1, 1)", "assert!(true)");
    let authority = commit_record(
        &root,
        &record(&weak, &["test-diff:modified:sample tests::first"]),
    );
    git(&root, &["checkout", "--quiet", "-b", "feature"]);
    let head = commit_change(&root, &weak);
    assert_eq!(
        remaining(&root, &authority, &head, &weak),
        ["test-diff:modified:sample tests::first"]
    );
}

#[test]
fn duplicate_or_unapproved_records_fail_closed() {
    for (name, record_text) in [
        (
            "h52-approval-duplicate",
            format!(
                "{}{}",
                record(AFTER_ONE, &["test-diff:modified:sample tests::first"]),
                record(AFTER_ONE, &["test-diff:modified:sample tests::first"])
            ),
        ),
        (
            "h52-approval-unapproved",
            record(AFTER_ONE, &["test-diff:modified:sample tests::first"])
                .replace("decision = \"approved\"", "decision = \"pending\""),
        ),
        (
            "h52-approval-pathspec",
            record(AFTER_ONE, &["test-diff:modified:sample tests::first"])
                .replace(SOURCE, ":(glob)crates/**/lib.rs"),
        ),
    ] {
        let (root, _) = repository(name);
        let authority = commit_record(&root, &record_text);
        git(&root, &["checkout", "--quiet", "-b", "feature"]);
        let head = commit_change(&root, AFTER_ONE);
        let before = files(BEFORE);
        let current = files(AFTER_ONE);
        let (findings, _) = compare_files(&before, &current);
        approval::apply_with_authority(&root, &authority, &head, (&before, &current), findings)
            .expect_err("invalid approval must fail");
    }
}

#[test]
fn approved_fixture_change_allows_literal_pipe_inside_an_unchanged_assertion() {
    let before_text = r#"#[cfg(test)] mod tests { #[test] fn first() { let _fixture = "old"; assert_eq!("a|b", "a|b"); } }"#;
    let after_text = r#"#[cfg(test)] mod tests { #[test] fn first() { let _fixture = "new"; assert_eq!("a|b", "a|b"); } }"#;
    let (root, _) = repository("h52-approval-pipe-literal");
    let text = record_for(
        SOURCE,
        &digest(before_text),
        &digest(after_text),
        &["test-diff:modified:sample tests::first"],
    );
    let authority = commit_record(&root, &text);
    git(&root, &["checkout", "--quiet", "-b", "feature"]);
    let head = commit_change(&root, after_text);
    let before = files(before_text);
    let current = files(after_text);
    let (findings, _) = compare_files(&before, &current);
    let remaining =
        approval::apply_with_authority(&root, &authority, &head, (&before, &current), findings)
            .expect("pipe literal fixture");
    assert!(remaining.is_empty(), "{remaining:?}");
}

#[test]
fn later_feature_cannot_reuse_a_same_commit_master_self_approval() {
    let (root, _) = repository("h52-approval-self-ancestor");
    write(
        &root,
        RECORD,
        &record(AFTER_ONE, &["test-diff:modified:sample tests::first"]),
    );
    write(&root, SOURCE, AFTER_ONE);
    git(&root, &["add", RECORD, SOURCE]);
    git(&root, &["commit", "--quiet", "-m", "self approval"]);
    write(&root, "later.txt", "later\n");
    git(&root, &["add", "later.txt"]);
    git(&root, &["commit", "--quiet", "-m", "later master"]);
    let authority = git(&root, &["rev-parse", "HEAD"]);
    git(&root, &["checkout", "--quiet", "-b", "feature"]);
    git(
        &root,
        &["commit", "--quiet", "--allow-empty", "-m", "feature"],
    );
    let head = git(&root, &["rev-parse", "HEAD"]);
    assert_eq!(
        remaining(&root, &authority, &head, AFTER_ONE),
        ["test-diff:modified:sample tests::first"]
    );
}

#[test]
fn approval_never_allows_removing_a_named_assertion_helper_call() {
    let before_text = r#"#[cfg(test)] mod tests { fn assert_state() { assert_eq!(1, 1); } #[test] fn first() { let _fixture = "old"; assert_state(); assert_eq!(2, 2); } }"#;
    let after_text = r#"#[cfg(test)] mod tests { fn assert_state() { assert_eq!(1, 1); } #[test] fn first() { let _fixture = "new"; assert_eq!(2, 2); } }"#;
    let (root, _) = repository("h52-approval-assertion-helper");
    let approval = record_for(
        SOURCE,
        &digest(before_text),
        &digest(after_text),
        &["test-diff:modified:sample tests::first"],
    );
    let authority = commit_record(&root, &approval);
    git(&root, &["checkout", "--quiet", "-b", "feature"]);
    let head = commit_change(&root, after_text);
    let before = files(before_text);
    let current = files(after_text);
    let (findings, _) = compare_files(&before, &current);
    let remaining =
        approval::apply_with_authority(&root, &authority, &head, (&before, &current), findings)
            .expect("assertion helper approval");
    assert_eq!(remaining.len(), 1);
}

#[test]
fn approval_never_allows_removing_an_assertion_helper_from_preparation() {
    let before_text = r#"#[cfg(test)] mod tests { fn verify_state() { assert_eq!(1, 1); } fn prepare() { verify_state(); let _fixture = "old"; } #[test] fn first() { prepare(); assert_eq!(2, 2); } }"#;
    let after_text = r#"#[cfg(test)] mod tests { fn verify_state() { assert_eq!(1, 1); } fn prepare() { let _fixture = "new"; } #[test] fn first() { prepare(); assert_eq!(2, 2); } }"#;
    let (root, _) = repository("h52-approval-support-assertion-helper");
    let approval = record_for(
        SOURCE,
        &digest(before_text),
        &digest(after_text),
        &["test-diff:support-modified:sample tests::fn prepare"],
    );
    let authority = commit_record(&root, &approval);
    git(&root, &["checkout", "--quiet", "-b", "feature"]);
    let head = commit_change(&root, after_text);
    let before = files(before_text);
    let current = files(after_text);
    let (findings, _) = compare_files(&before, &current);
    let remaining =
        approval::apply_with_authority(&root, &authority, &head, (&before, &current), findings)
            .expect("support assertion helper approval");
    assert_eq!(remaining.len(), 1);
}

#[test]
fn master_merge_may_use_the_latest_separate_main_approval() {
    let (root, _) = repository("h52-review-master-merge-approved");
    commit_record(
        &root,
        &record(AFTER_ONE, &["test-diff:modified:sample tests::first"]),
    );
    git(
        &root,
        &["fetch", "--quiet", ".", "master:refs/remotes/origin/master"],
    );
    git(&root, &["checkout", "--quiet", "-b", "feature"]);
    commit_change(&root, AFTER_ONE);
    git(&root, &["checkout", "--quiet", "master"]);
    git(
        &root,
        &["merge", "--quiet", "--no-ff", "--no-commit", "feature"],
    );
    let before = files(BEFORE);
    let current = files(AFTER_ONE);
    let (findings, _) = compare_files(&before, &current);
    let remaining = approval::apply(
        &root,
        &before,
        &current,
        findings,
        crate::git_base::BaseUse::Commit,
    )
    .expect("master merge approval");
    assert!(remaining.is_empty(), "{remaining:?}");
}

#[test]
fn master_merge_cannot_validate_a_same_commit_self_approval() {
    let (root, _) = repository("h52-review-master-merge-self-approval");
    write(
        &root,
        RECORD,
        &record(AFTER_ONE, &["test-diff:modified:sample tests::first"]),
    );
    write(&root, SOURCE, AFTER_ONE);
    git(&root, &["add", RECORD, SOURCE]);
    git(&root, &["commit", "--quiet", "-m", "self approval"]);
    git(
        &root,
        &["fetch", "--quiet", ".", "master:refs/remotes/origin/master"],
    );
    git(&root, &["checkout", "--quiet", "-b", "feature"]);
    write(&root, "later.txt", "unrelated\n");
    git(&root, &["add", "later.txt"]);
    git(&root, &["commit", "--quiet", "-m", "feature"]);
    git(&root, &["checkout", "--quiet", "master"]);
    git(
        &root,
        &["merge", "--quiet", "--no-ff", "--no-commit", "feature"],
    );
    let before = files(BEFORE);
    let current = files(AFTER_ONE);
    let (findings, _) = compare_files(&before, &current);
    let remaining = approval::apply(
        &root,
        &before,
        &current,
        findings,
        crate::git_base::BaseUse::Commit,
    )
    .expect("master merge self approval");
    assert_eq!(remaining.len(), 1);
}

#[test]
fn h55_explicitly_approved_assertion_expectation_change_passes_but_unmarked_does_not() {
    let changed = BEFORE.replace("assert_eq!(1, 1)", "assert_eq!(1, 3)");
    let finding = "test-diff:modified:sample tests::first";
    for (marked, expected) in [(false, vec![finding]), (true, Vec::new())] {
        let (root, _) = repository("h55-assertion-approval");
        let mut approval = record(&changed, &[finding]);
        if marked {
            approval = approval.replace("decision =", "assertion_change = true\ndecision =");
        }
        let authority = commit_record(&root, &approval);
        git(&root, &["checkout", "--quiet", "-b", "feature"]);
        let head = commit_change(&root, &changed);
        assert_eq!(remaining(&root, &authority, &head, &changed), expected);
    }
}

#[test]
fn h55_an_assertion_approval_does_not_authorize_assertion_deletion() {
    let (root, _) = repository("h55-no-assertion-deletion");
    let finding = "test-diff:modified:sample tests::first";
    let approval = record(ASSERTION_DELETED, &[finding])
        .replace("decision =", "assertion_change = true\ndecision =");
    let authority = commit_record(&root, &approval);
    git(&root, &["checkout", "--quiet", "-b", "feature"]);
    let head = commit_change(&root, ASSERTION_DELETED);
    assert_eq!(
        remaining(&root, &authority, &head, ASSERTION_DELETED),
        [finding]
    );
}

#[test]
fn h55_explicitly_approved_impl_and_method_signature_change_passes() {
    let old = r"#[cfg(test)] mod tests {
        struct Fixture;
        impl Fixture { fn value(&self, value: u8) -> u8 { value } }
        #[test] fn first() { let fixture = Fixture; assert_eq!(fixture.value(1), 1); }
    }";
    let new = r"#[cfg(test)] mod tests {
        struct Fixture;
        impl Fixture { fn value(&self, value: u8, scale: u8) -> u8 { value * scale } }
        #[test] fn first() { let fixture = Fixture; assert_eq!(fixture.value(1, 1), 1); }
    }";
    let (root, _) = repository("h55-impl-approval");
    write(&root, SOURCE, old);
    git(&root, &["add", SOURCE]);
    git(&root, &["commit", "--quiet", "-m", "impl baseline"]);
    let findings = [
        "test-diff:modified:sample tests::first",
        "test-diff:support-modified:sample tests::impl impl Fixture",
    ];
    let approval = record_for(SOURCE, &digest(old), &digest(new), &findings)
        .replace("decision =", "assertion_change = true\ndecision =");
    let authority = commit_record(&root, &approval);
    git(&root, &["checkout", "--quiet", "-b", "feature"]);
    let head = commit_change(&root, new);
    let before = files(old);
    let after = files(new);
    let (found, _) = compare_files(&before, &after);
    let actual: std::collections::BTreeSet<_> =
        found.iter().map(|entry| entry.rule.to_string()).collect();
    assert_eq!(actual, findings.into_iter().map(String::from).collect());
    let remaining =
        approval::apply_with_authority(&root, &authority, &head, (&before, &after), found)
            .expect("approved impl");
    assert!(remaining.is_empty(), "{remaining:?}");
}

struct SyntheticSources {
    old: String,
    new: String,
    findings: Vec<(String, bool)>,
}

fn synthetic_support(sources: &mut SyntheticSources, module: &str, index: usize, count: usize) {
    for support in 0..count {
        if index == 0 && support == 0 {
            sources
                .old
                .push_str("struct Fixture; impl Fixture { fn make(value: u8) -> u8 { value } }\n");
            sources.new.push_str("struct Fixture; impl Fixture { fn make(value: u8, offset: u8) -> u8 { value + offset } }\n");
            sources.findings.push((
                format!("test-diff:support-modified:sample {module}::impl impl Fixture"),
                false,
            ));
        } else {
            writeln!(
                sources.old,
                "fn helper_{support}(value: u8) -> u8 {{ value }}"
            )
            .expect("fixture source");
            writeln!(
                sources.new,
                "fn helper_{support}(value: u8, offset: u8) -> u8 {{ value + offset }}"
            )
            .expect("fixture source");
            sources.findings.push((
                format!("test-diff:support-modified:sample {module}::fn helper_{support}"),
                false,
            ));
        }
    }
}

fn synthetic_tests(
    sources: &mut SyntheticSources,
    module: &str,
    index: usize,
    supports: usize,
    count: usize,
) {
    if count == 0 {
        let sentinel = "#[test] fn sentinel() { assert_eq!(observed(11), 11); }\n";
        sources.old.push_str(sentinel);
        sources.new.push_str(sentinel);
    }
    let (prior, current) = if index == 0 {
        ("Fixture::make(7)", "Fixture::make(7, 9)")
    } else if supports > 0 {
        ("helper_0(7)", "helper_0(7, 9)")
    } else {
        ("7", "9")
    };
    for test_index in 0..count {
        let changed = index < 2 && test_index == 0;
        let value = 20_usize.checked_add(test_index).expect("fixture value");
        let after_value = value
            .checked_add(usize::from(changed))
            .expect("fixture after value");
        writeln!(sources.old, "#[test] fn case_{test_index}() {{ let _context = {prior}; assert_eq!(observed({value}), {value}); }}").expect("fixture source");
        writeln!(sources.new, "#[test] fn case_{test_index}() {{ let _context = {current}; assert_eq!(observed({after_value}), {after_value}); }}").expect("fixture source");
        sources.findings.push((
            format!("test-diff:modified:sample {module}::case_{test_index}"),
            changed,
        ));
    }
}

fn synthetic_file(index: usize, supports: usize, tests: usize) -> SyntheticSources {
    let source = String::from("fn observed(value: u8) -> u8 { value }\n");
    let mut result = SyntheticSources {
        old: source.clone(),
        new: source,
        findings: Vec::new(),
    };
    let module = format!("sample_{index}");
    synthetic_support(&mut result, &module, index, supports);
    synthetic_tests(&mut result, &module, index, supports, tests);
    result
}

const H55_DISTRIBUTION: [(usize, usize); 9] = [
    (3, 6),
    (1, 3),
    (1, 1),
    (0, 1),
    (0, 1),
    (3, 0),
    (3, 10),
    (1, 1),
    (1, 0),
];

#[test]
fn h55_nine_files_and_thirty_six_exact_findings_require_complete_approval() {
    let (root, _) = repository("h55-nine-files");
    let mut before = files("");
    let mut after = files("");
    let mut declarations = String::new();
    let mut records = String::new();
    let mut expected = std::collections::BTreeSet::new();
    for (index, (supports, tests)) in H55_DISTRIBUTION.into_iter().enumerate() {
        let module = format!("sample_{index}");
        let path = format!("crates/sample/src/{module}.rs");
        writeln!(declarations, "#[cfg(test)] mod {module};").expect("fixture module");
        let sources = synthetic_file(index, supports, tests);
        before.insert(path.clone(), sources.old.as_bytes().to_vec());
        after.insert(path.clone(), sources.new.as_bytes().to_vec());
        write(&root, &path, &sources.old);
        for (finding, assertion) in sources.findings {
            let row = record_for(
                &path,
                &digest(&sources.old),
                &digest(&sources.new),
                &[&finding],
            );
            records.push_str(&if assertion {
                row.replace("decision =", "assertion_change = true\ndecision =")
            } else {
                row
            });
            assert!(expected.insert(finding));
        }
    }
    before.insert(String::from(SOURCE), declarations.as_bytes().to_vec());
    after.insert(String::from(SOURCE), declarations.as_bytes().to_vec());
    write(&root, SOURCE, &declarations);
    git(&root, &["add", "crates/sample/src"]);
    git(&root, &["commit", "--quiet", "-m", "fixture baseline"]);
    let authority = commit_record(&root, &records);
    git(&root, &["checkout", "--quiet", "-b", "feature"]);
    for (path, bytes) in &after {
        if path.starts_with("crates/sample/src/") {
            write(
                &root,
                path,
                std::str::from_utf8(bytes).expect("fixture text"),
            );
        }
    }
    git(&root, &["add", "crates/sample/src"]);
    git(&root, &["commit", "--quiet", "-m", "fixture changes"]);
    let head = git(&root, &["rev-parse", "HEAD"]);
    let (found, _) = compare_files(&before, &after);
    let actual: std::collections::BTreeSet<_> =
        found.iter().map(|item| item.rule.to_string()).collect();
    assert_eq!(actual, expected);
    assert_eq!(actual.len(), 36);
    let remaining =
        approval::apply_with_authority(&root, &authority, &head, (&before, &after), found)
            .expect("matching approvals");
    assert!(remaining.is_empty(), "{remaining:?}");
}

#[test]
fn h55_explicit_approval_cannot_replace_an_assertion_with_a_trivial_one() {
    let old = BEFORE.replace("assert_eq!(1, 1)", "assert_eq!(observe(), 7)");
    let changed = old.replace("assert_eq!(observe(), 7)", "assert_eq!(9, 9)");
    let finding = "test-diff:modified:sample tests::first";
    let (root, _) = repository("h55-trivial-assertion");
    write(&root, SOURCE, &old);
    git(&root, &["add", SOURCE]);
    git(&root, &["commit", "--quiet", "-m", "assertion baseline"]);
    let approval = record_for(SOURCE, &digest(&old), &digest(&changed), &[finding])
        .replace("decision =", "assertion_change = true\ndecision =");
    let authority = commit_record(&root, &approval);
    git(&root, &["checkout", "--quiet", "-b", "feature"]);
    let head = commit_change(&root, &changed);
    let before = files(&old);
    let after = files(&changed);
    let (found, _) = compare_files(&before, &after);
    let remaining =
        approval::apply_with_authority(&root, &authority, &head, (&before, &after), found)
            .expect("trivial assertion evaluation");
    assert_eq!(remaining.len(), 1);
}

#[test]
fn h55_approved_impl_associated_constant_change_is_accepted() {
    let old = r"#[cfg(test)] mod tests {
        struct Fixture;
        impl Fixture { const LIMIT: u8 = 7; fn value(&self) -> u8 { Self::LIMIT } }
        #[test] fn first() { assert_eq!(Fixture.value(), 7); }
    }";
    let new = old.replace("LIMIT: u8 = 7", "LIMIT: u8 = 9");
    let (root, _) = repository("h55-impl-const");
    write(&root, SOURCE, old);
    git(&root, &["add", SOURCE]);
    git(&root, &["commit", "--quiet", "-m", "impl baseline"]);
    let finding = "test-diff:support-modified:sample tests::impl impl Fixture";
    let approval = record_for(SOURCE, &digest(old), &digest(&new), &[finding]);
    let authority = commit_record(&root, &approval);
    git(&root, &["checkout", "--quiet", "-b", "feature"]);
    let head = commit_change(&root, &new);
    let before = files(old);
    let after = files(&new);
    let (found, _) = compare_files(&before, &after);
    let remaining =
        approval::apply_with_authority(&root, &authority, &head, (&before, &after), found)
            .expect("impl const approval");
    assert!(remaining.is_empty(), "{remaining:?}");
}

#[test]
fn h55_approval_cannot_bypass_an_assertion_with_short_circuit() {
    let old = BEFORE.replace("assert_eq!(1, 1)", "assert!(observe())");
    let changed = old.replace("assert!(observe())", "assert!(true || observe())");
    let finding = "test-diff:modified:sample tests::first";
    let (root, _) = repository("h55-short-circuit");
    write(&root, SOURCE, &old);
    git(&root, &["add", SOURCE]);
    git(&root, &["commit", "--quiet", "-m", "assertion baseline"]);
    let approval = record_for(SOURCE, &digest(&old), &digest(&changed), &[finding])
        .replace("decision =", "assertion_change = true\ndecision =");
    let authority = commit_record(&root, &approval);
    git(&root, &["checkout", "--quiet", "-b", "feature"]);
    let head = commit_change(&root, &changed);
    let before = files(&old);
    let after = files(&changed);
    let (found, _) = compare_files(&before, &after);
    let remaining =
        approval::apply_with_authority(&root, &authority, &head, (&before, &after), found)
            .expect("short-circuit evaluation");
    assert_eq!(remaining.len(), 1);
}

#[test]
fn h55_assertion_approval_field_requires_a_boolean() {
    let changed = BEFORE.replace("assert_eq!(1, 1)", "assert_eq!(1, 3)");
    let finding = "test-diff:modified:sample tests::first";
    let (root, _) = repository("h55-invalid-assertion-field");
    let approval = record(&changed, &[finding])
        .replace("decision =", "assertion_change = \"true\"\ndecision =");
    let authority = commit_record(&root, &approval);
    git(&root, &["checkout", "--quiet", "-b", "feature"]);
    let head = commit_change(&root, &changed);
    let before = files(BEFORE);
    let after = files(&changed);
    let (found, _) = compare_files(&before, &after);
    approval::apply_with_authority(&root, &authority, &head, (&before, &after), found)
        .expect_err("invalid assertion field must fail");
}

#[test]
fn h55_impl_assertion_change_requires_its_own_explicit_field() {
    let old = r"#[cfg(test)] mod tests {
        struct Fixture;
        impl Fixture { fn verify(&self, value: u8) { assert_eq!(value, 7); } }
        #[test] fn first() { Fixture.verify(7); assert_eq!(3, 3); }
    }";
    let changed = old.replace("assert_eq!(value, 7)", "assert_eq!(value, 9)");
    let finding = "test-diff:support-modified:sample tests::impl impl Fixture";
    for (marked, expected) in [(false, 1_usize), (true, 0_usize)] {
        let (root, _) = repository("h55-impl-assertion");
        write(&root, SOURCE, old);
        git(&root, &["add", SOURCE]);
        git(&root, &["commit", "--quiet", "-m", "impl baseline"]);
        let mut approval = record_for(SOURCE, &digest(old), &digest(&changed), &[finding]);
        if marked {
            approval = approval.replace("decision =", "assertion_change = true\ndecision =");
        }
        let authority = commit_record(&root, &approval);
        git(&root, &["checkout", "--quiet", "-b", "feature"]);
        let head = commit_change(&root, &changed);
        let before = files(old);
        let after = files(&changed);
        let (found, _) = compare_files(&before, &after);
        let remaining =
            approval::apply_with_authority(&root, &authority, &head, (&before, &after), found)
                .expect("impl assertion approval");
        assert_eq!(remaining.len(), expected);
    }
}

#[test]
fn h55_impl_type_change_cannot_delete_a_compile_time_assertion() {
    let old = r"#[cfg(test)] mod tests {
        trait Example { type Output; }
        struct Fixture;
        impl Example for Fixture { type Output = [u8; { assert!(std::mem::size_of::<u8>() == 1); 1 }]; }
        #[test] fn first() { assert_eq!(std::mem::size_of::<<Fixture as Example>::Output>(), 1); }
    }";
    let changed = old.replace("{ assert!(std::mem::size_of::<u8>() == 1); 1 }", "1");
    let finding = "test-diff:support-modified:sample tests::impl impl Example for Fixture";
    let (root, _) = repository("h55-type-assertion");
    write(&root, SOURCE, old);
    git(&root, &["add", SOURCE]);
    git(&root, &["commit", "--quiet", "-m", "impl baseline"]);
    let approval = record_for(SOURCE, &digest(old), &digest(&changed), &[finding])
        .replace("decision =", "assertion_change = true\ndecision =");
    let authority = commit_record(&root, &approval);
    git(&root, &["checkout", "--quiet", "-b", "feature"]);
    let head = commit_change(&root, &changed);
    let before = files(old);
    let after = files(&changed);
    let (found, _) = compare_files(&before, &after);
    assert_eq!(found.len(), 1);
    let remaining =
        approval::apply_with_authority(&root, &authority, &head, (&before, &after), found)
            .expect("impl type evaluation");
    assert_eq!(remaining.len(), 1);
}

#[test]
fn h55_approval_cannot_hide_assertion_bypass_inside_nested_macro() {
    let old = BEFORE.replace("assert_eq!(1, 1)", "assert!(matches!(observe(), Some(7)))");
    let changed = old.replace(
        "matches!(observe(), Some(7))",
        "matches!({ if false { observe(); } Some(7) }, Some(7))",
    );
    let finding = "test-diff:modified:sample tests::first";
    let (root, _) = repository("h55-nested-assertion");
    write(&root, SOURCE, &old);
    git(&root, &["add", SOURCE]);
    git(&root, &["commit", "--quiet", "-m", "assertion baseline"]);
    let approval = record_for(SOURCE, &digest(&old), &digest(&changed), &[finding])
        .replace("decision =", "assertion_change = true\ndecision =");
    let authority = commit_record(&root, &approval);
    git(&root, &["checkout", "--quiet", "-b", "feature"]);
    let head = commit_change(&root, &changed);
    let before = files(&old);
    let after = files(&changed);
    let (found, _) = compare_files(&before, &after);
    let remaining =
        approval::apply_with_authority(&root, &authority, &head, (&before, &after), found)
            .expect("nested assertion evaluation");
    assert_eq!(remaining.len(), 1);
}

#[test]
fn h55_approved_impl_associated_type_change_is_accepted() {
    let old = r"#[cfg(test)] mod tests {
        trait Example { type Output; }
        struct Fixture;
        impl Example for Fixture { type Output = [u8; 7]; }
        #[test] fn first() { assert_eq!(3, 3); }
    }";
    let changed = old.replace("[u8; 7]", "[u8; 9]");
    let finding = "test-diff:support-modified:sample tests::impl impl Example for Fixture";
    let (root, _) = repository("h55-impl-type");
    write(&root, SOURCE, old);
    git(&root, &["add", SOURCE]);
    git(&root, &["commit", "--quiet", "-m", "impl baseline"]);
    let approval = record_for(SOURCE, &digest(old), &digest(&changed), &[finding]);
    let authority = commit_record(&root, &approval);
    git(&root, &["checkout", "--quiet", "-b", "feature"]);
    let head = commit_change(&root, &changed);
    let before = files(old);
    let after = files(&changed);
    let (found, _) = compare_files(&before, &after);
    let remaining =
        approval::apply_with_authority(&root, &authority, &head, (&before, &after), found)
            .expect("impl type approval");
    assert!(remaining.is_empty(), "{remaining:?}");
}

#[test]
fn h55_an_approval_introduced_in_a_root_commit_is_not_authority() {
    let root = fresh_directory(&repository_root(), "h55-root-approval");
    git(&root, &["init", "--quiet", "-b", "master"]);
    let finding = "test-diff:modified:sample tests::first";
    write(&root, RECORD, &record(AFTER_ONE, &[finding]));
    git(&root, &["add", RECORD]);
    git(&root, &["commit", "--quiet", "-m", "root approval"]);
    let authority = git(&root, &["rev-parse", "HEAD"]);
    write(&root, SOURCE, BEFORE);
    git(&root, &["add", SOURCE]);
    git(&root, &["commit", "--quiet", "-m", "baseline"]);
    git(&root, &["checkout", "--quiet", "-b", "feature"]);
    let head = commit_change(&root, AFTER_ONE);
    assert_eq!(remaining(&root, &authority, &head, AFTER_ONE), [finding]);
}

#[test]
fn h55_explicit_approval_rejects_obviously_true_comparisons() {
    let finding = "test-diff:modified:sample tests::first";
    for (name, before_check, after_check) in [
        (
            "comparison",
            "let value = 7; assert!(value == 7)",
            "let value = 7; assert!(value == value)",
        ),
        (
            "negation",
            "let flag = false; assert!(!flag)",
            "let flag = false; assert!(!false)",
        ),
        (
            "parentheses",
            "let value = 7; assert_eq!(value, 7)",
            "let value = 7; assert_eq!(value, (value))",
        ),
    ] {
        let old = BEFORE.replace("assert_eq!(1, 1)", before_check);
        let changed = BEFORE.replace("assert_eq!(1, 1)", after_check);
        let (root, _) = repository(&format!("h55-trivial-{name}"));
        write(&root, SOURCE, &old);
        git(&root, &["add", SOURCE]);
        git(&root, &["commit", "--quiet", "-m", "baseline"]);
        let approval = record_for(SOURCE, &digest(&old), &digest(&changed), &[finding])
            .replace("decision =", "assertion_change = true\ndecision =");
        let authority = commit_record(&root, &approval);
        git(&root, &["checkout", "--quiet", "-b", "feature"]);
        let head = commit_change(&root, &changed);
        let before = files(&old);
        let after = files(&changed);
        let (found, _) = compare_files(&before, &after);
        let remaining =
            approval::apply_with_authority(&root, &authority, &head, (&before, &after), found)
                .expect("trivial comparison evaluation");
        assert_eq!(remaining.len(), 1, "{name}");
    }
}
