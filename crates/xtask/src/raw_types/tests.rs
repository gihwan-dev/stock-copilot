use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use syn::visit::Visit;

use crate::data_variants_tests::element_variants;
use crate::harness_workspace_tests::repository_root;
use crate::workspace_sources::load_members;

use super::crates::{CRATES_FILE, CrateRules, Scope};
use super::rules::{RULES_FILE, RawRules};
use super::scan::{Definitions, ModuleScan};
use super::{
    Finding, ParsedFile, Rule, check_crate, check_members, crate_sources, file_findings,
    load_crate_rules, load_raw_rules,
};

const CASES_FILE: &str = "harness/raw-types/cases.toml";
const SOURCES_DIRECTORY: &str = "harness/raw-types/sources";
const CRATES_DIRECTORY: &str = "harness/raw-types/crates";
const SUPPORT_FILE: &str = "harness/raw-types/support.rs";

const REGRESSION_NAMES: [&str; 47] = [
    "String",
    "str",
    "char",
    "OsString",
    "OsStr",
    "CString",
    "CStr",
    "PathBuf",
    "Path",
    "i8",
    "i16",
    "i32",
    "i64",
    "i128",
    "isize",
    "u8",
    "u16",
    "u32",
    "u64",
    "u128",
    "usize",
    "NonZero",
    "NonZeroI8",
    "NonZeroI16",
    "NonZeroI32",
    "NonZeroI64",
    "NonZeroI128",
    "NonZeroIsize",
    "NonZeroU8",
    "NonZeroU16",
    "NonZeroU32",
    "NonZeroU64",
    "NonZeroU128",
    "NonZeroUsize",
    "f32",
    "f64",
    "Decimal",
    "bool",
    "c_int",
    "c_double",
    "c_char",
    "RawFd",
    "AtomicBool",
    "AtomicU64",
    "Duration",
    "Wrapping",
    "Ipv4Addr",
];

const REQUIRED_CLEAN_EXAMPLES: [&str; 11] = [
    "clean-newtype-borrowed.rs",
    "clean-newtype-fields.rs",
    "clean-generics.rs",
    "clean-standard-paths.rs",
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
    dependencies: Vec<String>,
    expect: BTreeSet<String>,
}

#[derive(Debug)]
struct Fixtures {
    crates: CrateRules,
    rules: RawRules,
    support: Definitions,
}

fn fixtures(root: &Path) -> Fixtures {
    let support = fs::read_to_string(root.join(SUPPORT_FILE)).expect("read support");
    let mut modules = ModuleScan::default();
    modules.visit_file(&syn::parse_file(&support).expect("parse support"));
    Fixtures {
        crates: load_crate_rules(root).expect("load crate rules"),
        rules: load_raw_rules(root).expect("load raw rules"),
        support: modules.definitions,
    }
}

fn check_source(
    label: &str,
    source: &str,
    scope: Scope,
    rules: &RawRules,
    support: &Definitions,
) -> BTreeSet<Finding> {
    let mut findings = BTreeSet::new();
    if scope == Scope::Unchecked {
        return findings;
    }
    match syn::parse_file(source) {
        Ok(file) => {
            let mut modules = ModuleScan::default();
            modules.visit_file(&file);
            let mut local = modules.definitions;
            local.extend(support);
            let parsed = ParsedFile {
                label: String::from(label),
                file,
            };
            file_findings(&parsed, rules, &local, &BTreeMap::new(), &mut findings);
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
            dependencies: texts(entry, "dependencies"),
            expect: texts(entry, "expect").into_iter().collect(),
        })
        .collect()
}

