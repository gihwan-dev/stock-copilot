use std::process::Command;

use crate::harness_workspace_tests::repository_root;

const NON_NEXTEST_RUNNERS: [&str; 4] = ["cargo test", "cargo t ", "cargo bench", "cargo-test"];

fn recipe(name: &str) -> Vec<String> {
    let output = Command::new("just")
        .args(["--show", name])
        .current_dir(repository_root())
        .output()
        .expect("just --show");
    assert!(output.status.success(), "recipe {name} missing");
    String::from_utf8(output.stdout)
        .expect("utf-8 recipe")
        .lines()
        .map(|line| String::from(line.trim()))
        .collect()
}

fn recipe_names() -> Vec<String> {
    let output = Command::new("just")
        .args(["--summary"])
        .current_dir(repository_root())
        .output()
        .expect("just --summary");
    String::from_utf8(output.stdout)
        .expect("utf-8 summary")
        .split_whitespace()
        .map(String::from)
        .collect()
}

#[test]
fn no_recipe_runs_rust_tests_outside_nextest() {
    let offending: Vec<String> = recipe_names()
        .iter()
        .flat_map(|name| recipe(name))
        .filter(|line| {
            NON_NEXTEST_RUNNERS
                .iter()
                .any(|runner| format!("{line} ").contains(runner))
        })
        .collect();
    assert!(offending.is_empty(), "{offending:?}");
}

#[test]
fn check_runs_nextest_coverage_and_test_diff() {
    let check = recipe("check");
    for expected in [
        "cargo nextest run --workspace --locked",
        "\"{{ just_executable() }}\" coverage-check",
        "\"{{ just_executable() }}\" test-diff {{ quote(base) }}",
    ] {
        assert!(
            check.iter().any(|line| line == expected),
            "{expected}: {check:?}"
        );
    }
    let position = |needle: &str| check.iter().position(|line| line.contains(needle));
    assert!(
        position("cargo nextest run") < position("test-diff"),
        "test-diff lists tests after they are built: {check:?}"
    );
}

#[test]
fn check_full_runs_mutation_testing_against_the_base() {
    let check_full = recipe("check-full");
    assert!(
        check_full
            .iter()
            .any(|line| line == "\"{{ just_executable() }}\" mutants-diff {{ quote(base) }}"),
        "{check_full:?}"
    );
    let mutants = recipe("mutants-diff");
    assert!(
        mutants
            .iter()
            .any(|line| line
                == "cargo run --locked --quiet --package xtask -- mutants --in-diff \"$1\""),
        "{mutants:?}"
    );
}
