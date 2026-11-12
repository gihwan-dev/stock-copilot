use std::collections::BTreeSet;
use std::fs::{self, File};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use crate::harness_workspace_tests::{fresh_directory, generate_lockfile, repository_root};

use super::{
    HOOK_RULES_FILE, HookFinding, HookRules, LEFTHOOK_CONFIG, LEFTHOOK_VARIABLES, ZERO_COMMIT,
    installed_findings, push_base,
};

const EXPECTED_LEFTHOOK: &str = "assert_lefthook_installed: true
min_version: 2.2.2

pre-commit:
  piped: true
  jobs:
    - name: commit-guard
      run: just commit-guard
    - name: fresh-build
      run: just fresh-build
    - name: check
      run: just base=HEAD check

pre-push:
  piped: true
  jobs:
    - name: fresh-build
      run: just fresh-build
    - name: check-full
      run: just push-check
      use_stdin: true
";

const PINNED_VERSION: &str = "2.2.2";

const FOREIGN_CONFIGS: [&str; 29] = [
    "lefthook.yaml",
    "lefthook.toml",
    "lefthook.json",
    "lefthook.jsonc",
    ".lefthook.yml",
    ".lefthook.yaml",
    ".lefthook.toml",
    ".lefthook.json",
    ".lefthook.jsonc",
    ".config/lefthook.yml",
    ".config/lefthook.yaml",
    ".config/lefthook.toml",
    ".config/lefthook.json",
    ".config/lefthook.jsonc",
    "lefthook-local.yml",
    "lefthook-local.yaml",
    "lefthook-local.toml",
    "lefthook-local.json",
    "lefthook-local.jsonc",
    ".lefthook-local.yml",
    ".lefthook-local.yaml",
    ".lefthook-local.toml",
    ".lefthook-local.json",
    ".lefthook-local.jsonc",
    ".config/lefthook-local.yml",
    ".config/lefthook-local.yaml",
    ".config/lefthook-local.toml",
    ".config/lefthook-local.json",
    ".config/lefthook-local.jsonc",
];

const HOOKS: [&str; 2] = ["pre-commit", "pre-push"];

const IDENTITY: [&str; 6] = [
    "-c",
    "user.name=harness",
    "-c",
    "user.email=harness@example.invalid",
    "-c",
    "commit.gpgsign=false",
];

fn isolated_environment() -> Vec<(String, String)> {
    vec![
        (String::from("GIT_CONFIG_GLOBAL"), String::from("/dev/null")),
        (String::from("GIT_CONFIG_NOSYSTEM"), String::from("1")),
    ]
}

fn isolate(command: &mut Command) {
    for variable in LEFTHOOK_VARIABLES {
        command.env_remove(variable);
    }
    command.envs(isolated_environment());
}

fn git_output(directory: &Path, arguments: &[&str]) -> Output {
    let mut command = Command::new("git");
    command
        .arg("-C")
        .arg(directory)
        .args(IDENTITY)
        .args(arguments)
        .stdin(Stdio::null());
    isolate(&mut command);
    command.output().expect("run git")
}

fn git(directory: &Path, arguments: &[&str]) -> String {
    let finished = git_output(directory, arguments);
    assert!(
        finished.status.success(),
        "git {arguments:?}: {}",
        String::from_utf8_lossy(&finished.stderr)
    );
    String::from_utf8_lossy(&finished.stdout).trim().to_owned()
}

