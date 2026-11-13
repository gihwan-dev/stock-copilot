use std::fs;
use std::path::Path;

use crate::git_base::git;
use crate::harness_workspace_tests::{
    fresh_directory, place_git_directory_outside, repository_root,
};

use super::{Verdict, compare, verdict};

const MEMBER: &str = "[package]\nname = \"sample\"\nversion = \"0.1.0\"\nbuild = \"build.rs\"\n\n[dependencies]\nserde = \"=1.0.229\"\n\n[dev-dependencies]\nproptest = \"=1.12.0\"\n";

const ROOT: &str = "[workspace]\nmembers = [\"crates/sample\"]\n\n[workspace.dependencies]\nserde = \"=1.0.229\"\n\n[workspace.lints.rust]\nunsafe_code = \"forbid\"\n";

fn judged(before: &str, from: &str, to: &str) -> Verdict {
    assert!(before.contains(from), "{from:?}");
    let after = before.replacen(from, to, 1);
    verdict(Some(before), Some(after.as_str()))
}

#[test]
fn dependency_tables_may_change_without_a_signature() {
    assert_eq!(
        judged(
            MEMBER,
            "serde = \"=1.0.229\"",
            "serde = \"=1.0.230\"\nrand = \"=0.9.0\""
        ),
        Verdict::DependenciesOnly
    );
    assert_eq!(
        judged(MEMBER, "proptest = \"=1.12.0\"\n", ""),
        Verdict::DependenciesOnly
    );
    assert_eq!(
        judged(
            MEMBER,
            "[dev-dependencies]",
            "[build-dependencies]\ncc = \"=1.2.0\"\n\n[dev-dependencies]"
        ),
        Verdict::DependenciesOnly
    );
    assert_eq!(
        judged(
            ROOT,
            "serde = \"=1.0.229\"",
            "serde = { version = \"=1.0.229\", features = [\"derive\"] }"
        ),
        Verdict::DependenciesOnly
    );
    for table in ["dev_dependencies", "build_dependencies"] {
        assert_eq!(
            judged(
                MEMBER,
                "[dev-dependencies]",
                &format!("[{table}]\nx = \"=1.0.0\"\n\n[dev-dependencies]")
            ),
            Verdict::DependenciesOnly,
            "{table}"
        );
    }
    assert_eq!(
        verdict(Some(MEMBER), Some(MEMBER)),
        Verdict::DependenciesOnly
    );
    assert_eq!(verdict(None, None), Verdict::DependenciesOnly);
}

#[test]
fn every_other_table_needs_a_signature() {
    for (before, from, to) in [
        (MEMBER, "build = \"build.rs\"", "build = \"other.rs\""),
        (MEMBER, "version = \"0.1.0\"", "version = \"0.2.0\""),
        (
            MEMBER,
            "[dependencies]",
            "[target.'cfg(unix)'.dependencies]\nlibc = \"=0.2.0\"\n\n[dependencies]",
        ),
        (
            MEMBER,
            "[dependencies]",
            "[patch.crates-io]\nserde = { path = \"../serde\" }\n\n[dependencies]",
        ),
        (
            MEMBER,
            "[dependencies]",
            "[features]\ndefault = []\n\n[dependencies]",
        ),
        (
            ROOT,
            "members = [\"crates/sample\"]",
            "members = [\"crates/sample\", \"crates/other\"]",
        ),
        (ROOT, "unsafe_code = \"forbid\"", "unsafe_code = \"allow\""),
        (
            MEMBER,
            "version = \"0.1.0\"",
            "version = \"0.1.0\"\ndescription = \"\"\"\n[dependencies]\n\"\"\"",
        ),
        (MEMBER, "[dependencies]", "[dependencies"),
    ] {
        assert_eq!(judged(before, from, to), Verdict::Outside, "{to}");
    }
    assert_eq!(verdict(None, Some(MEMBER)), Verdict::Outside);
    assert_eq!(verdict(Some(MEMBER), None), Verdict::Outside);
}

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

fn commit(directory: &Path, text: &str, message: &str) -> String {
    fs::write(directory.join("Cargo.toml"), text).expect("write manifest");
    run_git(directory, &["add", "--all"]);
    run_git(directory, &["commit", "--quiet", "-m", message]);
    run_git(directory, &["rev-parse", "HEAD"])
}

#[test]
fn manifest_changes_compares_two_commits_of_a_repository() {
    let directory = fresh_directory(&repository_root(), "manifest-changes");
    run_git(&directory, &["init", "--quiet"]);
    let first = commit(&directory, MEMBER, "first");
    let dependency = commit(
        &directory,
        &MEMBER.replace("=1.0.229", "=1.0.230"),
        "dependency",
    );
    let build = commit(
        &directory,
        &MEMBER
            .replace("=1.0.229", "=1.0.230")
            .replace("build.rs", "x.rs"),
        "build",
    );
    assert_eq!(
        compare(&directory, &first, &dependency, "Cargo.toml"),
        Ok(Verdict::DependenciesOnly)
    );
    assert_eq!(
        compare(&directory, &dependency, &build, "Cargo.toml"),
        Ok(Verdict::Outside)
    );
    assert_eq!(
        compare(&directory, &first, &build, "missing/Cargo.toml"),
        Ok(Verdict::DependenciesOnly)
    );
    assert!(
        compare(&directory, "HEAD", &build, "Cargo.toml").is_err(),
        "a revision name is rejected"
    );
}
