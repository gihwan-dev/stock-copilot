use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use crate::arch_check::rules::{TransitiveBan, parse_rules};
use crate::arch_check::{
    CrateName, RULES_PATH, Rules, Workspace, check_workspace, load_rules, load_workspace,
};
use crate::harness_workspace::{copy_tree, fresh_directory, generate_lockfile, repository_root};

type Expected = BTreeSet<(String, String)>;
type NameSetField = fn(&mut Rules) -> &mut BTreeSet<CrateName>;

fn expected_cases(root: &Path) -> BTreeMap<String, Expected> {
    let table: toml::Table = fs::read_to_string(root.join("harness/arch-check/cases.toml"))
        .expect("read cases.toml")
        .parse()
        .expect("parse cases.toml");
    table
        .get("case")
        .and_then(toml::Value::as_array)
        .expect("case array")
        .iter()
        .map(|case| {
            let name = case
                .get("name")
                .and_then(toml::Value::as_str)
                .expect("case name");
            let findings = case
                .get("findings")
                .and_then(toml::Value::as_array)
                .expect("findings array")
                .iter()
                .map(|finding| {
                    let field = |key: &str| {
                        String::from(
                            finding
                                .get(key)
                                .and_then(toml::Value::as_str)
                                .expect("finding field"),
                        )
                    };
                    (field("rule"), field("subject"))
                })
                .collect();
            (String::from(name), findings)
        })
        .collect()
}

fn loaded_cases(directory_name: &str) -> (Rules, BTreeMap<String, (Workspace, Expected)>) {
    let root = repository_root();
    let rules = load_rules(&root.join(RULES_PATH)).expect("load rules");
    let directory = fresh_directory(&root, directory_name);
    copy_tree(&root.join("harness").join("arch-check"), &directory);
    let cases = expected_cases(&root)
        .into_iter()
        .map(|(name, expected)| {
            let manifest = directory.join("cases").join(&name).join("Cargo.toml");
            generate_lockfile(&manifest);
            let workspace = load_workspace(&manifest).expect("load case workspace");
            (name, (workspace, expected))
        })
        .collect();
    (rules, cases)
}

fn reported(workspace: &Workspace, rules: &Rules) -> Expected {
    check_workspace(workspace, rules)
        .expect("run arch-check")
        .into_iter()
        .map(|finding| (String::from(finding.rule.name()), finding.subject))
        .collect()
}

fn mismatches(rules: &Rules, cases: &BTreeMap<String, (Workspace, Expected)>) -> Vec<String> {
    cases
        .iter()
        .filter_map(|(name, (workspace, expected))| {
            let actual = reported(workspace, rules);
            (&actual != expected)
                .then(|| format!("{name}\n  expected {expected:?}\n  actual   {actual:?}"))
        })
        .collect()
}

#[test]
fn case_directories_match_case_list() {
    let root = repository_root();
    let directories: BTreeSet<String> = fs::read_dir(root.join("harness/arch-check/cases"))
        .expect("read cases directory")
        .map(|entry| {
            entry
                .expect("case entry")
                .file_name()
                .into_string()
                .expect("utf-8 case name")
        })
        .collect();
    let listed: BTreeSet<String> = expected_cases(&root).into_keys().collect();
    assert_eq!(directories, listed);
}

