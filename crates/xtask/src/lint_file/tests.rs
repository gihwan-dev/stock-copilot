use std::collections::BTreeSet;
use std::fs;
use std::os::unix::fs::symlink;
use std::path::Path;
use std::process::Command;

use crate::data_variants_tests::element_variants;
use crate::harness_workspace_tests::{fresh_directory, generate_lockfile, repository_root};

use super::targets::{TARGETS_FILE, Targets};
use super::{Context, Linter, Outcome, PathProblem, Verdict, lint_text, render, resolve, verdict};

const CASES_FILE: &str = "harness/lint-file/cases.toml";
const EXAMPLES_DIRECTORY: &str = "harness/lint-file/examples";

const ALL_LINTERS: [Linter; 7] = [
    Linter::Rustfmt,
    Linter::SourceCheck,
    Linter::RawTypes,
    Linter::DisableCheck,
    Linter::SqlCheck,
    Linter::Prettier,
    Linter::Eslint,
];

#[derive(Debug)]
struct Case {
    label: String,
    path: String,
    text: String,
    applied: BTreeSet<String>,
    failed: BTreeSet<String>,
}

fn names(entry: &toml::Value, key: &str) -> BTreeSet<String> {
    entry
        .get(key)
        .and_then(toml::Value::as_array)
        .expect("name list")
        .iter()
        .map(|name| name.as_str().map(String::from).expect("name"))
        .collect()
}

fn cases(root: &Path) -> Vec<Case> {
    let table: toml::Table = fs::read_to_string(root.join(CASES_FILE))
        .expect("read cases")
        .parse()
        .expect("parse cases");
    table
        .get("case")
        .and_then(toml::Value::as_array)
        .expect("cases")
        .iter()
        .map(|entry| {
            let path = entry
                .get("path")
                .and_then(toml::Value::as_str)
                .map(String::from)
                .expect("case path");
            let example = entry.get("example").and_then(toml::Value::as_str);
            let source = example.map_or_else(
                || root.join(&path),
                |name| root.join(EXAMPLES_DIRECTORY).join(name),
            );
            Case {
                label: format!("{} as {path}", example.unwrap_or("real file")),
                text: fs::read_to_string(source).expect("read case text"),
                path,
                applied: names(entry, "applied"),
                failed: names(entry, "failed"),
            }
        })
        .collect()
}

fn summary(context: &Context, case: &Case) -> (BTreeSet<String>, BTreeSet<String>) {
    let results = lint_text(context, &case.path, &case.text).expect("lint text");
    let applied = results
        .iter()
        .filter(|result| match result.outcome {
            Outcome::Passed | Outcome::Failed(_) => true,
            Outcome::Skipped(_) => false,
        })
        .map(|result| String::from(result.check.name()))
        .collect();
    let failed = results
        .iter()
        .filter(|result| match result.outcome {
            Outcome::Failed(_) => true,
            Outcome::Passed | Outcome::Skipped(_) => false,
        })
        .map(|result| String::from(result.check.name()))
        .collect();
    (applied, failed)
}

#[test]
fn examples_apply_and_fail_the_expected_checks() {
    let root = repository_root();
    let context = Context::load(Some(&root)).expect("load context");
    let failures: Vec<String> = cases(&root)
        .iter()
        .filter_map(|case| {
            let (applied, failed) = summary(&context, case);
            (applied != case.applied || failed != case.failed).then(|| {
                format!(
                    "{}: expected applied {:?} failed {:?}, got applied {applied:?} failed {failed:?}",
                    case.label, case.applied, case.failed
                )
            })
        })
        .collect();
    assert!(failures.is_empty(), "{failures:#?}");
}

#[test]
fn every_linter_has_a_failing_example() {
    let root = repository_root();
    let failing: BTreeSet<String> = cases(&root)
        .into_iter()
        .flat_map(|case| case.failed)
        .collect();
    for linter in ALL_LINTERS {
        assert!(
            failing.contains(linter.name()),
            "no example fails {}",
            linter.name()
        );
    }
}

#[test]
fn verdict_and_report_are_honest() {
    let skipped = [super::CheckResult {
        check: Linter::Eslint,
        outcome: Outcome::Skipped("ESLint 설정이 다루지 않는 파일"),
    }];
    assert_eq!(verdict(&[]), Verdict::NothingApplied, "no checks");
    assert_eq!(verdict(&skipped), Verdict::NothingApplied, "only skipped");
    assert_eq!(
        render("docs/a.md", &[]),
        "lint-file: docs/a.md: 적용할 검사 없음",
        "nothing applied"
    );
    let mixed = [
        super::CheckResult {
            check: Linter::Prettier,
            outcome: Outcome::Passed,
        },
        super::CheckResult {
            check: Linter::Eslint,
            outcome: Outcome::Failed(vec![String::from("규칙 위반")]),
        },
    ];
    assert_eq!(verdict(&mixed), Verdict::Failed, "a failure wins");
    assert_eq!(
        render("a.ts", &mixed),
        "prettier: 통과\neslint: 위반\n  규칙 위반\nlint-file: a.ts: 위반 (prettier, eslint)",
        "report"
    );
}

