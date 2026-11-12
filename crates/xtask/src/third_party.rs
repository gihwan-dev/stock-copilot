use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};

use crate::harness_workspace::{copy_tree, fresh_directory, generate_lockfile, repository_root};

const ADVISORY_DB_DIRECTORY: &str = "advisory-db-3157b0e258782691";

#[derive(Debug)]
struct DenyRun {
    status: ExitStatus,
    error_codes: BTreeSet<String>,
    text: String,
}

fn prepared_case(name: &str) -> (PathBuf, PathBuf, PathBuf) {
    let root = repository_root();
    let directory = fresh_directory(&root, &format!("deny-{name}"));
    copy_tree(&root.join("harness").join("deny"), &directory);
    let case = directory.join(name);
    generate_lockfile(&case.join("Cargo.toml"));
    (root, directory, case)
}

fn run_deny(case: &Path, config: &Path, checks: &[&str]) -> DenyRun {
    let output = Command::new("cargo")
        .args(["deny", "--locked", "--offline", "--color", "never"])
        .arg("--manifest-path")
        .arg(case.join("Cargo.toml"))
        .arg("--config")
        .arg(config)
        .arg("check")
        .args(checks)
        .current_dir(case)
        .output()
        .expect("run cargo deny");
    let text = String::from_utf8(output.stderr).expect("utf-8 cargo deny output");
    let error_codes = text
        .lines()
        .filter_map(|line| line.strip_prefix("error["))
        .filter_map(|rest| rest.split_once(']'))
        .map(|(code, _)| String::from(code))
        .collect();
    DenyRun {
        status: output.status,
        error_codes,
        text,
    }
}

fn assert_rejected(run: &DenyRun, code: &str, subject: &str) {
    assert!(!run.status.success(), "cargo deny passed:\n{}", run.text);
    assert_eq!(
        run.error_codes,
        BTreeSet::from([String::from(code)]),
        "{}",
        run.text
    );
    assert!(
        run.text.contains(subject),
        "{subject} missing:\n{}",
        run.text
    );
}

fn git(repository: &Path, arguments: &[&str]) {
    let status = Command::new("git")
        .arg("-C")
        .arg(repository)
        .args([
            "-c",
            "user.name=harness",
            "-c",
            "user.email=harness@invalid",
        ])
        .args(["-c", "commit.gpgsign=false"])
        .args(arguments)
        .status()
        .expect("run git");
    assert!(status.success(), "git {arguments:?} failed");
}

fn config_with_local_advisory_db(root: &Path, directory: &Path, case: &Path) -> PathBuf {
    let databases = directory.join("advisory-dbs");
    let repository = databases.join(ADVISORY_DB_DIRECTORY);
    copy_tree(&case.join("advisory-db"), &repository);
    git(&repository, &["init", "--quiet"]);
    git(&repository, &["add", "--all"]);
    git(
        &repository,
        &["commit", "--quiet", "--message", "advisory-db"],
    );
    let mut config: toml::Table = fs::read_to_string(root.join("deny.toml"))
        .expect("read deny.toml")
        .parse()
        .expect("parse deny.toml");
    config
        .get_mut("advisories")
        .and_then(toml::Value::as_table_mut)
        .expect("advisories table")
        .insert(
            String::from("db-path"),
            toml::Value::from(databases.display().to_string()),
        );
    let path = directory.join("deny.toml");
    fs::write(
        &path,
        toml::to_string(&config).expect("serialize deny.toml"),
    )
    .expect("write deny.toml");
    path
}

#[test]
fn listed_dependency_passes() {
    let (root, _, case) = prepared_case("listed-dependency");
    let run = run_deny(
        &case,
        &root.join("deny.toml"),
        &["bans", "licenses", "sources"],
    );
    assert!(run.status.success(), "{}", run.text);
    assert!(run.error_codes.is_empty(), "{}", run.text);
}

#[test]
fn unlisted_direct_dependency_fails() {
    let (root, _, case) = prepared_case("unlisted-direct");
    let run = run_deny(&case, &root.join("deny.toml"), &["bans"]);
    assert_rejected(&run, "not-allowed", "crate 'harness-unlisted = 0.1.0'");
}

#[test]
fn unlisted_transitive_dependency_fails() {
    let (root, _, case) = prepared_case("unlisted-transitive");
    let run = run_deny(&case, &root.join("deny.toml"), &["bans"]);
    assert_rejected(&run, "not-allowed", "crate 'harness-unlisted = 0.1.0'");
}

#[test]
fn unlisted_build_script_fails() {
    let (root, _, case) = prepared_case("unlisted-build-script");
    let run = run_deny(&case, &root.join("deny.toml"), &["bans"]);
    assert_rejected(
        &run,
        "build-script-not-allowed",
        "crate 'serde_repr = 0.1.20' has a build script",
    );
}

#[test]
fn rejected_license_fails() {
    let (root, _, case) = prepared_case("rejected-license");
    let run = run_deny(&case, &root.join("deny.toml"), &["licenses"]);
    assert_rejected(&run, "rejected", "GPL-3.0-only");
}

#[test]
fn vulnerable_dependency_fails() {
    let (root, directory, case) = prepared_case("vulnerable-dependency");
    let config = config_with_local_advisory_db(&root, &directory, &case);
    let run = run_deny(&case, &config, &["advisories"]);
    assert_rejected(&run, "vulnerability", "RUSTSEC-2099-0001");
}

#[test]
fn check_recipe_runs_cargo_deny() {
    let output = Command::new("just")
        .args(["--show", "check"])
        .current_dir(repository_root())
        .output()
        .expect("just --show check");
    let recipe = String::from_utf8(output.stdout).expect("utf-8 recipe");
    assert!(
        recipe
            .lines()
            .any(|line| line.trim() == "cargo deny --locked check"),
        "{recipe}"
    );
}
