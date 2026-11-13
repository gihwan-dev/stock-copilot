use std::fs;
use std::path::Path;

use crate::git_base::git;
use crate::harness_workspace_tests::{
    fresh_directory, place_git_directory_outside, repository_root,
};

use super::{Verdict, compare, verdict};

const MEMBER: &str = "[package]\nname = \"sample\"\nversion = \"0.1.0\"\nbuild = \"build.rs\"\n\n[dependencies]\nserde = \"=1.0.229\"\n\n[dev-dependencies]\nproptest = \"=1.12.0\"\n";

const ROOT: &str = "[workspace]\nmembers = [\"crates/sample\"]\n\n[workspace.dependencies]\nserde = \"=1.0.229\"\n\n[workspace.lints.rust]\nunsafe_code = \"forbid\"\n";

const DIRECTORY: &str = "crates/sample";

fn members() -> Vec<String> {
    vec![String::from("crates/sample"), String::from("crates/domain")]
}

fn judged(before: &str, from: &str, to: &str) -> Verdict {
    assert!(before.contains(from), "{from:?}");
    let after = before.replacen(from, to, 1);
    verdict(DIRECTORY, &members(), Some(before), Some(after.as_str()))
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
        verdict(DIRECTORY, &members(), Some(MEMBER), Some(MEMBER)),
        Verdict::DependenciesOnly
    );
    assert_eq!(
        verdict(DIRECTORY, &members(), None, None),
        Verdict::DependenciesOnly
    );
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
    assert_eq!(
        verdict(DIRECTORY, &members(), None, Some(MEMBER)),
        Verdict::Outside
    );
    assert_eq!(
        verdict(DIRECTORY, &members(), Some(MEMBER), None),
        Verdict::Outside
    );
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
        compare(&directory, &first, &first, &dependency, "Cargo.toml"),
        Ok(Verdict::DependenciesOnly)
    );
    assert_eq!(
        compare(&directory, &first, &dependency, &build, "Cargo.toml"),
        Ok(Verdict::Outside)
    );
    assert_eq!(
        compare(&directory, &first, &first, &build, "missing/Cargo.toml"),
        Ok(Verdict::DependenciesOnly)
    );
    assert!(
        compare(&directory, &first, "HEAD", &build, "Cargo.toml").is_err(),
        "a revision name is rejected"
    );
}

#[test]
fn registry_versions_and_workspace_paths_are_plain_dependencies() {
    for entry in [
        "rand = \"=0.9.0\"",
        "rand = { version = \"=0.9.0\", default-features = false, features = [\"std\"], optional = true }",
        "random = { package = \"rand\", version = \">=0.9, <0.10\" }",
        "domain = { path = \"../domain\" }",
        "domain = { path = \"../../crates/domain\", version = \"=0.1.0\" }",
        "serde = { workspace = true, features = [\"derive\"] }",
    ] {
        assert_eq!(
            judged(MEMBER, "serde = \"=1.0.229\"", entry),
            Verdict::DependenciesOnly,
            "{entry}"
        );
    }
}

#[test]
fn dependencies_from_outside_the_registry_or_the_repository_need_a_signature() {
    for entry in [
        "evil = { git = \"https://example.com/evil\" }",
        "evil = { git = \"https://example.com/evil\", rev = \"abc\" }",
        "evil = { path = \"../../../outside\" }",
        "evil = { path = \"/tmp/evil\" }",
        "evil = { path = \"../../fixtures/evil\" }",
        "evil = { path = \"../sample/../../fixtures\" }",
        "evil = { version = \"=1.0.0\", registry = \"other\" }",
        "evil = { version = \"=1.0.0\", registry-index = \"https://example.com\" }",
        "evil = \"https://example.com/evil\"",
        "evil = { workspace = false }",
        "evil = 1",
    ] {
        assert_eq!(
            judged(MEMBER, "serde = \"=1.0.229\"", entry),
            Verdict::Outside,
            "{entry}"
        );
    }
    assert_eq!(
        judged(
            ROOT,
            "serde = \"=1.0.229\"",
            "serde = { git = \"https://example.com/serde\" }"
        ),
        Verdict::Outside
    );
}

#[test]
fn patch_replace_profile_build_and_workspace_changes_need_a_signature() {
    for (before, from, to) in [
        (
            ROOT,
            "[workspace.lints.rust]",
            "[replace]\n\"serde:1.0.229\" = { path = \"vendor/serde\" }\n\n[workspace.lints.rust]",
        ),
        (
            ROOT,
            "[workspace.lints.rust]",
            "[profile.release]\ndebug = true\n\n[workspace.lints.rust]",
        ),
        (
            ROOT,
            "members = [\"crates/sample\"]",
            "members = [\"crates/*\"]",
        ),
        (ROOT, "[workspace]", "[workspace]\nresolver = \"2\""),
        (MEMBER, "build = \"build.rs\"\n", ""),
    ] {
        assert_eq!(judged(before, from, to), Verdict::Outside, "{to}");
    }
}

#[test]
fn workspace_members_come_from_the_root_manifest() {
    assert_eq!(
        super::workspace_members(Some(ROOT)),
        vec![String::from("crates/sample")]
    );
    assert_eq!(
        super::workspace_members(Some("[workspace]\nmembers = [\"a/\", \"b\"]\n")),
        vec![String::from("a"), String::from("b")]
    );
    assert!(super::workspace_members(None).is_empty(), "no manifest");
    assert!(
        super::workspace_members(Some("[package]\nname = \"x\"\n")).is_empty(),
        "no workspace"
    );
}
