use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;
use std::fs;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::harness_workspace_tests::{fresh_directory, repository_root};

use super::{Files, approval, compare_files};

const PATHS: [&str; 3] = [
    "crates/application/tests/ports_a02_contract.rs",
    "crates/testkit-ports/tests/memory.rs",
    "crates/testkit-ports/tests/review_regressions.rs",
];
const FINDINGS: [&str; 6] = [
    "test-diff:modified:application::ports_a02_contract tests::content_and_signal_storage_preserve_domain_types",
    "test-diff:modified:testkit-ports::memory tests::transaction_and_signal_absence_are_memory_results",
    "test-diff:support-modified:testkit-ports::review_regressions tests::impl impl SignalRepository for DiscardSignals",
    "test-diff:support-modified:testkit-ports::review_regressions tests::struct DiscardSignals",
    "test-diff:modified:testkit-ports::review_regressions tests::generic_scenarios_reject_discarded_writes_and_changed_price_range",
    "test-diff:modified:testkit-ports::review_regressions tests::generic_signal_contract_requires_nonempty_replacement_and_clearing",
];

fn git(root: &Path, args: &[&str]) -> String {
    git_with(root, args, &[])
}

fn git_with(root: &Path, args: &[&str], inherited: &[(String, String)]) -> String {
    let result = crate::git_base::git_with(root, inherited)
        .args(args)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_AUTHOR_NAME", "Fixture")
        .env("GIT_AUTHOR_EMAIL", "fixture@example.invalid")
        .env("GIT_COMMITTER_NAME", "Fixture")
        .env("GIT_COMMITTER_EMAIL", "fixture@example.invalid")
        .output()
        .expect("git");
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    String::from_utf8(result.stdout)
        .expect("utf8")
        .trim()
        .to_owned()
}

fn files(version: &str) -> Files {
    let mut files = BTreeMap::from([
        (
            String::from("Cargo.toml"),
            b"[workspace]\nmembers = [\"crates/application\", \"crates/testkit-ports\"]\n".to_vec(),
        ),
        (
            String::from("crates/application/Cargo.toml"),
            b"[package]\nname = \"application\"\n".to_vec(),
        ),
        (
            String::from("crates/testkit-ports/Cargo.toml"),
            b"[package]\nname = \"testkit-ports\"\n".to_vec(),
        ),
    ]);
    for path in PATHS {
        let name = path.rsplit('/').next().expect("filename");
        let source = repository_root().join(format!(
            "harness/test-diff/fixtures/h56-a27/{version}/{name}.txt"
        ));
        files.insert(
            String::from(path),
            fs::read(source).expect("historical fixture"),
        );
    }
    files
}

fn write_files(root: &Path, files: &Files) {
    for (name, body) in files {
        let path = root.join(name);
        fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
        fs::write(path, body).expect("write");
    }
}

#[derive(Clone, Copy)]
enum Mark {
    Unmarked,
    Marked,
}

fn records(before: &Files, after: &Files, marked: Mark) -> String {
    let mut result = String::new();
    for finding in FINDINGS {
        let path = if finding.contains("ports_a02_contract") {
            "crates/application/tests/ports_a02_contract.rs"
        } else if finding.contains("::memory ") {
            "crates/testkit-ports/tests/memory.rs"
        } else {
            "crates/testkit-ports/tests/review_regressions.rs"
        };
        let old = format!("{:x}", Sha256::digest(before.get(path).expect("before")));
        let new = format!("{:x}", Sha256::digest(after.get(path).expect("after")));
        write!(result, "[[approval]]\ntask = \"A-27\"\npath = \"{path}\"\nbefore_sha256 = \"{old}\"\nafter_sha256 = \"{new}\"\nfinding = \"{finding}\"\ndecision = \"approved\"\nreason = \"reviewed change\"\n").expect("record");
        match marked {
            Mark::Marked => result.push_str("assertion_change = true\n"),
            Mark::Unmarked => {}
        }
    }
    result
}

fn merged_fixture(before: &Files, after: &Files, marked: Mark) -> (PathBuf, String) {
    let root = fresh_directory(&repository_root(), "h56-a27");
    git(&root, &["init", "--quiet", "-b", "master"]);
    write_files(&root, before);
    git(&root, &["add", "."]);
    git(&root, &["commit", "--quiet", "-m", "base"]);
    git(&root, &["checkout", "--quiet", "-b", "a-27"]);
    write_files(&root, after);
    git(&root, &["add", "."]);
    git(&root, &["commit", "--quiet", "-m", "feature"]);
    git(&root, &["checkout", "--quiet", "master"]);
    let record_path = root.join("harness/test-diff/approvals.toml");
    fs::create_dir_all(record_path.parent().expect("parent")).expect("mkdir");
    fs::write(&record_path, records(before, after, marked)).expect("record");
    git(&root, &["add", "."]);
    git(&root, &["commit", "--quiet", "-m", "owner approval"]);
    let authority = git(&root, &["rev-parse", "HEAD"]);
    git(
        &root,
        &["update-ref", "refs/remotes/origin/master", &authority],
    );
    git(&root, &["merge", "--no-commit", "--no-ff", "a-27"]);
    (root, authority)
}

fn verify_reasons(
    root: &Path,
    authority: &str,
    before: &Files,
    after: &Files,
    findings: &BTreeSet<crate::check_support::Finding<super::Rule>>,
) {
    let remaining = approval::apply_with_authority(
        root,
        authority,
        authority,
        (before, after),
        findings.clone(),
    )
    .expect("verdict");
    assert_eq!(&remaining, findings);
    let reasons = approval::diagnose(
        root,
        Some(authority),
        authority,
        (before, after),
        &remaining,
    )
    .into_reasons();
    assert_eq!(reasons.len(), 6);
    for finding in &remaining {
        assert!(
            reasons.contains_key(&finding.rule.to_string()),
            "{}",
            finding.rule
        );
    }
    assert!(
        reasons.values().any(|reason| reason.contains("struct")),
        "{reasons:#?}"
    );
    assert!(
        reasons
            .get(*FINDINGS.first().expect("finding"))
            .expect("reason")
            .contains("단언")
    );
    assert!(
        reasons
            .get(*FINDINGS.get(1).expect("finding"))
            .expect("reason")
            .contains("보조 함수")
    );
    verify_untrusted_and_hash_reasons(root, authority, before, after, &remaining);
}

