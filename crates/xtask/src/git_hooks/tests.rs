use std::collections::BTreeSet;
use std::fs::{self, File};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use crate::git_base::REDIRECTING_VARIABLES;
use crate::harness_workspace_tests::{
    fresh_directory, generate_lockfile, place_git_directory_outside, repository_root,
};

use super::{
    HOOK_RULES_FILE, HookFinding, HookRules, LEFTHOOK_CONFIG, LEFTHOOK_VARIABLES, ZERO_COMMIT,
    installed_findings, push_base,
};

const EXPECTED_LEFTHOOK: &str = "assert_lefthook_installed: true
min_version: 2.2.2

pre-commit:
  piped: true
  jobs:
    - name: fresh-build
      run: just fresh-build
    - name: commit-guard
      run: just commit-guard \"$GIT_INDEX_FILE\"
    - name: check
      run: just commit-check

pre-merge-commit:
  piped: true
  jobs:
    - name: fresh-build
      run: just fresh-build
    - name: commit-guard
      run: just commit-guard \"$GIT_INDEX_FILE\"
    - name: check
      run: just commit-check

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

const HOOKS: [&str; 3] = ["pre-commit", "pre-merge-commit", "pre-push"];

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
    for variable in LEFTHOOK_VARIABLES.into_iter().chain(REDIRECTING_VARIABLES) {
        command.env_remove(variable);
    }
    command.envs(isolated_environment());
}

