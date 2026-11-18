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
    fs::write(
        directory.join(".cargo/mutants.toml"),
        "examine_globs=[\"crates/domain/src/**/*.rs\"]\n",
    )
    .expect("fixture mutants settings");
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
    for value in (0_u16..128).map(|index| format!("{index}/128")) {
        super::scope_for(
            Path::new("."),
            &[String::from("--shard"), value],
            crate::git_base::BaseUse::Ancestor,
        )
        .expect("valid shard");
    }
    for args in [
        vec!["--shard", "128/128"],
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
fn h54_changed_diff_accepts_only_all_four_canonical_shards() {
    let root = fresh_directory(&repository_root(), "h54-diff-shards");
    fs::create_dir_all(root.join(".cargo")).expect("settings directory");
    fs::write(
        root.join(".cargo/mutants.toml"),
        "examine_globs=['crates/domain/src/**/*.rs']\n",
    )
    .expect("settings");
    h55_git(&root, &["init", "--quiet", "-b", "master"]);
    h55_git(&root, &["add", "."]);
    h55_git(&root, &["commit", "--quiet", "-m", "base"]);
    let base = h55_git(&root, &["rev-parse", "HEAD"]);
    for index in 0_u16..4 {
        let arguments = [
            "--in-diff".to_owned(),
            base.clone(),
            "--shard".to_owned(),
            format!("{index}/4"),
        ];
        let selected = super::scope_for(&root, &arguments, crate::git_base::BaseUse::Ancestor)
            .expect("canonical diff shard");
        assert_eq!(
            selected,
            Scope::DiffSharded(BTreeSet::new(), format!("{index}/4"))
        );
    }
    for shard in ["0/1", "4/4", "00/4", "0/5", "0/4;true"] {
        let arguments = [
            "--in-diff".to_owned(),
            base.clone(),
            "--shard".to_owned(),
            shard.to_owned(),
        ];
        super::scope_for(&root, &arguments, crate::git_base::BaseUse::Ancestor)
            .expect_err("invalid diff shard");
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

#[test]
fn nightly_partition_rejects_noncanonical_and_old_shards() {
    for value in [
        "-1/128", "00/128", "127/129", "0/4", "128/128", "0/127", " 0/128", "0/128 ",
    ] {
        super::scope_for(
            Path::new("."),
            &[String::from("--shard"), String::from(value)],
            crate::git_base::BaseUse::Ancestor,
        )
        .expect_err("noncanonical shard");
    }
}

#[test]
fn nightly_rejects_an_empty_catalog_and_clears_previous_evidence() {
    let directory = fresh_directory(&repository_root(), "h47-zero-catalog");
    fs::create_dir_all(directory.join("src")).expect("source");
    fs::create_dir_all(directory.join(".cargo")).expect("settings");
    fs::write(
        directory.join("Cargo.toml"),
        "[package]\nname=\"h47-zero\"\nversion=\"0.1.0\"\nedition=\"2024\"\n[workspace]\n",
    )
    .expect("manifest");
    fs::write(
        directory.join("src/lib.rs"),
        "pub fn positive(value: i32) -> bool { value > 0 }\n",
    )
    .expect("source");
    fs::copy(
        repository_root().join(".cargo/mutants.toml"),
        directory.join(".cargo/mutants.toml"),
    )
    .expect("unchanged configured targets");
    generate_lockfile(&directory.join("Cargo.toml"));
    let output = directory.join("target/mutants");
    fs::create_dir_all(output.join("mutants.out")).expect("old output");
    fs::write(
        output.join("catalog.json"),
        "[{\"name\":\"old\",\"file\":\"old.rs\"}]",
    )
    .expect("old catalog");
    let failure = run(
        &directory,
        &Scope::Sharded(String::from("127/128")),
        IgnoreFiles::Disregarded,
    )
    .expect_err("empty global catalog must fail");
    assert!(
        failure.contains("밤 전체 변이 목록이 비어 있다"),
        "{failure}"
    );
    assert!(!output.join("catalog.json").exists());
    assert!(!output.join("mutants.out").exists());
}

#[test]
fn nightly_allows_an_empty_shard_only_after_a_nonempty_catalog() {
    let directory = fresh_directory(&repository_root(), "h47-empty-shard");
    fs::create_dir_all(directory.join("crates/domain/src")).expect("source");
    fs::create_dir_all(directory.join(".cargo")).expect("settings");
    fs::write(
        directory.join("crates/domain/Cargo.toml"),
        "[package]\nname=\"domain\"\nversion=\"0.1.0\"\nedition=\"2024\"\n",
    )
    .expect("manifest");
    fs::write(
        directory.join("crates/domain/src/lib.rs"),
        "pub fn positive(value: i32) -> bool { value > 0 }\n",
    )
    .expect("source");
    fs::write(
        directory.join(".cargo/mutants.toml"),
        "examine_globs = [\"crates/domain/src/**/*.rs\"]\n",
    )
    .expect("fixture target");
    fs::write(
        directory.join("Cargo.toml"),
        "[workspace]\nresolver=\"3\"\nmembers=[\"crates/domain\"]\n",
    )
    .expect("workspace manifest");
    generate_lockfile(&directory.join("Cargo.toml"));
    assert_eq!(
        run(
            &directory,
            &Scope::Sharded(String::from("127/128")),
            IgnoreFiles::Disregarded
        )
        .expect("legitimate empty shard"),
        Outcomes::default()
    );
    let catalog: serde_json::Value = serde_json::from_slice(
        &fs::read(directory.join("target/mutants/catalog.json")).expect("catalog receipt"),
    )
    .expect("catalog JSON");
    let empty: &[serde_json::Value] = &[];
    assert_ne!(
        catalog.as_array().expect("catalog entries").as_slice(),
        empty
    );
    assert_eq!(
        fs::read_to_string(directory.join("target/mutants/mutants.out/mutants.json"))
            .expect("shard list")
            .trim(),
        "[]"
    );
}

fn cross_crate_fixture(name: &str) -> std::path::PathBuf {
    let root = fresh_directory(&repository_root(), name);
    for directory in [
        "crates/application/src",
        "crates/testkit-ports/src",
        ".cargo",
    ] {
        fs::create_dir_all(root.join(directory)).expect("fixture directory");
    }
    fs::write(
        root.join("Cargo.toml"),
        "[workspace]\nresolver=\"3\"\nmembers=[\"crates/application\",\"crates/testkit-ports\"]\n",
    )
    .expect("workspace");
    fs::write(
        root.join("crates/application/Cargo.toml"),
        "[package]\nname=\"application\"\nversion=\"0.1.0\"\nedition=\"2024\"\n",
    )
    .expect("application manifest");
    fs::write(
        root.join("crates/application/src/lib.rs"),
        "pub fn accepts(value: i32) -> bool { value > 7 }\n#[test] fn baseline() { assert_eq!(3 + 4, 7); }\n",
    )
    .expect("application source");
    fs::write(root.join("crates/testkit-ports/Cargo.toml"), "[package]\nname=\"testkit-ports\"\nversion=\"0.1.0\"\nedition=\"2024\"\n[dependencies]\napplication={path=\"../application\"}\n").expect("test manifest");
    fs::write(root.join("crates/testkit-ports/src/lib.rs"), "").expect("test library");
    fs::create_dir_all(root.join("crates/testkit-ports/tests")).expect("integration directory");
    let test_path = root.join("crates/testkit-ports/tests/accepts.rs");
    fs::write(&test_path, "#[test] fn boundary() { assert!(!application::accepts(7)); assert!(application::accepts(8)); }\n").expect("external tests");
    fs::write(
        root.join(".cargo/mutants.toml"),
        "examine_globs=[\"crates/application/src/**/*.rs\"]\n",
    )
    .expect("selection");
    generate_lockfile(&root.join("Cargo.toml"));
    root
}

#[test]
fn cross_crate_tests_catch_application_mutants_and_disconnection_survives() {
    let root = cross_crate_fixture("h49-cross-crate");
    let files = BTreeSet::from([String::from("crates/application/src/lib.rs")]);
    for scope in [
        Scope::Files(files.clone()),
        Scope::DailyFiles(files),
        Scope::Everything,
    ] {
        let connected = run(&root, &scope, IgnoreFiles::Disregarded).expect("connected run");
        assert_eq!(connected, Outcomes::default());
        let results: serde_json::Value = serde_json::from_slice(
            &fs::read(root.join("target/mutants/mutants.out/outcomes.json")).expect("outcomes"),
        )
        .expect("json");
        assert_eq!(results["caught"], 5_i32);
    }
    let patch = root.join("changes.diff");
    fs::write(&patch, "diff --git a/crates/application/src/lib.rs b/crates/application/src/lib.rs\n--- a/crates/application/src/lib.rs\n+++ b/crates/application/src/lib.rs\n@@ -1 +1 @@\n-pub fn accepts(value: i32) -> bool { value > 6 }\n+pub fn accepts(value: i32) -> bool { value > 7 }\n").expect("diff");
    assert_eq!(
        super::run_with_diff(
            &root,
            &Scope::Files(BTreeSet::from([String::from(
                "crates/application/src/lib.rs"
            )])),
            IgnoreFiles::Disregarded,
            Some(&patch)
        )
        .expect("diff execution"),
        Outcomes::default()
    );
    let disconnected = std::process::Command::new("cargo")
        .current_dir(&root)
        .args([
            "mutants",
            "--no-config",
            "--test-tool=nextest",
            "--cap-lints=true",
            "--no-shuffle",
            "--colors=never",
            "--file=crates/application/src/lib.rs",
            "--test-package=application",
            "--gitignore=false",
            "--output=target/disconnected",
        ])
        .output()
        .expect("disconnected configuration");
    assert_eq!(disconnected.status.code(), Some(2_i32));
    let results: serde_json::Value = serde_json::from_slice(
        &fs::read(root.join("target/disconnected/mutants.out/outcomes.json"))
            .expect("disconnected outcomes"),
    )
    .expect("json");
    assert_eq!(results["missed"], 5_i32);
    assert_eq!(results["caught"], 0_i32);
    assert_eq!(results["unviable"], 0_i32);
    assert_eq!(results["timeout"], 0_i32);
}

#[test]
fn companion_declaration_matches_workspace_dependency_and_test_location() {
    let root = repository_root();
    let manifest: toml::Table = read_text(&root.join("crates/testkit-ports/Cargo.toml"))
        .expect("manifest")
        .parse()
        .expect("toml");
    assert_eq!(manifest["package"]["name"].as_str(), Some("testkit-ports"));
    assert_eq!(
        manifest["dependencies"]["application"]["path"].as_str(),
        Some("../application")
    );
    for file in ["submit_estimate.rs", "run_full_inspection.rs"] {
        assert!(
            root.join("crates/testkit-ports/tests").join(file).is_file(),
            "{file}"
        );
    }
}

#[test]
fn broken_companion_tests_fail_baseline_instead_of_counting_as_caught() {
    let root = cross_crate_fixture("h49-broken-companion");
    fs::write(
        root.join("crates/testkit-ports/tests/accepts.rs"),
        "#[test] fn broken() { assert_eq!(2, 3); }\n",
    )
    .expect("broken external test");
    let error = run(&root, &Scope::Everything, IgnoreFiles::Disregarded)
        .expect_err("broken baseline must fail");
    assert!(error.contains("exit status: 4"), "{error}");
    let results: serde_json::Value = serde_json::from_slice(
        &fs::read(root.join("target/mutants/mutants.out/outcomes.json")).expect("outcomes"),
    )
    .expect("json");
    assert_eq!(results["caught"], 0_i32);
    assert_eq!(results["missed"], 0_i32);
}

#[test]
fn domain_file_scope_ignores_application_in_the_configured_targets() {
    let root = cross_crate_fixture("h49-domain-scope");
    fs::create_dir_all(root.join("crates/domain/src")).expect("domain directory");
    fs::write(
        root.join("crates/domain/Cargo.toml"),
        "[package]\nname=\"domain\"\nversion=\"0.1.0\"\nedition=\"2024\"\n",
    )
    .expect("domain manifest");
    fs::write(root.join("crates/domain/src/lib.rs"), "pub fn accepts(value: i32) -> bool { value > 7 }\n#[test] fn baseline() { assert_eq!(3 + 4, 7); }\n").expect("weak domain");
    fs::write(root.join("Cargo.toml"), "[workspace]\nresolver=\"3\"\nmembers=[\"crates/domain\",\"crates/application\",\"crates/testkit-ports\"]\n").expect("workspace");
    fs::write(root.join("crates/testkit-ports/Cargo.toml"), "[package]\nname=\"testkit-ports\"\nversion=\"0.1.0\"\nedition=\"2024\"\n[dependencies]\napplication={path=\"../application\"}\ndomain={path=\"../domain\"}\n").expect("test manifest");
    fs::write(root.join("crates/testkit-ports/tests/accepts.rs"), "#[test] fn boundaries() { assert!(!domain::accepts(7)); assert!(domain::accepts(8)); assert!(!application::accepts(7)); assert!(application::accepts(8)); }\n").expect("external boundaries");
    fs::write(
        root.join(".cargo/mutants.toml"),
        "examine_globs=[\"crates/domain/src/**/*.rs\",\"crates/application/src/**/*.rs\"]\n",
    )
    .expect("mixed configuration");
    generate_lockfile(&root.join("Cargo.toml"));
    let domain = Scope::Files(BTreeSet::from([String::from("crates/domain/src/lib.rs")]));
    let outcomes = run(&root, &domain, IgnoreFiles::Disregarded).expect("domain-only scope");
    assert_eq!(
        super::parse_outcomes(&outcomes.missed)
            .expect("missed")
            .len(),
        5
    );
    assert_eq!(outcomes.timeouts, "");
    let mixed = run(&root, &Scope::Everything, IgnoreFiles::Disregarded).expect("mixed scope");
    assert_eq!(mixed, Outcomes::default());
}

#[test]
fn companion_changes_mark_only_the_declared_source_for_weekly_tests() {
    let globs = [
        "crates/domain/src/**/*.rs",
        "crates/application/src/**/*.rs",
    ]
    .map(String::from);
    for path in [
        "crates/testkit-ports/tests/submit_estimate.rs",
        "crates/testkit-ports/Cargo.toml",
    ] {
        assert_eq!(
            super::touched_crates(&globs, &BTreeSet::from([String::from(path)])),
            BTreeSet::from([String::from("crates/application")])
        );
    }
    for path in [
        "crates/testkit-ports-other/tests/submit_estimate.rs",
        "crates/broker-toss/tests/test.rs",
    ] {
        assert_eq!(
            super::touched_crates(&globs, &BTreeSet::from([String::from(path)])),
            BTreeSet::new()
        );
    }
    assert_eq!(
        super::touched_crates(
            &[String::from("crates/domain/src/**/*.rs")],
            &BTreeSet::from([String::from(
                "crates/testkit-ports/tests/submit_estimate.rs"
            )])
        ),
        BTreeSet::new()
    );
}

fn h55_git(root: &Path, args: &[&str]) -> String {
    let mut command = crate::git_base::git(root);
    command.args([
        "-c",
        "user.name=Harness",
        "-c",
        "user.email=harness@example.invalid",
        "-c",
        "commit.gpgsign=false",
    ]);
    command.args(args);
    String::from_utf8(crate::git_base::output(command, "h55 fixture git").expect("git"))
        .expect("utf8")
        .trim()
        .to_owned()
}

fn h55_write(root: &Path, path: &str, text: &str) {
    let file = root.join(path);
    fs::create_dir_all(file.parent().expect("parent")).expect("directory");
    fs::write(file, text).expect("fixture file");
}

#[test]
fn h55_diff_precondition_accepts_only_the_exact_approved_assertion_change() {
    use sha2::{Digest, Sha256};
    let root = fresh_directory(&repository_root(), "h55-mutants-approved");
    let path = "crates/domain/src/sample.rs";
    let before = "pub fn value() -> usize { 1 } #[cfg(test)] mod tests { #[test] fn stable() { assert_eq!(super::value(), 1); } }";
    let after = "pub fn value() -> usize { 1 } #[cfg(test)] mod tests { #[test] fn stable() { assert_eq!(super::value(), 2); } }";
    for (file, text) in [
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
        (path, before),
    ] {
        h55_write(&root, file, text);
    }
    h55_git(&root, &["init", "--quiet", "-b", "master"]);
    h55_git(&root, &["add", "."]);
    h55_git(&root, &["commit", "--quiet", "-m", "baseline"]);
    let base = h55_git(&root, &["rev-parse", "HEAD"]);
    let old_hash = format!("{:x}", Sha256::digest(before.as_bytes()));
    let new_hash = format!("{:x}", Sha256::digest(after.as_bytes()));
    let approval = format!(
        "[[approval]]\ntask = 'H-55'\npath = '{path}'\nbefore_sha256 = '{old_hash}'\nafter_sha256 = '{new_hash}'\nfinding = 'test-diff:modified:domain sample::tests::stable'\nassertion_change = true\ndecision = 'approved'\nreason = 'changed expectation reviewed'\n"
    );
    h55_write(&root, "harness/test-diff/approvals.toml", &approval);
    h55_git(&root, &["add", "harness/test-diff/approvals.toml"]);
    h55_git(&root, &["commit", "--quiet", "-m", "main approval"]);
    h55_git(
        &root,
        &["fetch", "--quiet", ".", "master:refs/remotes/origin/master"],
    );
    h55_git(&root, &["checkout", "--quiet", "-b", "feature"]);
    h55_write(&root, path, after);
    h55_git(&root, &["add", path]);
    h55_git(&root, &["commit", "--quiet", "-m", "approved change"]);
    let args = [String::from("--in-diff"), base];
    super::checked_scope(&root, &args, crate::git_base::BaseUse::Ancestor)
        .expect("approved change passes precondition");
    h55_write(&root, path, &after.replace(", 2)", ", 3)"));
    let error = super::checked_scope(&root, &args, crate::git_base::BaseUse::Ancestor)
        .expect_err("wrong file hash must fail");
    assert!(error.contains("test-diff:modified"), "{error}");
}

#[test]
fn h54_sharded_diff_preserves_approved_test_diff_precondition() {
    use sha2::{Digest, Sha256};
    let root = fresh_directory(&repository_root(), "h54-shard-approved");
    let path = "crates/domain/src/sample.rs";
    let before = "pub fn value() -> usize { 1 } #[cfg(test)] mod tests { #[test] fn stable() { assert_eq!(super::value(), 1); } }";
    let after = "pub fn value() -> usize { 1 } #[cfg(test)] mod tests { #[test] fn stable() { assert_eq!(super::value(), 2); } }";
    for (file, text) in [
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
        (path, before),
    ] {
        h55_write(&root, file, text);
    }
    h55_git(&root, &["init", "--quiet", "-b", "master"]);
    h55_git(&root, &["add", "."]);
    h55_git(&root, &["commit", "--quiet", "-m", "baseline"]);
    let base = h55_git(&root, &["rev-parse", "HEAD"]);
    let old_hash = format!("{:x}", Sha256::digest(before.as_bytes()));
    let new_hash = format!("{:x}", Sha256::digest(after.as_bytes()));
    let approval = format!(
        "[[approval]]\ntask='H-55'\npath='{path}'\nbefore_sha256='{old_hash}'\nafter_sha256='{new_hash}'\nfinding='test-diff:modified:domain sample::tests::stable'\nassertion_change=true\ndecision='approved'\nreason='reviewed'\n"
    );
    h55_write(&root, "harness/test-diff/approvals.toml", &approval);
    h55_git(&root, &["add", "harness/test-diff/approvals.toml"]);
    h55_git(&root, &["commit", "--quiet", "-m", "main approval"]);
    h55_git(
        &root,
        &["fetch", "--quiet", ".", "master:refs/remotes/origin/master"],
    );
    h55_git(&root, &["checkout", "--quiet", "-b", "feature"]);
    h55_write(&root, path, after);
    h55_git(&root, &["add", path]);
    h55_git(&root, &["commit", "--quiet", "-m", "approved change"]);
    let args = [
        String::from("--in-diff"),
        base,
        String::from("--shard"),
        String::from("0/4"),
    ];
    super::checked_scope(&root, &args, crate::git_base::BaseUse::Ancestor)
        .expect("approved change passes the sharded precondition");
    h55_write(&root, path, &after.replace(", 2)", ", 3)"));
    let error = super::checked_scope(&root, &args, crate::git_base::BaseUse::Ancestor)
        .expect_err("wrong file hash must fail for a shard");
    assert!(error.contains("test-diff:modified"), "{error}");
}