fn lefthook_install(directory: &Path) {
    let mut command = Command::new("lefthook");
    command.arg("install").current_dir(directory);
    isolate(&mut command);
    let finished = command.output().expect("run lefthook install");
    assert!(finished.status.success(), "lefthook install");
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Hooks {
    Installed,
    Absent,
}

fn repository(name: &str, hooks: Hooks) -> PathBuf {
    let directory = fresh_directory(&repository_root(), name).join("repository");
    fs::create_dir_all(&directory).expect("create repository");
    git(&directory, &["init", "--quiet", "--initial-branch=main"]);
    fs::copy(
        repository_root().join(LEFTHOOK_CONFIG),
        directory.join(LEFTHOOK_CONFIG),
    )
    .expect("copy lefthook.yml");
    fs::write(directory.join("src.txt"), "clean\n").expect("write source");
    git(&directory, &["add", "--all"]);
    git(
        &directory,
        &["commit", "--quiet", "--no-verify", "-m", "first"],
    );
    if hooks == Hooks::Installed {
        lefthook_install(&directory);
    }
    directory
}

fn rules() -> HookRules {
    HookRules::load(&repository_root()).expect("load hook rules")
}

fn findings_with(repository: &Path, environment: &[(String, String)]) -> BTreeSet<HookFinding> {
    let scratch = repository
        .parent()
        .expect("repository parent")
        .join("expected");
    installed_findings(&rules(), repository, &scratch, environment).expect("installed findings")
}

fn findings(repository: &Path) -> BTreeSet<HookFinding> {
    findings_with(repository, &isolated_environment())
}

fn missing_both() -> BTreeSet<HookFinding> {
    HOOKS
        .into_iter()
        .map(|hook| HookFinding::Missing(String::from(hook)))
        .collect()
}

#[test]
fn lefthook_config_is_exactly_the_expected_text() {
    let text =
        fs::read_to_string(repository_root().join(LEFTHOOK_CONFIG)).expect("read lefthook.yml");
    assert_eq!(text, EXPECTED_LEFTHOOK, "{LEFTHOOK_CONFIG}");
}

#[test]
fn hook_rules_pin_the_researched_version_and_every_foreign_config() {
    let loaded = rules();
    assert_eq!(loaded.version, PINNED_VERSION, "{HOOK_RULES_FILE}");
    assert_eq!(
        loaded.foreign_configs,
        FOREIGN_CONFIGS.into_iter().map(String::from).collect(),
        "{HOOK_RULES_FILE}"
    );
    let output = Command::new("lefthook")
        .arg("version")
        .output()
        .expect("lefthook version");
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        PINNED_VERSION,
        "installed lefthook"
    );
    for broken in [
        "version = \"\"\nforeign-configs = []\n",
        "foreign-configs = []\n",
        "version = \"2.2.2\"\n",
        "version = \"2.2.2\"\nforeign-configs = [\"lefthook.yml\"]\n",
        "version = \"2.2.2\"\nforeign-configs = []\nextra = 1\n",
    ] {
        let table: toml::Table = broken.parse().expect("parse broken rules");
        assert!(HookRules::parse(&table).is_err(), "{broken}");
    }
}

#[test]
fn installed_hooks_pass_in_the_repository_and_its_worktrees() {
    let directory = repository("hooks-installed", Hooks::Installed);
    assert_eq!(findings(&directory), BTreeSet::new(), "main checkout");
    let worktree = directory.parent().expect("parent").join("worktree");
    git(
        &directory,
        &[
            "worktree",
            "add",
            "--quiet",
            "-b",
            "side",
            worktree.to_str().expect("utf-8 path"),
        ],
    );
    assert_eq!(findings(&worktree), BTreeSet::new(), "worktree");
}

#[test]
fn missing_hooks_fail_in_the_repository_and_its_worktrees() {
    let directory = repository("hooks-missing", Hooks::Absent);
    assert_eq!(findings(&directory), missing_both(), "main checkout");
    let worktree = directory.parent().expect("parent").join("worktree");
    git(
        &directory,
        &[
            "worktree",
            "add",
            "--quiet",
            "-b",
            "side",
            worktree.to_str().expect("utf-8 path"),
        ],
    );
    assert_hooks_in_common_directory(&worktree);
    assert_eq!(findings(&worktree), missing_both(), "worktree");
}

fn assert_hooks_in_common_directory(worktree: &Path) {
    let hooks = git(
        worktree,
        &["rev-parse", "--path-format=absolute", "--git-path", "hooks"],
    );
    let common = git(
        worktree,
        &["rev-parse", "--path-format=absolute", "--git-common-dir"],
    );
    assert_eq!(
        PathBuf::from(hooks),
        PathBuf::from(common).join("hooks"),
        "worktree hooks live in the common git directory"
    );
}

#[test]
fn edited_or_unexecutable_hooks_fail() {
    let directory = repository("hooks-edited", Hooks::Installed);
    let hooks = directory.join(".git").join("hooks");
    let pre_commit = hooks.join("pre-commit");
    let mut text = fs::read_to_string(&pre_commit).expect("read hook");
    text.push_str("exit 0\n");
    fs::write(&pre_commit, text).expect("edit hook");
    assert_eq!(
        findings(&directory),
        BTreeSet::from([HookFinding::Modified(String::from("pre-commit"))]),
        "edited hook"
    );
    fs::set_permissions(hooks.join("pre-push"), fs::Permissions::from_mode(0o644))
        .expect("drop execute bit");
    assert_eq!(
        findings(&directory),
        BTreeSet::from([
            HookFinding::Modified(String::from("pre-commit")),
            HookFinding::NotExecutable(String::from("pre-push")),
        ]),
        "unexecutable hook"
    );
    fs::remove_file(hooks.join("pre-push")).expect("remove hook");
    fs::create_dir_all(hooks.join("pre-push")).expect("replace hook with a folder");
    assert!(
        findings(&directory).contains(&HookFinding::Modified(String::from("pre-push"))),
        "folder in place of a hook"
    );
}