fn verify_untrusted_and_hash_reasons(
    root: &Path,
    authority: &str,
    before: &Files,
    after: &Files,
    remaining: &BTreeSet<crate::check_support::Finding<super::Rule>>,
) {
    let unrelated = "0000000000000000000000000000000000000000";
    let untrusted =
        approval::diagnose(root, Some(authority), unrelated, (before, after), remaining)
            .into_reasons();
    assert_eq!(untrusted.len(), 6);
    assert!(
        untrusted
            .values()
            .all(|reason| reason.contains("신뢰 실패"))
    );
    let mut altered = after.clone();
    altered
        .get_mut("crates/testkit-ports/tests/memory.rs")
        .expect("file")
        .push(b' ');
    let hashes = approval::diagnose(
        root,
        Some(authority),
        authority,
        (before, &altered),
        remaining,
    )
    .into_reasons();
    assert!(
        hashes
            .get(*FINDINGS.get(1).expect("finding"))
            .expect("reason")
            .contains("SHA-256 불일치")
    );
}

#[test]
fn h56_a27_actual_files_and_approval_history_explain_every_remaining_finding() {
    let before = files("before");
    let after = files("after");
    let expected: BTreeSet<String> = FINDINGS.into_iter().map(String::from).collect();
    let (findings, _) = compare_files(&before, &after);
    assert_eq!(
        findings
            .iter()
            .map(|f| f.rule.to_string())
            .collect::<BTreeSet<_>>(),
        expected
    );
    for marked in [Mark::Unmarked, Mark::Marked] {
        let (root, authority) = merged_fixture(&before, &after, marked);
        verify_reasons(&root, &authority, &before, &after, &findings);
    }
}
const S06_PATHS: [&str; 2] = [
    "crates/adapters/store-sqlite/src/migration_tests.rs",
    "crates/adapters/store-sqlite/src/tests.rs",
];
const S06_HASHES: [(&str, &str); 2] = [
    (
        "ae361c8482934f37387b6301a1f800dfd64f02e547a52fa9c36d098f7f0a8892",
        "dbf33466db87285fb773ecd6cd8d096ef45f9544d6305ea9b935eb5e4fca99e1",
    ),
    (
        "200945cca683d59e6b3278f7927f0b15be671ea374931d945227abce1a4a5929",
        "d29a3e513e90295682cc083ba69ba288b4c5d8c450e549e54df5a48633801e28",
    ),
];
const S06_FINDINGS: [&str; 2] = [
    "test-diff:modified:store-sqlite migration_tests::fresh_open_records_clock_and_reopen_preserves_ledger",
    "test-diff:modified:store-sqlite tests::raw_key_is_independently_readable_and_debug_is_closed",
];

fn s06_files(version: &str) -> Files {
    let mut result = BTreeMap::from([
        (
            String::from("Cargo.toml"),
            b"[workspace]\nmembers = [\"crates/adapters/store-sqlite\"]\n".to_vec(),
        ),
        (
            String::from("crates/adapters/store-sqlite/Cargo.toml"),
            b"[package]\nname = \"store-sqlite\"\n".to_vec(),
        ),
        (
            String::from("crates/adapters/store-sqlite/src/lib.rs"),
            b"#[cfg(test)] mod migration_tests;\n#[cfg(test)] mod tests;\n".to_vec(),
        ),
    ]);
    for path in S06_PATHS {
        let name = path.rsplit('/').next().expect("filename");
        let source = repository_root().join(format!(
            "harness/test-diff/fixtures/h56-s06/{version}/{name}.txt"
        ));
        result.insert(
            String::from(path),
            fs::read(source).expect("historical fixture"),
        );
    }
    result
}

fn s06_approvals() -> String {
    let mut result = String::new();
    for ((path, (before, after)), finding) in
        S06_PATHS.into_iter().zip(S06_HASHES).zip(S06_FINDINGS)
    {
        write!(result, "[[approval]]\ntask = \"S-06\"\npath = \"{path}\"\nbefore_sha256 = \"{before}\"\nafter_sha256 = \"{after}\"\nfinding = \"{finding}\"\ndecision = \"approved\"\nreason = \"migrations 0006 and 0007 raise the fixed ledger and schema object counts; reviewed by the main agent\"\nassertion_change = true\n").expect("approval");
    }
    result
}

fn s06_merge_fixture(before: &Files, after: &Files) -> (PathBuf, String) {
    let root = fresh_directory(&repository_root(), "h56-s06");
    git(&root, &["init", "--quiet", "-b", "master"]);
    write_files(&root, before);
    git(&root, &["add", "."]);
    git(&root, &["commit", "--quiet", "-m", "base"]);
    git(&root, &["checkout", "--quiet", "-b", "feature"]);
    write_files(&root, after);
    git(&root, &["add", "."]);
    git(&root, &["commit", "--quiet", "-m", "feature"]);
    git(
        &root,
        &["checkout", "--quiet", "-b", "feature-other", "HEAD^"],
    );
    fs::write(root.join("feature-note.txt"), "feature history\n").expect("feature note");
    git(&root, &["add", "."]);
    git(&root, &["commit", "--quiet", "-m", "feature preparation"]);
    git(&root, &["checkout", "--quiet", "feature"]);
    git(
        &root,
        &["merge", "--quiet", "--no-ff", "--no-edit", "feature-other"],
    );
    git(&root, &["checkout", "--quiet", "master"]);
    let path = root.join("harness/test-diff/approvals.toml");
    fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
    fs::write(path, s06_approvals()).expect("write approval");
    git(&root, &["add", "."]);
    git(&root, &["commit", "--quiet", "-m", "owner approval"]);
    git(&root, &["checkout", "--quiet", "-b", "unrelated", "HEAD^"]);
    fs::write(root.join("other.txt"), "other\n").expect("other");
    git(&root, &["add", "."]);
    git(&root, &["commit", "--quiet", "-m", "other work"]);
    git(&root, &["checkout", "--quiet", "master"]);
    git(
        &root,
        &["merge", "--quiet", "--no-ff", "--no-edit", "unrelated"],
    );
    let authority = git(&root, &["rev-parse", "HEAD"]);
    git(
        &root,
        &["update-ref", "refs/remotes/origin/master", &authority],
    );
    git(&root, &["merge", "--no-commit", "--no-ff", "feature"]);
    (root, authority)
}