fn source_mismatches(root: &Path, fixtures: &Fixtures) -> Vec<String> {
    source_cases(root)
        .into_iter()
        .filter_map(|case| {
            let source = fs::read_to_string(root.join(SOURCES_DIRECTORY).join(&case.example))
                .expect("read example");
            let reported: BTreeSet<String> = check_source(
                &case.example,
                &source,
                fixtures.crates.scope(&case.crate_name),
                &fixtures.rules,
                &fixtures.support,
            )
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

fn repository_outcome(root: &Path, fixtures: &Fixtures) -> Result<Vec<String>, String> {
    let members = load_members(Some(&root.join("Cargo.toml"))).map_err(|error| error.0)?;
    check_members(root, &members.packages, &fixtures.crates, &fixtures.rules)
        .map(|findings| findings.iter().map(ToString::to_string).collect())
        .map_err(|error| error.0)
}

fn rule_catalogue() -> BTreeSet<String> {
    REGRESSION_NAMES
        .into_iter()
        .map(|name| Rule::RawType(String::from(name)))
        .chain([
            Rule::TypeAlias,
            Rule::OpaqueItemMacro,
            Rule::Unparsable,
            Rule::Unit,
            Rule::GlobImport,
            Rule::UseRename,
        ])
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
    let mismatches = source_mismatches(&root, &fixtures(&root));
    assert!(mismatches.is_empty(), "{mismatches:#?}");
}

#[test]
fn option_string_and_vec_u64_fail() {
    let loaded = fixtures(&repository_root());
    let option = check_source(
        "option.rs",
        "pub struct AccountId; pub fn find(id: AccountId) -> Option<String> { id.label() }",
        loaded.crates.scope("domain"),
        &loaded.rules,
        &Definitions::default(),
    );
    let vector = check_source(
        "vector.rs",
        "pub struct Ids { values: Vec<u64> }",
        loaded.crates.scope("application"),
        &loaded.rules,
        &Definitions::default(),
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

fn crate_mismatches(root: &Path, fixtures: &Fixtures) -> Vec<String> {
    crate_cases(root)
        .into_iter()
        .filter_map(|case| {
            let directory = root.join(CRATES_DIRECTORY);
            let roots: Vec<_> = case
                .roots
                .iter()
                .map(|path| directory.join(&case.name).join(path))
                .collect();
            let dependencies: BTreeMap<String, Definitions> = case
                .dependencies
                .iter()
                .map(|name| {
                    let sources = crate_sources(
                        &directory,
                        &[directory.join(name).join("src").join("lib.rs")],
                    )
                    .expect("read dependency crate");
                    (name.replace('-', "_"), sources.definitions)
                })
                .collect();
            let mut findings = BTreeSet::new();
            check_crate(
                &directory.join(&case.name),
                &roots,
                &fixtures.rules,
                &dependencies,
                &mut findings,
            )
            .expect("check example crate");
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
        .collect()
}

#[test]
fn crate_examples_follow_the_module_tree() {
    let root = repository_root();
    let mismatches = crate_mismatches(&root, &fixtures(&root));
    assert!(mismatches.is_empty(), "{mismatches:#?}");
}

#[test]
fn repository_passes_raw_types() {
    let root = repository_root();
    assert_eq!(repository_outcome(&root, &fixtures(&root)), Ok(Vec::new()));
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
    let mut loaded = fixtures(&root);
    loaded.crates = CrateRules::parse(&table).expect("parse reduced rules");
    let outcome = repository_outcome(&root, &loaded);
    assert!(
        outcome
            .as_ref()
            .is_err_and(|message| message.contains("xtask")),
        "{outcome:?}"
    );
}

fn read_value(root: &Path, file: &str) -> toml::Value {
    toml::Value::Table(
        fs::read_to_string(root.join(file))
            .expect("read data file")
            .parse()
            .expect("parse data file"),
    )
}

fn variant_passes(root: &Path, base: &Fixtures, crates: &toml::Value, rules: &toml::Value) -> bool {
    let parsed = crates
        .as_table()
        .and_then(|table| CrateRules::parse(table).ok())
        .zip(
            rules
                .as_table()
                .and_then(|table| RawRules::parse(table).ok()),
        );
    parsed.is_some_and(|(crate_rules, raw_rules)| {
        let variant = Fixtures {
            crates: crate_rules,
            rules: raw_rules,
            support: base.support.clone(),
        };
        source_mismatches(root, &variant).is_empty()
            && crate_mismatches(root, &variant).is_empty()
            && repository_outcome(root, &variant) == Ok(Vec::new())
    })
}

#[test]
fn removing_any_data_element_breaks_some_test() {
    let root = repository_root();
    let base = fixtures(&root);
    let crates = read_value(&root, CRATES_FILE);
    let rules = read_value(&root, RULES_FILE);
    assert!(
        variant_passes(&root, &base, &crates, &rules),
        "data files must pass before mutation"
    );
    let crate_variants = element_variants(&crates, "crates")
        .into_iter()
        .map(|(label, variant)| (label, variant, rules.clone()));
    let rule_variants = element_variants(&rules, "rules")
        .into_iter()
        .map(|(label, variant)| (label, crates.clone(), variant));
    let variants: Vec<(String, toml::Value, toml::Value)> =
        crate_variants.chain(rule_variants).collect();
    assert!(variants.len() > 200, "too few variants: {}", variants.len());
    let unnoticed: Vec<String> = variants
        .into_iter()
        .filter(|(_, crate_variant, rule_variant)| {
            variant_passes(&root, &base, crate_variant, rule_variant)
        })
        .map(|(label, _, _)| label)
        .collect();
    assert!(
        unnoticed.is_empty(),
        "data elements without a test: {unnoticed:#?}"
    );
}