#[test]
fn hooks_path_elsewhere_fails_locally_and_globally() {
    let directory = repository("hooks-path", Hooks::Installed);
    git(&directory, &["config", "core.hooksPath", "elsewhere"]);
    let local = findings(&directory);
    assert!(
        local
            .iter()
            .any(|finding| matches_hooks_path(finding, "elsewhere")),
        "{local:?}"
    );
    git(&directory, &["config", "--unset", "core.hooksPath"]);
    assert_eq!(findings(&directory), BTreeSet::new(), "after unsetting");
    let global = directory.parent().expect("parent").join("global-config");
    fs::write(&global, "[core]\n\thooksPath = /tmp/elsewhere-hooks\n").expect("write global");
    let environment = vec![
        (
            String::from("GIT_CONFIG_GLOBAL"),
            global.display().to_string(),
        ),
        (String::from("GIT_CONFIG_NOSYSTEM"), String::from("1")),
    ];
    let reported = findings_with(&directory, &environment);
    assert!(
        reported
            .iter()
            .any(|finding| matches_hooks_path(finding, "elsewhere-hooks")),
        "{reported:?}"
    );
}

fn matches_hooks_path(finding: &HookFinding, fragment: &str) -> bool {
    match finding {
        HookFinding::HooksPath(path) => path.contains(fragment),
        HookFinding::LefthookVersion(_)
        | HookFinding::Missing(_)
        | HookFinding::Modified(_)
        | HookFinding::NotExecutable(_)
        | HookFinding::ForeignConfig(_) => false,
    }
}

#[test]
fn every_other_lefthook_config_file_fails() {
    let directory = repository("hooks-foreign", Hooks::Installed);
    for name in FOREIGN_CONFIGS {
        let path = directory.join(name);
        fs::create_dir_all(path.parent().expect("parent")).expect("create parent");
        fs::write(&path, "{}\n").expect("write foreign config");
        assert_eq!(
            findings(&directory),
            BTreeSet::from([HookFinding::ForeignConfig(String::from(name))]),
            "{name}"
        );
        fs::remove_file(&path).expect("remove foreign config");
    }
}

#[test]
fn other_lefthook_version_fails() {
    let directory = repository("hooks-version", Hooks::Installed);
    let mut other = rules();
    other.version = String::from("0.0.1");
    let scratch = directory.parent().expect("parent").join("expected");
    let reported = installed_findings(&other, &directory, &scratch, &isolated_environment())
        .expect("installed findings");
    assert_eq!(
        reported,
        BTreeSet::from([HookFinding::LefthookVersion(String::from(PINNED_VERSION))]),
        "version mismatch"
    );
}

const STUB_JUSTFILE: &str = "set shell := [\"bash\", \"-euo\", \"pipefail\", \"-c\"]

base := \"\"

commit-guard:
    printf 'commit-guard\\n' >> hook.log

fresh-build:
    printf 'fresh-build\\n' >> hook.log
    [ ! -e fail-fresh ]

check:
    printf 'check base=%s\\n' {{quote(base)}} >> hook.log
    ! grep -q violation src.txt

push-check:
    printf 'push-check\\n' >> hook.log
    cat >> hook.log
    ! grep -q violation src.txt
";

fn hook_log(directory: &Path) -> Vec<String> {
    let path = directory.join("hook.log");
    let lines = fs::read_to_string(&path)
        .map(|text| text.lines().map(String::from).collect())
        .expect("read hook log");
    fs::remove_file(path).expect("reset hook log");
    lines
}

fn commit(directory: &Path, message: &str) -> bool {
    git_output(directory, &["commit", "--quiet", "--all", "-m", message])
        .status
        .success()
}

