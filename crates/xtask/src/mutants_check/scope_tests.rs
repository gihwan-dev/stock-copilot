use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::git_base::{git, output};
use crate::harness_workspace_tests::{fresh_directory, repository_root};

fn fixture(name: &str) -> PathBuf {
    let root = fresh_directory(&repository_root(), name);
    for (path, content) in [
        (
            ".cargo/mutants.toml",
            "examine_globs = ['crates/domain/src/**/*.rs']",
        ),
        ("crates/domain/Cargo.toml", "[package]\nname='domain'\n"),
        (
            "crates/domain/src/lib.rs",
            "mod leaf;\npub use leaf::value;\n",
        ),
        (
            "crates/domain/src/leaf.rs",
            "pub fn value() -> usize { 1 }\n",
        ),
        (
            "crates/domain/src/other.rs",
            "pub fn other() -> usize { 2 }\n",
        ),
    ] {
        fs::create_dir_all(root.join(path).parent().expect("parent")).expect("directory");
        fs::write(root.join(path), content).expect("source");
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
        output(command, "fixture").expect("git fixture");
    }
    root
}

fn selected(root: &Path) -> BTreeSet<String> {
    let scope =
        super::scope_for(root, &[String::from("--in-diff"), String::from("HEAD")]).expect("scope");
    super::selected_globs(&super::target_globs(root).expect("globs"), &scope)
        .into_iter()
        .collect()
}

#[test]
fn file_scope_includes_worktree_and_inline_test_changes() {
    let root = fixture("h42-file");
    for source in [
        "pub fn value() -> usize { 3 }",
        "pub fn value() -> usize { 1 }\n#[cfg(test)] mod tests { #[test] fn check() { value(); } }",
    ] {
        fs::write(root.join("crates/domain/src/leaf.rs"), source).expect("change");
        assert_eq!(
            selected(&root),
            BTreeSet::from([
                String::from("crates/domain/src/leaf.rs"),
                String::from("crates/domain/src/lib.rs")
            ])
        );
    }
}

#[test]
fn uncertain_changes_expand_and_outside_changes_do_not() {
    let root = fixture("h42-wide");
    for path in [
        "crates/domain/tests/integration.rs",
        "crates/domain/src/test_support.rs",
        "crates/domain/Cargo.toml",
        "crates/domain/fixture.json",
        "crates/domain/src/lib.rs",
    ] {
        fs::create_dir_all(root.join(path).parent().expect("parent")).expect("directory");
        let original = fs::read(root.join(path)).ok();
        fs::write(root.join(path), "pub fn changed() {}").expect("change");
        assert_eq!(
            selected(&root),
            BTreeSet::from([String::from("crates/domain/src/**/*.rs")])
        );
        if let Some(saved) = original {
            fs::write(root.join(path), saved).expect("restore");
        } else {
            fs::remove_file(root.join(path)).expect("remove");
        }
    }
    fs::write(root.join("outside.rs"), "changed").expect("outside");
    assert!(selected(&root).is_empty());
    fs::rename(
        root.join("crates/domain/src/leaf.rs"),
        root.join("crates/domain/src/renamed.rs"),
    )
    .expect("rename");
    assert_eq!(
        selected(&root),
        BTreeSet::from([String::from("crates/domain/src/**/*.rs")])
    );
    fs::remove_file(root.join("crates/domain/src/renamed.rs")).expect("delete");
    assert_eq!(
        selected(&root),
        BTreeSet::from([String::from("crates/domain/src/**/*.rs")])
    );
}

#[test]
fn shared_module_includes_direct_consumers_and_reexports_are_narrow() {
    let root = fixture("h42-shared");
    fs::write(
        root.join("crates/domain/src/other.rs"),
        "use crate::leaf::value; pub fn other() -> usize { value() }",
    )
    .expect("consumer");
    fs::write(
        root.join("crates/domain/src/leaf.rs"),
        "pub fn value() -> usize { 4 }",
    )
    .expect("change");
    assert_eq!(
        selected(&root),
        BTreeSet::from([
            String::from("crates/domain/src/leaf.rs"),
            String::from("crates/domain/src/other.rs"),
            String::from("crates/domain/src/lib.rs")
        ])
    );
    fs::write(
        root.join("crates/domain/src/lib.rs"),
        "mod leaf;\npub use leaf::{value};\n",
    )
    .expect("reexport");
    assert!(!selected(&root).contains("crates/domain/src/**/*.rs"));
}