fn verify_s06_hashes(before: &Files, after: &Files) {
    for (path, (old, new)) in S06_PATHS.into_iter().zip(S06_HASHES) {
        assert_eq!(
            format!("{:x}", Sha256::digest(before.get(path).expect("before"))),
            old
        );
        assert_eq!(
            format!("{:x}", Sha256::digest(after.get(path).expect("after"))),
            new
        );
    }
}

#[test]
fn h56_s06_merge_after_separate_approval_preserves_structural_rejection() {
    let before = s06_files("before");
    let after = s06_files("after");
    verify_s06_hashes(&before, &after);
    let expected: BTreeSet<String> = S06_FINDINGS.into_iter().map(String::from).collect();
    let (findings, _) = compare_files(&before, &after);
    assert_eq!(
        findings
            .iter()
            .map(|item| item.rule.to_string())
            .collect::<BTreeSet<_>>(),
        expected
    );
    let (root, authority) = s06_merge_fixture(&before, &after);
    assert_eq!(
        git(&root, &["rev-parse", "refs/remotes/origin/master"]),
        authority
    );
    assert_eq!(git(&root, &["rev-parse", "MERGE_HEAD"]).len(), 40);
    assert_eq!(
        git(&root, &["rev-list", "--parents", "-n", "1", "HEAD"])
            .split_whitespace()
            .count(),
        3
    );
    assert_eq!(
        git(&root, &["rev-list", "--parents", "-n", "1", "MERGE_HEAD"])
            .split_whitespace()
            .count(),
        3
    );
    let remaining = approval::apply(
        &root,
        &before,
        &after,
        findings.clone(),
        crate::git_base::BaseUse::Commit,
    )
    .expect("verdict");
    assert_eq!(remaining, findings);
    let reasons = approval::diagnose(
        &root,
        Some(&authority),
        &authority,
        (&before, &after),
        &remaining,
    )
    .into_reasons();
    assert_eq!(reasons.len(), 2);
    assert!(
        reasons.values().all(|reason| reason.contains("클로저")),
        "{reasons:#?}"
    );
    let migration_reason = reasons
        .get(*S06_FINDINGS.first().expect("finding"))
        .expect("migration reason");
    assert!(migration_reason.contains("for 반복"), "{migration_reason}");
    assert!(
        migration_reason.contains("오류 전파(?)"),
        "{migration_reason}"
    );
}
#[test]
fn h56_approval_rejection_reports_an_incomplete_finding_set() {
    let before = files("before");
    let after = files("after");
    let (root, _) = merged_fixture(&before, &after, Mark::Marked);
    git(&root, &["merge", "--abort"]);
    let missing = FINDINGS.get(2).expect("finding");
    let revised = records(&before, &after, Mark::Marked).replace(
        *missing,
        "test-diff:support-modified:testkit-ports::review_regressions tests::impl impl Other",
    );
    fs::write(root.join("harness/test-diff/approvals.toml"), revised).expect("record");
    git(&root, &["add", "harness/test-diff/approvals.toml"]);
    git(&root, &["commit", "--quiet", "-m", "incomplete approval"]);
    let authority = git(&root, &["rev-parse", "HEAD"]);
    git(
        &root,
        &["update-ref", "refs/remotes/origin/master", &authority],
    );
    git(&root, &["merge", "--no-commit", "--no-ff", "a-27"]);
    let (findings, _) = compare_files(&before, &after);
    let remaining = approval::apply(
        &root,
        &before,
        &after,
        findings.clone(),
        crate::git_base::BaseUse::Commit,
    )
    .expect("verdict");
    assert_eq!(remaining, findings);
    let reasons = approval::diagnose(
        &root,
        Some(&authority),
        &authority,
        (&before, &after),
        &remaining,
    )
    .into_reasons();
    assert!(
        reasons
            .get(*FINDINGS.get(4).expect("finding"))
            .expect("reason")
            .contains("지적 집합 불일치")
    );
}
#[test]
fn h56_a27_fixture_bytes_match_the_historical_hashes() {
    let before = files("before");
    let after = files("after");
    let hashes = [
        (
            "crates/application/tests/ports_a02_contract.rs",
            "d40f27fcf9971cb92fa2bb53b77c3943a9e0c420f6bdd93d931c7ea37ec4fad3",
            "101f8cd1884b18d0c0516d7aa298f5ab461b0b1f99e06f3123c6378487c0f1dc",
        ),
        (
            "crates/testkit-ports/tests/memory.rs",
            "b38f3d7db8f8d0d088252dd822461148756a64c4fecec632c8cb0a0a2b7c9fce",
            "f178e0286aa072e38696fa3e8f284dc914748c645c66a20b909716c7f98052c9",
        ),
        (
            "crates/testkit-ports/tests/review_regressions.rs",
            "004f7cde08f270612fa4f7fb6a900aeeac96505d31220dcec06c1d8be9f79c92",
            "86cb3559a22f8ae0d5a91df101f68182c40c626e80f5741043fd1a5e8b9e2b01",
        ),
    ];
    for (path, old, new) in hashes {
        assert_eq!(
            format!("{:x}", Sha256::digest(before.get(path).expect("before"))),
            old
        );
        assert_eq!(
            format!("{:x}", Sha256::digest(after.get(path).expect("after"))),
            new
        );
    }
}

#[test]
fn h56_reports_every_candidate_when_one_finding_has_multiple_approvals() {
    let before = s06_files("before");
    let after = s06_files("after");
    let (root, _) = s06_merge_fixture(&before, &after);
    git(&root, &["merge", "--abort"]);
    let mut approvals = s06_approvals();
    let path = S06_PATHS.first().expect("path");
    let after_hash = S06_HASHES.first().expect("hash").1;
    let finding = S06_FINDINGS.first().expect("finding");
    write!(approvals, "[[approval]]\ntask = \"S-06\"\npath = \"{path}\"\nbefore_sha256 = \"ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff\"\nafter_sha256 = \"{after_hash}\"\nfinding = \"{finding}\"\ndecision = \"approved\"\nreason = \"stale approval\"\nassertion_change = true\n").expect("record");
    fs::write(root.join("harness/test-diff/approvals.toml"), approvals).expect("write");
    git(&root, &["add", "harness/test-diff/approvals.toml"]);
    git(&root, &["commit", "--quiet", "-m", "second approval"]);
    let authority = git(&root, &["rev-parse", "HEAD"]);
    git(
        &root,
        &["update-ref", "refs/remotes/origin/master", &authority],
    );
    git(&root, &["merge", "--no-commit", "--no-ff", "feature"]);
    let (findings, _) = compare_files(&before, &after);
    let remaining = approval::apply(
        &root,
        &before,
        &after,
        findings.clone(),
        crate::git_base::BaseUse::Commit,
    )
    .expect("verdict");
    assert_eq!(remaining, findings);
    let reasons = approval::diagnose(
        &root,
        Some(&authority),
        &authority,
        (&before, &after),
        &remaining,
    )
    .into_reasons();
    let reason = reasons.get(*finding).expect("reason");
    assert!(reason.contains("클로저"), "{reason}");
    assert!(reason.contains("SHA-256 불일치"), "{reason}");
}

