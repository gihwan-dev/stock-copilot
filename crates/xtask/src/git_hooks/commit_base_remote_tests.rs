use std::fs;
use std::path::{Path, PathBuf};

use crate::git_base::git;
use crate::harness_workspace_tests::{
    fresh_directory, place_git_directory_outside, repository_root,
};

use super::commit_base::{MergeBase, commit_base, merge_base_kind};

fn run(directory: &Path, arguments: &[&str]) -> String {
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

fn commit(directory: &Path, name: &str) -> String {
    fs::write(directory.join(name), name).expect("write file");
    run(directory, &["add", "--all"]);
    run(directory, &["commit", "--quiet", "-m", name]);
    run(directory, &["rev-parse", "HEAD"])
}

fn repo_with_origin(name: &str) -> (PathBuf, PathBuf, String) {
    let work = fresh_directory(&repository_root(), name);
    let local = work.join("local");
    let peer = work.join("peer");
    fs::create_dir_all(&local).expect("create local");
    run(&work, &["init", "--quiet", "--bare", "remote.git"]);
    run(&local, &["init", "--quiet", "--initial-branch=master"]);
    let remote = work.join("remote.git");
    run(
        &local,
        &["remote", "add", "origin", remote.to_str().expect("utf-8")],
    );
    let first = commit(&local, "a.txt");
    run(&local, &["push", "--quiet", "origin", "master"]);
    run(
        &work,
        &["clone", "--quiet", remote.to_str().expect("utf-8"), "peer"],
    );
    (local, peer, first)
}

#[test]
fn commit_base_uses_origin_master_only_when_local_master_is_a_strict_ancestor() {
    let (local, peer, first) = repo_with_origin("commit-base-remote-ahead");
    let remote_advanced = commit(&peer, "remote.txt");
    run(&peer, &["push", "--quiet", "origin", "master"]);
    run(&local, &["fetch", "--quiet", "origin"]);
    run(&local, &["switch", "--quiet", "-c", "work", &first]);
    commit(&local, "work.txt");
    run(
        &local,
        &[
            "merge",
            "--quiet",
            "--no-ff",
            "--no-commit",
            "origin/master",
        ],
    );
    assert_eq!(
        merge_base_kind(&local),
        Ok(MergeBase::IncomingDefault(remote_advanced.clone()))
    );
    assert_eq!(commit_base(&local), Ok(remote_advanced.clone()));
    run(&local, &["commit", "--quiet", "-m", "merge origin/master"]);
    assert_eq!(commit_base(&local), Ok(remote_advanced));
}

#[test]
fn commit_base_keeps_local_master_when_local_is_ahead_or_diverged_from_origin() {
    let (local, _peer, first) = repo_with_origin("commit-base-local-ahead");
    let local_advanced = commit(&local, "local.txt");
    run(&local, &["switch", "--quiet", "-c", "work"]);
    commit(&local, "work.txt");
    assert_eq!(commit_base(&local), Ok(local_advanced.clone()));

    let (div_local, div_peer, _) = repo_with_origin("commit-base-diverged");
    let div_local_tip = commit(&div_local, "local-div.txt");
    let _div_remote_tip = commit(&div_peer, "remote-div.txt");
    run(&div_peer, &["push", "--quiet", "origin", "master"]);
    run(&div_local, &["fetch", "--quiet", "origin"]);
    run(&div_local, &["switch", "--quiet", "-c", "work"]);
    commit(&div_local, "work-div.txt");
    assert_eq!(commit_base(&div_local), Ok(div_local_tip));
    assert_ne!(first, local_advanced);
}