#[test]
fn jobs_reject_invalid_or_increasing_overrides() {
    assert_eq!(super::jobs_for(16, None).expect("default"), 6);
    assert_eq!(super::jobs_for(2, None).expect("small"), 1);
    assert_eq!(super::jobs_for(16, Some("2")).expect("lower"), 2);
    for value in [
        "0",
        "-1",
        "+3",
        " 3",
        "3 ",
        "３",
        "٣",
        "9999999999999999999999",
        "7",
        "garbage",
        "",
    ] {
        assert!(super::jobs_for(16, Some(value)).is_err(), "{value}");
    }
}

#[test]
fn committed_and_staged_changes_are_included() {
    let root = fixture("h42-committed");
    let mut head = git(&root);
    head.args(["rev-parse", "HEAD"]);
    let base = String::from_utf8(output(head, "head").expect("head")).expect("utf8");
    fs::write(
        root.join("crates/domain/src/leaf.rs"),
        "pub fn value() -> usize { 8 }",
    )
    .expect("change");
    for args in [
        vec!["add", "."],
        vec![
            "-c",
            "user.name=Harness",
            "-c",
            "user.email=harness@example.invalid",
            "commit",
            "--no-verify",
            "-m",
            "change",
        ],
    ] {
        let mut command = git(&root);
        command.args(args);
        output(command, "commit").expect("commit");
    }
    let scope = super::scope_for(
        &root,
        &[String::from("--in-diff"), String::from(base.trim())],
    )
    .expect("scope");
    assert_eq!(
        super::selected_globs(&super::target_globs(&root).expect("globs"), &scope),
        vec![
            String::from("crates/domain/src/leaf.rs"),
            String::from("crates/domain/src/lib.rs")
        ]
    );
    fs::write(root.join("Cargo.toml"), "[workspace]").expect("global");
    assert_eq!(
        selected(&root),
        BTreeSet::from([String::from("crates/domain/src/**/*.rs")])
    );
}

#[test]
fn check_full_keeps_only_diff_mutants() {
    let source = fs::read_to_string(repository_root().join("justfile")).expect("justfile");
    let body = source
        .split("check-full:")
        .nth(1)
        .expect("recipe")
        .split("\n\n")
        .next()
        .expect("body");
    assert!(body.contains("mutants-diff"));
    assert!(!body.contains("mutants-all"));
}

#[test]
fn external_test_modules_and_parse_failures_expand() {
    let root = fixture("h42-support");
    fs::write(
        root.join("crates/domain/src/lib.rs"),
        "mod leaf;\n#[cfg(test)] mod helper;",
    )
    .expect("lib");
    fs::write(root.join("crates/domain/src/helper.rs"), "fn helper() {}").expect("helper");
    assert_eq!(
        selected(&root),
        BTreeSet::from([String::from("crates/domain/src/**/*.rs")])
    );
    fs::write(
        root.join("crates/domain/src/lib.rs"),
        "mod leaf;\npub use leaf::value;\n",
    )
    .expect("restore lib");
    fs::write(root.join("crates/domain/src/leaf.rs"), "invalid (").expect("parse error");
    assert_eq!(
        selected(&root),
        BTreeSet::from([String::from("crates/domain/src/**/*.rs")])
    );
}

#[test]
fn partial_crate_targets_ignore_other_production_files() {
    let root = fixture("h42-partial");
    let globs = vec![String::from("crates/domain/src/other.rs")];
    let changed = BTreeSet::from([String::from("crates/domain/src/leaf.rs")]);
    let scope = super::scope::diff_scope(&root, "HEAD", &globs, &changed);
    assert_eq!(super::selected_globs(&globs, &scope), Vec::<String>::new());
}

#[test]
fn attributed_external_module_support_expands_without_lib_changes() {
    let root = fixture("h42-structural-support");
    fs::write(
        root.join("crates/domain/src/lib.rs"),
        "#[cfg(test)] mod helper;",
    )
    .expect("lib");
    fs::write(root.join("crates/domain/src/helper.rs"), "fn helper() {}").expect("helper");
    let mut command = git(&root);
    command.args(["add", "."]);
    output(command, "add").expect("add");
    let mut commit = git(&root);
    commit.args([
        "-c",
        "user.name=Harness",
        "-c",
        "user.email=harness@example.invalid",
        "commit",
        "--no-verify",
        "-m",
        "support",
    ]);
    output(commit, "commit").expect("commit");
    fs::write(
        root.join("crates/domain/src/helper.rs"),
        "fn helper() { assert!(true); }",
    )
    .expect("support change");
    assert_eq!(
        selected(&root),
        BTreeSet::from([String::from("crates/domain/src/**/*.rs")])
    );
}