#[test]
fn h56_git_fixture_ignores_inherited_repository_and_index() {
    let victim = fresh_directory(&repository_root(), "h56-git-victim");
    git(&victim, &["init", "--quiet", "-b", "master"]);
    fs::write(victim.join("victim.txt"), "keep\n").expect("victim");
    git(&victim, &["add", "victim.txt"]);
    git(&victim, &["commit", "--quiet", "-m", "victim"]);
    let index = victim.join(".git/index");
    let before = fs::read(&index).expect("victim index");
    let head = git(&victim, &["rev-parse", "HEAD"]);
    let fixture = fresh_directory(&repository_root(), "h56-git-probe");
    let inherited = [
        (
            "GIT_DIR".to_owned(),
            victim.join(".git").to_string_lossy().into_owned(),
        ),
        (
            "GIT_WORK_TREE".to_owned(),
            victim.to_string_lossy().into_owned(),
        ),
        (
            "GIT_INDEX_FILE".to_owned(),
            index.to_string_lossy().into_owned(),
        ),
    ];
    git_with(&fixture, &["init", "--quiet", "-b", "master"], &inherited);
    fs::write(fixture.join("fixture-only.txt"), "fixture\n").expect("fixture");
    git_with(&fixture, &["add", "fixture-only.txt"], &inherited);
    git_with(
        &fixture,
        &["commit", "--quiet", "-m", "fixture"],
        &inherited,
    );
    assert_eq!(fs::read(&index).expect("victim index"), before);
    assert_eq!(git(&victim, &["rev-parse", "HEAD"]), head);
    assert_eq!(
        fs::read(victim.join("victim.txt")).expect("victim"),
        b"keep\n"
    );
    assert!(!victim.join("fixture-only.txt").exists());
}

#[test]
fn h56_reports_untrusted_approval_without_remote_master() {
    let before = s06_files("before");
    let after = s06_files("after");
    let (root, _) = s06_merge_fixture(&before, &after);
    git(&root, &["update-ref", "-d", "refs/remotes/origin/master"]);
    let (findings, _) = compare_files(&before, &after);
    let mut output = Vec::new();
    let remaining = approval::apply_to(
        &root,
        (&before, &after),
        findings.clone(),
        crate::git_base::BaseUse::Commit,
        &mut output,
    )
    .expect("verdict");
    assert_eq!(remaining, findings);
    let reasons = String::from_utf8(output).expect("utf8");
    assert_eq!(reasons.lines().count(), S06_FINDINGS.len(), "{reasons}");
    for finding in S06_FINDINGS {
        assert!(
            reasons.lines().any(|line| {
                line.contains(finding) && line.contains("승인 이력 또는 기준 커밋 신뢰 실패")
            }),
            "{finding}: {reasons}"
        );
    }
}

#[test]
fn h56_reports_untrusted_approval_only_in_branch_history() {
    let before = s06_files("before");
    let after = s06_files("after");
    let (root, _) = s06_merge_fixture(&before, &after);
    let earlier = git(&root, &["rev-parse", "HEAD^1^1"]);
    git(
        &root,
        &["update-ref", "refs/remotes/origin/master", &earlier],
    );
    let (findings, _) = compare_files(&before, &after);
    let mut output = Vec::new();
    let remaining = approval::apply_to(
        &root,
        (&before, &after),
        findings.clone(),
        crate::git_base::BaseUse::Commit,
        &mut output,
    )
    .expect("verdict");
    assert_eq!(remaining, findings);
    let reasons = String::from_utf8(output).expect("utf8");
    assert_eq!(reasons.lines().count(), S06_FINDINGS.len(), "{reasons}");
    for finding in S06_FINDINGS {
        assert!(
            reasons.lines().any(|line| {
                line.contains(finding) && line.contains("승인 이력 또는 기준 커밋 신뢰 실패")
            }),
            "{finding}: {reasons}"
        );
    }
}

#[test]
fn h56_reports_untrusted_approval_only_in_current_files() {
    let before = s06_files("before");
    let mut after = s06_files("after");
    after.insert(
        String::from("harness/test-diff/approvals.toml"),
        s06_approvals().into_bytes(),
    );
    let root = fresh_directory(&repository_root(), "h56-current-approval");
    git(&root, &["init", "--quiet", "-b", "master"]);
    write_files(&root, &before);
    git(&root, &["add", "."]);
    git(&root, &["commit", "--quiet", "-m", "base"]);
    let authority = git(&root, &["rev-parse", "HEAD"]);
    git(
        &root,
        &["update-ref", "refs/remotes/origin/master", &authority],
    );
    let (findings, _) = compare_files(&before, &after);
    let mut output = Vec::new();
    let remaining = approval::apply_to(
        &root,
        (&before, &after),
        findings.clone(),
        crate::git_base::BaseUse::Commit,
        &mut output,
    )
    .expect("verdict");
    assert_eq!(remaining, findings);
    let reasons = String::from_utf8(output).expect("utf8");
    assert_eq!(reasons.lines().count(), S06_FINDINGS.len(), "{reasons}");
    for finding in S06_FINDINGS {
        assert!(
            reasons.lines().any(|line| {
                line.contains(finding) && line.contains("승인 이력 또는 기준 커밋 신뢰 실패")
            }),
            "{finding}: {reasons}"
        );
    }
}

