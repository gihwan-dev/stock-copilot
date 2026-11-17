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
