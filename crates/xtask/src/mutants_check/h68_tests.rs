use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use crate::harness_workspace_tests::{fresh_directory, repository_root};

use super::{TARGETS_FILE, crate_of, load_targets, target_globs};

fn write_text(root: &Path, relative: &str, content: &str) {
    let full = root.join(relative);
    fs::create_dir_all(full.parent().expect("parent")).expect("directory");
    fs::write(full, content).expect("write");
}

fn minimal_targets_workspace(name: &str) -> std::path::PathBuf {
    let root = fresh_directory(&repository_root(), name);
    write_text(
        &root,
        "crates/domain/src/money.rs",
        "pub fn positive(value: i32) -> bool { value > 0 }\n",
    );
    write_text(
        &root,
        "crates/adapters/store-sqlite/src/connection.rs",
        "pub fn open() -> bool { true }\n",
    );
    write_text(
        &root,
        TARGETS_FILE,
        "[[target]]\npath = \"crates/domain/src/money.rs\"\ncategory = \"money_quantity_fx\"\nreason = \"금액 계산\"\n\n[[target]]\npath = \"crates/adapters/store-sqlite/src/connection.rs\"\ncategory = \"secret_auth_boundary\"\nreason = \"암호화 연결 경계\"\n",
    );
    write_text(
        &root,
        ".cargo/mutants.toml",
        "test_tool = \"nextest\"\ncap_lints = true\nexamine_globs = [\n    \"crates/domain/src/money.rs\",\n    \"crates/adapters/store-sqlite/src/connection.rs\",\n]\n",
    );
    root
}

#[test]
fn h68_crate_of_preserves_nested_adapter_paths() {
    assert_eq!(
        crate_of("crates/adapters/store-sqlite/src/connection.rs"),
        "crates/adapters/store-sqlite"
    );
    assert_eq!(
        crate_of("crates/adapters/broker-toss/src/endpoint.rs"),
        "crates/adapters/broker-toss"
    );
    assert_eq!(crate_of("crates/domain/src/money.rs"), "crates/domain");
    assert_eq!(
        crate_of("crates/order-execution/src/**/*.rs"),
        "crates/order-execution"
    );
}

#[test]
fn h68_repository_targets_load_and_match_mutants_config() {
    let root = repository_root();
    let targets = load_targets(&root).expect("repository targets");
    assert_ne!(targets, Vec::<String>::new());
    assert_eq!(target_globs(&root).expect("target globs"), targets);
}

#[test]
fn h68_load_targets_rejects_missing_files_and_config_drift() {
    let root = minimal_targets_workspace("h68-targets-valid");
    assert_eq!(
        load_targets(&root).expect("valid targets"),
        vec![
            String::from("crates/domain/src/money.rs"),
            String::from("crates/adapters/store-sqlite/src/connection.rs"),
        ]
    );

    let missing_file = minimal_targets_workspace("h68-targets-missing");
    fs::remove_file(missing_file.join("crates/domain/src/money.rs")).expect("remove");
    assert!(
        load_targets(&missing_file)
            .expect_err("missing target must fail")
            .contains("존재하지 않는다")
    );

    let drifted = minimal_targets_workspace("h68-targets-drift");
    write_text(
        &drifted,
        ".cargo/mutants.toml",
        "test_tool = \"nextest\"\ncap_lints = true\nexamine_globs = [\"crates/domain/src/money.rs\"]\n",
    );
    assert!(
        load_targets(&drifted)
            .expect_err("drifted examine_globs must fail")
            .contains("일치하지 않는다")
    );
}

#[test]
fn h68_load_targets_rejects_bad_category_duplicates_and_empty_globs() {
    let bad_category = minimal_targets_workspace("h68-targets-category");
    write_text(
        &bad_category,
        TARGETS_FILE,
        "[[target]]\npath = \"crates/domain/src/money.rs\"\ncategory = \"other\"\nreason = \"잘못된 분류\"\n",
    );
    assert!(
        load_targets(&bad_category)
            .expect_err("unknown category must fail")
            .contains("알 수 없는 분류")
    );

    let duplicate = minimal_targets_workspace("h68-targets-duplicate");
    write_text(
        &duplicate,
        TARGETS_FILE,
        "[[target]]\npath = \"crates/domain/src/money.rs\"\ncategory = \"money_quantity_fx\"\nreason = \"첫 항목\"\n\n[[target]]\npath = \"crates/domain/src/money.rs\"\ncategory = \"money_quantity_fx\"\nreason = \"중복 항목\"\n",
    );
    assert!(
        load_targets(&duplicate)
            .expect_err("duplicate path must fail")
            .contains("중복 대상 경로")
    );

    let empty_glob = minimal_targets_workspace("h68-targets-empty-glob");
    fs::create_dir_all(empty_glob.join("crates/order-execution/src")).expect("empty src");
    write_text(
        &empty_glob,
        TARGETS_FILE,
        "[[target]]\npath = \"crates/order-execution/src/**/*.rs\"\ncategory = \"order_decision_execution\"\nreason = \"빈 디렉터리\"\n",
    );
    write_text(
        &empty_glob,
        ".cargo/mutants.toml",
        "test_tool = \"nextest\"\ncap_lints = true\nexamine_globs = [\"crates/order-execution/src/**/*.rs\"]\n",
    );
    assert!(
        load_targets(&empty_glob)
            .expect_err("glob with zero rs files must fail")
            .contains(".rs 파일이 없다")
    );
}

#[test]
fn h68_mutants_profile_disables_shared_build_cache() {
    let mut command = std::process::Command::new("cargo");
    super::configure_profile(&mut command);
    let envs: std::collections::BTreeMap<_, _> = command.get_envs().collect();
    assert_eq!(
        envs.get(std::ffi::OsStr::new("STOCK_COPILOT_BUILD_CACHE")),
        Some(&Some(std::ffi::OsStr::new("off")))
    );
    assert_eq!(
        envs.get(std::ffi::OsStr::new("RUSTC_WRAPPER")),
        Some(&Some(std::ffi::OsStr::new("")))
    );
}

#[test]
fn h68_diff_scope_excludes_files_outside_targets() {
    let root = minimal_targets_workspace("h68-diff-scope");
    write_text(
        &root,
        "crates/domain/src/candle.rs",
        "pub fn close() -> i32 { 1 }\n",
    );
    let globs = target_globs(&root).expect("globs");
    let changed = BTreeSet::from([
        String::from("crates/domain/src/money.rs"),
        String::from("crates/domain/src/candle.rs"),
    ]);
    assert_eq!(
        super::scope::diff_scope(&root, "base", &globs, &changed),
        super::Scope::Files(BTreeSet::from([String::from("crates/domain/src/money.rs")]))
    );
}
