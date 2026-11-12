use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use crate::check_support::read_text;
use crate::harness_workspace_tests::{
    copy_tree, fresh_directory, generate_lockfile, repository_root,
};

use super::functions::function_lines;
use super::lcov::parse_lcov;
use super::{
    EXCEPTIONS_FILE, POLICY_FILE, Policy, Rule, Threshold, check_workspace, evaluate,
    load_exceptions, parse_policy,
};

const CASES_FILE: &str = "harness/coverage/cases.toml";
const EXAMPLES_DIRECTORY: &str = "harness/coverage";
const WORKSPACE_EXAMPLE: &str = "harness/coverage/workspace";
const UNTESTED_EXAMPLE: &str = "harness/coverage/untested-workspace";

const RULES: [Rule; 2] = [Rule::UncoveredLine, Rule::UntestedFile];

const DOCUMENTED_PACKAGES: [&str; 3] = ["domain", "order-execution", "application"];

const DOCUMENTED_THRESHOLDS: [(&str, u32); 10] = [
    ("crates/domain/src", 100),
    ("crates/order-execution/src", 100),
    ("crates/application/src/order_gate", 100),
    ("crates/domain/src/signal", 100),
    ("crates/domain/src/threshold", 100),
    ("crates/domain/src/margin", 100),
    ("crates/domain/src/calibration", 100),
    ("crates/domain/src/order", 100),
    ("crates/domain/src/validate", 100),
    ("crates/domain/src/outcome", 100),
];

fn cases_table(root: &Path) -> toml::Table {
    read_text(&root.join(CASES_FILE))
        .expect("read coverage cases")
        .parse()
        .expect("parse coverage cases")
}

fn text_list(value: Option<&toml::Value>) -> BTreeSet<String> {
    value
        .and_then(toml::Value::as_array)
        .expect("text list")
        .iter()
        .map(|item| item.as_str().map(String::from).expect("text item"))
        .collect()
}

fn root_policy(root: &Path) -> Policy {
    parse_policy(&read_text(&root.join(POLICY_FILE)).expect("read policy")).expect("parse policy")
}

fn excepted_from(entry: &toml::Value) -> BTreeMap<String, BTreeSet<usize>> {
    entry
        .get("excepted")
        .and_then(toml::Value::as_table)
        .map_or_else(BTreeMap::new, |table| {
            table
                .iter()
                .map(|(path, lines)| {
                    let numbers = lines
                        .as_array()
                        .expect("excepted lines")
                        .iter()
                        .map(|line| {
                            line.as_integer()
                                .and_then(|value| usize::try_from(value).ok())
                                .expect("line number")
                        })
                        .collect();
                    (path.clone(), numbers)
                })
                .collect()
        })
}

fn shown<R: std::fmt::Display, I: IntoIterator<Item = R>>(findings: I) -> BTreeSet<String> {
    findings.into_iter().map(|item| item.to_string()).collect()
}

#[test]
fn coverage_policy_holds_the_documented_thresholds() {
    let policy = root_policy(&repository_root());
    assert_eq!(policy.packages, DOCUMENTED_PACKAGES.map(String::from));
    let expected: Vec<Threshold> = DOCUMENTED_THRESHOLDS
        .into_iter()
        .map(|(path, percent)| Threshold {
            path: String::from(path),
            percent,
        })
        .collect();
    assert_eq!(policy.thresholds, expected);
}

#[test]
fn coverage_exceptions_start_empty() {
    let exceptions = load_exceptions(&repository_root(), EXCEPTIONS_FILE).expect("load exceptions");
    assert!(exceptions.is_empty(), "{exceptions:?}");
}

#[test]
fn lcov_cases_report_exactly_their_findings() {
    let root = repository_root();
    let policy = root_policy(&root);
    let table = cases_table(&root);
    let mismatches: Vec<String> = table
        .get("case")
        .and_then(toml::Value::as_array)
        .expect("coverage cases")
        .iter()
        .filter_map(|entry| {
            let name = entry
                .get("name")
                .and_then(toml::Value::as_str)
                .expect("name");
            let lcov = entry
                .get("lcov")
                .and_then(toml::Value::as_str)
                .expect("lcov");
            let expected = text_list(entry.get("expect"));
            let reported = shown(evaluate(
                &policy,
                &parse_lcov(lcov, &root),
                &excepted_from(entry),
            ));
            (reported != expected)
                .then(|| format!("{name}: expected {expected:?}, reported {reported:?}"))
        })
        .collect();
    assert!(mismatches.is_empty(), "{mismatches:#?}");
}