#[test]
fn h56_keeps_trusted_reasons_when_untrusted_copies_match() {
    let before = s06_files("before");
    let mut after = s06_files("after");
    after.insert(
        String::from("harness/test-diff/approvals.toml"),
        s06_approvals().into_bytes(),
    );
    let (root, _) = s06_merge_fixture(&before, &after);
    let (findings, _) = compare_files(&before, &after);
    let mut output = Vec::new();
    let remaining = approval::apply_to(
        &root,
        (&before, &after),
        findings.clone(),
        crate::git_base::BaseUse::Commit,
        &mut output,
    )
    .expect("verdict");
    assert_eq!(remaining, findings);
    let reasons = String::from_utf8(output).expect("utf8");
    assert_eq!(reasons.lines().count(), S06_FINDINGS.len(), "{reasons}");
    assert!(
        reasons.lines().all(|line| line.contains("클로저")),
        "{reasons}"
    );
    assert!(!reasons.contains("신뢰 실패"), "{reasons}");
}

fn simple_test_files(body: &str) -> Files {
    BTreeMap::from([
        (
            String::from("Cargo.toml"),
            b"[workspace]\nmembers = [\"crates/sample\"]\n".to_vec(),
        ),
        (
            String::from("crates/sample/Cargo.toml"),
            b"[package]\nname = \"sample\"\n".to_vec(),
        ),
        (
            String::from("crates/sample/src/lib.rs"),
            body.as_bytes().to_vec(),
        ),
    ])
}

fn direct_master_base(before: &Files, parent_approval: &str) -> PathBuf {
    let root = fresh_directory(&repository_root(), "h56-direct-master");
    git(&root, &["init", "--quiet", "-b", "master"]);
    write_files(&root, before);
    git(&root, &["add", "."]);
    git(&root, &["commit", "--quiet", "-m", "base"]);
    let approval_path = root.join("harness/test-diff/approvals.toml");
    fs::create_dir_all(approval_path.parent().expect("parent")).expect("mkdir");
    fs::write(&approval_path, parent_approval).expect("write parent approval");
    git(&root, &["add", "harness/test-diff/approvals.toml"]);
    git(&root, &["commit", "--quiet", "-m", "owner approval"]);
    root
}

fn assert_parent_structural_reason(
    root: &Path,
    before: &Files,
    after: &Files,
    findings: &BTreeSet<crate::check_support::Finding<super::Rule>>,
) {
    let mut output = Vec::new();
    let remaining = approval::apply_to(
        root,
        (before, after),
        findings.clone(),
        crate::git_base::BaseUse::Commit,
        &mut output,
    )
    .expect("verdict");
    assert_eq!(&remaining, findings);
    let rendered = String::from_utf8(output).expect("utf8");
    assert!(
        rendered.contains("단언 변경에 명시 승인이 없다"),
        "{rendered}"
    );
    assert!(!rendered.contains("신뢰 실패"), "{rendered}");
}

#[test]
fn h56_diagnose_uses_selected_master_parent_authority_on_direct_master_commit() {
    let before = simple_test_files(
        "#[cfg(test)]\nmod tests {\n    #[test]\n    fn first() { assert_eq!(1, 1); }\n}\n",
    );
    let after = simple_test_files(
        "#[cfg(test)]\nmod tests {\n    #[test]\n    fn first() { assert_eq!(1, 2); }\n}\n",
    );
    let path = "crates/sample/src/lib.rs";
    let old_hash = format!("{:x}", Sha256::digest(before.get(path).expect("before")));
    let new_hash = format!("{:x}", Sha256::digest(after.get(path).expect("after")));
    let parent_approval = format!(
        "[[approval]]\ntask = \"H-56\"\npath = \"{path}\"\nbefore_sha256 = \"{old_hash}\"\nafter_sha256 = \"{new_hash}\"\nfinding = \"test-diff:modified:sample tests::first\"\ndecision = \"approved\"\nreason = \"reviewed change\"\n"
    );
    let root = direct_master_base(&before, &parent_approval);
    let head_approval = format!(
        "{parent_approval}\n[[approval]]\ntask = \"H-56\"\npath = \"crates/sample/src/other.rs\"\nbefore_sha256 = \"{}\"\nafter_sha256 = \"{}\"\nfinding = \"test-diff:modified:sample tests::other\"\ndecision = \"approved\"\nreason = \"unrelated head change\"\n",
        "1".repeat(64),
        "2".repeat(64),
    );
    fs::write(root.join("harness/test-diff/approvals.toml"), head_approval).expect("write head");
    git(&root, &["add", "harness/test-diff/approvals.toml"]);
    git(&root, &["commit", "--quiet", "-m", "unrelated head row"]);
    let head = git(&root, &["rev-parse", "HEAD"]);
    git(&root, &["update-ref", "refs/remotes/origin/master", &head]);
    let (findings, _) = compare_files(&before, &after);
    assert_parent_structural_reason(&root, &before, &after, &findings);

    let parent_commit = git(&root, &["rev-parse", "HEAD^1"]);
    git(&root, &["reset", "--quiet", "--hard", &parent_commit]);
    git(
        &root,
        &["rm", "--quiet", "harness/test-diff/approvals.toml"],
    );
    git(&root, &["commit", "--quiet", "-m", "remove approval file"]);
    let deleted_head = git(&root, &["rev-parse", "HEAD"]);
    git(
        &root,
        &["update-ref", "refs/remotes/origin/master", &deleted_head],
    );
    assert_parent_structural_reason(&root, &before, &after, &findings);
}

fn unapprovable_cases(path: &str) -> [crate::check_support::Finding<super::Rule>; 6] {
    [
        crate::check_support::Finding::whole_file(
            path,
            super::Rule::Removed(String::from("sample tests::first")),
        ),
        crate::check_support::Finding::whole_file(
            "cargo nextest list",
            super::Rule::NotListed(String::from("sample tests::first")),
        ),
        crate::check_support::Finding::whole_file(
            "cargo nextest list",
            super::Rule::CountDecreased {
                base: 2,
                current: 1,
            },
        ),
        crate::check_support::Finding::whole_file(
            path,
            super::Rule::Unreachable(String::from(path)),
        ),
        crate::check_support::Finding::whole_file(
            path,
            super::Rule::SupportRemoved(String::from("sample tests::fn helper")),
        ),
        crate::check_support::Finding::whole_file(
            path,
            super::Rule::Shadowing(String::from("sample tests::fn helper")),
        ),
    ]
}

