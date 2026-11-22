use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

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
    let result = Command::new("git")
        .arg("-C")
        .arg(root)
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
    let reasons = approval::diagnose(root, authority, authority, (before, after), &remaining);
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
    let unrelated = "0000000000000000000000000000000000000000";
    let untrusted = approval::diagnose(root, authority, unrelated, (before, after), &remaining);
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
    let hashes = approval::diagnose(root, authority, authority, (before, &altered), &remaining);
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
