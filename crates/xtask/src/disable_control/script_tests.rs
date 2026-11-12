use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::process::Command;

use crate::data_variants_tests::element_variants;
use crate::harness_workspace_tests::{fresh_directory, generate_lockfile, repository_root};
use crate::lint_file::targets::{Coverage, TARGETS_FILE, Targets};

use super::scoped::in_module;

use super::script::{
    COMMENTS_SCRIPT, Comment, DIRECTIVES, SCRIPT_RULES_FILE, ScriptRules, comments_of, keyed_rows,
    repository_script_rows, script_rows,
};
use super::{
    RULES_FILE, RUST_EXPECT_KIND, Row, all_rows, new_rows, render_languages, render_new,
    rust_applied_line, unmatched,
};

const IPC_MODULE: &str = "apps/desktop/src-tauri/src/ipc";

const IPC_COMMAND_LIST: &str = "apps/desktop/src-tauri/ipc/commands.txt";

const CASES_FILE: &str = "harness/disable-control/script-cases.toml";
const SOURCES_DIRECTORY: &str = "harness/disable-control/script-sources";

type Expected = BTreeSet<(usize, String, String, String)>;

#[derive(Debug)]
struct ScriptCase {
    example: String,
    path: String,
    coverage: Coverage,
    rows: Expected,
}

fn field(entry: &toml::Value, key: &str) -> String {
    entry
        .get(key)
        .and_then(toml::Value::as_str)
        .map(String::from)
        .expect("case field")
}

fn script_cases(root: &Path) -> Vec<(ScriptCase, String)> {
    let table: toml::Table = fs::read_to_string(root.join(CASES_FILE))
        .expect("read script cases")
        .parse()
        .expect("parse script cases");
    table
        .get("source")
        .and_then(toml::Value::as_array)
        .expect("script source cases")
        .iter()
        .map(|entry| {
            let rows = entry
                .get("rows")
                .and_then(toml::Value::as_array)
                .expect("rows")
                .iter()
                .map(|row| {
                    (
                        row.get("line")
                            .and_then(toml::Value::as_integer)
                            .and_then(|line| usize::try_from(line).ok())
                            .expect("row line"),
                        field(row, "kind"),
                        field(row, "rule"),
                        field(row, "reason"),
                    )
                })
                .collect();
            let coverage = match field(entry, "coverage").as_str() {
                "linted" => Coverage::Linted,
                "unlinted" => Coverage::Unlinted,
                other => panic!("unknown coverage {other}"),
            };
            let case = ScriptCase {
                example: field(entry, "example"),
                path: field(entry, "path"),
                coverage,
                rows,
            };
            let source = fs::read_to_string(root.join(SOURCES_DIRECTORY).join(&case.example))
                .expect("read script example");
            (case, source)
        })
        .collect()
}

fn reported(rows: &[Row]) -> Expected {
    rows.iter()
        .map(|row| {
            (
                row.line,
                String::from(row.kind),
                row.rule.clone(),
                row.reason.clone(),
            )
        })
        .collect()
}

fn case_comments(root: &Path, cases: &[(ScriptCase, String)]) -> Vec<Vec<Comment>> {
    let files: Vec<(String, String)> = cases
        .iter()
        .map(|(case, source)| (case.path.clone(), source.clone()))
        .collect();
    comments_of(root, &files).expect("collect comments")
}

fn mismatches(
    cases: &[(ScriptCase, String)],
    comments: &[Vec<Comment>],
    rules: &ScriptRules,
) -> Vec<String> {
    cases
        .iter()
        .zip(comments)
        .filter_map(|((case, source), found)| {
            let coverage = |_: &str| case.coverage;
            let rows: Vec<Row> = if rules.scanned(&case.path).is_some() {
                keyed_rows(&case.path, source, found, &coverage)
                    .into_iter()
                    .map(|(row, _)| row)
                    .collect()
            } else {
                Vec::new()
            };
            let reported = reported(&rows);
            (reported != case.rows).then(|| {
                format!(
                    "{} as {}: expected {:?}, reported {reported:?}",
                    case.example, case.path, case.rows
                )
            })
        })
        .collect()
}

fn repository_rows_with(root: &Path, rules: &ScriptRules) -> Vec<Row> {
    let targets = Targets::load(root).expect("load lint-file targets");
    repository_script_rows(root, rules, &targets).expect("collect script rows")
}

#[test]
fn script_examples_report_expected_rows() {
    let root = repository_root();
    let rules = ScriptRules::load(&root).expect("load script rules");
    let cases = script_cases(&root);
    let comments = case_comments(&root, &cases);
    let failures = mismatches(&cases, &comments, &rules);
    assert!(failures.is_empty(), "{failures:#?}");
}

