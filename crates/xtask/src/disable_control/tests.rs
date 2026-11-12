use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::data_variants_tests::element_variants;
use crate::harness_workspace_tests::{fresh_directory, repository_root};

use super::allowlist::{ALLOWLIST_FILE, Allowlist};
use super::expects::Target;
use super::lints::{LINTS_FILE, LintCatalogue};
use super::{
    Policy, RUST_EXPECT_KIND, Row, Rule, check_source, render_report, repository_rows,
    repository_sources,
};

const CASES_FILE: &str = "harness/disable-control/cases.toml";
const SOURCES_DIRECTORY: &str = "harness/disable-control/sources";
const MAIN_RS: &str = "apps/desktop/src-tauri/src/main.rs";
const GENERATE_CONTEXT: &str = "let context = tauri::generate_context!();";

const DISABLEABLE_GROUPS: [&str; 3] = ["clippy::pedantic", "clippy::nursery", "clippy::cargo"];

const RUSTC_GROUPS_HEADER: &str = "Lint groups provided by rustc:";
const TOOL_GROUPS_HEADER: &str = "Lint groups loaded by this crate:";

const PLACEMENT_RULES: [Target; 8] = [
    Target::Module,
    Target::Impl,
    Target::Trait,
    Target::ForeignBlock,
    Target::Macro,
    Target::Block,
    Target::Inner,
    Target::Other,
];

const ALLOWLISTED_LINTS: [&str; 3] = [
    "clippy::disallowed_macros",
    "clippy::disallowed_methods",
    "clippy::disallowed_types",
];

const REQUIRED_CLEAN_EXAMPLES: [&str; 7] = [
    "clean-pedantic-function.rs",
    "clean-nursery-statement.rs",
    "clean-expression-statement.rs",
    "clean-cargo-item.rs",
    "clean-field-variant.rs",
    "clean-impl-method.rs",
    "clean-allowlisted-generate-context.rs",
];

#[derive(Debug)]
struct SourceCase {
    example: String,
    path: String,
    expect: BTreeSet<String>,
}

fn text(entry: &toml::Value, key: &str) -> String {
    entry
        .get(key)
        .and_then(toml::Value::as_str)
        .map(String::from)
        .expect("case text field")
}

fn source_cases(root: &Path) -> Vec<SourceCase> {
    let table: toml::Table = fs::read_to_string(root.join(CASES_FILE))
        .expect("read cases")
        .parse()
        .expect("parse cases");
    table
        .get("source")
        .and_then(toml::Value::as_array)
        .expect("source cases")
        .iter()
        .map(|entry| SourceCase {
            example: text(entry, "example"),
            path: text(entry, "path"),
            expect: entry
                .get("expect")
                .and_then(toml::Value::as_array)
                .expect("expect list")
                .iter()
                .map(|rule| rule.as_str().map(String::from).expect("rule name"))
                .collect(),
        })
        .collect()
}

fn read_document(root: &Path, file: &str) -> toml::Table {
    fs::read_to_string(root.join(file))
        .expect("read data file")
        .parse()
        .expect("parse data file")
}

fn example_sources(root: &Path) -> Vec<(SourceCase, String)> {
    source_cases(root)
        .into_iter()
        .map(|case| {
            let source = fs::read_to_string(root.join(SOURCES_DIRECTORY).join(&case.example))
                .expect("read example");
            (case, source)
        })
        .collect()
}

fn mismatches(examples: &[(SourceCase, String)], policy: &Policy) -> Vec<String> {
    examples
        .iter()
        .filter_map(|(case, source)| {
            let reported: BTreeSet<String> = check_source(&case.path, source, policy)
                .iter()
                .map(|finding| finding.rule.to_string())
                .collect();
            (reported != case.expect).then(|| {
                format!(
                    "{} as {}: expected {:?}, reported {reported:?}",
                    case.example, case.path, case.expect
                )
            })
        })
        .collect()
}

fn repository_findings(sources: &[(String, String)], policy: &Policy) -> Vec<String> {
    sources
        .iter()
        .flat_map(|(path, source)| check_source(path, source, policy))
        .map(|finding| finding.to_string())
        .collect()
}

fn clippy_driver(root: &Path) -> PathBuf {
    let output = Command::new("rustup")
        .args(["which", "clippy-driver"])
        .current_dir(root)
        .output()
        .expect("rustup which clippy-driver");
    assert!(output.status.success(), "rustup which clippy-driver failed");
    PathBuf::from(
        String::from_utf8(output.stdout)
            .expect("utf-8 driver path")
            .trim(),
    )
}

fn driver_output(root: &Path, arguments: &[&str]) -> String {
    let output = Command::new(clippy_driver(root))
        .args(arguments)
        .current_dir(root)
        .output()
        .expect("run clippy-driver");
    let mut text = String::from_utf8(output.stdout).expect("utf-8 stdout");
    text.push_str(&String::from_utf8(output.stderr).expect("utf-8 stderr"));
    text
}

