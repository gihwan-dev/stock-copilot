use super::commit_base::{WorkBranch, work_branch};
use crate::git_base;
use crate::harness_workspace_tests::{
    fresh_directory, place_git_directory_outside, repository_root,
};
use std::fs;
use std::path::Path;

fn git(root: &Path, args: &[&str]) -> String {
    place_git_directory_outside(root, args);
    let mut command = git_base::git(root);
    command.args([
        "-c",
        "user.name=harness",
        "-c",
        "user.email=harness@example.invalid",
        "-c",
        "commit.gpgsign=false",
    ]);
    command.args(args);
    let result = command.output().expect("git command");
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    String::from_utf8_lossy(&result.stdout).trim().to_owned()
}

#[test]
fn master_detached_symbolic_alias_and_worktree_are_classified_from_git() {
    let root = fresh_directory(&repository_root(), "h46-branches").join("repo");
    fs::create_dir_all(&root).expect("repository");
    git(&root, &["init", "--initial-branch=master"]);
    fs::write(root.join("file"), "kept").expect("file");
    git(&root, &["add", "."]);
    git(
        &root,
        &["-c", "core.hooksPath=/dev/null", "commit", "-m", "first"],
    );
    assert_eq!(work_branch(&root), Ok(WorkBranch::Other));
    for name in ["topic", "master-topic", "check-fast"] {
        git(&root, &["checkout", "-b", name]);
        assert_eq!(work_branch(&root), Ok(WorkBranch::NamedWork), "{name}");
    }
    git(&root, &["symbolic-ref", "HEAD", "refs/heads/Master"]);
    assert_eq!(work_branch(&root), Ok(WorkBranch::Other));
    git(&root, &["symbolic-ref", "HEAD", "refs/heads/master"]);
    git(&root, &["checkout", "--detach"]);
    assert_eq!(work_branch(&root), Ok(WorkBranch::Other));
    let tip = git(&root, &["rev-parse", "HEAD"]);
    git(&root, &["update-ref", "refs/custom/work", &tip]);
    git(&root, &["symbolic-ref", "HEAD", "refs/custom/work"]);
    assert_eq!(work_branch(&root), Ok(WorkBranch::Other));
    git(&root, &["symbolic-ref", "HEAD", "refs/heads/master"]);
    let linked = root.parent().expect("parent").join("linked");
    git(
        &root,
        &[
            "worktree",
            "add",
            "-b",
            "linked-work",
            linked.to_str().expect("utf8"),
        ],
    );
    assert_eq!(work_branch(&linked), Ok(WorkBranch::NamedWork));
    assert_eq!(work_branch(&root), Ok(WorkBranch::Other));
    for variable in git_base::REDIRECTING_VARIABLES {
        let mut command =
            git_base::git_with(&root, &[(String::from(variable), String::from("injected"))]);
        command.args(["symbolic-ref", "HEAD"]);
        let output = git_base::output(command, "symbolic-ref").expect("sanitized git");
        assert_eq!(
            String::from_utf8_lossy(&output).trim(),
            "refs/heads/master",
            "{variable}"
        );
    }
}
