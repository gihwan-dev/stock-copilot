use std::fs;
use std::path::{Path, PathBuf};

use crate::git_base::git;
use crate::harness_workspace_tests::{
    fresh_directory, place_git_directory_outside, repository_root,
};

use super::{
    CONTAINER_RUNTIME_PROBE, ContainerRuntime, PushPlan, PushRecipe, RuntimeProbe,
    detect_container_runtime_with, push_plan,
};

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

fn repo(name: &str) -> (PathBuf, String) {
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
    (local, fork)
}

#[test]
fn push_plan_selects_fast_only_for_single_checked_out_work_branch_without_container_runtime() {
    let (local, fork) = repo("push-check-work-branch");
    run(&local, &["switch", "--quiet", "-c", "h-61"]);
    let head = commit(&local, "b.txt");
    let line = format!("refs/heads/h-61 {head} refs/heads/h-61 {ZERO}\n");
    assert_eq!(
        push_plan(&local, &line, ContainerRuntime::Absent),
        Ok(PushPlan {
            base: fork.clone(),
            recipe: PushRecipe::Fast,
        })
    );
    assert_eq!(
        push_plan(&local, &line, ContainerRuntime::Present),
        Ok(PushPlan {
            base: fork,
            recipe: PushRecipe::Full,
        })
    );
}

#[test]
fn push_plan_keeps_full_for_protected_branches() {
    let (local, fork) = repo("push-check-protected");
    let master_next = commit(&local, "master.txt");
    let master_line = format!("refs/heads/master {master_next} refs/heads/master {fork}\n");
    assert_eq!(
        push_plan(&local, &master_line, ContainerRuntime::Absent)
            .expect("master push")
            .recipe,
        PushRecipe::Full
    );
    for branch in [
        "main",
        "merge/h-61",
        "MERGE/H-62",
        "review/h-61",
        "critic/h-61",
        "t3/deadbeef",
    ] {
        run(&local, &["switch", "--quiet", "-c", branch, &fork]);
        let tip = commit(&local, &format!("{}.txt", branch.replace('/', "_")));
        let line = format!("refs/heads/{branch} {tip} refs/heads/{branch} {ZERO}\n");
        assert_eq!(
            push_plan(&local, &line, ContainerRuntime::Absent)
                .expect(branch)
                .recipe,
            PushRecipe::Full,
            "{branch}"
        );
    }
}

#[test]
fn push_plan_keeps_full_for_renames_deletions_detached_and_multi_refs() {
    let (local, fork) = repo("push-check-shapes");
    run(&local, &["switch", "--quiet", "-c", "h-62", &fork]);
    let tip = commit(&local, "h62.txt");
    let renamed = format!("refs/heads/h-62 {tip} refs/heads/other {ZERO}\n");
    assert_eq!(
        push_plan(&local, &renamed, ContainerRuntime::Absent)
            .expect("rename")
            .recipe,
        PushRecipe::Full
    );
    let multi = format!(
        "refs/heads/h-62 {tip} refs/heads/h-62 {ZERO}\nrefs/heads/other {ZERO} refs/heads/other {fork}\n"
    );
    assert_eq!(
        push_plan(&local, &multi, ContainerRuntime::Absent)
            .expect("multi")
            .recipe,
        PushRecipe::Full
    );
    let tag = format!("refs/tags/v1 {tip} refs/tags/v1 {ZERO}\n");
    assert_eq!(
        push_plan(&local, &tag, ContainerRuntime::Absent)
            .expect("tag")
            .recipe,
        PushRecipe::Full
    );
    let deletion = format!("refs/heads/h-62 {ZERO} refs/heads/h-62 {fork}\n");
    assert_eq!(
        push_plan(&local, &deletion, ContainerRuntime::Absent)
            .expect("deletion")
            .recipe,
        PushRecipe::Full
    );
    assert_eq!(
        push_plan(&local, "", ContainerRuntime::Absent)
            .expect("empty stdin")
            .recipe,
        PushRecipe::Full
    );
    run(&local, &["branch", "h-other", &tip]);
    let other_branch = format!("refs/heads/h-other {tip} refs/heads/h-other {ZERO}\n");
    assert_eq!(
        push_plan(&local, &other_branch, ContainerRuntime::Absent)
            .expect("non-checked-out branch")
            .recipe,
        PushRecipe::Full
    );
    run(&local, &["checkout", "--quiet", "--detach", &tip]);
    let detached = format!("refs/heads/h-62 {tip} refs/heads/h-62 {ZERO}\n");
    assert_eq!(
        push_plan(&local, &detached, ContainerRuntime::Absent)
            .expect("detached")
            .recipe,
        PushRecipe::Full
    );
}