fn git_output(directory: &Path, arguments: &[&str]) -> Output {
    place_git_directory_outside(directory, arguments);
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
    let root = crate::harness_workspace_tests::unique_directory(&repository_root(), name);
    let directory = fresh_directory(&root, name).join("repository");
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

const STUB_RECIPES: &str = "commit-check:
    \"{{just_executable()}}\" base=HEAD check

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

fn stub_justfile() -> String {
    let real = repository_root().join("justfile");
    format!(
        "set shell := [\"bash\", \"-euo\", \"pipefail\", \"-c\"]\nset positional-arguments\n\nbase := \"\"\n\ncommit-guard index=\"\":\n    printf 'commit-guard\\n' >> hook.log\n    just --justfile '{}' --working-directory . commit-guard \"$1\"\n\n{STUB_RECIPES}",
        real.display()
    )
}

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

fn stub_repository(name: &str) -> PathBuf {
    let directory = repository(name, Hooks::Installed);
    fs::write(directory.join("justfile"), stub_justfile()).expect("write stub justfile");
    fs::write(directory.join(".gitignore"), "hook.log\nfail-fresh\n").expect("write ignore");
    fs::write(directory.join("other.txt"), "other\n").expect("write other");
    git(&directory, &["add", "--all"]);
    git(
        &directory,
        &["commit", "--quiet", "--no-verify", "-m", "stub"],
    );
    directory
}

#[test]
fn commit_hook_runs_the_check_and_rejects_violating_commits() {
    let directory = stub_repository("hooks-commit");
    fs::write(directory.join("src.txt"), "changed\n").expect("change source");
    assert!(commit(&directory, "clean"), "clean commit is accepted");
    assert_eq!(
        hook_log(&directory),
        ["fresh-build", "commit-guard", "check base=HEAD"],
        "commit hook order"
    );
    let before = git(&directory, &["rev-parse", "HEAD"]);
    fs::write(directory.join("src.txt"), "violation\n").expect("write violation");
    assert!(!commit(&directory, "bad"), "violating commit is rejected");
    assert_eq!(git(&directory, &["rev-parse", "HEAD"]), before, "HEAD kept");
    assert_eq!(
        hook_log(&directory),
        ["fresh-build", "commit-guard", "check base=HEAD"],
        "checked before rejecting"
    );
    fs::write(directory.join("src.txt"), "fixed\n").expect("fix");
    File::create(directory.join("fail-fresh")).expect("make fresh build fail");
    assert!(!commit(&directory, "stale"), "failed clean stops the hook");
    assert_eq!(
        hook_log(&directory),
        ["fresh-build"],
        "nothing runs after a failed clean"
    );
}

#[test]
fn commit_hook_refuses_partial_commits_and_untracked_files() {
    let directory = stub_repository("hooks-partial");
    let before = git(&directory, &["rev-parse", "HEAD"]);
    fs::write(directory.join("src.txt"), "first\n").expect("change source");
    fs::write(directory.join("other.txt"), "violation\n").expect("change other");
    let partial = git_output(
        &directory,
        &["commit", "--quiet", "-m", "partial", "--", "src.txt"],
    );
    assert!(!partial.status.success(), "partial commit is rejected");
    assert_eq!(
        hook_log(&directory),
        ["fresh-build", "commit-guard"],
        "guard stops it"
    );
    fs::write(directory.join("other.txt"), "other\n").expect("restore other");
    fs::write(directory.join("new.txt"), "new\n").expect("write untracked");
    assert!(
        !commit(&directory, "untracked"),
        "untracked file is rejected"
    );
    assert_eq!(
        hook_log(&directory),
        ["fresh-build", "commit-guard"],
        "guard stops it"
    );
    assert_eq!(git(&directory, &["rev-parse", "HEAD"]), before, "HEAD kept");
    fs::remove_file(directory.join("new.txt")).expect("remove untracked");
    assert!(commit(&directory, "all"), "commit --all is accepted");
}

#[test]
fn push_hook_runs_check_full_with_the_pushed_refs() {
    let directory = stub_repository("hooks-push");
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
fn commit_guard_ignores_build_outputs_listed_in_the_real_gitignore() {
    let directory = repository("commit-guard-outputs", Hooks::Absent);
    fs::copy(
        repository_root().join(".gitignore"),
        directory.join(".gitignore"),
    )
    .expect("copy .gitignore");
    git(&directory, &["add", ".gitignore"]);
    git(
        &directory,
        &["commit", "--quiet", "--no-verify", "-m", "ignore"],
    );
    for output in [
        "apps/desktop/dist/index.html",
        "apps/desktop/node_modules/.vite/vitest/results.json",
        "node_modules/.cache/knip/cache.json",
        "target/debug/xtask",
        "apps/desktop/src-tauri/gen/schemas/acl.json",
    ] {
        let path = directory.join(output);
        fs::create_dir_all(path.parent().expect("parent")).expect("create output folder");
        fs::write(&path, "output\n").expect("write output");
    }
    let guarded = just_recipe(&directory, "commit-guard");
    assert!(
        guarded.status.success(),
        "{}",
        String::from_utf8_lossy(&guarded.stderr)
    );
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
        String::from_utf8_lossy(&unstaged.stderr).contains("commit-guard:changed: src.txt"),
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
    let cleaned = just_recipe(&work, "fresh-cargo");
    assert!(
        cleaned.status.success(),
        "{}",
        String::from_utf8_lossy(&cleaned.stderr)
    );
    assert_eq!(run_probe(&work), "new", "fresh-build forces the rebuild");
}

#[test]
fn just_drops_git_variables_that_redirect_to_another_repository() {
    let justfile = fs::read_to_string(repository_root().join("justfile")).expect("read justfile");
    let unexported: BTreeSet<&str> = justfile
        .lines()
        .filter_map(|line| line.strip_prefix("unexport "))
        .map(str::trim)
        .collect();
    for variable in REDIRECTING_VARIABLES {
        assert!(
            unexported.contains(variable),
            "justfile must unexport {variable}"
        );
    }
    let clean = repository("guard-clean", Hooks::Absent);
    let dirty = repository("guard-dirty", Hooks::Absent);
    fs::write(dirty.join("src.txt"), "unstaged\n").expect("dirty change");
    let mut command = Command::new("just");
    command
        .arg("--justfile")
        .arg(repository_root().join("justfile"))
        .arg("--working-directory")
        .arg(&clean)
        .arg("commit-guard")
        .stdin(Stdio::null());
    isolate(&mut command);
    command
        .env("GIT_DIR", dirty.join(".git"))
        .env("GIT_WORK_TREE", &dirty)
        .env("GIT_INDEX_FILE", dirty.join(".git").join("index"));
    let finished = command.output().expect("run commit-guard");
    assert!(
        finished.status.success(),
        "{}",
        String::from_utf8_lossy(&finished.stderr)
    );
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

const FRESH_NODE_RECIPE: [&str; 2] = [
    "fresh-node:",
    "pnpm install --frozen-lockfile --force --ignore-scripts",
];

#[test]
fn check_ends_with_the_new_disables_and_the_hook_installation() {
    let check = recipe("check-audited");
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
        ["fresh-build: fresh-cargo fresh-node"],
        "fresh-build recipe"
    );
    assert_eq!(
        recipe("fresh-cargo"),
        [
            "fresh-cargo:",
            "cargo clean --locked --workspace",
            "cargo llvm-cov clean --locked --workspace"
        ],
        "fresh-cargo recipe"
    );
    assert_eq!(recipe("fresh-node"), FRESH_NODE_RECIPE, "fresh-node recipe");
    assert_eq!(
        recipe("commit-guard"),
        [
            "commit-guard index=\"\":",
            "cargo run --locked --quiet --manifest-path \"{{ justfile_directory() }}/Cargo.toml\" --package xtask -- commit-guard \"$1\""
        ],
        "commit-guard recipe"
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

fn guard_stderr(directory: &Path) -> String {
    let guarded = just_recipe(directory, "commit-guard");
    assert!(!guarded.status.success(), "commit-guard must refuse");
    String::from_utf8_lossy(&guarded.stderr).into_owned()
}

fn staged_blob(directory: &Path, text: &str) -> String {
    let blob = directory.parent().expect("parent").join("weakened.txt");
    fs::write(&blob, text).expect("write weakened blob");
    git(
        directory,
        &["hash-object", "-w", blob.to_str().expect("utf-8 path")],
    )
}

#[test]
fn commit_guard_refuses_content_that_lives_only_in_the_index() {
    for flag in ["--skip-worktree", "--assume-unchanged"] {
        let directory = repository(&format!("guard-index{flag}"), Hooks::Absent);
        let blob = staged_blob(&directory, "weakened\n");
        git(
            &directory,
            &[
                "update-index",
                "--cacheinfo",
                &format!("100644,{blob},src.txt"),
            ],
        );
        let changed = guard_stderr(&directory);
        assert!(
            changed.contains("commit-guard:changed: src.txt"),
            "{changed}"
        );
        git(&directory, &["update-index", flag, "src.txt"]);
        let flagged = guard_stderr(&directory);
        assert!(
            flagged.contains("commit-guard:flagged: src.txt")
                && flagged.contains("commit-guard:changed: src.txt"),
            "{flag}: {flagged}"
        );
    }
}

#[test]
fn commit_guard_refuses_a_flag_even_when_contents_match() {
    let directory = repository("guard-flag-only", Hooks::Absent);
    git(&directory, &["update-index", "--skip-worktree", "src.txt"]);
    let flagged = guard_stderr(&directory);
    assert!(
        flagged.contains("commit-guard:flagged: src.txt")
            && !flagged.contains("commit-guard:changed"),
        "{flagged}"
    );
}

#[test]
fn commit_guard_uses_only_the_root_gitignore() {
    let directory = repository("guard-hidden", Hooks::Absent);
    fs::write(directory.join(".gitignore"), "ignored.txt\n").expect("write root ignore");
    git(&directory, &["add", ".gitignore"]);
    git(
        &directory,
        &["commit", "--quiet", "--no-verify", "-m", "ignore"],
    );
    fs::write(directory.join("ignored.txt"), "x\n").expect("write ignored");
    assert!(
        just_recipe(&directory, "commit-guard").status.success(),
        "root .gitignore applies"
    );
    let exclude = directory.join(".git").join("info").join("exclude");
    fs::create_dir_all(exclude.parent().expect("parent")).expect("create info");
    fs::write(&exclude, "hidden.txt\n").expect("write info exclude");
    fs::write(directory.join("hidden.txt"), "x\n").expect("write hidden");
    let excluded = guard_stderr(&directory);
    assert!(
        excluded.contains("commit-guard:untracked: hidden.txt"),
        "{excluded}"
    );
    fs::remove_file(directory.join("hidden.txt")).expect("remove hidden");
    fs::create_dir_all(directory.join("sub")).expect("create sub");
    fs::write(directory.join("sub").join(".gitignore"), "*\n").expect("write sub ignore");
    fs::write(directory.join("sub").join("hidden.rs"), "fn x() {}\n").expect("write sub file");
    let nested = guard_stderr(&directory);
    assert!(
        nested.contains("commit-guard:untracked: sub/hidden.rs")
            && nested.contains("commit-guard:untracked: sub/.gitignore"),
        "{nested}"
    );
    let listed = crate::git_base::listed_paths(&directory).expect("listed paths");
    for hidden in ["sub/hidden.rs", "sub/.gitignore"] {
        assert!(listed.contains(hidden), "{hidden} is listed for the checks");
    }
    fs::write(directory.join("hidden.txt"), "x\n").expect("write hidden again");
    let changed = crate::git_base::changed_paths(&directory, "HEAD").expect("changed paths");
    assert!(changed.contains("hidden.txt"), "{changed:?}");
    assert!(!changed.contains("ignored.txt"), "{changed:?}");
}

#[test]
fn merge_commits_run_the_check_and_reject_violations() {
    let directory = stub_repository("hooks-merge");
    git(&directory, &["switch", "--quiet", "-c", "side"]);
    fs::write(directory.join("src.txt"), "violation\n").expect("write violation");
    git(
        &directory,
        &["commit", "--quiet", "--all", "--no-verify", "-m", "bad"],
    );
    git(&directory, &["switch", "--quiet", "main"]);
    fs::write(directory.join("other.txt"), "main change\n").expect("change other");
    git(
        &directory,
        &["commit", "--quiet", "--all", "--no-verify", "-m", "main"],
    );
    let before = git(&directory, &["rev-parse", "HEAD"]);
    let merged = git_output(
        &directory,
        &["merge", "--quiet", "--no-ff", "-m", "merge", "side"],
    );
    assert!(!merged.status.success(), "violating merge is rejected");
    assert_eq!(git(&directory, &["rev-parse", "HEAD"]), before, "HEAD kept");
    assert_eq!(
        hook_log(&directory),
        ["fresh-build", "commit-guard", "check base=HEAD"],
        "merge commit hook order"
    );
}

fn install_node_fixture(work: &Path) {
    fs::copy(repository_root().join(".npmrc"), work.join(".npmrc")).expect("copy .npmrc");
    assert!(
        Command::new("node")
            .arg(repository_root().join("harness/git-hooks/node-fixture.mjs"))
            .arg(work)
            .status()
            .expect("write pinned Node fixture")
            .success()
    );
    let cache = work.join("cache");
    assert!(!cache.exists(), "pnpm metadata cache starts empty");
    let mut install = Command::new("pnpm");
    install
        .args([
            "--config.trust-lockfile=true",
            "install",
            "--offline",
            "--frozen-lockfile",
            "--silent",
        ])
        .env("pnpm_config_cache_dir", &cache)
        .env("npm_config_cache_dir", &cache)
        .current_dir(work)
        .stdin(Stdio::null());
    assert!(
        install.status().expect("pnpm install").success(),
        "offline install"
    );
}

#[test]
fn fresh_node_restores_a_replaced_tool() {
    let work = fresh_directory(&repository_root(), "fresh-node");
    install_node_fixture(&work);
    let cache = work.join("cache");
    let shim = work.join("node_modules").join(".bin").join("prettier");
    let genuine = fs::read(&shim).expect("read shim");
    fs::remove_file(&shim).expect("unlink shim");
    fs::write(&shim, "#!/bin/sh\nexit 0\n").expect("replace shim");
    let module = work
        .join("node_modules")
        .join("prettier")
        .join("package.json");
    let package = fs::read(&module).expect("read prettier package");
    fs::remove_file(&module).expect("unlink prettier package");
    fs::write(&module, "{}\n").expect("replace prettier package");
    let mut restore = Command::new("just");
    restore
        .arg("--justfile")
        .arg(repository_root().join("justfile"))
        .arg("--working-directory")
        .arg(&work)
        .arg("fresh-node")
        .env("pnpm_config_cache_dir", &cache)
        .env("npm_config_cache_dir", &cache)
        .env("pnpm_config_trust_lockfile", "true")
        .env("pnpm_config_offline", "true")
        .env("npm_config_offline", "true")
        .stdin(Stdio::null());
    isolate(&mut restore);
    let restored = restore.output().expect("run fresh-node");
    assert!(
        restored.status.success(),
        "{}",
        String::from_utf8_lossy(&restored.stderr)
    );
    assert_eq!(
        fs::read(&shim).expect("read shim"),
        genuine,
        "shim restored"
    );
    assert_eq!(
        fs::read(&module).expect("read prettier package"),
        package,
        "package file restored"
    );
    fs::remove_dir_all(&work).expect("remove Node fixture");
}

#[test]
fn missing_base_commit_reports_the_git_cause() {
    let directory = repository("base-cause", Hooks::Absent);
    let missing = "1234567890123456789012345678901234567890";
    let failure = crate::git_base::resolve_base(&directory, missing).expect_err("missing base");
    assert!(
        failure.contains(missing) && failure.contains("fatal:"),
        "{failure}"
    );
}

#[test]
fn commit_guard_reads_output_while_it_writes_input_larger_than_a_pipe() {
    let directory = repository("guard-large-listing", Hooks::Absent);
    let lines = 40_000;
    let line = "0".repeat(64);
    let input = format!("{line}\n").repeat(lines);
    let echoed = crate::commit_guard::hashed(&directory, Command::new("cat"), input.as_bytes())
        .expect("echo");
    assert_eq!(echoed.len(), lines);
    assert!(echoed.iter().all(|found| found == &line));
}

const H44_TESTS: &str =
    "#[test]\nfn approved() { assert_eq!(1, 1); }\n#[test]\nfn protected() { assert_eq!(2, 2); }\n";
const H44_APPROVED: &str =
    "#[test]\nfn approved() { assert_eq!(3, 3); }\n#[test]\nfn protected() { assert_eq!(2, 2); }\n";

fn h44_repository(name: &str) -> PathBuf {
    let directory = repository(name, Hooks::Absent);
    git(&directory, &["branch", "-m", "master"]);
    fs::create_dir_all(directory.join("fixture/src")).expect("source folder");
    fs::write(
        directory.join("fixture/Cargo.toml"),
        "[package]\nname = \"h44\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .expect("manifest");
    fs::write(
        directory.join("Cargo.toml"),
        "[workspace]\nmembers = [\"fixture\"]\nresolver = \"3\"\n",
    )
    .expect("workspace");
    commit_file(&directory, "fixture/src/lib.rs", H44_TESTS);
    generate_lockfile(&directory.join("Cargo.toml"));
    commit_file(&directory, ".gitignore", "target/\n");
    git(&directory, &["switch", "-c", "work"]);
    directory
}

fn h44_diverged(name: &str) -> PathBuf {
    let directory = h44_repository(name);
    commit_file(&directory, "work.txt", "work\n");
    git(&directory, &["switch", "master"]);
    commit_file(&directory, "fixture/src/lib.rs", H44_APPROVED);
    git(&directory, &["switch", "work"]);
    directory
}

fn h44_begin(directory: &Path, revision: &str) {
    git(directory, &["merge", "--no-ff", "--no-commit", revision]);
}

fn h44_findings(
    directory: &Path,
    base: &str,
) -> BTreeSet<crate::check_support::Finding<crate::test_diff::Rule>> {
    let resolved =
        crate::git_base::resolve_base_for(directory, base, crate::git_base::BaseUse::Commit)
            .expect("resolve comparison base");
    crate::test_diff::compare_files(
        &crate::test_diff::base_files(directory, &resolved).expect("base files"),
        &crate::test_diff::current_files(directory).expect("current files"),
    )
    .0
}

#[test]
fn h44_incoming_approved_tests_use_master_tip_and_pass_real_test_diff() {
    let directory = h44_diverged("h44-incoming");
    let old = git(&directory, &["merge-base", "HEAD", "master"]);
    let tip = git(&directory, &["rev-parse", "master"]);
    h44_begin(&directory, "master");
    assert!(!h44_findings(&directory, &old).is_empty());
    assert_eq!(super::commit_base::commit_base(&directory), Ok(tip.clone()));
    assert_eq!(
        crate::git_base::resolve_base_for(&directory, &tip, crate::git_base::BaseUse::Commit),
        Ok(tip.clone())
    );
    assert!(
        crate::test_diff::check_for(&directory, &tip, crate::git_base::BaseUse::Commit)
            .expect("test-diff")
            .is_empty()
    );
}

#[test]
fn h44_work_branch_test_changes_remain_violations_during_merge() {
    let directory = h44_repository("h44-work-change");
    commit_file(
        &directory,
        "fixture/src/lib.rs",
        &H44_TESTS.replace("2, 2", "4, 4"),
    );
    git(&directory, &["switch", "master"]);
    commit_file(&directory, "approved.txt", "approved\n");
    git(&directory, &["switch", "work"]);
    h44_begin(&directory, "master");
    let base = super::commit_base::commit_base(&directory).expect("base");
    assert!(!h44_findings(&directory, &base).is_empty());
}

#[test]
fn h44_conflict_resolution_cannot_change_existing_tests() {
    let directory = h44_repository("h44-conflict");
    commit_file(
        &directory,
        "fixture/src/lib.rs",
        &H44_TESTS.replace("1, 1", "4, 4"),
    );
    git(&directory, &["switch", "master"]);
    commit_file(&directory, "fixture/src/lib.rs", H44_APPROVED);
    git(&directory, &["switch", "work"]);
    assert!(
        !git_output(&directory, &["merge", "--no-ff", "--no-commit", "master"])
            .status
            .success()
    );
    fs::write(
        directory.join("fixture/src/lib.rs"),
        H44_APPROVED.replace("2, 2", "5, 5"),
    )
    .expect("resolve conflict");
    git(&directory, &["add", "--all"]);
    let base = super::commit_base::commit_base(&directory).expect("base");
    assert_eq!(base, git(&directory, &["rev-parse", "master"]));
    assert!(!h44_findings(&directory, &base).is_empty());
}

#[test]
fn h44_merging_work_into_master_keeps_the_old_verdict() {
    let directory = h44_repository("h44-reverse");
    commit_file(
        &directory,
        "fixture/src/lib.rs",
        &H44_TESTS.replace("2, 2", "4, 4"),
    );
    let work = git(&directory, &["rev-parse", "HEAD"]);
    git(&directory, &["switch", "master"]);
    let head = git(&directory, &["rev-parse", "HEAD"]);
    h44_begin(&directory, "work");
    assert_eq!(
        super::commit_base::commit_base(&directory),
        Ok(head.clone())
    );
    assert!(!h44_findings(&directory, &head).is_empty());
    h44_assert_rejected(&directory, &work);
}

#[test]
fn h44_non_tip_and_other_branch_merges_do_not_open_ancestry() {
    let directory = h44_diverged("h44-non-tip");
    let stale = git(&directory, &["rev-parse", "master"]);
    git(&directory, &["switch", "master"]);
    commit_file(&directory, "later.txt", "later\n");
    git(&directory, &["switch", "work"]);
    let old = git(&directory, &["merge-base", "HEAD", "master"]);
    h44_begin(&directory, &stale);
    assert_eq!(super::commit_base::commit_base(&directory), Ok(old.clone()));
    h44_assert_rejected(&directory, &stale);
    git(&directory, &["merge", "--abort"]);
    git(&directory, &["branch", "other", &stale]);
    h44_begin(&directory, "other");
    assert_eq!(super::commit_base::commit_base(&directory), Ok(old));
    h44_assert_rejected(&directory, &stale);
}

#[test]
fn h44_octopus_and_detached_merges_do_not_open_ancestry() {
    let directory = h44_diverged("h44-octopus");
    let old = git(&directory, &["merge-base", "HEAD", "master"]);
    let tip = git(&directory, &["rev-parse", "master"]);
    h44_begin(&directory, "master");
    let merge_path = git(
        &directory,
        &[
            "rev-parse",
            "--path-format=absolute",
            "--git-path",
            "MERGE_HEAD",
        ],
    );
    fs::write(&merge_path, format!("{tip}\n{old}\n")).expect("octopus heads");
    assert_eq!(super::commit_base::commit_base(&directory), Ok(old.clone()));
    h44_assert_rejected(&directory, &tip);
    git(&directory, &["merge", "--abort"]);
    git(&directory, &["checkout", "--detach"]);
    h44_begin(&directory, "master");
    assert_eq!(super::commit_base::commit_base(&directory), Ok(old));
    h44_assert_rejected(&directory, &tip);
}

#[test]
fn h44_after_merge_normal_commits_keep_approved_tests_out_of_review() {
    let directory = h44_diverged("h44-after-merge");
    let tip = git(&directory, &["rev-parse", "master"]);
    h44_begin(&directory, "master");
    git(&directory, &["commit", "--quiet", "-m", "merge"]);
    commit_file(&directory, "work.txt", "next\n");
    assert_eq!(super::commit_base::commit_base(&directory), Ok(tip.clone()));
    let findings = h44_findings(&directory, &tip);
    assert!(findings.is_empty(), "{findings:?}");
}

#[test]
fn h44_master_tip_is_not_accepted_outside_its_exact_merge() {
    let directory = h44_diverged("h44-closed");
    let tip = git(&directory, &["rev-parse", "master"]);
    h44_assert_rejected(&directory, &tip);
    h44_begin(&directory, "master");
    let unrelated = git(&directory, &["rev-parse", "HEAD"]);
    assert_eq!(
        crate::git_base::resolve_base(&directory, &unrelated),
        Ok(unrelated)
    );
    git(&directory, &["merge", "--abort"]);
    h44_assert_rejected(&directory, &tip);
}

#[test]
fn h44_malformed_merge_heads_and_unrelated_bases_are_closed() {
    let directory = h44_diverged("h44-malformed");
    let old = git(&directory, &["merge-base", "HEAD", "master"]);
    let tip = git(&directory, &["rev-parse", "master"]);
    git(&directory, &["switch", "-c", "unrelated"]);
    let unrelated = commit_file(&directory, "unrelated.txt", "unrelated\n");
    git(&directory, &["switch", "work"]);
    h44_begin(&directory, "master");
    h44_assert_rejected(&directory, &unrelated);
    let merge_path = git(
        &directory,
        &[
            "rev-parse",
            "--path-format=absolute",
            "--git-path",
            "MERGE_HEAD",
        ],
    );
    for malformed in ["", "HEAD\n", "invalid\n", &format!("{tip}\n\n")] {
        fs::write(&merge_path, malformed).expect("malformed merge heads");
        assert_eq!(super::commit_base::commit_base(&directory), Ok(old.clone()));
        h44_assert_rejected(&directory, &tip);
    }
}

#[test]
fn h44_linked_worktree_reads_its_own_merge_state() {
    let directory = h44_diverged("h44-linked");
    let tip = git(&directory, &["rev-parse", "master"]);
    let linked = directory.parent().expect("parent").join("linked");
    git(
        &directory,
        &[
            "worktree",
            "add",
            "-b",
            "linked-work",
            linked.to_str().expect("path"),
        ],
    );
    h44_begin(&linked, "master");
    assert_eq!(super::commit_base::commit_base(&linked), Ok(tip.clone()));
    assert_eq!(
        crate::git_base::resolve_base_for(&linked, &tip, crate::git_base::BaseUse::Commit),
        Ok(tip.clone())
    );
    h44_assert_rejected(&directory, &tip);
}

fn h44_assert_rejected(directory: &Path, base: &str) {
    let failure = crate::git_base::resolve_base(directory, base).expect_err("nonancestor rejected");
    assert!(failure.contains("HEAD의 조상이 아니다"), "{failure}");
}

fn h44_review_quit_merge(directory: &Path) {
    h44_begin(directory, "master");
    git(directory, &["merge", "--quit"]);
}

#[test]
fn h44_review_normal_commit_with_clean_merge_index_keeps_work_changes_visible() {
    let directory = h44_repository("h44-review-normal-index");
    commit_file(
        &directory,
        "fixture/src/lib.rs",
        &H44_TESTS.replace("2, 2", "9, 9"),
    );
    git(&directory, &["switch", "master"]);
    commit_file(&directory, "approved.txt", "approved\n");
    let tip = git(&directory, &["rev-parse", "HEAD"]);
    git(&directory, &["switch", "work"]);
    h44_review_quit_merge(&directory);
    assert_eq!(super::commit_base::commit_base(&directory), Ok(tip.clone()));
    let findings = h44_findings(&directory, &tip);
    assert!(
        findings.iter().any(
            |finding| finding.to_string().contains("test-diff:modified:")
                && finding.to_string().contains("protected")
        ),
        "{findings:?}"
    );
    crate::git_base::resolve_base(&directory, &tip).expect_err("ordinary validation stays strict");
    assert_eq!(
        crate::git_base::resolve_base_for(&directory, &tip, crate::git_base::BaseUse::Commit),
        Ok(tip)
    );
}

#[test]
fn h44_review_empty_index_on_descendant_keeps_the_existing_base() {
    let directory = h44_repository("h44-review-empty");
    let tip = git(&directory, &["rev-parse", "master"]);
    commit_file(&directory, "work.txt", "work\n");
    let head = git(&directory, &["rev-parse", "HEAD"]);
    assert_eq!(super::commit_base::commit_base(&directory), Ok(tip.clone()));
    assert_eq!(git(&directory, &["merge-base", "HEAD", "master"]), tip);
    assert_eq!(
        git(&directory, &["write-tree"]),
        git(&directory, &["rev-parse", &format!("{head}^{{tree}}")])
    );
    fs::write(directory.join("work.txt"), "staged\n").expect("stage ordinary edit");
    git(&directory, &["add", "--all"]);
    assert_eq!(super::commit_base::commit_base(&directory), Ok(tip));
}

#[test]
fn h44_review_clean_index_accepts_approved_incoming_changes_without_metadata() {
    let directory = h44_diverged("h44-review-clean-index");
    let tip = git(&directory, &["rev-parse", "master"]);
    let old = git(&directory, &["merge-base", "HEAD", "master"]);
    assert_eq!(super::commit_base::commit_base(&directory), Ok(old.clone()));
    h44_review_quit_merge(&directory);
    assert_eq!(super::commit_base::commit_base(&directory), Ok(tip.clone()));
    assert!(h44_findings(&directory, &tip).is_empty());
    fs::write(directory.join("work.txt"), "injected\n").expect("change index tree");
    git(&directory, &["add", "--all"]);
    assert_eq!(super::commit_base::commit_base(&directory), Ok(old));
}

#[test]
fn h44_review_push_and_ci_reject_nonancestor_during_merge() {
    let directory = h44_diverged("h44-review-push");
    let tip = git(&directory, &["rev-parse", "master"]);
    let head = git(&directory, &["rev-parse", "HEAD"]);
    h44_begin(&directory, "master");
    let input = format!("refs/heads/work {head} refs/heads/work {tip}\n");
    let failure = push_base(&directory, &input).expect_err("push refuses pending parent");
    assert!(failure.contains("HEAD의 조상이 아니다"), "{failure}");
    h44_assert_rejected(&directory, &tip);
    let ci_failure = crate::test_diff::check(&directory, &tip).expect_err("CI stays strict");
    assert!(ci_failure.contains("HEAD의 조상이 아니다"), "{ci_failure}");
    assert_eq!(
        crate::git_base::BaseUse::parse("ancestor"),
        Ok(crate::git_base::BaseUse::Ancestor)
    );
    assert_eq!(
        crate::git_base::BaseUse::parse("commit"),
        Ok(crate::git_base::BaseUse::Commit)
    );
    crate::git_base::BaseUse::parse("anything").expect_err("closed mode");
}

#[test]
fn h44_review_merge_head_formats_and_defensive_guards_are_explicit() {
    let directory = h44_diverged("h44-review-formats");
    let tip = git(&directory, &["rev-parse", "master"]);
    let head = git(&directory, &["rev-parse", "HEAD"]);
    h44_begin(&directory, "master");
    let path = git(
        &directory,
        &[
            "rev-parse",
            "--path-format=absolute",
            "--git-path",
            "MERGE_HEAD",
        ],
    );
    for text in [tip.clone(), format!("{tip}\n"), format!("{tip}\r\n")] {
        fs::write(&path, text).expect("accepted format");
        assert_eq!(
            super::commit_base::merge_base_kind(&directory),
            Ok(super::commit_base::MergeBase::IncomingDefault(tip.clone()))
        );
    }
    for text in [
        "master",
        tip.get(..39).expect("ASCII prefix"),
        &tip.to_uppercase(),
        &format!(" {tip}\n"),
    ] {
        fs::write(&path, text).expect("rejected format");
        assert_eq!(
            super::commit_base::merge_base_kind(&directory),
            Ok(super::commit_base::MergeBase::ForkPoint)
        );
    }
    git(&directory, &["merge", "--abort"]);
    git(&directory, &["switch", "master"]);
    fs::write(&path, format!("{head}\n")).expect("foreign head on master");
    assert_eq!(
        super::commit_base::merge_base_kind(&directory),
        Ok(super::commit_base::MergeBase::ForkPoint)
    );
    fs::write(&path, format!("{tip}\n")).expect("same head on master");
    assert_eq!(
        super::commit_base::merge_base_kind(&directory),
        Ok(super::commit_base::MergeBase::ForkPoint)
    );
    fs::remove_file(&path).expect("remove merge head");
    git(&directory, &["switch", "work"]);
    git(&directory, &["tag", "work-tag", &head]);
    let head_path = git(
        &directory,
        &["rev-parse", "--path-format=absolute", "--git-path", "HEAD"],
    );
    fs::write(&head_path, "ref: refs/tags/work-tag\n").expect("nonbranch symbolic HEAD");
    fs::write(&path, format!("{tip}\n")).expect("incoming tip");
    assert_eq!(
        super::commit_base::merge_base_kind(&directory),
        Ok(super::commit_base::MergeBase::ForkPoint)
    );
}

#[test]
fn h44_review_unreadable_merge_head_never_falls_through_to_tree_match() {
    let directory = h44_diverged("h44-review-read-error");
    h44_review_quit_merge(&directory);
    let path = git(
        &directory,
        &[
            "rev-parse",
            "--path-format=absolute",
            "--git-path",
            "MERGE_HEAD",
        ],
    );
    fs::create_dir_all(&path).expect("directory instead of metadata");
    let failure = super::commit_base::commit_base(&directory).expect_err("read error is fatal");
    assert!(failure.contains("MERGE_HEAD"), "{failure}");
    fs::remove_dir(&path).expect("remove directory");
    fs::write(&path, [0xff_u8]).expect("invalid UTF-8");
    let text_failure =
        super::commit_base::commit_base(&directory).expect_err("invalid text is fatal");
    assert!(text_failure.contains("MERGE_HEAD"), "{text_failure}");
}

#[test]
fn h44_review_ours_strategy_is_not_a_clean_merge_tree() {
    let directory = h44_diverged("h44-review-ours");
    let old = git(&directory, &["merge-base", "HEAD", "master"]);
    git(
        &directory,
        &["merge", "-s", "ours", "--no-commit", "master"],
    );
    git(&directory, &["merge", "--quit"]);
    assert_eq!(super::commit_base::commit_base(&directory), Ok(old));
}

#[test]
fn h44_review_deleted_test_resurrection_remains_new_after_merge_and_push() {
    let directory = h44_repository("h44-review-resurrection");
    commit_file(
        &directory,
        "fixture/src/lib.rs",
        &H44_TESTS.replace("1, 1", "8, 8"),
    );
    git(&directory, &["switch", "master"]);
    commit_file(
        &directory,
        "fixture/src/lib.rs",
        "#[test]\nfn protected() { assert_eq!(2, 2); }\n",
    );
    let tip = git(&directory, &["rev-parse", "HEAD"]);
    git(&directory, &["switch", "work"]);
    let merge = git_output(&directory, &["merge", "--no-ff", "--no-commit", "master"]);
    assert!(!merge.status.success());
    fs::write(
        directory.join("fixture/src/lib.rs"),
        H44_TESTS.replace("1, 1", "8, 8"),
    )
    .expect("resurrect removed test");
    git(&directory, &["add", "--all"]);
    assert!(h44_findings(&directory, &tip).is_empty());
    git(&directory, &["commit", "-qm", "resolve"]);
    commit_file(&directory, "work.txt", "next\n");
    assert_eq!(super::commit_base::commit_base(&directory), Ok(tip.clone()));
    assert!(h44_findings(&directory, &tip).is_empty());
    let head = git(&directory, &["rev-parse", "HEAD"]);
    let input = format!("refs/heads/work {head} refs/heads/work {tip}\n");
    let pushed = push_base(&directory, &input).expect("push base");
    assert_eq!(pushed, tip);
    assert!(h44_findings(&directory, &pushed).is_empty());
}

#[test]
fn h44_review_require_base_cli_stays_strict_in_commit_environment() {
    let directory = h44_diverged("h44-review-require");
    let tip = git(&directory, &["rev-parse", "master"]);
    h44_begin(&directory, "master");
    let mut validation = Command::new("cargo");
    validation.args(["run", "--locked", "--quiet", "--manifest-path"]);
    validation.arg(repository_root().join("Cargo.toml"));
    validation
        .args(["--package", "xtask", "--", "require-base", &tip])
        .current_dir(&directory);
    isolate(&mut validation);
    validation.env("STOCK_COPILOT_BASE_USE", "commit");
    let result = validation.output().expect("require-base command");
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("HEAD의 조상이 아니다"));
    let mut recipe = Command::new("just");
    recipe
        .arg("--justfile")
        .arg(repository_root().join("justfile"));
    recipe.arg("--working-directory").arg(&directory);
    recipe.arg(format!("base={tip}")).arg("require-base");
    isolate(&mut recipe);
    recipe.env_remove("STOCK_COPILOT_BASE_USE");
    let recipe_result = recipe.output().expect("require-base recipe");
    assert!(!recipe_result.status.success());
    assert!(String::from_utf8_lossy(&recipe_result.stderr).contains("HEAD의 조상이 아니다"));
}

#[test]
fn hook_fixture_creation_preserves_another_repository() {
    let first = repository("h47-overlapping-hook-fixture", Hooks::Absent);
    fs::write(first.join("live.txt"), "first fixture stays intact").expect("live fixture marker");
    let second = repository("h47-overlapping-hook-fixture", Hooks::Absent);
    assert_ne!(
        first, second,
        "separate executions must never share a fixture path"
    );
    assert_eq!(
        fs::read_to_string(first.join("live.txt")).expect("first fixture preserved"),
        "first fixture stays intact"
    );
    assert_eq!(git(&first, &["rev-parse", "--is-inside-work-tree"]), "true");
    assert_eq!(
        git(&second, &["rev-parse", "--is-inside-work-tree"]),
        "true"
    );
}