#[test]
fn every_case_reports_exactly_its_findings() {
    let (rules, cases) = loaded_cases("arch-check-cases");
    let failures = mismatches(&rules, &cases);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn real_workspace_passes_arch_check() {
    let root = repository_root();
    let rules = load_rules(&root.join(RULES_PATH)).expect("load rules");
    let workspace = load_workspace(&root.join("Cargo.toml")).expect("load workspace");
    let findings = reported(&workspace, &rules);
    assert!(findings.is_empty(), "{findings:?}");
}

#[test]
fn unknown_rule_keys_are_rejected() {
    let root = repository_root();
    let content = fs::read_to_string(root.join(RULES_PATH)).expect("read rules");
    assert!(parse_rules(&content).is_ok(), "real rules must parse");
    let misspelled = content.replacen("transitive-forbidden =", "transitive-forbiden =", 1);
    assert_ne!(
        misspelled, content,
        "rules must contain transitive-forbidden"
    );
    assert!(
        parse_rules(&misspelled).is_err(),
        "a misspelled key must not silently drop a rule"
    );
    let extra = format!("{content}\n[surprise]\nkey = 1\n");
    assert!(parse_rules(&extra).is_err(), "unknown tables must fail");
}

fn without<T: Ord + Clone>(set: &BTreeSet<T>, entry: &T) -> BTreeSet<T> {
    let mut copy = set.clone();
    copy.remove(entry);
    copy
}

fn edge_variants(rules: &Rules, variants: &mut Vec<(String, Rules)>) {
    for (name, allowed) in &rules.graph.edges {
        for entry in allowed {
            let mut variant = rules.clone();
            variant
                .graph
                .edges
                .insert(name.clone(), without(allowed, entry));
            variants.push((format!("edges.{name} - {entry}"), variant));
        }
    }
    for (name, allowed) in &rules.graph.dev_edges.crates {
        for entry in allowed {
            let mut variant = rules.clone();
            variant
                .graph
                .dev_edges
                .crates
                .insert(name.clone(), without(allowed, entry));
            variants.push((format!("dev-edges.{name} - {entry}"), variant));
        }
        if allowed != &rules.graph.dev_edges.other_crates {
            let mut variant = rules.clone();
            variant.graph.dev_edges.crates.remove(name);
            variants.push((format!("dev-edges.{name} removed"), variant));
        }
    }
    for entry in &rules.graph.dev_edges.other_crates {
        let mut variant = rules.clone();
        variant.graph.dev_edges.other_crates = without(&rules.graph.dev_edges.other_crates, entry);
        variants.push((format!("dev-edges.other-crates - {entry}"), variant));
    }
}

fn drop_at<T: Clone>(rows: &[T], position: usize) -> Vec<T> {
    rows.iter()
        .enumerate()
        .filter(|(index, _)| *index != position)
        .map(|(_, row)| row.clone())
        .collect()
}

fn restriction_variants(rules: &Rules, variants: &mut Vec<(String, Rules)>) {
    for (index, restriction) in rules.graph.restricted.iter().enumerate() {
        let target = &restriction.target;
        let mut removed = rules.clone();
        removed.graph.restricted = drop_at(&rules.graph.restricted, index);
        variants.push((format!("restricted {target} removed"), removed));
        let mut changed = Vec::new();
        for position in 0..restriction.dependents.len() {
            let mut row = restriction.clone();
            row.dependents = drop_at(&restriction.dependents, position);
            changed.push((format!("restricted {target} dependent {position}"), row));
        }
        if !restriction.any_dependent_kinds.is_empty() {
            let mut row = restriction.clone();
            row.any_dependent_kinds.clear();
            changed.push((format!("restricted {target} any-dependent-kinds"), row));
        }
        match restriction.transitive {
            TransitiveBan::Nothing => {}
            TransitiveBan::Listed(_) | TransitiveBan::AllExcept(_) => {
                let mut row = restriction.clone();
                row.transitive = TransitiveBan::Nothing;
                changed.push((format!("restricted {target} transitive"), row));
            }
        }
        for (label, row) in changed {
            let mut variant = rules.clone();
            if let Some(slot) = variant.graph.restricted.get_mut(index) {
                *slot = row;
            }
            variants.push((label, variant));
        }
    }
}

fn row_variants<T: Clone>(
    rules: &Rules,
    label: &str,
    rows: fn(&mut Rules) -> &mut Vec<T>,
    variants: &mut Vec<(String, Rules)>,
) {
    let mut probe = rules.clone();
    let original = rows(&mut probe).clone();
    for position in 0..original.len() {
        let mut variant = rules.clone();
        *rows(&mut variant) = drop_at(&original, position);
        variants.push((format!("{label} row {position}"), variant));
    }
}

fn graph_row_variants(rules: &Rules, variants: &mut Vec<(String, Rules)>) {
    for entry in &rules.graph.product_bin.forbidden {
        let mut variant = rules.clone();
        variant.graph.product_bin.forbidden = without(&rules.graph.product_bin.forbidden, entry);
        variants.push((format!("product-bin - {entry}"), variant));
    }
    row_variants(
        rules,
        "external-location",
        |target| &mut target.graph.external_locations,
        variants,
    );
    row_variants(
        rules,
        "external-allowlist",
        |target| &mut target.graph.external_allowlists,
        variants,
    );
    row_variants(
        rules,
        "feature-location",
        |target| &mut target.graph.feature_locations,
        variants,
    );
    row_variants(
        rules,
        "feature-allowlist",
        |target| &mut target.graph.feature_allowlists,
        variants,
    );
}

fn source_variants(rules: &Rules, variants: &mut Vec<(String, Rules)>) {
    let source = &rules.source;
    let name_sets: [(&str, NameSetField); 4] = [
        ("source.reexport", |target| {
            &mut target.source.reexport_crates
        }),
        ("source.user-presence", |target| {
            &mut target.source.presence_impl_crates
        }),
        ("source.toss-order", |target| {
            &mut target.source.order_write_crates
        }),
        ("source.toss-host", |target| &mut target.source.host_crates),
    ];
    for (label, field) in name_sets {
        let mut probe = rules.clone();
        for entry in field(&mut probe).clone() {
            let mut variant = rules.clone();
            field(&mut variant).remove(&entry);
            variants.push((format!("{label} - {entry}"), variant));
        }
    }
    for entry in &source.host_files {
        let mut variant = rules.clone();
        variant.source.host_files.remove(entry);
        variants.push((format!("source.toss-host file - {entry}"), variant));
    }
    for entry in &source.write_methods {
        let mut variant = rules.clone();
        variant
            .source
            .write_methods
            .retain(|method| method != entry);
        variants.push((format!("source.write-methods - {entry}"), variant));
    }
    for entry in &source.order_path_segments {
        let mut variant = rules.clone();
        variant
            .source
            .order_path_segments
            .retain(|segment| segment != entry);
        variants.push((format!("source.order-path-segments - {entry}"), variant));
    }
}

#[test]
fn removing_any_rule_entry_breaks_some_case() {
    let (rules, cases) = loaded_cases("arch-check-mutations");
    assert!(
        mismatches(&rules, &cases).is_empty(),
        "cases must pass before mutation"
    );
    let mut variants = Vec::new();
    edge_variants(&rules, &mut variants);
    restriction_variants(&rules, &mut variants);
    graph_row_variants(&rules, &mut variants);
    source_variants(&rules, &mut variants);
    assert!(variants.len() > 100, "too few variants: {}", variants.len());
    let unnoticed: Vec<&String> = variants
        .iter()
        .filter(|(_, variant)| mismatches(variant, &cases).is_empty())
        .map(|(label, _)| label)
        .collect();
    assert!(
        unnoticed.is_empty(),
        "rule entries without a case: {unnoticed:?}"
    );
}

fn command_exit(case: &str, directory_name: &str) -> std::process::ExitCode {
    let root = repository_root();
    let directory = fresh_directory(&root, directory_name);
    copy_tree(&root.join("harness").join("arch-check"), &directory);
    let case_root = directory.join("cases").join(case);
    let rules = case_root.join(RULES_PATH);
    fs::create_dir_all(rules.parent().expect("rules directory")).expect("create rules directory");
    fs::copy(root.join(RULES_PATH), &rules).expect("copy rules");
    generate_lockfile(&case_root.join("Cargo.toml"));
    crate::arch_check::command(&case_root.join("Cargo.toml"))
}

#[test]
fn command_fails_on_violation_and_passes_clean_workspace() {
    assert_eq!(
        command_exit(
            "testkit-depends-on-order-execution",
            "arch-check-command-violation"
        ),
        std::process::ExitCode::FAILURE
    );
    assert_eq!(
        command_exit("full-architecture", "arch-check-command-clean"),
        std::process::ExitCode::SUCCESS
    );
}