fn detect_with_node(
    program: &str,
    fixed: &Path,
    script: &Path,
    set_env: &[(&str, &str)],
    remove_env: &[&str],
) -> ContainerRuntime {
    let probe = RuntimeProbe {
        program: "node",
        script,
        set_env,
        remove_env,
    };
    detect_container_runtime_with(program, &[fixed], &probe)
}

#[test]
fn container_runtime_detector_reports_absent_and_present_deterministically() {
    let root = repository_root();
    let script = root.join(CONTAINER_RUNTIME_PROBE);
    let dir = fresh_directory(&root, "push-check-docker-detect");
    let missing = dir.join("no-such-socket");
    let present = dir.join("fake-docker");
    let empty_home = dir.join("empty-home");
    fs::create_dir_all(&empty_home).expect("create empty home");
    fs::write(&present, "stub").expect("write stub");
    let home = empty_home.to_str().expect("utf-8 home");
    let absent = "nonexistent-docker-binary-h61";
    let only_home = [("HOME", home)];
    let no_host = ["DOCKER_HOST"];
    assert_eq!(
        detect_with_node(absent, &missing, &script, &only_home, &no_host),
        ContainerRuntime::Absent
    );
    assert_eq!(
        detect_with_node(
            absent,
            &missing,
            &script,
            &[("HOME", home), ("DOCKER_HOST", "   ")],
            &[]
        ),
        ContainerRuntime::Absent
    );
    assert_eq!(
        detect_with_node(absent, &present, &script, &only_home, &no_host),
        ContainerRuntime::Present
    );
    assert_eq!(
        detect_with_node(
            absent,
            &missing,
            &script,
            &[("HOME", home), ("DOCKER_HOST", "unix:///tmp/custom.sock")],
            &[],
        ),
        ContainerRuntime::Present
    );
    assert_eq!(
        detect_with_node("git", &missing, &script, &only_home, &no_host),
        ContainerRuntime::Present
    );
}

#[test]
fn container_runtime_detector_fails_closed_when_probe_fails() {
    let root = repository_root();
    let script = root.join(CONTAINER_RUNTIME_PROBE);
    let dir = fresh_directory(&root, "push-check-docker-fail-closed");
    let missing = dir.join("no-such-socket");
    let empty_home = dir.join("empty-home");
    fs::create_dir_all(&empty_home).expect("create empty home");
    let home = empty_home.to_str().expect("utf-8 home");
    let set_home = [("HOME", home)];
    let broken_program = RuntimeProbe {
        program: "nonexistent-node-binary-h61",
        script: &script,
        set_env: &set_home,
        remove_env: &["DOCKER_HOST"],
    };
    assert_eq!(
        detect_container_runtime_with(
            "nonexistent-docker-binary-h61",
            &[&missing],
            &broken_program
        ),
        ContainerRuntime::Present
    );
    let missing_script = dir.join("missing-probe.mjs");
    assert_eq!(
        detect_with_node(
            "nonexistent-docker-binary-h61",
            &missing,
            &missing_script,
            &[("HOME", home)],
            &["DOCKER_HOST"],
        ),
        ContainerRuntime::Present
    );
}

#[test]
fn container_runtime_detector_checks_home_docker_sockets_and_binaries() {
    let root = repository_root();
    let script = root.join(CONTAINER_RUNTIME_PROBE);
    let dir = fresh_directory(&root, "push-check-docker-home");
    let missing = dir.join("no-such-socket");
    for (label, relative) in [
        ("colima", ".colima/default/docker.sock"),
        ("docker-sock", ".docker/run/docker.sock"),
        ("docker-bin", ".docker/bin/docker"),
        ("orbstack-sock", ".orbstack/run/docker.sock"),
        ("orbstack-bin", ".orbstack/bin/docker"),
        ("rd-sock", ".rd/docker.sock"),
        ("rd-bin", ".rd/bin/docker"),
    ] {
        let home = dir.join(label);
        let file = home.join(relative);
        fs::create_dir_all(file.parent().expect("parent")).expect("create dir");
        fs::write(&file, "stub").expect("write stub");
        let home_text = home.to_str().expect("utf-8 home");
        assert_eq!(
            detect_with_node(
                "nonexistent-docker-binary-h61",
                &missing,
                &script,
                &[("HOME", home_text)],
                &["DOCKER_HOST"],
            ),
            ContainerRuntime::Present,
            "{label}"
        );
    }
}
