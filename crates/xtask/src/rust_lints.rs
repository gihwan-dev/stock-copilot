use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use cargo_metadata::Message;
use cargo_metadata::diagnostic::{Diagnostic, DiagnosticLevel};

use crate::harness_workspace::{fresh_directory, generate_lockfile, repository_root};

const HARNESS_FORBID_LINTS: [&str; 38] = [
    "unsafe_code",
    "clippy::unwrap_used",
    "clippy::expect_used",
    "clippy::panic",
    "clippy::todo",
    "clippy::unimplemented",
    "clippy::unreachable",
    "clippy::panic_in_result_fn",
    "clippy::unwrap_in_result",
    "clippy::get_unwrap",
    "clippy::exit",
    "clippy::mem_forget",
    "clippy::fallible_impl_from",
    "clippy::indexing_slicing",
    "clippy::string_slice",
    "clippy::as_conversions",
    "clippy::as_underscore",
    "clippy::as_pointer_underscore",
    "clippy::arithmetic_side_effects",
    "clippy::integer_division",
    "clippy::modulo_arithmetic",
    "clippy::float_arithmetic",
    "clippy::float_cmp_const",
    "clippy::lossy_float_literal",
    "clippy::default_numeric_fallback",
    "clippy::wildcard_enum_match_arm",
    "clippy::rest_pat_in_fully_bound_structs",
    "unused_must_use",
    "clippy::let_underscore_must_use",
    "clippy::let_underscore_untyped",
    "clippy::unused_result_ok",
    "clippy::map_err_ignore",
    "clippy::allow_attributes",
    "clippy::allow_attributes_without_reason",
    "clippy::print_stdout",
    "clippy::print_stderr",
    "clippy::dbg_macro",
    "unstable_features",
];

const STABLE_CHANNEL_FEATURE_GATE: (&str, &str) = ("unstable_features", "E0554");

const FIXED_VARIABLES: [&str; 16] = [
    "RUSTFLAGS",
    "CARGO_ENCODED_RUSTFLAGS",
    "CARGO_BUILD_RUSTFLAGS",
    "RUSTDOCFLAGS",
    "CARGO_BUILD_RUSTDOCFLAGS",
    "CLIPPY_CONF_DIR",
    "CARGO_TARGET_DIR",
    "CARGO_BUILD_TARGET_DIR",
    "RUSTC",
    "CARGO_BUILD_RUSTC",
    "CARGO_BUILD_RUSTC_WRAPPER",
    "CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER",
    "RUSTUP_TOOLCHAIN",
    "RUSTC_BOOTSTRAP",
    "RUSTC_WRAPPER",
    "RUSTC_WORKSPACE_WRAPPER",
];

#[derive(Debug, Clone, PartialEq, Eq)]
enum Expectation {
    ForbiddenLint(String),
    DeniedLint(String),
    RejectedOverride(String),
    DisallowedItem { lint: String, path: String },
    UndetectedDisallowedItem { lint: String, path: String },
    Clean,
}

