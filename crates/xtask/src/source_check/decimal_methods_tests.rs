use std::collections::BTreeSet;
use std::fs;

use crate::harness_workspace_tests::repository_root;

use super::allowlist::{ALLOWLIST_FILE, Allowlists};
use super::check_source;
use super::decimal_methods::DecimalRules;
use super::finding::Rule;

const EXAMPLES: &str = "harness/source-checks/decimal-sources";
const CASES: &str = "harness/source-checks/decimal-cases.toml";
const OUTSIDE: &str = "crates/application/src/valuation.rs";
const DISPLAY: &str = "crates/domain/src/money_display.rs";
const PARSER: &str = "crates/domain/src/money.rs";

fn policy() -> toml::Table {
    fs::read_to_string(repository_root().join(ALLOWLIST_FILE))
        .expect("policy exists")
        .parse()
        .expect("valid TOML")
}

fn examples() -> Vec<(String, String, String)> {
    let root = repository_root();
    let table: toml::Table = fs::read_to_string(root.join(CASES))
        .expect("cases exist")
        .parse()
        .expect("cases TOML");
    table
        .get("case")
        .and_then(toml::Value::as_array)
        .expect("case list")
        .iter()
        .map(|case| {
            let file = case
                .get("file")
                .and_then(toml::Value::as_str)
                .expect("file");
            let method = case
                .get("method")
                .and_then(toml::Value::as_str)
                .expect("method");
            (
                String::from(file),
                String::from(method),
                fs::read_to_string(root.join(EXAMPLES).join(file)).expect("example source"),
            )
        })
        .collect()
}

fn reported(policy: &DecimalRules, path: &str, source: &str) -> BTreeSet<String> {
    policy
        .findings(path, source.parse().expect("tokens"))
        .into_iter()
        .map(|finding| finding.rule.to_string())
        .collect()
}

#[test]
fn decimal_policy_examples_and_real_source_check_agree() {
    let root = repository_root();
    let allowlists = Allowlists::load(&root).expect("load policy");
    let rules = DecimalRules::parse(&policy()).expect("decimal policy");
    let cases = examples();
    let present: BTreeSet<String> = fs::read_dir(root.join(EXAMPLES))
        .expect("examples folder")
        .map(|entry| {
            entry
                .expect("entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    assert_eq!(
        present,
        cases.iter().map(|(file, _, _)| file.clone()).collect()
    );
    for (_, method, source) in &cases {
        let expected = BTreeSet::from([format!("decimal-method:{method}")]);
        assert_eq!(reported(&rules, OUTSIDE, source), expected, "{source}");
        let actual: BTreeSet<String> = check_source(OUTSIDE, source, &allowlists)
            .into_iter()
            .map(|finding| finding.rule.to_string())
            .collect();
        assert_eq!(actual, expected, "integration: {source}");
    }
}

#[test]
fn deleting_any_decimal_method_or_allowlisted_file_changes_a_case() {
    let original = policy();
    let cases = examples();
    for section in ["rounding", "parsing", "exact-parsing", "generic-parsing"] {
        let methods = original
            .get("decimal-methods")
            .and_then(|value| value.get(section))
            .and_then(toml::Value::as_array)
            .expect("method array");
        assert_ne!(methods.as_slice(), []);
        for index in 0..methods.len() {
            let mut mutated = original.clone();
            let entries = mutated
                .get_mut("decimal-methods")
                .and_then(|value| value.get_mut(section))
                .and_then(toml::Value::as_array_mut)
                .expect("method array");
            *entries = entries
                .iter()
                .enumerate()
                .filter(|(position, _)| *position != index)
                .map(|(_, value)| value.clone())
                .collect();
            let rules = DecimalRules::parse(&mutated).expect("mutated policy");
            assert!(
                cases
                    .iter()
                    .any(|(_, method, source)| reported(&rules, OUTSIDE, source)
                        != BTreeSet::from([format!("decimal-method:{method}")])),
                "undetected deletion: {section}[{index}]"
            );
        }
    }
}

#[test]
fn deleting_decimal_allowlisted_file_changes_a_case() {
    let original = policy();
    for (section, allowed, source) in [
        (
            "decimal-rounding",
            DISPLAY,
            "fn value() { amount.round_dp(2); }",
        ),
        (
            "decimal-parsing",
            PARSER,
            "fn value() { Decimal::from_str_exact(input); }",
        ),
    ] {
        assert!(repository_root().join(allowed).is_file());
        let rules = DecimalRules::parse(&original).expect("policy");
        assert!(reported(&rules, allowed, source).is_empty());
        let mut mutated = original.clone();
        mutated
            .get_mut(section)
            .and_then(toml::Value::as_array_mut)
            .expect("paths")
            .clear();
        assert_eq!(
            DecimalRules::parse(&mutated)
                .expect("policy")
                .findings(allowed, source.parse().expect("tokens"))
                .len(),
            1
        );
    }
}

#[test]
fn decimal_aliases_qualified_traits_macros_and_generic_calls_are_checked() {
    let rules = DecimalRules::parse(&policy()).expect("policy");
    for source in [
        "use rust_decimal::Decimal as D; fn value() { D::from_str(input); }",
        "fn value() { <rust_decimal::Decimal as core::str::FromStr>::from_str(input); }",
        "fn value() { run!(Decimal::from_str(input)); }",
        "fn value() { input.parse::<rust_decimal::Decimal>(); }",
        "use rust_decimal::{Decimal as D}; fn value() { input.parse::<D>(); }",
        "fn value() { Decimal::round_dp(&amount, 2); }",
        "fn value() { core::convert::TryFrom::try_from(Decimal::from_f64_retain(value)); }",
    ] {
        assert_eq!(
            rules
                .findings(OUTSIDE, source.parse().expect("tokens"))
                .len(),
            1,
            "{source}"
        );
    }
    for source in [
        "fn value() { TokenStream::from_str(input); }",
        "fn value() { input.parse::<u32>(); }",
        "fn value() { let text = \"amount.round_dp(2)\"; }",
    ] {
        assert!(reported(&rules, OUTSIDE, source).is_empty(), "{source}");
    }
    assert_eq!(
        reported(&rules, PARSER, "fn value() { Decimal::from_str(input); }"),
        BTreeSet::from([String::from("decimal-method:from_str")])
    );
    assert_eq!(
        Rule::DecimalMethod(String::from("round")).to_string(),
        "decimal-method:round"
    );
}
