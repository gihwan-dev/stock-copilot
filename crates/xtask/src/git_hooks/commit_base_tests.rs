use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use crate::git_base::REDIRECTING_VARIABLES;
use crate::harness_workspace_tests::{
    fresh_directory, place_git_directory_outside, repository_root,
};

use super::commit_base::commit_base;
use super::{LEFTHOOK_CONFIG, LEFTHOOK_VARIABLES};

const BASE_IDENTITY: [&str; 6] = [
    "-c",
    "user.name=harness",
    "-c",
    "user.email=harness@example.invalid",
    "-c",
    "commit.gpgsign=false",
];

const BASE_STUB_RECIPES: &str = "fresh-build:
    printf 'fresh-build\\n' >> hook.log

check:
    printf 'check\\n' >> hook.log
    git show {{quote(base)}}:tests.txt | while IFS= read -r kept; do grep -qxF \"$kept\" tests.txt; done
";

fn isolated(command: &mut Command) {
    for variable in LEFTHOOK_VARIABLES.into_iter().chain(REDIRECTING_VARIABLES) {
        command.env_remove(variable);
    }
    command
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1");
}

fn run_git(directory: &Path, arguments: &[&str]) -> Output {
    place_git_directory_outside(directory, arguments);
    let mut command = Command::new("git");
    command
        .arg("-C")
        .arg(directory)
        .args(BASE_IDENTITY)
        .args(arguments)
        .stdin(Stdio::null());
    isolated(&mut command);
    command.output().expect("run git")
}

fn git_text(directory: &Path, arguments: &[&str]) -> String {
    let finished = run_git(directory, arguments);
    assert!(
        finished.status.success(),
        "git {arguments:?}: {}",
        String::from_utf8_lossy(&finished.stderr)
    );
    String::from_utf8_lossy(&finished.stdout).trim().to_owned()
}

fn commit_all(directory: &Path, message: &str, verify: &str) -> bool {
    run_git(
        directory,
        &["commit", "--quiet", "--all", verify, "-m", message],
    )
    .status
    .success()
}

fn base_repository(name: &str, branch: &str) -> PathBuf {
    let directory = fresh_directory(&repository_root(), name).join("repository");
    fs::create_dir_all(&directory).expect("create repository");
    git_text(
        &directory,
        &["init", "--quiet", &format!("--initial-branch={branch}")],
    );
    fs::write(directory.join("tests.txt"), "kept\n").expect("write tests");
    git_text(&directory, &["add", "--all"]);
    assert!(
        commit_all(&directory, "first", "--no-verify"),
        "first commit"
    );
    directory
}

fn real_recipe(name: &str) -> String {
    let justfile = fs::read_to_string(repository_root().join("justfile")).expect("read justfile");
    let header = format!("{name}:\n");
    let start = justfile.find(&header).expect("recipe in justfile");
    justfile
        .get(start..)
        .expect("recipe text")
        .lines()
        .take_while(|line| !line.is_empty())
        .fold(String::new(), |mut body, line| {
            body.push_str(line);
            body.push('\n');
            body
        })
}

fn hooked_repository(name: &str) -> PathBuf {
    let directory = base_repository(name, "master");
    let real = repository_root().join("justfile");
    let justfile = format!(
        "set shell := [\"bash\", \"-euo\", \"pipefail\", \"-c\"]\nset positional-arguments\n\nbase := \"\"\n\ncommit-guard index=\"\":\n    just --justfile '{0}' --working-directory . commit-guard \"$1\"\n\ncommit-base:\n    just --justfile '{0}' --working-directory . commit-base\n\n{1}\n{BASE_STUB_RECIPES}",
        real.display(),
        real_recipe("commit-check")
    );
    fs::write(directory.join("justfile"), justfile).expect("write stub justfile");
    fs::write(directory.join(".gitignore"), "hook.log\n").expect("write ignore");
    fs::copy(
        repository_root().join(LEFTHOOK_CONFIG),
        directory.join(LEFTHOOK_CONFIG),
    )
    .expect("copy lefthook.yml");
    git_text(&directory, &["add", "--all"]);
    assert!(commit_all(&directory, "stub", "--no-verify"), "stub commit");
    let mut install = Command::new("lefthook");
    install.arg("install").current_dir(&directory);
    isolated(&mut install);
    assert!(
        install.output().expect("run lefthook").status.success(),
        "lefthook install"
    );
    directory
}

#[test]
fn commit_base_is_head_on_master_and_the_fork_point_on_a_branch() {
    let directory = base_repository("commit-base-branch", "master");
    let head = git_text(&directory, &["rev-parse", "HEAD"]);
    assert_eq!(commit_base(&directory), Ok(head.clone()), "on master");
    git_text(&directory, &["switch", "--quiet", "-c", "work"]);
    fs::write(directory.join("tests.txt"), "kept\nadded\n").expect("add test");
    assert!(
        commit_all(&directory, "branch", "--no-verify"),
        "branch commit"
    );
    assert_eq!(commit_base(&directory), Ok(head), "fork point on a branch");
}

#[test]
fn commit_base_fails_without_master() {
    let directory = base_repository("commit-base-missing", "main");
    let found = commit_base(&directory);
    assert!(
        found
            .as_ref()
            .is_err_and(|failure| failure.contains("master")),
        "{found:?}"
    );
}

#[test]
fn branch_commits_may_change_their_own_tests_but_not_tests_from_master() {
    let directory = hooked_repository("commit-base-hook");
    git_text(&directory, &["switch", "--quiet", "-c", "work"]);
    fs::write(directory.join("tests.txt"), "kept\nadded\n").expect("add test");
    assert!(
        commit_all(&directory, "add", "--verify"),
        "adding a test passes"
    );
    fs::write(directory.join("tests.txt"), "kept\nadded again\n").expect("change own test");
    assert!(
        commit_all(&directory, "revise", "--verify"),
        "changing a test added on the branch passes"
    );
    let before = git_text(&directory, &["rev-parse", "HEAD"]);
    fs::write(directory.join("tests.txt"), "changed\nadded again\n").expect("change master test");
    assert!(
        !commit_all(&directory, "weaken", "--verify"),
        "changing a test from master is rejected"
    );
    assert_eq!(
        git_text(&directory, &["rev-parse", "HEAD"]),
        before,
        "HEAD kept"
    );
}

#[test]
fn commit_hook_without_master_is_rejected() {
    let directory = hooked_repository("commit-base-hook-no-master");
    git_text(&directory, &["branch", "--quiet", "-m", "master", "main"]);
    fs::write(directory.join("tests.txt"), "kept\nadded\n").expect("add test");
    assert!(
        !commit_all(&directory, "add", "--verify"),
        "no master means no commit"
    );
}