#[test]
fn every_directive_kind_has_an_example() {
    let root = repository_root();
    let cases = script_cases(&root);
    let covered: BTreeSet<String> = cases
        .iter()
        .flat_map(|(case, _)| case.rows.iter().map(|row| row.1.clone()))
        .collect();
    for directive in DIRECTIVES {
        assert!(
            [Coverage::Linted, Coverage::Unlinted]
                .into_iter()
                .any(|coverage| covered.contains(directive.kind(coverage))),
            "no example reports {}",
            directive.name()
        );
    }
    for coverage in [Coverage::Linted, Coverage::Unlinted] {
        assert!(
            DIRECTIVES
                .into_iter()
                .any(|directive| covered.contains(directive.kind(coverage))),
            "no example has coverage {coverage:?}"
        );
    }
}

#[test]
fn repository_has_no_script_disables() {
    let root = repository_root();
    let rules = ScriptRules::load(&root).expect("load script rules");
    let found = repository_rows_with(&root, &rules);
    assert!(found.is_empty(), "{found:#?}");
}

#[test]
fn full_report_counts_rust_and_typescript_separately() {
    let root = repository_root();
    let found = all_rows(&root).expect("collect all rows");
    let ipc = found
        .iter()
        .filter(|row| row.kind == RUST_EXPECT_KIND && in_module(IPC_MODULE, &row.path))
        .count();
    let commands = fs::read_to_string(root.join(IPC_COMMAND_LIST))
        .expect("read command list")
        .lines()
        .count();
    assert!(
        ipc <= commands.saturating_mul(2).saturating_add(1),
        "{ipc} IPC expects for {commands} commands"
    );
    assert_eq!(
        render_languages(&found),
        format!("disable-report: Rust {}건, TS 0건", ipc.saturating_add(3)),
        "{found:#?}"
    );
    let rows = [
        Row {
            kind: RUST_EXPECT_KIND,
            path: String::from("crates/domain/src/lib.rs"),
            line: 1,
            rule: String::from("clippy::needless_pass_by_value"),
            reason: String::from("이유"),
        },
        Row {
            kind: "eslint-disable-next-line (ESLint 밖)",
            path: String::from("harness/x.mjs"),
            line: 2,
            rule: String::from("no-console"),
            reason: String::from("이유"),
        },
        Row {
            kind: "eslint-disable-next-line",
            path: String::from("apps/desktop/src/app/App.tsx"),
            line: 3,
            rule: String::from("no-console"),
            reason: String::from("이유"),
        },
    ];
    assert_eq!(
        render_languages(&rows),
        "disable-report: Rust 1건, TS 2건",
        "language counts"
    );
}

#[test]
fn eslint_coverage_follows_the_real_configuration() {
    let root = repository_root();
    let targets = Targets::load(&root).expect("load targets");
    for (path, expected) in [
        ("apps/desktop/src/app/App.tsx", Coverage::Linted),
        ("harness/eslint-violations/check.mjs", Coverage::Linted),
        ("apps/desktop/tests/paths.mjs", Coverage::Linted),
        (
            "harness/eslint-violations/src/app/comments/no-reason.ts",
            Coverage::Unlinted,
        ),
        (
            "harness/tsc-violations/support/shape.ts",
            Coverage::Unlinted,
        ),
        ("design/components/bundle.js", Coverage::Unlinted),
        ("apps/desktop/src-tauri/build.rs", Coverage::Unlinted),
    ] {
        assert_eq!(targets.eslint_coverage(&root, path), expected, "{path}");
    }
}

#[test]
fn removing_any_script_rule_element_breaks_some_test() {
    let root = repository_root();
    let cases = script_cases(&root);
    let comments = case_comments(&root, &cases);
    let document = toml::Value::Table(
        fs::read_to_string(root.join(SCRIPT_RULES_FILE))
            .expect("read script rules")
            .parse()
            .expect("parse script rules"),
    );
    let passes = |candidate: &toml::Value| {
        let Some(table) = candidate.as_table() else {
            return false;
        };
        ScriptRules::parse(table).is_ok_and(|rules| {
            mismatches(&cases, &comments, &rules).is_empty()
                && repository_rows_with(&root, &rules).is_empty()
        })
    };
    assert!(passes(&document), "the unmodified rules must pass");
    let surviving: Vec<String> = element_variants(&document, "typescript")
        .into_iter()
        .filter(|(_, variant)| passes(variant))
        .map(|(name, _)| name)
        .collect();
    assert!(
        surviving.is_empty(),
        "data elements without a test: {surviving:?}"
    );
}

