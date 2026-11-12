use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use crate::check_support::read_text;
use crate::harness_workspace_tests::{
    copy_tree, fresh_directory, generate_lockfile, repository_root,
};

use super::{
    IgnoreFiles, Outcomes, Rule, SURVIVORS_FILE, Scope, Survivor, evaluate, load_survivors, run,
};

const CASES_FILE: &str = "harness/mutants/cases.toml";
const WORKSPACE_EXAMPLE: &str = "harness/mutants/workspace";
const EXAMPLE_MEMBERS: [&str; 2] = ["crates/domain", "apps/desktop/src-tauri"];
const FAILED: &str = "error";

fn cases_table(root: &Path) -> toml::Table {
    read_text(&root.join(CASES_FILE))
        .expect("read mutants cases")
        .parse()
        .expect("parse mutants cases")
}

fn text(entry: &toml::Value, key: &str) -> String {
    entry
        .get(key)
        .and_then(toml::Value::as_str)
        .map(String::from)
        .expect("text field")
}

fn text_list(entry: &toml::Value, key: &str) -> BTreeSet<String> {
    entry
        .get(key)
        .and_then(toml::Value::as_array)
        .expect("text list")
        .iter()
        .map(|item| item.as_str().map(String::from).expect("text item"))
        .collect()
}

fn shown(
    outcome: Result<BTreeSet<crate::check_support::Finding<Rule>>, String>,
) -> BTreeSet<String> {
    match outcome {
        Ok(findings) => findings.iter().map(ToString::to_string).collect(),
        Err(_) => BTreeSet::from([String::from(FAILED)]),
    }
}

fn root_lints(root: &Path) -> toml::Value {
    let manifest: toml::Table = read_text(&root.join("Cargo.toml"))
        .expect("read root manifest")
        .parse()
        .expect("parse root manifest");
    manifest
        .get("workspace")
        .and_then(|workspace| workspace.get("lints"))
        .cloned()
        .expect("workspace lints")
}

fn prepared_workspace(root: &Path) -> std::path::PathBuf {
    let directory = fresh_directory(root, "mutants-workspace");
    copy_tree(&root.join(WORKSPACE_EXAMPLE), &directory);
    let mut workspace = toml::Table::new();
    workspace.insert(String::from("resolver"), toml::Value::from("3"));
    workspace.insert(
        String::from("members"),
        toml::Value::Array(EXAMPLE_MEMBERS.map(toml::Value::from).to_vec()),
    );
    workspace.insert(String::from("lints"), root_lints(root));
    let mut manifest = toml::Table::new();
    manifest.insert(String::from("workspace"), toml::Value::Table(workspace));
    fs::write(
        directory.join("Cargo.toml"),
        toml::to_string(&manifest).expect("serialize manifest"),
    )
    .expect("write manifest");
    fs::create_dir_all(directory.join(".cargo")).expect("create .cargo");
    fs::copy(
        root.join(".cargo/mutants.toml"),
        directory.join(".cargo/mutants.toml"),
    )
    .expect("copy mutants settings");
    generate_lockfile(&directory.join("Cargo.toml"));
    directory
}

#[test]
fn survivor_cases_report_exactly_their_findings() {
    let root = repository_root();
    let table = cases_table(&root);
    let mismatches: Vec<String> = table
        .get("case")
        .and_then(toml::Value::as_array)
        .expect("mutants cases")
        .iter()
        .filter_map(|entry| {
            let survivors: Vec<Survivor> = entry
                .get("survivors")
                .and_then(toml::Value::as_array)
                .expect("survivors")
                .iter()
                .map(|survivor| Survivor {
                    file: text(survivor, "file"),
                    mutant: text(survivor, "mutant"),
                })
                .collect();
            let outcomes = Outcomes {
                missed: text(entry, "missed"),
                timeouts: text(entry, "timeouts"),
            };
            let reported = shown(evaluate(&outcomes, &survivors));
            let expected = text_list(entry, "expect");
            (reported != expected).then(|| {
                format!(
                    "{}: expected {expected:?}, reported {reported:?}",
                    text(entry, "name")
                )
            })
        })
        .collect();
    assert!(mismatches.is_empty(), "{mismatches:#?}");
}

#[test]
fn every_mutants_rule_has_an_example() {
    let root = repository_root();
    let table = cases_table(&root);
    let reported: Vec<String> = table
        .get("case")
        .and_then(toml::Value::as_array)
        .expect("mutants cases")
        .iter()
        .flat_map(|entry| text_list(entry, "expect"))
        .collect();
    let missing: Vec<String> = [Rule::Missed(String::new()), Rule::Timeout(String::new())]
        .iter()
        .map(ToString::to_string)
        .filter(|prefix| !reported.iter().any(|line| line.contains(prefix.as_str())))
        .collect();
    assert!(missing.is_empty(), "rules without an example: {missing:?}");
}

#[test]
fn survivor_list_starts_empty() {
    let survivors = load_survivors(&repository_root(), SURVIVORS_FILE).expect("load survivors");
    assert!(survivors.is_empty(), "{survivors:?}");
}

#[test]
fn survivor_list_rejects_missing_files_and_reasons() {
    let root = repository_root();
    let directory = fresh_directory(&root, "mutants-survivors");
    fs::create_dir_all(directory.join("src")).expect("create source directory");
    fs::write(directory.join("src/lib.rs"), "pub fn kept() {}\n").expect("write source");
    let attempt = |list: &str| {
        fs::write(directory.join("survivors.toml"), list).expect("write list");
        load_survivors(&directory, "survivors.toml")
    };
    assert_eq!(
        attempt("[[survivor]]\nfile = \"src/lib.rs\"\nmutant = \"replace kept with ()\"\nreason = \"harness\"\n")
            .expect("existing file"),
        vec![Survivor {
            file: String::from("src/lib.rs"),
            mutant: String::from("replace kept with ()"),
        }]
    );
    for (list, message) in [
        (
            "[[survivor]]\nfile = \"src/gone.rs\"\nmutant = \"replace kept with ()\"\nreason = \"harness\"\n",
            "gone.rs",
        ),
        (
            "[[survivor]]\nfile = \"src/lib.rs\"\nmutant = \"replace kept with ()\"\n",
            "reason",
        ),
        (
            "[[survivor]]\nfile = \"src/lib.rs\"\nreason = \"harness\"\n",
            "mutant",
        ),
        ("survivors = []\n", "survivor"),
    ] {
        let outcome = attempt(list);
        assert!(
            outcome
                .as_ref()
                .is_err_and(|failure| failure.contains(message)),
            "{list}: {outcome:?}"
        );
    }
}

#[test]
fn weak_test_example_leaves_surviving_mutants() {
    let root = repository_root();
    let directory = prepared_workspace(&root);
    assert!(
        !directory.join("apps/desktop/dist").exists(),
        "example must not have a frontend bundle"
    );
    let outcomes =
        run(&directory, &Scope::Everything, IgnoreFiles::Disregarded).expect("run cargo mutants");
    let table = cases_table(&root);
    let workspace_case = table.get("workspace").expect("workspace case");
    assert_eq!(
        shown(evaluate(&outcomes, &[])),
        text_list(workspace_case, "expect")
    );
    fs::copy(
        root.join(text(workspace_case, "survivors")),
        directory.join("survivors.toml"),
    )
    .expect("copy example survivors");
    let survivors = load_survivors(&directory, "survivors.toml").expect("load example survivors");
    assert_eq!(shown(evaluate(&outcomes, &survivors)), BTreeSet::new());
}
