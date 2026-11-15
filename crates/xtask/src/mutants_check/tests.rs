use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use crate::check_support::read_text;
use crate::harness_workspace_tests::{
    copy_tree, fresh_directory, generate_lockfile, repository_root,
};

use super::{
    IgnoreFiles, Outcomes, Rule, SURVIVORS_FILE, Scope, Survivor, evaluate, load_survivors, run,
};

const CASES_FILE: &str = "harness/mutants/cases.toml";
const WORKSPACE_EXAMPLE: &str = "harness/mutants/workspace";
const EXAMPLE_MEMBERS: [&str; 2] = ["crates/domain", "apps/desktop/src-tauri"];
const FAILED: &str = "error";

fn cases_table(root: &Path) -> toml::Table {
    read_text(&root.join(CASES_FILE))
        .expect("read mutants cases")
        .parse()
        .expect("parse mutants cases")
}

fn text(entry: &toml::Value, key: &str) -> String {
    entry
        .get(key)
        .and_then(toml::Value::as_str)
        .map(String::from)
        .expect("text field")
}

fn text_list(entry: &toml::Value, key: &str) -> BTreeSet<String> {
    entry
        .get(key)
        .and_then(toml::Value::as_array)
        .expect("text list")
        .iter()
        .map(|item| item.as_str().map(String::from).expect("text item"))
        .collect()
}

fn shown(
    outcome: Result<BTreeSet<crate::check_support::Finding<Rule>>, String>,
) -> BTreeSet<String> {
    match outcome {
        Ok(findings) => findings.iter().map(ToString::to_string).collect(),
        Err(_) => BTreeSet::from([String::from(FAILED)]),
    }
}

fn root_lints(root: &Path) -> toml::Value {
    let manifest: toml::Table = read_text(&root.join("Cargo.toml"))
        .expect("read root manifest")
        .parse()
        .expect("parse root manifest");
    manifest
        .get("workspace")
        .and_then(|workspace| workspace.get("lints"))
        .cloned()
        .expect("workspace lints")
}

fn prepared_workspace(root: &Path) -> std::path::PathBuf {
    let directory = fresh_directory(root, "mutants-workspace");
    copy_tree(&root.join(WORKSPACE_EXAMPLE), &directory);
    let mut workspace = toml::Table::new();
    workspace.insert(String::from("resolver"), toml::Value::from("3"));
    workspace.insert(
        String::from("members"),
        toml::Value::Array(EXAMPLE_MEMBERS.map(toml::Value::from).to_vec()),
    );
    workspace.insert(String::from("lints"), root_lints(root));
    let mut manifest = toml::Table::new();
    manifest.insert(String::from("workspace"), toml::Value::Table(workspace));
    fs::write(
        directory.join("Cargo.toml"),
        toml::to_string(&manifest).expect("serialize manifest"),
    )
    .expect("write manifest");
    fs::create_dir_all(directory.join(".cargo")).expect("create .cargo");
    fs::copy(
        root.join(".cargo/mutants.toml"),
        directory.join(".cargo/mutants.toml"),
    )
    .expect("copy mutants settings");
    generate_lockfile(&directory.join("Cargo.toml"));
    directory
}

#[test]
fn survivor_cases_report_exactly_their_findings() {
    let root = repository_root();
    let table = cases_table(&root);
    let mismatches: Vec<String> = table
        .get("case")
        .and_then(toml::Value::as_array)
        .expect("mutants cases")
        .iter()
        .filter_map(|entry| {
            let survivors: Vec<Survivor> = entry
                .get("survivors")
                .and_then(toml::Value::as_array)
                .expect("survivors")
                .iter()
                .map(|survivor| Survivor {
                    file: text(survivor, "file"),
                    mutant: text(survivor, "mutant"),
                })
                .collect();
            let outcomes = Outcomes {
                missed: text(entry, "missed"),
                timeouts: text(entry, "timeouts"),
            };
            let reported = shown(evaluate(&outcomes, &survivors));
            let expected = text_list(entry, "expect");
            (reported != expected).then(|| {
                format!(
                    "{}: expected {expected:?}, reported {reported:?}",
                    text(entry, "name")
                )
            })
        })
        .collect();
    assert!(mismatches.is_empty(), "{mismatches:#?}");
}