#[test]
fn paths_outside_links_folders_and_missing_files_are_refused() {
    let root = repository_root();
    let work = fresh_directory(&root, "lint-file-paths");
    let real = work.join("real.rs");
    fs::write(&real, "fn probe() {}\n").expect("write real file");
    fs::create_dir_all(work.join("folder")).expect("create folder");
    symlink(&real, work.join("link.rs")).expect("file link");
    symlink(work.join("folder"), work.join("linked-folder")).expect("folder link");
    fs::write(work.join("folder").join("inside.rs"), "fn inside() {}\n").expect("write inside");
    let relative = "target/harness/lint-file-paths";
    assert_eq!(
        resolve(&root, &real),
        Ok(format!("{relative}/real.rs")),
        "a real file inside the repository"
    );
    assert_eq!(
        resolve(&root, &work.join("folder").join("..").join("real.rs")),
        Ok(format!("{relative}/real.rs")),
        "parent components inside the repository"
    );
    for (path, problem) in [
        (work.join("missing.rs"), PathProblem::Missing),
        (work.join("folder"), PathProblem::Directory),
        (work.join("link.rs"), PathProblem::Symlink),
        (
            work.join("linked-folder").join("inside.rs"),
            PathProblem::Symlink,
        ),
        (root.join("..").join("outside.rs"), PathProblem::Outside),
        (Path::new("/etc/hosts").to_path_buf(), PathProblem::Outside),
        (root.clone(), PathProblem::Outside),
    ] {
        assert_eq!(resolve(&root, &path), Err(problem), "{}", path.display());
    }
}

fn check_ts_lines(root: &Path) -> Vec<String> {
    let output = Command::new("just")
        .args(["--show", "check-ts"])
        .current_dir(root)
        .output()
        .expect("just --show check-ts");
    assert!(output.status.success(), "check-ts recipe");
    String::from_utf8(output.stdout)
        .expect("utf-8 recipe")
        .lines()
        .map(|line| String::from(line.trim()))
        .collect()
}

fn joined(directory: &str, path: &str) -> String {
    if directory == "." {
        String::from(path)
    } else {
        format!("{directory}/{path}")
    }
}

type Invocation = (String, String, Vec<String>);

fn recipe_targets(lines: &[String]) -> (Vec<String>, Vec<Invocation>, BTreeSet<String>) {
    let mut prettier = Vec::new();
    let mut eslint = Vec::new();
    let mut binaries = BTreeSet::new();
    for line in lines {
        let tokens: Vec<&str> = line.split_whitespace().collect();
        if let Some(position) = tokens.iter().position(|token| *token == "--check")
            && tokens.contains(&"prettier")
        {
            prettier.extend(
                tokens
                    .iter()
                    .skip(position.saturating_add(1))
                    .map(|token| String::from(*token)),
            );
        }
        let Some(flag) = tokens.iter().position(|token| *token == "--max-warnings") else {
            continue;
        };
        if let Some(first) = tokens.first()
            && first.ends_with("/eslint")
        {
            binaries.insert(String::from(*first));
        }
        let directory = tokens
            .iter()
            .position(|token| *token == "-C")
            .and_then(|index| tokens.get(index.saturating_add(1)))
            .map_or_else(|| String::from("."), |value| String::from(*value));
        let mut rest = tokens.iter().skip(flag.saturating_add(2)).peekable();
        let config = if rest.peek() == Some(&&"--config") {
            rest.next();
            String::from(*rest.next().expect("config value"))
        } else {
            joined(&directory, "eslint.config.mjs")
        };
        let paths = rest.map(|path| joined(&directory, path)).collect();
        eslint.push((directory, config, paths));
    }
    (prettier, eslint, binaries)
}

fn mirrors(targets: &Targets, lines: &[String]) -> bool {
    let (prettier, eslint, binaries) = recipe_targets(lines);
    let listed: Vec<Invocation> = targets
        .eslint
        .iter()
        .map(|target| {
            (
                target.directory.clone(),
                target.config.clone(),
                target.paths.clone(),
            )
        })
        .collect();
    targets.prettier == prettier
        && listed == eslint
        && binaries == BTreeSet::from([targets.eslint_binary.clone()])
}

#[test]
fn targets_mirror_the_check_ts_recipe() {
    let root = repository_root();
    let targets = Targets::load(&root).expect("load targets");
    let lines = check_ts_lines(&root);
    assert!(
        mirrors(&targets, &lines),
        "{TARGETS_FILE} must list what check-ts runs: {:#?} vs {:#?}",
        targets,
        recipe_targets(&lines)
    );
    assert!(
        root.join(&targets.prettier_binary).is_file(),
        "prettier binary"
    );
}

