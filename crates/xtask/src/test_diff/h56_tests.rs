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
    );
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
        approval::diagnose(root, Some(authority), unrelated, (before, after), remaining);
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
    );
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
    );
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
    );
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
    );
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
    for finding in S06_FINDINGS {
        assert!(
            reasons.lines().any(|line| {
                line.contains(finding) && line.contains("승인 이력 또는 기준 커밋 신뢰 실패")
            }),
            "{finding}: {reasons}"
        );
    }
}
