use std::fs;
use std::path::{Path, PathBuf};

use crate::git_base::git;
use crate::harness_workspace_tests::{
    fresh_directory, place_git_directory_outside, repository_root,
};

use super::push_base;

const ZERO: &str = "0000000000000000000000000000000000000000";

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

struct Remote {
    local: PathBuf,
    fork: String,
}

fn with_master(name: &str) -> Remote {
    let work = fresh_directory(&repository_root(), name);
    let local = work.join("local");
    fs::create_dir_all(&local).expect("create local");
    run(&work, &["init", "--quiet", "--bare", "remote.git"]);
    run(&local, &["init", "--quiet", "--initial-branch=master"]);
    let remote = work.join("remote.git");
    run(
        &local,
        &["remote", "add", "origin", remote.to_str().expect("utf-8")],
    );
    let fork = commit(&local, "a.txt");
    run(&local, &["push", "--quiet", "origin", "master"]);
    Remote { local, fork }
}

#[test]
fn branch_pushes_use_the_fork_point_with_origin_master() {
    let remote = with_master("push-base-branch-fork");
    let local = &remote.local;
    run(local, &["switch", "--quiet", "-c", "feature"]);
    let first = commit(local, "b.txt");
    run(local, &["push", "--quiet", "origin", "feature"]);
    let second = commit(local, "c.txt");
    let update = format!("refs/heads/feature {second} refs/heads/feature {first}\n");
    assert_eq!(push_base(local, &update), Ok(remote.fork.clone()), "update");
    let fresh = format!("refs/heads/other {second} refs/heads/other {ZERO}\n");
    assert_eq!(
        push_base(local, &fresh),
        Ok(remote.fork.clone()),
        "new branch"
    );
}

#[test]
fn master_pushes_keep_the_previous_remote_tip() {
    let remote = with_master("push-base-master-tip");
    let local = &remote.local;
    let next = commit(local, "b.txt");
    let update = format!(
        "refs/heads/master {next} refs/heads/master {}\n",
        remote.fork
    );
    assert_eq!(push_base(local, &update), Ok(remote.fork.clone()));
}

#[test]
fn branch_pushes_after_master_moved_use_the_newer_fork_point() {
    let remote = with_master("push-base-moved-master");
    let local = &remote.local;
    let advanced = commit(local, "b.txt");
    run(local, &["push", "--quiet", "origin", "master"]);
    run(local, &["switch", "--quiet", "-c", "feature", &remote.fork]);
    let first = commit(local, "c.txt");
    run(local, &["push", "--quiet", "origin", "feature"]);
    run(local, &["merge", "--quiet", "--no-edit", "master"]);
    let merged = run(local, &["rev-parse", "HEAD"]);
    let update = format!("refs/heads/feature {merged} refs/heads/feature {first}\n");
    assert_eq!(push_base(local, &update), Ok(advanced));
}
