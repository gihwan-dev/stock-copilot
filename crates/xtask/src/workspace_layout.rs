use std::collections::BTreeSet;
use std::process::Command;

use cargo_metadata::{Metadata, MetadataCommand};

use crate::harness_workspace::repository_root;

const ARCHITECTURE_CRATES: [(&str, &str); 22] = [
    ("domain", "crates/domain"),
    ("application", "crates/application"),
    ("order-execution", "crates/order-execution"),
    ("approval-command", "crates/approval-command"),
    ("broker-toss-schema", "crates/adapters/broker-toss-schema"),
    ("broker-toss", "crates/adapters/broker-toss"),
    ("broker-toss-trade", "crates/adapters/broker-toss-trade"),
    ("sources", "crates/adapters/sources"),
    ("ai-claude-code", "crates/adapters/ai-claude-code"),
    ("ai-codex", "crates/adapters/ai-codex"),
    ("store-sqlite", "crates/adapters/store-sqlite"),
    ("secrets-keychain", "crates/adapters/secrets-keychain"),
    ("platform-macos", "crates/adapters/platform-macos"),
    ("mcp-server", "crates/mcp-server"),
    ("mcp-bridge", "crates/mcp-bridge"),
    ("scheduler", "crates/scheduler"),
    ("testkit", "crates/testkit"),
    ("trade-fakes", "crates/trade-fakes"),
    ("toss-smoke", "crates/toss-smoke"),
    ("xtask", "crates/xtask"),
    ("src-tauri", "apps/desktop/src-tauri"),
    ("e2e-app", "apps/desktop/e2e-app"),
];

fn workspace_metadata() -> Metadata {
    MetadataCommand::new()
        .manifest_path(repository_root().join("Cargo.toml"))
        .no_deps()
        .other_options(vec![String::from("--locked")])
        .exec()
        .expect("cargo metadata")
}

#[test]
fn workspace_members_match_architecture_layout() {
    let metadata = workspace_metadata();
    let actual: BTreeSet<(String, String)> = metadata
        .workspace_packages()
        .into_iter()
        .map(|package| {
            let directory = package
                .manifest_path
                .parent()
                .expect("manifest directory")
                .strip_prefix(&metadata.workspace_root)
                .expect("member inside workspace root")
                .to_string();
            (package.name.to_string(), directory)
        })
        .collect();
    let expected: BTreeSet<(String, String)> = ARCHITECTURE_CRATES
        .into_iter()
        .map(|(name, directory)| (String::from(name), String::from(directory)))
        .collect();
    assert_eq!(actual, expected);
}

#[test]
fn build_output_goes_to_root_target_directory() {
    let metadata = workspace_metadata();
    assert_eq!(
        metadata.target_directory,
        metadata.workspace_root.join("target")
    );
}

#[test]
fn src_tauri_splits_into_library_and_two_binaries() {
    let metadata = workspace_metadata();
    let src_tauri = metadata
        .workspace_packages()
        .into_iter()
        .find(|package| package.name.as_str() == "src-tauri")
        .expect("src-tauri package");
    let libraries: BTreeSet<String> = src_tauri
        .targets
        .iter()
        .filter(|target| target.is_lib())
        .map(|target| target.name.clone())
        .collect();
    let binaries: BTreeSet<String> = src_tauri
        .targets
        .iter()
        .filter(|target| target.is_bin())
        .map(|target| target.name.clone())
        .collect();
    assert_eq!(libraries, BTreeSet::from([String::from("src_tauri")]));
    assert_eq!(
        binaries,
        BTreeSet::from([String::from("src-tauri"), String::from("gen-bindings")])
    );
}

#[test]
fn repository_rustc_is_pinned_release() {
    let output = Command::new("rustc")
        .arg("--version")
        .current_dir(repository_root())
        .env_remove("RUSTUP_TOOLCHAIN")
        .env_remove("RUSTC")
        .output()
        .expect("rustc --version");
    let version = String::from_utf8(output.stdout).expect("utf-8 version line");
    assert!(
        version.starts_with("rustc 1.99.1 "),
        "unexpected rustc version: {version}"
    );
}

fn lint_file_output_lines(path: &str) -> Vec<String> {
    let output = Command::new("just")
        .arg("lint-file")
        .arg(path)
        .current_dir(repository_root())
        .output()
        .expect("just lint-file");
    let mut text = String::from_utf8(output.stdout).expect("utf-8 stdout");
    text.push_str(&String::from_utf8(output.stderr).expect("utf-8 stderr"));
    text.lines().map(|line| String::from(line.trim())).collect()
}

#[test]
fn lint_file_does_not_run_command_substitution_in_path() {
    let lines = lint_file_output_lines("a$(echo INJECTED >&2).txt");
    assert!(!lines.iter().any(|line| line == "INJECTED"), "{lines:?}");
}

#[test]
fn lint_file_does_not_run_commands_after_quote_in_path() {
    let lines = lint_file_output_lines("x\" ; echo PWNED ; \"y.rs");
    assert!(!lines.iter().any(|line| line == "PWNED"), "{lines:?}");
}