#[test]
fn commit_hook_runs_the_check_and_rejects_violating_commits() {
    let directory = repository("hooks-commit", Hooks::Installed);
    fs::write(directory.join("justfile"), STUB_JUSTFILE).expect("write stub justfile");
    fs::write(directory.join(".gitignore"), "hook.log\nfail-fresh\n").expect("write ignore");
    git(&directory, &["add", "--all"]);
    assert!(commit(&directory, "stub"), "clean commit is accepted");
    assert_eq!(
        hook_log(&directory),
        ["commit-guard", "fresh-build", "check base=HEAD"],
        "commit hook order"
    );
    let before = git(&directory, &["rev-parse", "HEAD"]);
    fs::write(directory.join("src.txt"), "violation\n").expect("write violation");
    assert!(!commit(&directory, "bad"), "violating commit is rejected");
    assert_eq!(git(&directory, &["rev-parse", "HEAD"]), before, "HEAD kept");
    assert_eq!(
        hook_log(&directory),
        ["commit-guard", "fresh-build", "check base=HEAD"],
        "checked before rejecting"
    );
    fs::write(directory.join("src.txt"), "fixed\n").expect("fix");
    File::create(directory.join("fail-fresh")).expect("make fresh build fail");
    assert!(!commit(&directory, "stale"), "failed clean stops the hook");
    assert_eq!(
        hook_log(&directory),
        ["commit-guard", "fresh-build"],
        "check does not run after a failed clean"
    );
}

#[test]
fn push_hook_runs_check_full_with_the_pushed_refs() {
    let directory = repository("hooks-push", Hooks::Installed);
    let remote = directory.parent().expect("parent").join("remote.git");
    git(
        &directory,
        &[
            "init",
            "--quiet",
            "--bare",
            remote.to_str().expect("utf-8 path"),
        ],
    );
    git(
        &directory,
        &[
            "remote",
            "add",
            "origin",
            remote.to_str().expect("utf-8 path"),
        ],
    );
    fs::write(directory.join("justfile"), STUB_JUSTFILE).expect("write stub justfile");
    fs::write(directory.join(".gitignore"), "hook.log\nfail-fresh\n").expect("write ignore");
    git(&directory, &["add", "--all"]);
    git(
        &directory,
        &["commit", "--quiet", "--no-verify", "-m", "stub"],
    );
    let head = git(&directory, &["rev-parse", "HEAD"]);
    let pushed = git_output(&directory, &["push", "--quiet", "origin", "main"]);
    assert!(pushed.status.success(), "clean push");
    assert_eq!(
        hook_log(&directory),
        [
            String::from("fresh-build"),
            String::from("push-check"),
            format!("refs/heads/main {head} refs/heads/main {ZERO_COMMIT}"),
        ],
        "push hook input"
    );
    fs::write(directory.join("src.txt"), "violation\n").expect("write violation");
    git(
        &directory,
        &["commit", "--quiet", "--all", "--no-verify", "-m", "bad"],
    );
    let rejected = git_output(&directory, &["push", "--quiet", "origin", "main"]);
    assert!(!rejected.status.success(), "violating push is rejected");
    assert_eq!(
        git(&remote, &["rev-parse", "main"]),
        head,
        "remote branch kept"
    );
}

fn commit_file(directory: &Path, name: &str, text: &str) -> String {
    fs::write(directory.join(name), text).expect("write file");
    git(directory, &["add", "--all"]);
    git(directory, &["commit", "--quiet", "--no-verify", "-m", name]);
    git(directory, &["rev-parse", "HEAD"])
}

#[derive(Debug)]
struct Pushes {
    local: PathBuf,
    first: String,
    second: String,
    feature: String,
}

fn pushes(name: &str) -> Pushes {
    let work = fresh_directory(&repository_root(), name);
    let remote = work.join("remote.git");
    let local = work.join("local");
    fs::create_dir_all(&local).expect("create local");
    git(&work, &["init", "--quiet", "--bare", "remote.git"]);
    git(&local, &["init", "--quiet", "--initial-branch=main"]);
    git(
        &local,
        &[
            "remote",
            "add",
            "origin",
            remote.to_str().expect("utf-8 path"),
        ],
    );
    let first = commit_file(&local, "a.txt", "a\n");
    git(
        &local,
        &["push", "--quiet", "--no-verify", "origin", "main"],
    );
    let second = commit_file(&local, "b.txt", "b\n");
    git(&local, &["switch", "--quiet", "-c", "feature"]);
    let feature = commit_file(&local, "c.txt", "c\n");
    Pushes {
        local,
        first,
        second,
        feature,
    }
}