#[test]
fn weakened_portfolio_boundary_selects_exact_arithmetic() {
    let root = fixture("h42-regression-a");
    fs::write(root.join("crates/domain/src/lib.rs"), "mod portfolio_valuation; mod holding_valuation; mod exact_arithmetic; pub use holding_valuation::compare_money;").expect("lib");
    fs::write(
        root.join("crates/domain/src/exact_arithmetic.rs"),
        "pub fn compare_reported() {}",
    )
    .expect("exact");
    fs::write(
        root.join("crates/domain/src/holding_valuation.rs"),
        "pub fn compare_money() { crate::exact_arithmetic::compare_reported(); }",
    )
    .expect("holding");
    fs::write(
        root.join("crates/domain/src/portfolio_valuation.rs"),
        "#[cfg(test)] mod tests { #[test] fn boundary() { crate::compare_money(); crate::compare_money(); crate::compare_money(); crate::compare_money(); } }",
    )
    .expect("portfolio");
    for args in [
        vec!["add", "."],
        vec![
            "-c",
            "user.name=Harness",
            "-c",
            "user.email=harness@example.invalid",
            "commit",
            "--no-verify",
            "-m",
            "portfolio boundary baseline",
        ],
    ] {
        let mut command = git(&root);
        command.args(args);
        output(command, "baseline").expect("git");
    }
    fs::write(
        root.join("crates/domain/src/portfolio_valuation.rs"),
        "#[cfg(test)] mod tests { #[test] fn boundary() {} }",
    )
    .expect("delete boundary checks");
    let changed = BTreeSet::from([String::from("crates/domain/src/portfolio_valuation.rs")]);
    let scope = super::scope::diff_scope(
        &root,
        "HEAD",
        &super::target_globs(&root).expect("globs"),
        &changed,
    );
    assert!(
        super::selected_globs(&super::target_globs(&root).expect("globs"), &scope)
            .contains(&String::from("crates/domain/src/exact_arithmetic.rs"))
    );
}

#[test]
fn removed_holding_reference_keeps_baseline_exact_arithmetic() {
    let root = fixture("h42-regression-b");
    fs::write(
        root.join("crates/domain/src/lib.rs"),
        "mod holding_valuation; mod exact_arithmetic;",
    )
    .expect("lib");
    fs::write(
        root.join("crates/domain/src/exact_arithmetic.rs"),
        "pub fn compare_reported() {}",
    )
    .expect("exact");
    fs::write(
        root.join("crates/domain/src/holding_valuation.rs"),
        "pub fn compare_money() { crate::exact_arithmetic::compare_reported(); }",
    )
    .expect("holding");
    for args in [
        vec!["add", "."],
        vec![
            "-c",
            "user.name=Harness",
            "-c",
            "user.email=harness@example.invalid",
            "commit",
            "--no-verify",
            "-m",
            "baseline",
        ],
    ] {
        let mut command = git(&root);
        command.args(args);
        output(command, "baseline").expect("git");
    }
    fs::write(
        root.join("crates/domain/src/holding_valuation.rs"),
        "pub fn compare_money() { local_compare(); } fn local_compare() {}",
    )
    .expect("local replacement");
    assert!(selected(&root).contains("crates/domain/src/exact_arithmetic.rs"));
}

#[test]
fn environment_boundary_has_exactly_one_named_var_call() {
    let source =
        fs::read_to_string(repository_root().join("crates/xtask/src/mutants_check/environment.rs"))
            .expect("boundary");
    let parsed = syn::parse_file(&source).expect("parse boundary");
    let mut calls = EnvironmentCalls::default();
    syn::visit::Visit::visit_file(&mut calls, &parsed);
    assert_eq!(calls.var_paths, 1);
    assert_eq!(calls.imports, 0);
    assert_eq!(
        calls.names,
        vec![String::from("STOCK_COPILOT_MUTANTS_JOBS")]
    );
}

#[test]
fn outside_target_reference_selects_target_consumer() {
    let root = fixture("h42-outside-consumer");
    fs::write(
        root.join("crates/domain/src/other.rs"),
        "pub fn other() { crate::leaf::value(); }",
    )
    .expect("consumer");
    let globs = vec![String::from("crates/domain/src/other.rs")];
    let changed = BTreeSet::from([String::from("crates/domain/src/leaf.rs")]);
    let scope = super::scope::diff_scope(&root, "HEAD", &globs, &changed);
    assert_eq!(super::selected_globs(&globs, &scope), globs);
}