#[derive(Debug)]
struct Case {
    package: String,
    source: String,
    dependencies: String,
    expectation: Expectation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Compilation {
    Built,
    NotBuilt,
}

#[derive(Debug, Clone, Copy)]
enum Runner {
    JustRecipe,
    PlainCargo,
}

#[derive(Debug)]
struct ClippyRun {
    diagnostics: BTreeMap<String, Vec<Diagnostic>>,
    compiled: BTreeSet<String>,
}

fn package_suffix(stem: &str) -> String {
    stem.replace("__", "_")
}

fn lint_name(stem: &str) -> String {
    stem.replacen("__", "::", 1)
}

fn examples(root: &Path, group: &str) -> Vec<(String, String)> {
    let directory = root.join("harness").join("rust-lints").join(group);
    let mut found: Vec<(String, String)> = fs::read_dir(&directory)
        .expect("read example directory")
        .map(|listed| {
            let path = listed.expect("example entry").path();
            let stem = path
                .file_stem()
                .and_then(|name| name.to_str())
                .map(String::from)
                .expect("example file stem");
            (stem, fs::read_to_string(&path).expect("read example"))
        })
        .collect();
    found.sort();
    found
}

fn forbid_cases(root: &Path) -> Vec<Case> {
    examples(root, "forbid")
        .into_iter()
        .flat_map(|(stem, source)| {
            let lint = lint_name(&stem);
            [
                Case {
                    package: format!("forbid_{}", package_suffix(&stem)),
                    source: source.clone(),
                    dependencies: String::new(),
                    expectation: Expectation::ForbiddenLint(lint.clone()),
                },
                Case {
                    package: format!("allow_{}", package_suffix(&stem)),
                    dependencies: String::new(),
                    source: format!("#[allow({lint})]\npub mod wrapped {{\n{source}}}\n"),
                    expectation: Expectation::RejectedOverride(lint.clone()),
                },
                Case {
                    package: format!("inner_allow_{}", package_suffix(&stem)),
                    dependencies: String::new(),
                    source: format!("#![allow({lint})]\n{source}"),
                    expectation: Expectation::RejectedOverride(lint.clone()),
                },
                Case {
                    package: format!("expect_{}", package_suffix(&stem)),
                    dependencies: String::new(),
                    source: format!(
                        "#[expect({lint}, reason = \"harness\")]\npub mod wrapped {{\n{source}}}\n"
                    ),
                    expectation: Expectation::RejectedOverride(lint),
                },
            ]
        })
        .collect()
}

fn deny_cases(root: &Path) -> Vec<Case> {
    examples(root, "deny")
        .into_iter()
        .map(|(stem, source)| Case {
            package: format!("deny_{}", package_suffix(&stem)),
            source,
            dependencies: String::new(),
            expectation: Expectation::DeniedLint(lint_name(&stem)),
        })
        .collect()
}

fn clean_cases(root: &Path) -> Vec<Case> {
    examples(root, "clean")
        .into_iter()
        .map(|(stem, source)| Case {
            package: format!("clean_{}", package_suffix(&stem)),
            source,
            dependencies: String::new(),
            expectation: Expectation::Clean,
        })
        .collect()
}

const KNOWN_DISALLOWED_GAPS: [&str; 1] = ["std::include"];

const DISALLOWED_GROUPS: [(&str, &str, &str); 3] = [
    ("disallowed-methods", "clippy::disallowed_methods", "method"),
    ("disallowed-macros", "clippy::disallowed_macros", "macro"),
    ("disallowed-types", "clippy::disallowed_types", "type"),
];

fn example_dependency(path: &str) -> String {
    match path.split("::").next() {
        Some("rust_decimal") => {
            String::from("rust_decimal = { version = \"=1.43.0\", default-features = false }\n")
        }
        Some("num_traits") => {
            String::from("num-traits = { version = \"=0.2.19\", default-features = false }\n")
        }
        Some(_) | None => String::new(),
    }
}

fn disallowed_cases(root: &Path) -> Vec<Case> {
    DISALLOWED_GROUPS
        .into_iter()
        .flat_map(|(group, lint, kind)| {
            examples(root, group)
                .into_iter()
                .map(move |(stem, source)| {
                    let path = stem.replace("__", "::");
                    Case {
                        package: format!(
                            "disallowed_{kind}_{}",
                            package_suffix(&stem).to_lowercase()
                        ),
                        source,
                        dependencies: example_dependency(&path),
                        expectation: if KNOWN_DISALLOWED_GAPS.contains(&path.as_str()) {
                            Expectation::UndetectedDisallowedItem {
                                lint: String::from(lint),
                                path,
                            }
                        } else {
                            Expectation::DisallowedItem {
                                lint: String::from(lint),
                                path,
                            }
                        },
                    }
                })
        })
        .collect()
}

fn clippy_config_paths(root: &Path, key: &str) -> BTreeSet<String> {
    let config: toml::Table = fs::read_to_string(root.join("clippy.toml"))
        .expect("read clippy.toml")
        .parse()
        .expect("parse clippy.toml");
    config
        .get(key)
        .and_then(toml::Value::as_array)
        .expect("disallowed list")
        .iter()
        .map(|entry| {
            entry
                .get("path")
                .and_then(toml::Value::as_str)
                .map(String::from)
                .expect("disallowed path")
        })
        .collect()
}

fn all_cases(root: &Path) -> Vec<Case> {
    let mut cases = forbid_cases(root);
    cases.extend(deny_cases(root));
    cases.extend(disallowed_cases(root));
    cases.extend(clean_cases(root));
    cases
}

fn root_lints(root: &Path) -> toml::Table {
    let manifest: toml::Table = fs::read_to_string(root.join("Cargo.toml"))
        .expect("read root manifest")
        .parse()
        .expect("parse root manifest");
    manifest
        .get("workspace")
        .and_then(toml::Value::as_table)
        .and_then(|workspace| workspace.get("lints"))
        .and_then(toml::Value::as_table)
        .cloned()
        .expect("workspace lints table")
}

fn config_forbid_lints(root: &Path) -> BTreeSet<String> {
    let mut forbidden = BTreeSet::new();
    for (tool, table) in root_lints(root) {
        for (name, value) in table.as_table().expect("lint tool table") {
            let level = value
                .as_str()
                .or_else(|| value.get("level").and_then(toml::Value::as_str));
            if level == Some("forbid") {
                forbidden.insert(match tool.as_str() {
                    "rust" => name.clone(),
                    _ => format!("{tool}::{name}"),
                });
            }
        }
    }
    forbidden
}

fn write_workspace(root: &Path, directory: &Path, cases: &[Case]) {
    fs::create_dir_all(directory).expect("create harness workspace");
    let mut workspace = toml::Table::new();
    workspace.insert(String::from("resolver"), toml::Value::from("3"));
    workspace.insert(
        String::from("members"),
        toml::Value::Array(vec![toml::Value::from("cases/*")]),
    );
    workspace.insert(String::from("lints"), toml::Value::Table(root_lints(root)));
    let mut manifest = toml::Table::new();
    manifest.insert(String::from("workspace"), toml::Value::Table(workspace));
    fs::write(
        directory.join("Cargo.toml"),
        toml::to_string(&manifest).expect("serialize harness manifest"),
    )
    .expect("write harness manifest");
    for name in ["clippy.toml", "rust-toolchain.toml"] {
        fs::copy(root.join(name), directory.join(name)).expect("copy root config");
    }
    for case in cases {
        let crate_directory = directory.join("cases").join(&case.package);
        fs::create_dir_all(crate_directory.join("src")).expect("create case directory");
        fs::write(
            crate_directory.join("Cargo.toml"),
            format!(
                "[package]\nname = \"{}\"\nversion = \"0.1.0\"\nedition = \"2024\"\npublish = false\n\n[lints]\nworkspace = true\n\n[dependencies]\n{}",
                case.package, case.dependencies
            ),
        )
        .expect("write case manifest");
        fs::write(crate_directory.join("src").join("fragment.rs"), "1_u8\n")
            .expect("write case fragment");
        fs::write(crate_directory.join("src").join("lib.rs"), &case.source)
            .expect("write case source");
    }
    generate_lockfile(&directory.join("Cargo.toml"));
}

fn clippy_command(root: &Path, manifest: &Path, runner: Runner) -> Command {
    match runner {
        Runner::JustRecipe => {
            let mut through_just = Command::new("just");
            through_just.arg("clippy").arg(manifest);
            through_just
        }
        Runner::PlainCargo => {
            let mut plain = Command::new("cargo");
            plain
                .args(["clippy", "--workspace", "--all-targets", "--locked"])
                .arg("--manifest-path")
                .arg(manifest);
            for name in FIXED_VARIABLES {
                plain.env_remove(name);
            }
            plain.current_dir(root);
            plain
        }
    }
}

fn run_clippy(
    root: &Path,
    directory: &Path,
    runner: Runner,
    environment: &[(&str, String)],
) -> ClippyRun {
    let mut command = clippy_command(root, &directory.join("Cargo.toml"), runner);
    command
        .current_dir(root)
        .arg("--message-format=json")
        .arg("--keep-going");
    for (name, value) in environment {
        command.env(name, value);
    }
    let output = command.output().expect("run clippy");
    let mut run = ClippyRun {
        diagnostics: BTreeMap::new(),
        compiled: BTreeSet::new(),
    };
    for parsed in Message::parse_stream(output.stdout.as_slice()) {
        let message = parsed.expect("cargo json message");
        if let Message::CompilerMessage(compiler) = message {
            run.diagnostics
                .entry(compiler.target.name)
                .or_default()
                .push(compiler.message);
        } else if let Message::CompilerArtifact(artifact) = message {
            run.compiled.insert(artifact.target.name);
        }
    }
    run
}

fn is_error_with_code(diagnostic: &Diagnostic, code: &str) -> bool {
    diagnostic.level == DiagnosticLevel::Error
        && diagnostic
            .code
            .as_ref()
            .is_some_and(|found| found.code == code)
}

fn rendered_contains(diagnostics: &[Diagnostic], needle: &str) -> bool {
    diagnostics.iter().any(|diagnostic| {
        diagnostic
            .rendered
            .as_ref()
            .is_some_and(|rendered| rendered.contains(needle))
    })
}

fn expectation_met(
    expectation: &Expectation,
    diagnostics: &[Diagnostic],
    compilation: Compilation,
) -> bool {
    match expectation {
        Expectation::ForbiddenLint(lint) if lint == STABLE_CHANNEL_FEATURE_GATE.0 => diagnostics
            .iter()
            .any(|diagnostic| is_error_with_code(diagnostic, STABLE_CHANNEL_FEATURE_GATE.1)),
        Expectation::ForbiddenLint(lint) => {
            diagnostics
                .iter()
                .any(|diagnostic| is_error_with_code(diagnostic, lint))
                && rendered_contains(diagnostics, &format!("`-F {}`", lint.replace('_', "-")))
        }
        Expectation::DeniedLint(lint) => diagnostics
            .iter()
            .any(|diagnostic| is_error_with_code(diagnostic, lint)),
        Expectation::RejectedOverride(lint) => diagnostics.iter().any(|diagnostic| {
            is_error_with_code(diagnostic, "E0453")
                && diagnostic.message.contains(&format!("({lint}"))
                && diagnostic
                    .message
                    .contains("incompatible with previous forbid")
        }),
        Expectation::DisallowedItem { lint, path } => diagnostics.iter().any(|diagnostic| {
            is_error_with_code(diagnostic, lint)
                && diagnostic.message.contains(&format!("`{path}`"))
        }),
        Expectation::UndetectedDisallowedItem { lint, path } => {
            !diagnostics.iter().any(|diagnostic| {
                is_error_with_code(diagnostic, lint)
                    && diagnostic.message.contains(&format!("`{path}`"))
            })
        }
        Expectation::Clean => {
            compilation == Compilation::Built
                && diagnostics.iter().all(|diagnostic| {
                    diagnostic.level != DiagnosticLevel::Error
                        && diagnostic.level != DiagnosticLevel::Warning
                })
        }
    }
}

fn unmet_expectations(cases: &[Case], run: &ClippyRun) -> Vec<String> {
    cases
        .iter()
        .filter(|case| {
            let diagnostics = run
                .diagnostics
                .get(&case.package)
                .map_or(&[][..], Vec::as_slice);
            !expectation_met(
                &case.expectation,
                diagnostics,
                if run.compiled.contains(&case.package) {
                    Compilation::Built
                } else {
                    Compilation::NotBuilt
                },
            )
        })
        .map(|case| format!("{}: {:?}", case.package, case.expectation))
        .collect()
}

fn cases_compiled_despite_expected_failure(cases: &[Case], run: &ClippyRun) -> Vec<String> {
    cases
        .iter()
        .filter(|case| case.expectation != Expectation::Clean)
        .filter(|case| run.compiled.contains(&case.package))
        .map(|case| case.package.clone())
        .collect()
}

fn hostile_environment(directory: &Path) -> Vec<(&'static str, String)> {
    let empty_config = directory.join("empty-clippy-config");
    fs::create_dir_all(&empty_config).expect("create empty clippy config directory");
    fs::write(empty_config.join("clippy.toml"), "").expect("write empty clippy config");
    let wrapper = directory.join("cap-lints-wrapper.sh");
    fs::write(&wrapper, "#!/bin/sh\nexec \"$@\" --cap-lints=allow\n").expect("write wrapper");
    fs::set_permissions(&wrapper, fs::Permissions::from_mode(0o755))
        .expect("make wrapper executable");
    let target = directory.join("hostile-target").display().to_string();
    let wrapper_path = wrapper.display().to_string();
    vec![
        ("RUSTFLAGS", String::from("--cap-lints=allow")),
        ("CARGO_ENCODED_RUSTFLAGS", String::from("--cap-lints=allow")),
        ("CARGO_BUILD_RUSTFLAGS", String::from("--cap-lints=allow")),
        ("RUSTDOCFLAGS", String::from("--cap-lints=allow")),
        ("CLIPPY_CONF_DIR", empty_config.display().to_string()),
        ("RUSTC_WRAPPER", wrapper_path.clone()),
        ("CARGO_BUILD_RUSTC_WRAPPER", wrapper_path.clone()),
        ("RUSTC_WORKSPACE_WRAPPER", wrapper_path),
        ("CARGO_TARGET_DIR", target.clone()),
        ("CARGO_BUILD_TARGET_DIR", target),
        (
            "RUSTUP_TOOLCHAIN",
            String::from("harness-missing-toolchain"),
        ),
        ("RUSTC_BOOTSTRAP", String::from("1")),
    ]
}

fn prepared(name: &str, select: fn(&Path) -> Vec<Case>) -> (PathBuf, PathBuf, Vec<Case>) {
    let root = repository_root();
    let directory = fresh_directory(&root, name);
    let cases = select(&root);
    write_workspace(&root, &directory.join("workspace"), &cases);
    (root, directory, cases)
}

const ENVIRONMENT_PROBES: [&str; 2] = ["forbid_clippy_unwrap_used", "disallowed_macro_std_matches"];

const EXPECT_PROBES: [&str; 3] = [
    "clean_expect_on_deny_group_lint",
    "deny_clippy_needless_pass_by_value",
    "expect_clippy_unwrap_used",
];

fn selected_cases(root: &Path, packages: &[&str]) -> Vec<Case> {
    all_cases(root)
        .into_iter()
        .filter(|case| packages.contains(&case.package.as_str()))
        .collect()
}

fn assert_variable_is_neutralized(variable: &str) {
    let (root, directory, cases) = prepared(&format!("rust-lints-{variable}"), |root| {
        selected_cases(root, &ENVIRONMENT_PROBES)
    });
    assert_eq!(cases.len(), ENVIRONMENT_PROBES.len(), "probe examples");
    let workspace = directory.join("workspace");
    let hostile: Vec<(&str, String)> = hostile_environment(&directory)
        .into_iter()
        .filter(|(name, _)| *name == variable)
        .collect();
    assert_eq!(hostile.len(), 1, "unknown hostile variable {variable}");
    let fixed = run_clippy(&root, &workspace, Runner::JustRecipe, &hostile);
    let unmet = unmet_expectations(&cases, &fixed);
    assert!(
        unmet.is_empty(),
        "{variable} reached just check: {unmet:#?}"
    );
    fs::remove_dir_all(workspace.join("target")).expect("remove harness target");
    let plain = run_clippy(&root, &workspace, Runner::PlainCargo, &hostile);
    let escaped = cases_compiled_despite_expected_failure(&cases, &plain);
    assert!(
        !escaped.is_empty(),
        "{variable} did not disable any lint even without just"
    );
}

#[test]
fn workspace_config_forbids_every_harness_forbid_lint() {
    let configured = config_forbid_lints(&repository_root());
    let missing: Vec<&str> = HARNESS_FORBID_LINTS
        .into_iter()
        .filter(|lint| !configured.contains(*lint))
        .collect();
    assert!(missing.is_empty(), "not forbidden: {missing:?}");
}

#[test]
fn every_forbidden_lint_has_a_violation_example() {
    let root = repository_root();
    let exemplified: BTreeSet<String> = examples(&root, "forbid")
        .into_iter()
        .map(|(stem, _)| lint_name(&stem))
        .collect();
    assert_eq!(config_forbid_lints(&root), exemplified);
}

#[test]
fn every_clippy_disallowed_entry_has_a_violation_example() {
    let root = repository_root();
    for (group, _, _) in DISALLOWED_GROUPS {
        let exemplified: BTreeSet<String> = examples(&root, group)
            .into_iter()
            .map(|(stem, _)| stem.replace("__", "::"))
            .collect();
        assert_eq!(clippy_config_paths(&root, group), exemplified, "{group}");
    }
}

#[test]
fn violation_examples_fail_with_their_lint() {
    let (root, directory, cases) = prepared("rust-lints-check", all_cases);
    let run = run_clippy(&root, &directory.join("workspace"), Runner::JustRecipe, &[]);
    let unmet = unmet_expectations(&cases, &run);
    assert!(unmet.is_empty(), "{unmet:#?}");
}

#[test]
fn violation_examples_fail_in_hostile_environment() {
    let (root, directory, cases) = prepared("rust-lints-hostile", all_cases);
    let hostile = hostile_environment(&directory);
    let run = run_clippy(
        &root,
        &directory.join("workspace"),
        Runner::JustRecipe,
        &hostile,
    );
    let unmet = unmet_expectations(&cases, &run);
    assert!(unmet.is_empty(), "{unmet:#?}");
    assert!(
        !directory.join("hostile-target").exists(),
        "CARGO_TARGET_DIR reached cargo"
    );
}

#[test]
fn item_expect_disables_deny_group_lint_but_not_forbid_lint() {
    let (root, directory, cases) = prepared("rust-lints-expect", |root| {
        selected_cases(root, &EXPECT_PROBES)
    });
    assert_eq!(cases.len(), EXPECT_PROBES.len(), "probe examples");
    let run = run_clippy(&root, &directory.join("workspace"), Runner::JustRecipe, &[]);
    let unmet = unmet_expectations(&cases, &run);
    assert!(unmet.is_empty(), "{unmet:#?}");
}

#[test]
fn rustflags_is_neutralized() {
    assert_variable_is_neutralized("RUSTFLAGS");
}

#[test]
fn cargo_encoded_rustflags_is_neutralized() {
    assert_variable_is_neutralized("CARGO_ENCODED_RUSTFLAGS");
}

#[test]
fn cargo_build_rustflags_is_neutralized() {
    assert_variable_is_neutralized("CARGO_BUILD_RUSTFLAGS");
}

#[test]
fn clippy_conf_dir_is_neutralized() {
    assert_variable_is_neutralized("CLIPPY_CONF_DIR");
}

#[test]
fn rustc_wrapper_is_neutralized() {
    assert_variable_is_neutralized("RUSTC_WRAPPER");
}

#[test]
fn cargo_build_rustc_wrapper_is_neutralized() {
    assert_variable_is_neutralized("CARGO_BUILD_RUSTC_WRAPPER");
}