fn help_groups(help: &str) -> BTreeMap<String, Vec<String>> {
    let mut groups = BTreeMap::new();
    let mut inside = false;
    for line in help.lines() {
        if line == RUSTC_GROUPS_HEADER || line == TOOL_GROUPS_HEADER {
            inside = true;
            continue;
        }
        if !line.is_empty() && !line.starts_with(' ') {
            inside = false;
            continue;
        }
        let Some((name, members)) = line.trim().split_once("  ") else {
            continue;
        };
        if !inside || name == "name" || name == "----" {
            continue;
        }
        groups.insert(
            name.replace('-', "_"),
            members
                .trim()
                .split(", ")
                .map(|member| member.replace('-', "_"))
                .collect(),
        );
    }
    groups
}

fn clippy_catalogue(root: &Path) -> LintCatalogue {
    let groups = help_groups(&driver_output(root, &["-W", "help"]));
    let disableable = DISABLEABLE_GROUPS
        .into_iter()
        .flat_map(|group| groups.get(group).cloned().expect("disableable group"))
        .collect();
    LintCatalogue {
        clippy: String::from(driver_output(root, &["--version"]).trim()),
        groups: groups.into_keys().collect(),
        disableable,
    }
}

fn render_catalogue(catalogue: &LintCatalogue) -> String {
    let list = |names: &BTreeSet<String>| {
        names
            .iter()
            .map(|name| format!("  \"{name}\",\n"))
            .collect::<Vec<_>>()
            .concat()
    };
    [
        format!("clippy = \"{}\"\n", catalogue.clippy),
        format!("groups = [\n{}]\n", list(&catalogue.groups)),
        format!("disableable = [\n{}]\n", list(&catalogue.disableable)),
    ]
    .join("\n")
}

fn rule_catalogue() -> BTreeSet<String> {
    [Rule::LintGroup, Rule::NotDisableable, Rule::Unparsable]
        .into_iter()
        .chain(PLACEMENT_RULES.map(Rule::Placement))
        .map(|rule| rule.to_string())
        .collect()
}