#[test]
fn mod_rs_and_macro_uncertainty_expand_to_crate() {
    let root = fixture("h42-mod-rs");
    fs::create_dir_all(root.join("crates/domain/src/foo")).expect("folder");
    fs::write(
        root.join("crates/domain/src/foo/mod.rs"),
        "pub fn item() {}",
    )
    .expect("module");
    assert_eq!(
        selected(&root),
        BTreeSet::from([String::from("crates/domain/src/**/*.rs")])
    );
    fs::remove_file(root.join("crates/domain/src/foo/mod.rs")).expect("remove");
    fs::write(
        root.join("crates/domain/src/leaf.rs"),
        "pub fn value() { include!(\"generated.rs\"); }",
    )
    .expect("macro");
    assert_eq!(
        selected(&root),
        BTreeSet::from([String::from("crates/domain/src/**/*.rs")])
    );
}

#[test]
fn root_reexport_and_use_aliases_reach_the_owner() {
    let root = fixture("h42-aliases");
    fs::write(
        root.join("crates/domain/src/lib.rs"),
        "mod leaf; pub use leaf::value as RootValue;",
    )
    .expect("exports");
    fs::write(
        root.join("crates/domain/src/other.rs"),
        "use crate::RootValue as alias; pub fn other() { alias(); }",
    )
    .expect("alias");
    let changed = BTreeSet::from([String::from("crates/domain/src/other.rs")]);
    let globs = super::target_globs(&root).expect("globs");
    let scope = super::scope::diff_scope(&root, "HEAD", &globs, &changed);
    assert!(
        super::selected_globs(&globs, &scope).contains(&String::from("crates/domain/src/leaf.rs"))
    );
}

#[test]
fn shard_selection_accepts_only_the_fixed_nightly_partition() {
    let root = repository_root();
    for shard in ["0/4", "1/4", "2/4", "3/4"] {
        assert_eq!(
            super::scope_for(&root, &[String::from("--shard"), String::from(shard)])
                .expect("shard"),
            super::Scope::Sharded(String::from(shard))
        );
    }
    for shard in ["4/4", "5/4", "1/0", "1/1", "--baseline=skip"] {
        assert!(
            super::scope_for(&root, &[String::from("--shard"), String::from(shard)]).is_err(),
            "{shard}"
        );
    }
}

#[derive(Default)]
struct EnvironmentCalls {
    var_paths: usize,
    imports: usize,
    names: Vec<String>,
}

impl<'ast> syn::visit::Visit<'ast> for EnvironmentCalls {
    fn visit_path(&mut self, i: &'ast syn::Path) {
        if i.segments
            .last()
            .is_some_and(|segment| segment.ident == "var")
        {
            self.var_paths = self.var_paths.saturating_add(1);
        }
        syn::visit::visit_path(self, i);
    }
    fn visit_item_use(&mut self, i: &'ast syn::ItemUse) {
        self.imports = self.imports.saturating_add(1);
        syn::visit::visit_item_use(self, i);
    }
    fn visit_expr_call(&mut self, i: &'ast syn::ExprCall) {
        if let syn::Expr::Path(path) = i.func.as_ref()
            && path
                .path
                .segments
                .last()
                .is_some_and(|segment| segment.ident == "var")
        {
            let argument = i.args.first().expect("var argument");
            let syn::Expr::Lit(literal) = argument else {
                panic!("literal variable name required");
            };
            let syn::Lit::Str(name) = &literal.lit else {
                panic!("string variable name required");
            };
            self.names.push(name.value());
        }
        syn::visit::visit_expr_call(self, i);
    }
}

#[test]
fn unresolved_rooted_paths_expand_to_crate() {
    let root = fixture("h42-unknown-root");
    fs::write(
        root.join("crates/domain/src/leaf.rs"),
        "pub fn value() { crate::unknown_symbol(); }",
    )
    .expect("unknown root");
    assert_eq!(
        selected(&root),
        BTreeSet::from([String::from("crates/domain/src/**/*.rs")])
    );
}

#[test]
fn an_unchanged_macro_can_hide_an_incoming_reference() {
    let root = fixture("h42-hidden-incoming");
    fs::write(root.join("crates/domain/src/other.rs"), "custom_macro!();")
        .expect("hidden reference");
    let changed = BTreeSet::from([String::from("crates/domain/src/leaf.rs")]);
    let globs = super::target_globs(&root).expect("globs");
    let scope = super::scope::diff_scope(&root, "HEAD", &globs, &changed);
    assert_eq!(super::selected_globs(&globs, &scope), globs);
}