fn git(directory: &Path, arguments: &[&str]) -> String {
    let finished = Command::new("git")
        .arg("-C")
        .arg(directory)
        .args([
            "-c",
            "user.name=harness",
            "-c",
            "user.email=harness@example.invalid",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(arguments)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output()
        .expect("run git");
    assert!(finished.status.success(), "git {arguments:?}");
    String::from_utf8_lossy(&finished.stdout).trim().to_owned()
}

fn write(directory: &Path, relative: &str, text: &str) {
    let path = directory.join(relative);
    fs::create_dir_all(path.parent().expect("parent")).expect("create parent");
    fs::write(path, text).expect("write file");
}

const OLD_RUST: &str = "#[expect(clippy::needless_pass_by_value, reason = \"옛 끄기\")]\npub fn first(value: String) -> usize {\n    value.len()\n}\n";

const NEW_RUST: &str = "#[expect(clippy::too_many_lines, reason = \"새 끄기\")]\npub fn zero() {}\n\n#[expect(clippy::needless_pass_by_value, reason = \"옛 끄기\")]\npub fn first(value: String) -> usize {\n    value.len()\n}\n";

const OLD_SCRIPT: &str = "// eslint-disable-next-line no-console -- 옛 이유\nconsole.log(1);\n";

const NEW_SCRIPT: &str = "const top = 1;\n// eslint-disable-next-line no-console -- 옛 이유\nconsole.log(1);\n// eslint-disable-next-line no-console -- 새 이유\nconsole.log(top);\n";

const OLD_MOVED: &str =
    "// eslint-disable-next-line no-console -- 옮김\nconsole.log(1);\nconsole.log(3);\n";

const NEW_MOVED: &str =
    "console.log(1);\n// eslint-disable-next-line no-console -- 옮김\nconsole.log(3);\n";

fn since_workspace(root: &Path) -> (std::path::PathBuf, String) {
    let work = fresh_directory(root, "disable-report-since");
    write(
        &work,
        "Cargo.toml",
        "[workspace]\nresolver = \"3\"\nmembers = [\"probe\"]\n",
    );
    write(
        &work,
        "probe/Cargo.toml",
        "[package]\nname = \"probe\"\nversion = \"0.1.0\"\nedition = \"2024\"\npublish = false\n",
    );
    write(&work, "probe/src/lib.rs", OLD_RUST);
    write(&work, "scripts/tool.mjs", OLD_SCRIPT);
    write(&work, "scripts/moved.mjs", OLD_MOVED);
    for file in [RULES_FILE, SCRIPT_RULES_FILE, TARGETS_FILE, COMMENTS_SCRIPT] {
        write(
            &work,
            file,
            &fs::read_to_string(root.join(file)).expect("read data file"),
        );
    }
    fs::create_dir_all(work.join("apps").join("desktop")).expect("create desktop folder");
    std::os::unix::fs::symlink(
        root.join("apps").join("desktop").join("node_modules"),
        work.join("apps").join("desktop").join("node_modules"),
    )
    .expect("link desktop node_modules");
    write(&work, ".gitignore", "node_modules\n/target/\n");
    generate_lockfile(&work.join("Cargo.toml"));
    git(&work, &["init", "--quiet", "--initial-branch=main"]);
    git(&work, &["add", "--all"]);
    git(&work, &["commit", "--quiet", "-m", "base"]);
    let base = git(&work, &["rev-parse", "HEAD"]);
    write(&work, "probe/src/lib.rs", NEW_RUST);
    write(&work, "scripts/tool.mjs", NEW_SCRIPT);
    write(&work, "scripts/moved.mjs", NEW_MOVED);
    write(
        &work,
        "scripts/fresh.cjs",
        "/* eslint-disable */\nmodule.exports = 1;\n",
    );
    (work, base)
}

#[test]
fn since_lists_only_disables_added_after_the_base() {
    let (work, base) = since_workspace(&repository_root());
    let (resolved, found) = new_rows(&work, &base).expect("new rows");
    assert_eq!(resolved, base, "resolved base");
    let listed: BTreeSet<(String, usize, String, String)> = found
        .iter()
        .map(|row| {
            (
                row.path.clone(),
                row.line,
                row.rule.clone(),
                row.reason.clone(),
            )
        })
        .collect();
    let expected = BTreeSet::from([
        (
            String::from("probe/src/lib.rs"),
            1,
            String::from("clippy::too_many_lines"),
            String::from("새 끄기"),
        ),
        (
            String::from("scripts/tool.mjs"),
            4,
            String::from("no-console"),
            String::from("새 이유"),
        ),
        (
            String::from("scripts/fresh.cjs"),
            1,
            String::new(),
            String::new(),
        ),
        (
            String::from("scripts/moved.mjs"),
            2,
            String::from("no-console"),
            String::from("옮김"),
        ),
    ]);
    assert_eq!(listed, expected, "{found:#?}");
    let rendered = render_new(&resolved, &found);
    assert_eq!(
        rendered.lines().last(),
        Some(format!("disable-report: {base} 이후 새 끄기 4건 (Rust 1건, TS 3건)").as_str()),
        "{rendered}"
    );
    let unchanged = new_rows(&work, "HEAD").expect("rows since HEAD");
    assert_eq!(unchanged.1.len(), 4, "uncommitted rows count against HEAD");
    for refused in ["main", "HEAD~0", "abc"] {
        assert!(
            new_rows(&work, refused).is_err(),
            "{refused} must be refused"
        );
    }
}

#[test]
fn unmatched_keeps_duplicates_beyond_the_base_count() {
    let row = |line: usize| {
        (
            Row {
                kind: "eslint-disable-next-line",
                path: String::from("apps/desktop/src/app/App.tsx"),
                line,
                rule: String::from("no-console"),
                reason: String::from("같은 이유"),
            },
            String::from("console.log(1);"),
        )
    };
    let left = unmatched(vec![row(3), row(9)], vec![row(1)]);
    assert_eq!(left.len(), 1, "{left:#?}");
    let moved = unmatched(vec![row(7)], vec![row(1)]);
    assert!(moved.is_empty(), "{moved:#?}");
    let mut other = row(2);
    other.0.reason = String::from("다른 이유");
    let changed = unmatched(vec![other], vec![row(2)]);
    assert_eq!(changed.len(), 1, "{changed:#?}");
    let mut retargeted = row(2);
    retargeted.1 = String::from("console.log(2);");
    let applied = unmatched(vec![retargeted], vec![row(2)]);
    assert_eq!(applied.len(), 1, "{applied:#?}");
}

#[test]
fn rust_applied_line_skips_attributes_and_blank_lines() {
    let source = "#[expect(\n    clippy::too_many_lines,\n    reason = \"긴 함수\"\n)]\n#[inline]\n\npub fn long() {}\n";
    assert_eq!(
        rust_applied_line(source, 1),
        "pub fn long() {}",
        "multi-line attribute"
    );
    assert_eq!(
        rust_applied_line(
            "#[expect(clippy::a, reason = \"b\")] pub fn one() {}\nfn two() {}\n",
            1
        ),
        "fn two() {}",
        "attribute on the item line uses the next line"
    );
}

#[test]
fn new_scripts_and_configs_are_scanned() {
    let root = repository_root();
    let rules = ScriptRules::load(&root).expect("load script rules");
    let listed = super::script::listed_files(&root).expect("list files");
    for prefix in [
        "apps/desktop/vitest.config.mjs",
        "apps/desktop/eslint/boundaries.mjs",
        "harness/source-tree/",
        "harness/knip-check/",
        "harness/vitest-check/",
    ] {
        assert!(
            listed
                .iter()
                .any(|path| path.starts_with(prefix) && rules.scanned(path).is_some()),
            "{prefix} is scanned"
        );
    }
    let targets = Targets::load(&root).expect("load targets");
    assert_eq!(
        targets.eslint_coverage(&root, "apps/desktop/vitest.config.mjs"),
        Coverage::Unlinted,
        "vitest.config.mjs is outside the check-ts ESLint targets"
    );
    let coverage = |path: &str| targets.eslint_coverage(&root, path);
    let found = script_rows(
        &root,
        &[(
            String::from("apps/desktop/vitest.config.mjs"),
            String::from("// eslint-disable-next-line no-console -- 예시\nexport default {};\n"),
        )],
        &rules,
        &coverage,
    )
    .expect("script rows");
    assert_eq!(
        found.first().map(|(row, _)| row.kind),
        Some("eslint-disable-next-line (ESLint 밖)"),
        "{found:#?}"
    );
}

#[test]
fn comments_survive_quotes_backslashes_tabs_and_control_characters() {
    let root = repository_root();
    let text = "const s = \"\\\"\\\\\u{1}\";\n// eslint-disable-next-line a -- 따옴표 \"와 \\ 그리고\t탭\nconst t = 1;\n";
    let found = comments_of(&root, &[(String::from("probe.ts"), String::from(text))])
        .expect("collect comments");
    let comments = found.first().expect("one file");
    assert_eq!(comments.len(), 1, "{comments:#?}");
    let coverage = |_: &str| Coverage::Linted;
    let rows = keyed_rows("probe.ts", text, comments, &coverage);
    assert_eq!(
        rows.first()
            .map(|(row, applied)| (row.reason.as_str(), applied.as_str())),
        Some(("따옴표 \"와 \\ 그리고\t탭", "const t = 1;")),
        "{rows:#?}"
    );
}