#[test]
fn push_base_uses_the_remote_commit_or_the_fork_point() {
    let setup = pushes("push-base-fork");
    let local = &setup.local;
    git(local, &["switch", "--quiet", "main"]);
    let update = format!(
        "refs/heads/main {} refs/heads/main {}\n",
        setup.second, setup.first
    );
    assert_eq!(push_base(local, &update), Ok(setup.first.clone()), "update");
    git(local, &["switch", "--quiet", "feature"]);
    let fresh = format!(
        "refs/heads/feature {} refs/heads/feature {ZERO_COMMIT}\n",
        setup.feature
    );
    assert_eq!(
        push_base(local, &fresh),
        Ok(setup.first.clone()),
        "new branch"
    );
    let deletion = format!("(delete) {ZERO_COMMIT} refs/heads/old {}\n", setup.first);
    assert_eq!(
        push_base(local, &deletion),
        Ok(setup.feature.clone()),
        "deletion only"
    );
    git(local, &["switch", "--quiet", "--detach", &setup.first]);
    let known = format!(
        "refs/heads/copy {} refs/heads/copy {ZERO_COMMIT}\n",
        setup.first
    );
    assert_eq!(
        push_base(local, &known),
        Ok(setup.first.clone()),
        "commit already on the remote"
    );
}

#[test]
fn push_base_falls_back_to_the_root_without_common_history() {
    let work = fresh_directory(&repository_root(), "push-base-root");
    git(&work, &["init", "--quiet", "--initial-branch=main"]);
    let root = commit_file(&work, "a.txt", "a\n");
    let head = commit_file(&work, "b.txt", "b\n");
    let line = format!("refs/heads/main {head} refs/heads/main {ZERO_COMMIT}\n");
    assert_eq!(push_base(&work, &line), Ok(root), "root commit");
}

#[test]
fn push_base_refuses_ambiguous_or_foreign_pushes() {
    let setup = pushes("push-base-refused");
    let local = &setup.local;
    let not_head = format!(
        "refs/heads/main {} refs/heads/main {}\n",
        setup.second, setup.first
    );
    let unknown = format!(
        "refs/heads/feature {} refs/heads/feature {}\n",
        setup.feature, "1234567890123456789012345678901234567890"
    );
    let two_bases = format!(
        "refs/heads/feature {} refs/heads/feature {ZERO_COMMIT}\nrefs/heads/other {} refs/heads/other {}\n",
        setup.feature, setup.feature, setup.second
    );
    for (label, input) in [
        ("not HEAD", not_head),
        ("unknown remote commit", unknown),
        ("two bases", two_bases),
        ("malformed", String::from("refs/heads/feature\n")),
    ] {
        assert!(push_base(local, &input).is_err(), "{label}");
    }
    git(local, &["switch", "--quiet", "main"]);
    git(local, &["switch", "--quiet", "-c", "left", &setup.first]);
    commit_file(local, "left.txt", "l\n");
    git(local, &["push", "--quiet", "--no-verify", "origin", "left"]);
    git(local, &["switch", "--quiet", "-c", "right", &setup.first]);
    commit_file(local, "right.txt", "r\n");
    git(
        local,
        &["push", "--quiet", "--no-verify", "origin", "right"],
    );
    git(local, &["switch", "--quiet", "-c", "merged"]);
    git(
        local,
        &["merge", "--quiet", "--no-edit", "--no-verify", "left"],
    );
    let merged = git(local, &["rev-parse", "HEAD"]);
    let line = format!("refs/heads/merged {merged} refs/heads/merged {ZERO_COMMIT}\n");
    assert!(push_base(local, &line).is_err(), "two fork points");
}

fn just_recipe(directory: &Path, recipe: &str) -> Output {
    let mut command = Command::new("just");
    command
        .arg("--justfile")
        .arg(repository_root().join("justfile"))
        .arg("--working-directory")
        .arg(directory)
        .arg(recipe)
        .stdin(Stdio::null());
    isolate(&mut command);
    command.output().expect("run just")
}