#[test]
fn every_coverage_rule_and_threshold_has_an_example() {
    let root = repository_root();
    let table = cases_table(&root);
    let mut reported: Vec<String> = table
        .get("case")
        .and_then(toml::Value::as_array)
        .expect("coverage cases")
        .iter()
        .flat_map(|entry| text_list(entry.get("expect")))
        .collect();
    reported.extend(text_list(
        table
            .get("untested-workspace")
            .and_then(|entry| entry.get("expect")),
    ));
    let missing_rules: Vec<String> = RULES
        .iter()
        .map(ToString::to_string)
        .filter(|rule| !reported.iter().any(|line| line.ends_with(rule.as_str())))
        .collect();
    assert!(missing_rules.is_empty(), "{missing_rules:?}");
    let policy = root_policy(&root);
    let failing_paths: Vec<&str> = reported
        .iter()
        .filter_map(|line| line.split(':').next())
        .collect();
    let most_specific = |path: &str| {
        policy
            .thresholds
            .iter()
            .filter(|threshold| super::matches(&threshold.path, path))
            .max_by_key(|threshold| threshold.path.len())
            .map(|threshold| threshold.path.clone())
    };
    let missing_paths: Vec<&Threshold> = policy
        .thresholds
        .iter()
        .filter(|threshold| {
            !failing_paths
                .iter()
                .any(|path| most_specific(path).as_ref() == Some(&threshold.path))
        })
        .collect();
    assert!(
        missing_paths.is_empty(),
        "thresholds without a failing example: {missing_paths:?}"
    );
}

#[test]
fn exception_functions_resolve_to_their_lines() {
    let root = repository_root();
    let table = cases_table(&root);
    let mismatches: Vec<String> = table
        .get("function")
        .and_then(toml::Value::as_array)
        .expect("function cases")
        .iter()
        .filter_map(|entry| {
            let example = entry
                .get("example")
                .and_then(toml::Value::as_str)
                .expect("example");
            let name = entry
                .get("name")
                .and_then(toml::Value::as_str)
                .expect("name");
            let expected: Vec<BTreeSet<usize>> = entry
                .get("expect")
                .and_then(toml::Value::as_array)
                .expect("expect spans")
                .iter()
                .map(|span| {
                    let bounds: Vec<usize> = span
                        .as_array()
                        .expect("span")
                        .iter()
                        .map(|bound| {
                            bound
                                .as_integer()
                                .and_then(|value| usize::try_from(value).ok())
                                .expect("bound")
                        })
                        .collect();
                    match bounds.as_slice() {
                        [start, end] => (*start..=*end).collect(),
                        _ => BTreeSet::new(),
                    }
                })
                .collect();
            let source = read_text(&root.join(EXAMPLES_DIRECTORY).join(example)).expect("example");
            let found = function_lines(&source, name);
            (found != expected).then(|| format!("{name}: expected {expected:?}, found {found:?}"))
        })
        .collect();
    assert!(mismatches.is_empty(), "{mismatches:#?}");
}

#[test]
fn exception_list_rejects_missing_paths_and_functions() {
    let root = repository_root();
    let directory = fresh_directory(&root, "coverage-exceptions");
    fs::create_dir_all(directory.join("src")).expect("create source directory");
    fs::write(
        directory.join("src/lib.rs"),
        "fn kept() {}\nmod a { fn twice() {} }\nmod b { fn twice() {} }\n",
    )
    .expect("write source");
    let write =
        |text: &str| fs::write(directory.join("exceptions.toml"), text).expect("write list");
    let attempt = || -> Result<BTreeMap<String, BTreeSet<usize>>, String> {
        let exceptions = load_exceptions(&directory, "exceptions.toml")?;
        super::excepted_lines(&directory, &exceptions)
    };
    write("[[exception]]\npath = \"src/lib.rs\"\nfunction = \"kept\"\nreason = \"harness\"\n");
    assert_eq!(
        attempt().expect("existing function"),
        BTreeMap::from([(String::from("src/lib.rs"), BTreeSet::from([1]))])
    );
    for (text, message) in [
        (
            "[[exception]]\npath = \"src/gone.rs\"\nfunction = \"kept\"\nreason = \"harness\"\n",
            "gone.rs",
        ),
        (
            "[[exception]]\npath = \"src/lib.rs\"\nfunction = \"absent\"\nreason = \"harness\"\n",
            "absent",
        ),
        (
            "[[exception]]\npath = \"src/lib.rs\"\nfunction = \"twice\"\nreason = \"harness\"\n",
            "여러 개",
        ),
        (
            "[[exception]]\npath = \"src/lib.rs\"\nfunction = \"kept\"\n",
            "reason",
        ),
        ("exception = 1\n", "exception"),
    ] {
        write(text);
        let outcome = attempt();
        assert!(
            outcome
                .as_ref()
                .is_err_and(|failure| failure.contains(message)),
            "{text}: {outcome:?}"
        );
    }
}

#[test]
fn policy_rejects_malformed_thresholds() {
    for text in [
        "packages = []\nthreshold = []\n",
        "packages = [\"domain\"]\n",
        "packages = [\"domain\"]\n[[threshold]]\npath = \"crates/domain/src\"\npercent = 0\nreason = \"x\"\n",
        "packages = [\"domain\"]\n[[threshold]]\npath = \"crates/domain/src\"\npercent = 101\nreason = \"x\"\n",
        "packages = [\"domain\"]\n[[threshold]]\npath = \"crates/domain/src\"\npercent = 95\n",
        "packages = [\"domain\"]\n[[threshold]]\npath = \" \"\npercent = 95\nreason = \"x\"\n",
    ] {
        assert!(parse_policy(text).is_err(), "{text}");
    }
}

