use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use syn::visit::Visit;

use crate::data_variants_tests::element_variants;
use crate::harness_workspace_tests::repository_root;
use crate::workspace_sources::load_members;

use super::catalogue::raw_names;
use super::crates::{CRATES_FILE, CrateRules, Scope};
use super::scan::ModuleScan;
use super::{
    Finding, ParsedFile, Rule, check_crate, check_members, file_findings, load_crate_rules,
};

const CASES_FILE: &str = "harness/raw-types/cases.toml";
const SOURCES_DIRECTORY: &str = "harness/raw-types/sources";
const CRATES_DIRECTORY: &str = "harness/raw-types/crates";

const REQUIRED_CLEAN_EXAMPLES: [&str; 7] = [
    "clean-newtype.rs",
    "clean-standard-trait-bool.rs",
    "clean-enum-instead-of-bool.rs",
    "clean-function-body.rs",
    "clean-test-module.rs",
    "clean-exception-crate.rs",
    "clean-outside-crate.rs",
];

const DOCUMENTED_TARGETS: [&str; 3] = ["domain", "application", "order-execution"];

#[derive(Debug)]
struct SourceCase {
    example: String,
    crate_name: String,
    expect: BTreeSet<String>,
}

#[derive(Debug)]
struct CrateCase {
    name: String,
    roots: Vec<String>,
    expect: BTreeSet<String>,
}

fn check_source(label: &str, source: &str, scope: Scope) -> BTreeSet<Finding> {
    let mut findings = BTreeSet::new();
    if scope == Scope::Unchecked {
        return findings;
    }
    match syn::parse_file(source) {
        Ok(file) => {
            let mut modules = ModuleScan::default();
            modules.visit_file(&file);
            let parsed = ParsedFile {
                label: String::from(label),
                file,
            };
            file_findings(&parsed, &modules.traits, &mut findings);
        }
        Err(_) => {
            findings.insert(Finding {
                path: String::from(label),
                line: 0,
                rule: Rule::Unparsable,
            });
        }
    }
    findings
}

fn text(entry: &toml::Value, key: &str) -> String {
    entry
        .get(key)
        .and_then(toml::Value::as_str)
        .map(String::from)
        .expect("case text field")
}

fn texts(entry: &toml::Value, key: &str) -> Vec<String> {
    entry
        .get(key)
        .and_then(toml::Value::as_array)
        .expect("case list field")
        .iter()
        .map(|value| value.as_str().map(String::from).expect("case list text"))
        .collect()
}

fn entries(root: &Path, key: &str) -> Vec<toml::Value> {
    let table: toml::Table = fs::read_to_string(root.join(CASES_FILE))
        .expect("read cases")
        .parse()
        .expect("parse cases");
    table
        .get(key)
        .and_then(toml::Value::as_array)
        .cloned()
        .expect("case list")
}

fn source_cases(root: &Path) -> Vec<SourceCase> {
    entries(root, "source")
        .iter()
        .map(|entry| SourceCase {
            example: text(entry, "example"),
            crate_name: text(entry, "crate"),
            expect: texts(entry, "expect").into_iter().collect(),
        })
        .collect()
}

fn crate_cases(root: &Path) -> Vec<CrateCase> {
    entries(root, "crate")
        .iter()
        .map(|entry| CrateCase {
            name: text(entry, "name"),
            roots: texts(entry, "roots"),
            expect: texts(entry, "expect").into_iter().collect(),
        })
        .collect()
}

fn source_mismatches(root: &Path, rules: &CrateRules) -> Vec<String> {
    source_cases(root)
        .into_iter()
        .filter_map(|case| {
            let source = fs::read_to_string(root.join(SOURCES_DIRECTORY).join(&case.example))
                .expect("read example");
            let reported: BTreeSet<String> =
                check_source(&case.example, &source, rules.scope(&case.crate_name))
                    .iter()
                    .map(|finding| finding.rule.to_string())
                    .collect();
            (reported != case.expect).then(|| {
                format!(
                    "{} in {}: expected {:?}, reported {reported:?}",
                    case.example, case.crate_name, case.expect
                )
            })
        })
        .collect()
}

fn repository_outcome(root: &Path, rules: &CrateRules) -> Result<Vec<String>, String> {
    let members = load_members(Some(&root.join("Cargo.toml"))).map_err(|error| error.0)?;
    check_members(root, &members.packages, rules)
        .map(|findings| findings.iter().map(ToString::to_string).collect())
        .map_err(|error| error.0)
}

fn rule_catalogue() -> BTreeSet<String> {
    raw_names()
        .into_iter()
        .map(|name| Rule::RawType(String::from(name)))
        .chain([Rule::TypeAlias, Rule::OpaqueItemMacro, Rule::Unparsable])
        .map(|rule| rule.to_string())
        .collect()
}

#[test]
fn every_source_example_is_listed_in_cases() {
    let root = repository_root();
    let listed: BTreeSet<String> = source_cases(&root)
        .into_iter()
        .map(|case| case.example)
        .collect();
    let present: BTreeSet<String> = fs::read_dir(root.join(SOURCES_DIRECTORY))
        .expect("read examples")
        .map(|entry| {
            entry
                .expect("example entry")
                .file_name()
                .to_str()
                .map(String::from)
                .expect("utf-8 example name")
        })
        .collect();
    assert_eq!(
        listed, present,
        "examples on disk and in {CASES_FILE} differ"
    );
}

