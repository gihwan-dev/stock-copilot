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
        "pub fn value() -> usize { 1 }\n#[cfg(test)] mod tests { #[test] fn check() { assert!(true); } }",
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
    for value in ["0", "-1", "9999999999999999999999", "7", "garbage", ""] {
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