#[test]
fn every_mutants_rule_has_an_example() {
    let root = repository_root();
    let table = cases_table(&root);
    let reported: Vec<String> = table
        .get("case")
        .and_then(toml::Value::as_array)
        .expect("mutants cases")
        .iter()
        .flat_map(|entry| text_list(entry, "expect"))
        .collect();
    let missing: Vec<String> = [Rule::Missed(String::new()), Rule::Timeout(String::new())]
        .iter()
        .map(ToString::to_string)
        .filter(|prefix| !reported.iter().any(|line| line.contains(prefix.as_str())))
        .collect();
    assert!(missing.is_empty(), "rules without an example: {missing:?}");
}

#[test]
fn survivor_list_starts_empty() {
    let survivors = load_survivors(&repository_root(), SURVIVORS_FILE).expect("load survivors");
    assert!(survivors.is_empty(), "{survivors:?}");
}

#[test]
fn survivor_list_rejects_missing_files_and_reasons() {
    let root = repository_root();
    let directory = fresh_directory(&root, "mutants-survivors");
    fs::create_dir_all(directory.join("src")).expect("create source directory");
    fs::write(directory.join("src/lib.rs"), "pub fn kept() {}\n").expect("write source");
    let attempt = |list: &str| {
        fs::write(directory.join("survivors.toml"), list).expect("write list");
        load_survivors(&directory, "survivors.toml")
    };
    assert_eq!(
        attempt("[[survivor]]\nfile = \"src/lib.rs\"\nmutant = \"replace kept with ()\"\nreason = \"harness\"\n")
            .expect("existing file"),
        vec![Survivor {
            file: String::from("src/lib.rs"),
            mutant: String::from("replace kept with ()"),
        }]
    );
    for (list, message) in [
        (
            "[[survivor]]\nfile = \"src/gone.rs\"\nmutant = \"replace kept with ()\"\nreason = \"harness\"\n",
            "gone.rs",
        ),
        (
            "[[survivor]]\nfile = \"src/lib.rs\"\nmutant = \"replace kept with ()\"\n",
            "reason",
        ),
        (
            "[[survivor]]\nfile = \"src/lib.rs\"\nreason = \"harness\"\n",
            "mutant",
        ),
        ("survivors = []\n", "survivor"),
    ] {
        let outcome = attempt(list);
        assert!(
            outcome
                .as_ref()
                .is_err_and(|failure| failure.contains(message)),
            "{list}: {outcome:?}"
        );
    }
}

#[test]
fn weak_test_example_leaves_surviving_mutants() {
    let root = repository_root();
    let directory = prepared_workspace(&root);
    assert!(
        !directory.join("apps/desktop/dist").exists(),
        "example must not have a frontend bundle"
    );
    let outcomes =
        run(&directory, &Scope::Everything, IgnoreFiles::Disregarded).expect("run cargo mutants");
    let table = cases_table(&root);
    let workspace_case = table.get("workspace").expect("workspace case");
    assert_eq!(
        shown(evaluate(&outcomes, &[])),
        text_list(workspace_case, "expect")
    );
    fs::copy(
        root.join(text(workspace_case, "survivors")),
        directory.join("survivors.toml"),
    )
    .expect("copy example survivors");
    let survivors = load_survivors(&directory, "survivors.toml").expect("load example survivors");
    assert_eq!(shown(evaluate(&outcomes, &survivors)), BTreeSet::new());
}