#[test]
fn every_example_is_listed_in_cases() {
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
fn examples_report_exactly_their_rules() {
    let root = repository_root();
    let policy = Policy::load(&root).expect("load policy");
    let found = mismatches(&example_sources(&root), &policy);
    assert!(found.is_empty(), "{found:#?}");
}

#[test]
fn every_rule_has_a_violation_example() {
    let exemplified: BTreeSet<String> = source_cases(&repository_root())
        .into_iter()
        .flat_map(|case| case.expect)
        .collect();
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
fn lint_catalogue_matches_pinned_clippy() {
    let root = repository_root();
    let pinned = LintCatalogue::parse(&read_document(&root, LINTS_FILE)).expect("parse lints");
    let actual = clippy_catalogue(&root);
    assert!(
        pinned == actual,
        "{LINTS_FILE} differs from clippy-driver -W help; regenerated:\n{}",
        render_catalogue(&actual)
    );
}

#[test]
fn allowlist_holds_only_generate_context_in_main_rs() {
    let root = repository_root();
    let table = read_document(&root, ALLOWLIST_FILE);
    let listed: BTreeSet<(String, String, String)> = table
        .get("expect")
        .and_then(toml::Value::as_array)
        .expect("allowlist entries")
        .iter()
        .map(|entry| {
            (
                text(entry, "path"),
                text(entry, "statement"),
                text(entry, "lint"),
            )
        })
        .collect();
    let documented: BTreeSet<(String, String, String)> = ALLOWLISTED_LINTS
        .into_iter()
        .map(|lint| {
            (
                String::from(MAIN_RS),
                String::from(GENERATE_CONTEXT),
                String::from(lint),
            )
        })
        .collect();
    assert_eq!(listed, documented, "allowlist entries");
}

#[test]
fn allowlist_rejects_missing_files_and_non_let_statements() {
    let root = repository_root();
    let directory = fresh_directory(&root, "disable-control-allowlist");
    let entry = |path: &str, statement: &str| -> toml::Table {
        format!(
            "[[expect]]\npath = \"{path}\"\nstatement = \"{statement}\"\nlint = \"clippy::disallowed_types\"\nreason = \"harness\"\n"
        )
        .parse()
        .expect("parse allowlist")
    };
    fs::write(directory.join("main.rs"), "fn main() {}\n").expect("write main");
    let missing = Allowlist::parse(&directory, &entry("absent.rs", "let value = make();"));
    assert!(
        missing
            .as_ref()
            .is_err_and(|failure| failure.0.contains("absent.rs")),
        "{missing:?}"
    );
    let expression = Allowlist::parse(&directory, &entry("main.rs", "make();"));
    assert!(expression.is_err(), "{expression:?}");
    let accepted = Allowlist::parse(&directory, &entry("main.rs", "let value = make();"));
    assert!(accepted.is_ok(), "{accepted:?}");
}

#[test]
fn repository_passes_disable_check() {
    let root = repository_root();
    let policy = Policy::load(&root).expect("load policy");
    let sources = repository_sources(Some(&root)).expect("read repository sources");
    let found = repository_findings(&sources, &policy);
    assert!(found.is_empty(), "{found:#?}");
}

#[test]
fn disable_report_lists_the_three_main_rs_expects() {
    let root = repository_root();
    let found = repository_rows(&root).expect("collect rows");
    let listed: BTreeSet<(&str, &str, &str)> = found
        .iter()
        .map(|row| (row.kind, row.path.as_str(), row.rule.as_str()))
        .collect();
    let expected: BTreeSet<(&str, &str, &str)> = ALLOWLISTED_LINTS
        .into_iter()
        .map(|lint| (RUST_EXPECT_KIND, MAIN_RS, lint))
        .collect();
    assert_eq!(listed, expected, "report rows");
    assert_eq!(found.len(), 3, "one row per expect");
    assert!(
        found
            .iter()
            .all(|row| row.line > 0 && row.reason.contains("generate_context!")),
        "{found:#?}"
    );
    let report = render_report(&found);
    assert_eq!(
        report.lines().last(),
        Some("disable-report: 끄기 3건 (rust-expect 3건)"),
        "{report}"
    );
}

#[test]
fn disable_report_table_takes_other_kinds() {
    let rows = [
        Row {
            kind: RUST_EXPECT_KIND,
            path: String::from("crates/domain/src/lib.rs"),
            line: 3,
            rule: String::from("clippy::needless_pass_by_value"),
            reason: String::from("소비형 | 토큰"),
        },
        Row {
            kind: "eslint-disable-next-line",
            path: String::from("apps/desktop/src/main.tsx"),
            line: 7,
            rule: String::from("local/no-comments"),
            reason: String::from("이유"),
        },
    ];
    assert_eq!(
        render_report(&rows),
        [
            "| 종류 | 파일 | 줄 | 규칙 | 이유 |",
            "|---|---|---|---|---|",
            "| rust-expect | crates/domain/src/lib.rs | 3 | clippy::needless_pass_by_value | 소비형 \\| 토큰 |",
            "| eslint-disable-next-line | apps/desktop/src/main.tsx | 7 | local/no-comments | 이유 |",
            "disable-report: 끄기 2건 (eslint-disable-next-line 1건, rust-expect 1건)",
        ]
        .join("\n"),
        "report table"
    );
}

#[test]
fn removing_any_data_element_breaks_some_test() {
    let root = repository_root();
    let examples = example_sources(&root);
    let sources = repository_sources(Some(&root)).expect("read repository sources");
    let actual = clippy_catalogue(&root);
    let lints = read_document(&root, LINTS_FILE);
    let allowlist = read_document(&root, ALLOWLIST_FILE);
    let passes = |lints_table: &toml::Table, allowlist_table: &toml::Table| {
        Policy::from_tables(&root, lints_table, allowlist_table).is_ok_and(|policy| {
            LintCatalogue::parse(lints_table).is_ok_and(|catalogue| catalogue == actual)
                && mismatches(&examples, &policy).is_empty()
                && repository_findings(&sources, &policy).is_empty()
        })
    };
    assert!(
        passes(&lints, &allowlist),
        "data files must pass before mutation"
    );
    let lint_variants = element_variants(&toml::Value::Table(lints.clone()), "lints")
        .into_iter()
        .map(|(label, variant)| (label, variant, toml::Value::Table(allowlist.clone())));
    let allowlist_variants = element_variants(&toml::Value::Table(allowlist.clone()), "allowlist")
        .into_iter()
        .map(|(label, variant)| (label, toml::Value::Table(lints.clone()), variant));
    let variants: Vec<(String, toml::Value, toml::Value)> =
        lint_variants.chain(allowlist_variants).collect();
    assert!(variants.len() > 240, "too few variants: {}", variants.len());
    let unnoticed: Vec<String> = variants
        .into_iter()
        .filter(|(_, lints_variant, allowlist_variant)| {
            match (lints_variant.as_table(), allowlist_variant.as_table()) {
                (Some(lints_table), Some(allowlist_table)) => passes(lints_table, allowlist_table),
                (Some(_) | None, Some(_) | None) => false,
            }
        })
        .map(|(label, _, _)| label)
        .collect();
    assert!(
        unnoticed.is_empty(),
        "data elements without a test: {unnoticed:#?}"
    );
}