#[test]
fn source_examples_report_exactly_their_rules() {
    let root = repository_root();
    let rules = load_crate_rules(&root).expect("load crate rules");
    let mismatches = source_mismatches(&root, &rules);
    assert!(mismatches.is_empty(), "{mismatches:#?}");
}

#[test]
fn option_string_and_vec_u64_fail() {
    let rules = load_crate_rules(&repository_root()).expect("load crate rules");
    let option = check_source(
        "option.rs",
        "pub fn find(id: AccountId) -> Option<String> { id.label() }",
        rules.scope("domain"),
    );
    let vector = check_source(
        "vector.rs",
        "pub struct Ids { values: Vec<u64> }",
        rules.scope("application"),
    );
    let reported: Vec<String> = option
        .iter()
        .chain(&vector)
        .map(|finding| finding.rule.to_string())
        .collect();
    assert_eq!(
        reported,
        vec![
            String::from("raw-type:String"),
            String::from("raw-type:u64")
        ],
        "Option<String> and Vec<u64> must fail"
    );
}

#[test]
fn every_rule_has_a_violation_example() {
    let root = repository_root();
    let mut exemplified: BTreeSet<String> = source_cases(&root)
        .into_iter()
        .flat_map(|case| case.expect)
        .collect();
    exemplified.extend(crate_cases(&root).into_iter().flat_map(|case| {
        case.expect
            .into_iter()
            .filter_map(|line| line.split_once(' ').map(|(_, rule)| String::from(rule)))
    }));
    let missing: Vec<String> = rule_catalogue()
        .into_iter()
        .filter(|rule| !exemplified.contains(rule))
        .collect();
    assert!(
        missing.is_empty(),
        "rules without a violation example: {missing:?}"
    );
}

#[test]
fn clean_examples_expect_no_findings() {
    let cases = source_cases(&repository_root());
    let missing: Vec<&str> = REQUIRED_CLEAN_EXAMPLES
        .into_iter()
        .filter(|example| {
            !cases
                .iter()
                .any(|case| case.example == *example && case.expect.is_empty())
        })
        .collect();
    assert!(missing.is_empty(), "clean examples missing: {missing:?}");
}

#[test]
fn every_target_crate_has_a_violation_example() {
    let root = repository_root();
    let rules = load_crate_rules(&root).expect("load crate rules");
    let cases = source_cases(&root);
    let uncovered: Vec<&str> = DOCUMENTED_TARGETS
        .into_iter()
        .filter(|target| {
            rules.scope(target) != Scope::Checked
                || !cases
                    .iter()
                    .any(|case| case.crate_name == *target && !case.expect.is_empty())
        })
        .collect();
    assert!(
        uncovered.is_empty(),
        "targets without an example: {uncovered:?}"
    );
}

#[test]
fn crate_examples_follow_the_module_tree() {
    let root = repository_root();
    let mismatches: Vec<String> = crate_cases(&root)
        .into_iter()
        .filter_map(|case| {
            let directory = root.join(CRATES_DIRECTORY).join(&case.name);
            let roots: Vec<_> = case.roots.iter().map(|path| directory.join(path)).collect();
            let mut findings = BTreeSet::new();
            check_crate(&directory, &roots, &mut findings).expect("check example crate");
            let reported: BTreeSet<String> = findings
                .iter()
                .map(|finding| format!("{} {}", finding.path, finding.rule))
                .collect();
            (reported != case.expect).then(|| {
                format!(
                    "{}: expected {:?}, reported {reported:?}",
                    case.name, case.expect
                )
            })
        })
        .collect();
    assert!(mismatches.is_empty(), "{mismatches:#?}");
}

#[test]
fn repository_passes_raw_types() {
    let root = repository_root();
    let rules = load_crate_rules(&root).expect("load crate rules");
    assert_eq!(repository_outcome(&root, &rules), Ok(Vec::new()));
}

#[test]
fn unclassified_member_fails() {
    let root = repository_root();
    let mut table: toml::Table = fs::read_to_string(root.join(CRATES_FILE))
        .expect("read crate rules")
        .parse()
        .expect("parse crate rules");
    let outside = table
        .get_mut("outside")
        .and_then(toml::Value::as_array_mut)
        .expect("outside list");
    outside.retain(|entry| entry.get("crate").and_then(toml::Value::as_str) != Some("xtask"));
    let rules = CrateRules::parse(&table).expect("parse reduced rules");
    let outcome = repository_outcome(&root, &rules);
    assert!(
        outcome
            .as_ref()
            .is_err_and(|message| message.contains("xtask")),
        "{outcome:?}"
    );
}

#[test]
fn removing_any_crate_rule_element_breaks_some_test() {
    let root = repository_root();
    let document = toml::Value::Table(
        fs::read_to_string(root.join(CRATES_FILE))
            .expect("read crate rules")
            .parse()
            .expect("parse crate rules"),
    );
    let variants = element_variants(&document, "crates");
    assert!(variants.len() > 60, "too few variants: {}", variants.len());
    let unnoticed: Vec<String> = variants
        .into_iter()
        .filter(|(_, variant)| {
            variant
                .as_table()
                .and_then(|table| CrateRules::parse(table).ok())
                .is_some_and(|rules| {
                    source_mismatches(&root, &rules).is_empty()
                        && repository_outcome(&root, &rules) == Ok(Vec::new())
                })
        })
        .map(|(label, _)| label)
        .collect();
    assert!(
        unnoticed.is_empty(),
        "crate rule elements without a test: {unnoticed:#?}"
    );
}