#[test]
fn mutation_profile_keeps_debug_assertions_and_overflow_checks() {
    let mut command = std::process::Command::new("cargo");
    super::configure_profile(&mut command);
    for (key, value) in [
        ("CARGO_PROFILE_DEV_DEBUG", "0"),
        ("CARGO_PROFILE_TEST_DEBUG", "0"),
        ("CARGO_PROFILE_DEV_OPT_LEVEL", "1"),
        ("CARGO_PROFILE_TEST_OPT_LEVEL", "1"),
        ("CARGO_PROFILE_DEV_DEBUG_ASSERTIONS", "true"),
        ("CARGO_PROFILE_TEST_DEBUG_ASSERTIONS", "true"),
        ("CARGO_PROFILE_DEV_OVERFLOW_CHECKS", "true"),
        ("CARGO_PROFILE_TEST_OVERFLOW_CHECKS", "true"),
    ] {
        assert_eq!(
            command
                .get_envs()
                .find(|(name, _)| *name == std::ffi::OsStr::new(key))
                .and_then(|(_, actual)| actual),
            Some(std::ffi::OsStr::new(value)),
            "{key}"
        );
    }
}

#[test]
fn support_only_workspace_changes_mark_all_nightly_targets() {
    let globs = [
        "crates/domain/src/**/*.rs",
        "crates/application/src/**/*.rs",
        "crates/order-execution/src/**/*.rs",
    ]
    .map(String::from);
    let all = [
        "crates/domain",
        "crates/application",
        "crates/order-execution",
    ]
    .map(String::from)
    .into();
    for path in [
        "Cargo.lock",
        "Cargo.toml",
        ".cargo/mutants.toml",
        ".config/nextest.toml",
        "harness/rules.json",
        "crates/xtask/src/main.rs",
        "justfile",
        "lefthook.yml",
        ".github/workflows/nightly.yml",
    ] {
        assert_eq!(
            super::touched_crates(&globs, &BTreeSet::from([String::from(path)])),
            all,
            "{path}"
        );
    }
    assert_eq!(
        super::touched_crates(
            &globs,
            &BTreeSet::from([String::from("crates/domain/tests/a.rs")])
        ),
        BTreeSet::from([String::from("crates/domain")])
    );
    assert_eq!(
        super::touched_crates(&globs, &BTreeSet::from([String::from("docs/design.md")])),
        BTreeSet::new()
    );
}

#[test]
fn shard_selection_accepts_only_fixed_partition() {
    for value in ["0/4", "1/4", "2/4", "3/4"] {
        super::scope_for(
            Path::new("."),
            &[String::from("--shard"), String::from(value)],
            crate::git_base::BaseUse::Ancestor,
        )
        .expect("valid shard");
    }
    for args in [
        vec!["--shard", "4/4"],
        vec!["--shard", "0/1"],
        vec!["--baseline=skip"],
        vec!["--shard", "0/4", "--baseline=skip"],
    ] {
        super::scope_for(
            Path::new("."),
            &args.into_iter().map(String::from).collect::<Vec<_>>(),
            crate::git_base::BaseUse::Ancestor,
        )
        .expect_err("invalid shard");
    }
}

#[test]
fn environment_reads_only_jobs_key() {
    use syn::visit::Visit;
    struct Calls(Vec<String>);
    impl<'ast> Visit<'ast> for Calls {
        fn visit_expr_call(&mut self, i: &'ast syn::ExprCall) {
            if let syn::Expr::Path(path) = i.func.as_ref()
                && path.path.segments.last().is_some_and(|segment| {
                    ["var", "var_os", "vars", "vars_os"]
                        .contains(&segment.ident.to_string().as_str())
                })
            {
                assert_eq!(path.path.segments.last().expect("call").ident, "var");
                assert_eq!(i.args.len(), 1);
                self.0
                    .push(quote::ToTokens::to_token_stream(&i.args).to_string());
            }
            syn::visit::visit_expr_call(self, i);
        }
    }

    let source =
        fs::read_to_string(repository_root().join("crates/xtask/src/mutants_check/environment.rs"))
            .expect("environment");
    let mut calls = Calls(Vec::new());
    calls.visit_file(&syn::parse_file(&source).expect("AST"));
    assert_eq!(calls.0, ["\"STOCK_COPILOT_MUTANTS_JOBS\""]);
    let mut command = std::process::Command::new("cargo");
    super::configure_profile(&mut command);
    assert!(
        command
            .get_envs()
            .any(|(key, value)| key == "STOCK_COPILOT_PROPTEST_CASES" && value.is_none())
    );
}