fn assert_ineligible_reasons(
    root: &Path,
    before: &Files,
    after: &Files,
    cases: &[crate::check_support::Finding<super::Rule>],
) {
    let findings: BTreeSet<_> = cases.iter().cloned().collect();
    let mut output = Vec::new();
    let remaining = approval::apply_to(
        root,
        (before, after),
        findings.clone(),
        crate::git_base::BaseUse::Commit,
        &mut output,
    )
    .expect("verdict");
    assert_eq!(remaining, findings);
    let rendered = String::from_utf8(output).expect("utf8");
    assert_eq!(rendered.lines().count(), cases.len(), "{rendered}");
    for item in cases {
        let name = item.rule.to_string();
        assert!(
            rendered
                .lines()
                .any(|line| line.contains(&name) && line.contains("승인 불가 지적 종류")),
            "{name}: {rendered}"
        );
    }
    assert!(!rendered.contains("신뢰 실패"), "{rendered}");
}

#[test]
fn h56_reports_ineligible_rule_reason_for_all_unapprovable_finding_kinds() {
    let before = simple_test_files(
        "#[cfg(test)]\nmod tests {\n    #[test]\n    fn first() { assert_eq!(1, 1); }\n}\n",
    );
    let after = simple_test_files("#[cfg(test)]\nmod tests {}\n");
    let path = "crates/sample/src/lib.rs";
    let old_hash = format!("{:x}", Sha256::digest(before.get(path).expect("before")));
    let new_hash = format!("{:x}", Sha256::digest(after.get(path).expect("after")));
    let cases = unapprovable_cases(path);
    let mut toml_body = String::new();
    for item in &cases {
        write!(
            toml_body,
            "[[approval]]\ntask = \"H-56\"\npath = \"{path}\"\nbefore_sha256 = \"{old_hash}\"\nafter_sha256 = \"{new_hash}\"\nfinding = \"{}\"\ndecision = \"approved\"\nreason = \"ineligible test\"\n",
            item.rule
        )
        .expect("record");
    }

    let untrusted_root = fresh_directory(&repository_root(), "h56-ineligible-untrusted");
    git(&untrusted_root, &["init", "--quiet", "-b", "master"]);
    write_files(&untrusted_root, &before);
    git(&untrusted_root, &["add", "."]);
    git(&untrusted_root, &["commit", "--quiet", "-m", "base"]);
    let mut current_with_approval = after.clone();
    current_with_approval.insert(
        String::from("harness/test-diff/approvals.toml"),
        toml_body.as_bytes().to_vec(),
    );
    assert_ineligible_reasons(&untrusted_root, &before, &current_with_approval, &cases);

    let trusted_root = direct_master_base(&before, &toml_body);
    let authority = git(&trusted_root, &["rev-parse", "HEAD"]);
    git(
        &trusted_root,
        &["update-ref", "refs/remotes/origin/master", &authority],
    );
    git(&trusted_root, &["checkout", "--quiet", "-b", "branch"]);
    write_files(&trusted_root, &after);
    git(&trusted_root, &["add", "."]);
    git(&trusted_root, &["commit", "--quiet", "-m", "branch"]);
    assert_ineligible_reasons(&trusted_root, &before, &after, &cases);
}

fn single_file_approval_run(
    name: &str,
    before: &Files,
    after: &Files,
    approval_toml: &str,
) -> (usize, String) {
    let root = fresh_directory(&repository_root(), name);
    git(&root, &["init", "--quiet", "-b", "master"]);
    write_files(&root, before);
    git(&root, &["add", "."]);
    git(&root, &["commit", "--quiet", "-m", "base"]);
    let approval_path = root.join("harness/test-diff/approvals.toml");
    fs::create_dir_all(approval_path.parent().expect("parent")).expect("mkdir");
    fs::write(&approval_path, approval_toml).expect("write approval");
    git(&root, &["add", "harness/test-diff/approvals.toml"]);
    git(&root, &["commit", "--quiet", "-m", "owner approval"]);
    let authority = git(&root, &["rev-parse", "HEAD"]);
    git(
        &root,
        &["update-ref", "refs/remotes/origin/master", &authority],
    );
    git(&root, &["checkout", "--quiet", "-b", "branch"]);
    write_files(&root, after);
    git(&root, &["add", "."]);
    git(&root, &["commit", "--quiet", "-m", "branch"]);
    let (findings, _) = compare_files(before, after);
    let mut output = Vec::new();
    let remaining = approval::apply_to(
        &root,
        (before, after),
        findings,
        crate::git_base::BaseUse::Commit,
        &mut output,
    )
    .expect("verdict");
    (remaining.len(), String::from_utf8(output).expect("utf8"))
}

#[test]
fn h56_reports_exact_mismatched_hash_fields_and_missing_file_sides() {
    let before = simple_test_files(
        "#[cfg(test)]\nmod tests {\n    #[test]\n    fn first() { let _x = 1; assert_eq!(1, 1); }\n}\n",
    );
    let after = simple_test_files(
        "#[cfg(test)]\nmod tests {\n    #[test]\n    fn first() { let _x = 2; assert_eq!(1, 1); }\n}\n",
    );
    let path = "crates/sample/src/lib.rs";
    let old_hash = format!("{:x}", Sha256::digest(before.get(path).expect("before")));
    let new_hash = format!("{:x}", Sha256::digest(after.get(path).expect("after")));
    let bad_before = "a".repeat(64);
    let bad_after = "b".repeat(64);
    let finding = "test-diff:modified:sample tests::first";

    let only_before_toml = format!(
        "[[approval]]\ntask = \"H-56\"\npath = \"{path}\"\nbefore_sha256 = \"{bad_before}\"\nafter_sha256 = \"{new_hash}\"\nfinding = \"{finding}\"\ndecision = \"approved\"\nreason = \"check\"\n"
    );
    let (before_rem, before_out) =
        single_file_approval_run("h56-hash-before", &before, &after, &only_before_toml);
    assert_eq!(before_rem, 1);
    assert!(before_out.contains(&format!("path={path}")), "{before_out}");
    assert!(before_out.contains("before_sha256:"), "{before_out}");
    assert!(before_out.contains(&old_hash), "{before_out}");
    assert!(before_out.contains(&bad_before), "{before_out}");
    assert!(!before_out.contains("after_sha256:"), "{before_out}");

    let only_after_toml = format!(
        "[[approval]]\ntask = \"H-56\"\npath = \"{path}\"\nbefore_sha256 = \"{old_hash}\"\nafter_sha256 = \"{bad_after}\"\nfinding = \"{finding}\"\ndecision = \"approved\"\nreason = \"check\"\n"
    );
    let (after_rem, after_out) =
        single_file_approval_run("h56-hash-after", &before, &after, &only_after_toml);
    assert_eq!(after_rem, 1);
    assert!(after_out.contains("after_sha256:"), "{after_out}");
    assert!(after_out.contains(&new_hash), "{after_out}");
    assert!(after_out.contains(&bad_after), "{after_out}");
    assert!(!after_out.contains("before_sha256:"), "{after_out}");

    let wrong_path = "crates/sample/src/missing.rs";
    let missing_toml = format!(
        "[[approval]]\ntask = \"H-56\"\npath = \"{wrong_path}\"\nbefore_sha256 = \"{old_hash}\"\nafter_sha256 = \"{new_hash}\"\nfinding = \"{finding}\"\ndecision = \"approved\"\nreason = \"check\"\n"
    );
    let (missing_rem, missing_out) =
        single_file_approval_run("h56-missing-path", &before, &after, &missing_toml);
    assert_eq!(missing_rem, 1);
    assert!(
        missing_out.contains(&format!("path={wrong_path}")),
        "{missing_out}"
    );
    assert!(missing_out.contains("누락=before·current"), "{missing_out}");
}