#[test]
fn commit_guard_refuses_unstaged_and_untracked_changes() {
    let directory = repository("commit-guard", Hooks::Absent);
    fs::write(directory.join(".gitignore"), "ignored.txt\n").expect("write ignore");
    git(&directory, &["add", "--all"]);
    git(
        &directory,
        &["commit", "--quiet", "--no-verify", "-m", "ignore"],
    );
    assert!(
        just_recipe(&directory, "commit-guard").status.success(),
        "clean"
    );
    fs::write(directory.join("ignored.txt"), "x\n").expect("write ignored");
    assert!(
        just_recipe(&directory, "commit-guard").status.success(),
        "ignored files"
    );
    fs::write(directory.join("src.txt"), "changed\n").expect("change tracked");
    let unstaged = just_recipe(&directory, "commit-guard");
    assert!(!unstaged.status.success(), "unstaged change");
    assert!(
        String::from_utf8_lossy(&unstaged.stderr).contains("스테이징하지 않은 변경"),
        "unstaged message"
    );
    git(&directory, &["add", "src.txt"]);
    assert!(
        just_recipe(&directory, "commit-guard").status.success(),
        "staged change"
    );
    fs::write(directory.join("new.txt"), "new\n").expect("write untracked");
    let untracked = just_recipe(&directory, "commit-guard");
    assert!(!untracked.status.success(), "untracked file");
    assert!(
        String::from_utf8_lossy(&untracked.stderr).contains("new.txt"),
        "untracked message"
    );
}

fn run_probe(directory: &Path) -> String {
    let output = Command::new("cargo")
        .args(["run", "--quiet", "--locked", "--manifest-path"])
        .arg(directory.join("Cargo.toml"))
        .stdin(Stdio::null())
        .output()
        .expect("cargo run probe");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

#[test]
fn fresh_build_rebuilds_sources_whose_mtime_was_rewound() {
    let work = fresh_directory(&repository_root(), "fresh-build");
    let source = work.join("probe").join("src").join("main.rs");
    fs::create_dir_all(source.parent().expect("parent")).expect("create probe");
    fs::write(
        work.join("Cargo.toml"),
        "[workspace]\nresolver = \"3\"\nmembers = [\"probe\"]\n",
    )
    .expect("write workspace");
    fs::write(
        work.join("probe").join("Cargo.toml"),
        "[package]\nname = \"probe\"\nversion = \"0.1.0\"\nedition = \"2024\"\npublish = false\n",
    )
    .expect("write probe manifest");
    fs::write(&source, "fn main() {\n    println!(\"old\");\n}\n").expect("write old");
    generate_lockfile(&work.join("Cargo.toml"));
    assert_eq!(run_probe(&work), "old", "first build");
    let written = fs::metadata(&source)
        .and_then(|metadata| metadata.modified())
        .expect("source mtime");
    fs::write(&source, "fn main() {\n    println!(\"new\");\n}\n").expect("write new");
    File::options()
        .write(true)
        .open(&source)
        .and_then(|file| file.set_modified(written))
        .expect("rewind mtime");
    assert_eq!(run_probe(&work), "old", "cargo trusts the rewound mtime");
    let cleaned = just_recipe(&work, "fresh-build");
    assert!(
        cleaned.status.success(),
        "{}",
        String::from_utf8_lossy(&cleaned.stderr)
    );
    assert_eq!(run_probe(&work), "new", "fresh-build forces the rebuild");
}

fn recipe(name: &str) -> Vec<String> {
    let output = Command::new("just")
        .args(["--show", name])
        .current_dir(repository_root())
        .output()
        .expect("just --show");
    assert!(output.status.success(), "recipe {name}");
    String::from_utf8(output.stdout)
        .expect("utf-8 recipe")
        .lines()
        .map(|line| String::from(line.trim()))
        .collect()
}

#[test]
fn check_ends_with_the_new_disables_and_the_hook_installation() {
    let check = recipe("check");
    let tail: Vec<&str> = check
        .iter()
        .rev()
        .take(3)
        .rev()
        .map(String::as_str)
        .collect();
    assert_eq!(
        tail,
        [
            "\"{{ just_executable() }}\" disable-report",
            "\"{{ just_executable() }}\" disable-report-since {{ quote(base) }}",
            "\"{{ just_executable() }}\" hooks-installed",
        ],
        "{check:?}"
    );
    assert_eq!(
        recipe("hooks-installed"),
        [
            "hooks-installed:",
            "cargo run --locked --quiet --package xtask -- hooks-installed"
        ],
        "hooks-installed recipe"
    );
    assert_eq!(
        recipe("fresh-build"),
        [
            "fresh-build:",
            "cargo clean --locked --workspace",
            "cargo llvm-cov clean --locked --workspace"
        ],
        "fresh-build recipe"
    );
    assert_eq!(
        recipe("push-check"),
        [
            "push-check:",
            "base=\"$(cargo run --locked --quiet --package xtask -- push-base)\"; \"{{ just_executable() }}\" base=\"$base\" check-full"
        ],
        "push-check recipe"
    );
}