#[test]
fn diff_patch_includes_committed_staged_worktree_and_untracked() {
    use crate::git_base::{git, output};
    let root = fresh_directory(&repository_root(), "h46-diff-patch");
    for args in [
        vec!["init"],
        vec!["config", "user.name", "harness"],
        vec!["config", "user.email", "harness@example.invalid"],
    ] {
        let mut command = git(&root);
        command.args(args);
        output(command, "git").expect("git");
    }
    fs::write(root.join("sample.rs"), "fn first() {}\n").expect("source");
    for args in [
        vec!["add", "."],
        vec!["-c", "core.hooksPath=/dev/null", "commit", "-m", "base"],
    ] {
        let mut command = git(&root);
        command.args(args);
        output(command, "git").expect("git");
    }
    let base = crate::git_base::resolve_base(&root, "HEAD").expect("base");
    for (file, source, commit) in [
        ("committed.rs", "fn committed() {}", true),
        ("staged.rs", "fn staged() {}", false),
    ] {
        fs::write(root.join(file), source).expect("source");
        let mut command = git(&root);
        command.args(["add", file]);
        output(command, "add").expect("stage");
        if commit {
            let mut commit_command = git(&root);
            commit_command.args(["-c", "core.hooksPath=/dev/null", "commit", "-m", "change"]);
            output(commit_command, "commit").expect("commit");
        }
    }
    fs::write(root.join("sample.rs"), "fn worktree() {}\n").expect("worktree");
    fs::write(root.join("untracked.rs"), "fn untracked() {}\n").expect("untracked");
    let patch = fs::read_to_string(super::diff_patch(&root, &base).expect("patch")).expect("read");
    for expected in ["fn committed", "fn staged", "fn worktree", "fn untracked"] {
        assert!(patch.contains(expected), "{expected}: {patch}");
    }
}

#[test]
fn diff_command_requires_test_diff_at_the_resolved_base() {
    use crate::git_base::{git, output};
    let root = fresh_directory(&repository_root(), "h43-addition-gate");
    for (path, source) in [
        (
            "Cargo.lock",
            "version = 4\n[[package]]\nname='domain'\nversion='0.1.0'\n",
        ),
        (
            "Cargo.toml",
            "[workspace]\nmembers=['crates/domain']\nresolver='2'\n",
        ),
        (
            "crates/domain/Cargo.toml",
            "[package]\nname='domain'\nversion='0.1.0'\nedition='2024'\n",
        ),
        (
            ".cargo/mutants.toml",
            "examine_globs=['crates/domain/src/**/*.rs']\n",
        ),
        ("crates/domain/src/lib.rs", "pub mod sample;"),
        (
            "crates/domain/src/sample.rs",
            "pub fn value() -> usize { 1 } #[cfg(test)] mod tests { #[test] fn stable() { assert_eq!(super::value(), 1); } }",
        ),
    ] {
        let full = root.join(path);
        fs::create_dir_all(full.parent().expect("parent")).expect("directory");
        fs::write(full, source).expect("source");
    }
    for args in [
        vec!["init"],
        vec!["add", "."],
        vec![
            "-c",
            "user.name=Harness",
            "-c",
            "user.email=harness@example.invalid",
            "commit",
            "--no-verify",
            "-m",
            "fixture",
        ],
    ] {
        let mut command = git(&root);
        command.args(args);
        output(command, "fixture").expect("git");
    }
    let args = [String::from("--in-diff"), String::from("HEAD")];
    super::checked_scope(&root, &args, crate::git_base::BaseUse::Ancestor)
        .expect("baseline passes");
    fs::write(root.join("crates/domain/src/sample.rs"), "pub fn value() -> usize { 1 } #[cfg(test)] mod tests { #[test] fn stable() { assert!(true); } }").expect("weaken test");
    let error = super::checked_scope(&root, &args, crate::git_base::BaseUse::Ancestor)
        .expect_err("test-diff required");
    assert!(error.contains("test-diff 실패"), "{error}");
    assert!(error.contains("test-diff:modified"), "{error}");
}