#[test]
fn h56_support_approved_assertion_addition_reports_actual_failure_reason() {
    let before = simple_test_files(
        "#[cfg(test)]\nmod tests {\n    fn helper(v: i32) { assert_eq!(v, 1); }\n    #[test]\n    fn first() { helper(1); assert_eq!(1, 1); }\n}\n",
    );
    let path = "crates/sample/src/lib.rs";
    let old_hash = format!("{:x}", Sha256::digest(before.get(path).expect("before")));
    let finding = "test-diff:support-modified:sample tests::fn helper";

    for (label, replacement, marked, expected_substr, forbidden_substr) in [
        (
            "h56-add-trivial",
            "fn helper(v: i32) { assert_eq!(v, 1); assert!(true); }",
            true,
            "자명한 참 단언은 추가할 수 없다",
            "보호된 단언 수 또는 순서가 달라짐",
        ),
        (
            "h56-add-nonstandard",
            "fn helper(v: i32) { assert_eq!(v, 1); check_extra(v); }",
            true,
            "표준 assert 계열 매크로만 허용한다",
            "보호된 단언 수 또는 순서가 달라짐",
        ),
        (
            "h56-add-lost-old",
            "fn helper(v: i32) { assert!(v > 0); assert!(v < 5); }",
            true,
            "기존 보호된 단언 1을 보존하지 못함",
            "보호된 단언 수 또는 순서가 달라짐",
        ),
        (
            "h56-add-unmarked",
            "fn helper(v: i32) { assert_eq!(v, 1); assert!(v < 5); }",
            false,
            "단언 변경에 명시 승인이 없다",
            "보호된 단언 수 또는 순서가 달라짐",
        ),
    ] {
        let after = simple_test_files(&format!(
            "#[cfg(test)]\nmod tests {{\n    {replacement}\n    #[test]\n    fn first() {{ helper(1); assert_eq!(1, 1); }}\n}}\n"
        ));
        let new_hash = format!("{:x}", Sha256::digest(after.get(path).expect("after")));
        let mark_line = if marked {
            "assertion_change = true\n"
        } else {
            ""
        };
        let toml = format!(
            "[[approval]]\ntask = \"H-56\"\npath = \"{path}\"\nbefore_sha256 = \"{old_hash}\"\nafter_sha256 = \"{new_hash}\"\nfinding = \"{finding}\"\ndecision = \"approved\"\nreason = \"check\"\n{mark_line}"
        );
        let (rem, out) = single_file_approval_run(label, &before, &after, &toml);
        assert_eq!(rem, 1, "{label}: {out}");
        assert!(out.contains(expected_substr), "{label}: {out}");
        assert!(!out.contains(forbidden_substr), "{label}: {out}");
        assert!(
            !out.contains(&format!("{finding}: {finding}:")),
            "{label}: {out}"
        );
    }
}

#[test]
fn h56_parse_failure_does_not_misreport_new_helper_reference() {
    let path = "crates/sample/src/lib.rs";

    let before_fn = simple_test_files(
        "#[cfg(test)]\nmod tests {\n    struct S<const N: usize>;\n    impl S<{ 1 }> { fn f() -> i32 { 1 } }\n    #[test]\n    fn first() { assert_eq!(S::<1>::f(), 1); }\n}\n",
    );
    let after_fn = simple_test_files(
        "#[cfg(test)]\nmod tests {\n    struct S<const N: usize>;\n    fn extra_helper() {}\n    impl S<{ 1 }> { fn f() -> i32 { 2 } }\n    #[test]\n    fn first() { assert_eq!(S::<1>::f(), 1); }\n}\n",
    );
    let fn_old_hash = format!("{:x}", Sha256::digest(before_fn.get(path).expect("before")));
    let fn_new_hash = format!("{:x}", Sha256::digest(after_fn.get(path).expect("after")));
    let (fn_findings, _) = compare_files(&before_fn, &after_fn);
    let fn_finding = fn_findings.first().expect("fn finding").rule.to_string();
    let fn_toml = format!(
        "[[approval]]\ntask = \"H-56\"\npath = \"{path}\"\nbefore_sha256 = \"{fn_old_hash}\"\nafter_sha256 = \"{fn_new_hash}\"\nfinding = \"{fn_finding}\"\ndecision = \"approved\"\nreason = \"check\"\nassertion_change = true\n"
    );
    let (fn_rem, fn_out) =
        single_file_approval_run("h56-parse-fail-fn", &before_fn, &after_fn, &fn_toml);
    assert_eq!(fn_rem, 1, "{fn_out}");
    assert!(fn_out.contains("함수 구조를 파싱할 수 없음"), "{fn_out}");
    assert!(!fn_out.contains("새 보조 함수 참조가 추가됨"), "{fn_out}");

    let before_impl = simple_test_files(
        "#[cfg(test)]\nmod tests {\n    struct S<const N: usize>;\n    impl S<{ 1 }> {}\n    #[test]\n    fn first() { assert_eq!(1, 1); }\n}\n",
    );
    let after_impl = simple_test_files(
        "#[cfg(test)]\nmod tests {\n    struct S<const N: usize>;\n    fn extra_helper() {}\n    impl S<{ 2 }> {}\n    #[test]\n    fn first() { assert_eq!(1, 1); }\n}\n",
    );
    let impl_old_hash = format!(
        "{:x}",
        Sha256::digest(before_impl.get(path).expect("before"))
    );
    let impl_new_hash = format!("{:x}", Sha256::digest(after_impl.get(path).expect("after")));
    let (impl_findings, _) = compare_files(&before_impl, &after_impl);
    let mut impl_toml = String::new();
    for item in &impl_findings {
        write!(
            impl_toml,
            "[[approval]]\ntask = \"H-56\"\npath = \"{path}\"\nbefore_sha256 = \"{impl_old_hash}\"\nafter_sha256 = \"{impl_new_hash}\"\nfinding = \"{}\"\ndecision = \"approved\"\nreason = \"check\"\nassertion_change = true\n",
            item.rule
        )
        .expect("toml");
    }
    let (impl_rem, impl_out) =
        single_file_approval_run("h56-parse-fail-impl", &before_impl, &after_impl, &impl_toml);
    assert!(impl_rem >= 1, "{impl_out}");
    assert!(
        impl_out.contains("impl 문맥 변경 또는 파싱 실패"),
        "{impl_out}"
    );
    assert!(
        !impl_out.contains("새 보조 함수 참조가 추가됨"),
        "{impl_out}"
    );
}