#[test]
fn removing_any_target_element_breaks_some_test() {
    let root = repository_root();
    let lines = check_ts_lines(&root);
    let document = toml::Value::Table(
        fs::read_to_string(root.join(TARGETS_FILE))
            .expect("read targets")
            .parse()
            .expect("parse targets"),
    );
    let passes = |candidate: &toml::Value| {
        candidate.as_table().is_some_and(|table| {
            Targets::parse(table).is_ok_and(|targets| {
                mirrors(&targets, &lines) && root.join(&targets.prettier_binary).is_file()
            })
        })
    };
    assert!(passes(&document), "the unmodified targets must pass");
    let surviving: Vec<String> = element_variants(&document, "targets")
        .into_iter()
        .filter(|(_, variant)| passes(variant))
        .map(|(name, _)| name)
        .collect();
    assert!(
        surviving.is_empty(),
        "data elements without a test: {surviving:?}"
    );
}

fn just_lint(path: &str) -> (bool, String) {
    let output = Command::new("just")
        .arg("lint-file")
        .arg(path)
        .current_dir(repository_root())
        .output()
        .expect("just lint-file");
    let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
    text.push_str(&String::from_utf8_lossy(&output.stderr));
    (output.status.success(), text)
}

#[test]
fn just_lint_file_exit_status_follows_the_verdict() {
    let (passed, text) = just_lint("crates/domain/src/lib.rs");
    assert!(passed, "{text}");
    assert!(
        text.contains("lint-file: crates/domain/src/lib.rs: 통과 (rustfmt, source-check, raw-types, disable-check, sql-check)"),
        "{text}"
    );
    let (nothing, quiet) = just_lint("docs/harness.md");
    assert!(nothing, "{quiet}");
    assert!(
        quiet.contains("lint-file: docs/harness.md: 적용할 검사 없음"),
        "{quiet}"
    );
    for (path, message) in [
        ("missing-file.rs", "없는 파일이다"),
        ("crates", "폴더다"),
        ("/etc/hosts", "저장소 밖의 경로다"),
    ] {
        let (refused, output) = just_lint(path);
        assert!(!refused, "{path}: {output}");
        assert!(output.contains(message), "{path}: {output}");
    }
}

#[test]
fn xtask_lint_file_fails_on_a_violating_file() {
    let root = repository_root();
    let work = fresh_directory(&root, "lint-file-command");
    fs::create_dir_all(work.join("probe").join("src")).expect("create probe");
    fs::write(
        work.join("Cargo.toml"),
        "[workspace]\nresolver = \"3\"\nmembers = [\"probe\"]\n",
    )
    .expect("write workspace");
    fs::write(
        work.join("probe").join("Cargo.toml"),
        "[package]\nname = \"probe\"\nversion = \"0.1.0\"\nedition = \"2024\"\npublish = false\n",
    )
    .expect("write probe manifest");
    fs::write(work.join("probe").join("src").join("lib.rs"), "").expect("write probe lib");
    generate_lockfile(&work.join("Cargo.toml"));
    fs::create_dir_all(work.join("harness").join("lint-file")).expect("create data folder");
    fs::copy(root.join(TARGETS_FILE), work.join(TARGETS_FILE)).expect("copy targets");
    symlink(root.join("node_modules"), work.join("node_modules")).expect("link node_modules");
    fs::write(work.join("package.json"), "{\"a\":1}\n").expect("write bad json");
    fs::write(work.join("pnpm-workspace.yaml"), "packages:\n  - probe\n").expect("write good yaml");
    let run = |path: &str| {
        let output = Command::new("cargo")
            .args(["run", "--locked", "--quiet", "--manifest-path"])
            .arg(root.join("Cargo.toml"))
            .args(["--package", "xtask", "--", "lint-file", path])
            .current_dir(&work)
            .output()
            .expect("run xtask lint-file");
        let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
        text.push_str(&String::from_utf8_lossy(&output.stderr));
        (output.status.success(), text)
    };
    let (bad, bad_output) = run("package.json");
    assert!(!bad, "{bad_output}");
    assert!(
        bad_output.contains("lint-file: package.json: 위반 (prettier)"),
        "{bad_output}"
    );
    let (good, good_output) = run("pnpm-workspace.yaml");
    assert!(good, "{good_output}");
    assert!(
        good_output.contains("lint-file: pnpm-workspace.yaml: 통과 (prettier)"),
        "{good_output}"
    );
}

fn recipe_lines(name: &str) -> Vec<String> {
    let output = Command::new("just")
        .args(["--show", name])
        .current_dir(repository_root())
        .output()
        .expect("just --show");
    assert!(output.status.success(), "recipe {name}");
    String::from_utf8(output.stdout)
        .expect("utf-8 recipe")
        .lines()
        .map(|line| String::from(line.trim()))
        .collect()
}

#[test]
fn lint_file_goes_through_cargo_run_and_never_through_target() {
    assert_eq!(
        recipe_lines("lint-file"),
        [
            "lint-file path:",
            "cargo run --locked --quiet --package xtask -- lint-file \"$1\""
        ],
        "lint-file recipe"
    );
    let summary = Command::new("just")
        .arg("--summary")
        .current_dir(repository_root())
        .output()
        .expect("just --summary");
    let offending: Vec<String> = String::from_utf8_lossy(&summary.stdout)
        .split_whitespace()
        .flat_map(recipe_lines)
        .filter(|line| line.contains("target/") || line.contains("{{path}}"))
        .collect();
    assert!(offending.is_empty(), "{offending:?}");
}