#[test]
fn measured_example_workspace_reports_expected_findings() {
    let root = repository_root();
    let table = cases_table(&root);
    let workspace_case = table.get("workspace").expect("workspace case");
    let directory = fresh_directory(&root, "coverage-workspace");
    copy_tree(&root.join(WORKSPACE_EXAMPLE), &directory);
    generate_lockfile(&directory.join("Cargo.toml"));
    let exceptions_file = workspace_case
        .get("exceptions")
        .and_then(toml::Value::as_str)
        .expect("exceptions file");
    fs::copy(
        root.join(exceptions_file),
        directory.join("coverage-exceptions.toml"),
    )
    .expect("copy exceptions");
    let exceptions =
        load_exceptions(&directory, "coverage-exceptions.toml").expect("load example exceptions");
    let findings =
        check_workspace(&directory, &root_policy(&root), &exceptions).expect("measure coverage");
    assert_eq!(shown(findings), text_list(workspace_case.get("expect")));
}

#[test]
fn workspace_without_tests_fails_files_with_functions() {
    let root = repository_root();
    let table = cases_table(&root);
    let directory = fresh_directory(&root, "coverage-untested-workspace");
    copy_tree(&root.join(UNTESTED_EXAMPLE), &directory);
    generate_lockfile(&directory.join("Cargo.toml"));
    let findings = check_workspace(&directory, &root_policy(&root), &[]).expect("check workspace");
    assert_eq!(
        shown(findings),
        text_list(
            table
                .get("untested-workspace")
                .and_then(|entry| entry.get("expect"))
        )
    );
}

const RELAXED_EXAMPLE: &str = "crates/domain/src/report";

fn lcov_text(path: &str, hit: usize, missed: usize) -> String {
    let records: Vec<String> = (1..=hit.saturating_add(missed))
        .map(|line| format!("DA:{line},{}", u8::from(line <= hit)))
        .collect();
    format!("SF:{path}\n{}\nend_of_record\n", records.join("\n"))
}

fn relaxed_text(root: &Path, path: &str) -> String {
    read_text(&root.join(POLICY_FILE))
        .expect("read policy")
        .replacen(
            "relaxed = []",
            &format!("relaxed = [{{ path = \"{path}\", reason = \"harness\" }}]"),
            1,
        )
}

#[test]
fn relaxed_list_starts_empty() {
    assert_eq!(
        root_policy(&repository_root()).relaxed,
        Vec::<String>::new()
    );
}

#[test]
fn only_relaxed_paths_get_ninety_five_percent() {
    let root = repository_root();
    let policy = parse_policy(&relaxed_text(&root, RELAXED_EXAMPLE)).expect("relaxed policy");
    let cases = [
        (format!("{RELAXED_EXAMPLE}.rs"), 19, 1, BTreeSet::new()),
        (
            format!("{RELAXED_EXAMPLE}/detail.rs"),
            18,
            2,
            BTreeSet::from([format!(
                "{RELAXED_EXAMPLE}/detail.rs: coverage:below-threshold"
            )]),
        ),
        (
            String::from("crates/domain/src/sizing.rs"),
            19,
            1,
            BTreeSet::from([String::from(
                "crates/domain/src/sizing.rs:20: coverage:uncovered-line",
            )]),
        ),
    ];
    for (path, hit, missed, expected) in cases {
        let reported = shown(evaluate(
            &policy,
            &parse_lcov(&lcov_text(&path, hit, missed), &root),
            &BTreeMap::new(),
        ));
        assert_eq!(reported, expected, "{path}");
    }
}

#[test]
fn relaxed_list_cannot_lower_full_modules() {
    let root = repository_root();
    for path in [
        "crates/domain/src/signal",
        "crates/domain/src/signal/decision",
        "crates/domain/src/order.rs",
        "crates/domain/src",
        "crates/order-execution/src/lib.rs",
        "crates/application/src/order_gate",
        "crates/application/src/portfolio",
    ] {
        assert!(
            parse_policy(&relaxed_text(&root, path)).is_err(),
            "{path} was relaxed"
        );
    }
    assert_eq!(
        parse_policy(&relaxed_text(&root, RELAXED_EXAMPLE))
            .expect("relaxed policy")
            .relaxed,
        vec![String::from(RELAXED_EXAMPLE)]
    );
}

#[test]
fn region_and_unmeasured_rules_have_examples() {
    let table = cases_table(&repository_root());
    let expected = text_list(table.get("workspace").and_then(|entry| entry.get("expect")));
    for rule in [Rule::UncoveredRegion, Rule::UnmeasuredFile] {
        let name = rule.to_string();
        assert!(
            expected.iter().any(|line| line.ends_with(name.as_str())),
            "{name} has no example"
        );
    }
}