#[test]
fn h56_stale_trusted_row_does_not_mask_new_untrusted_branch_row() {
    let before = simple_test_files(
        "#[cfg(test)]\nmod tests {\n    #[test]\n    fn first() { let _x = 1; assert_eq!(1, 1); }\n}\n",
    );
    let mut after = simple_test_files(
        "#[cfg(test)]\nmod tests {\n    #[test]\n    fn first() { let _x = 2; assert_eq!(1, 1); }\n}\n",
    );
    let path = "crates/sample/src/lib.rs";
    let old_hash = format!("{:x}", Sha256::digest(before.get(path).expect("before")));
    let new_hash = format!("{:x}", Sha256::digest(after.get(path).expect("after")));
    let stale_before = "c".repeat(64);
    let finding = "test-diff:modified:sample tests::first";
    let stale_toml = format!(
        "[[approval]]\ntask = \"H-56\"\npath = \"{path}\"\nbefore_sha256 = \"{stale_before}\"\nafter_sha256 = \"{new_hash}\"\nfinding = \"{finding}\"\ndecision = \"approved\"\nreason = \"stale\"\n"
    );
    let branch_toml = format!(
        "[[approval]]\ntask = \"H-56\"\npath = \"{path}\"\nbefore_sha256 = \"{old_hash}\"\nafter_sha256 = \"{new_hash}\"\nfinding = \"{finding}\"\ndecision = \"approved\"\nreason = \"untrusted new row\"\n"
    );
    after.insert(
        String::from("harness/test-diff/approvals.toml"),
        branch_toml.into_bytes(),
    );
    let (rem, out) = single_file_approval_run("h56-stale-untrusted", &before, &after, &stale_toml);
    assert_eq!(rem, 1, "{out}");
    assert!(out.contains("before_sha256:"), "{out}");
    assert!(out.contains("승인 이력 또는 기준 커밋 신뢰 실패"), "{out}");
}

#[test]
fn h56_impl_method_and_assertion_problem_report_specific_causes() {
    let before = simple_test_files(
        "#[cfg(test)]\nmod tests {\n    struct S;\n    impl S { fn check_val(v: i32) { assert_eq!(v, 1); } fn drop_me() {} }\n    #[test]\n    fn first() { S::check_val(1); assert_eq!(1, 1); }\n}\n",
    );
    let after_removed = simple_test_files(
        "#[cfg(test)]\nmod tests {\n    struct S;\n    impl S { fn check_val(v: i32) { assert_eq!(v, 1); } }\n    #[test]\n    fn first() { S::check_val(1); assert_eq!(1, 1); }\n}\n",
    );
    let path = "crates/sample/src/lib.rs";
    let old_hash = format!("{:x}", Sha256::digest(before.get(path).expect("before")));
    let new_hash = format!(
        "{:x}",
        Sha256::digest(after_removed.get(path).expect("after"))
    );
    let impl_finding = "test-diff:support-modified:sample tests::impl impl S";
    let toml_removed = format!(
        "[[approval]]\ntask = \"H-56\"\npath = \"{path}\"\nbefore_sha256 = \"{old_hash}\"\nafter_sha256 = \"{new_hash}\"\nfinding = \"{impl_finding}\"\ndecision = \"approved\"\nreason = \"check\"\nassertion_change = true\n"
    );
    let (removed_rem, removed_out) =
        single_file_approval_run("h56-impl-removed", &before, &after_removed, &toml_removed);
    assert_eq!(removed_rem, 1, "{removed_out}");
    assert!(
        removed_out.contains("impl 메서드 drop_me의 구조 보존 실패: 메서드가 삭제됨"),
        "{removed_out}"
    );

    let after_trivial = simple_test_files(
        "#[cfg(test)]\nmod tests {\n    struct S;\n    impl S { fn check_val(v: i32) { assert_eq!(v, v); } fn drop_me() {} }\n    #[test]\n    fn first() { S::check_val(1); assert_eq!(1, 1); }\n}\n",
    );
    let trivial_hash = format!(
        "{:x}",
        Sha256::digest(after_trivial.get(path).expect("after"))
    );
    let toml_trivial = format!(
        "[[approval]]\ntask = \"H-56\"\npath = \"{path}\"\nbefore_sha256 = \"{old_hash}\"\nafter_sha256 = \"{trivial_hash}\"\nfinding = \"{impl_finding}\"\ndecision = \"approved\"\nreason = \"check\"\nassertion_change = true\n"
    );
    let (trivial_rem, trivial_out) =
        single_file_approval_run("h56-impl-trivial", &before, &after_trivial, &toml_trivial);
    assert_eq!(trivial_rem, 1, "{trivial_out}");
    assert!(
        trivial_out.contains(
            "impl 메서드 check_val의 구조 보존 실패: 보호된 단언 1: 자명한 참 단언은 승인할 수 없다"
        ),
        "{trivial_out}"
    );
}
