mod allowlist;
mod expects;
mod lints;
pub(crate) mod script;
mod script_comments;

#[cfg(test)]
mod script_tests;
#[cfg(test)]
mod tests;

use std::collections::BTreeSet;
use std::fmt;
use std::fs;
use std::io::{self, Write};
use std::path::Path;
use std::process::ExitCode;

use syn::visit::Visit;

use crate::git_base;
use crate::lint_file::targets::Targets;
use crate::workspace_sources::{
    SourceFailure, io_failure, load_members, member_rust_files, relative_text,
};
use allowlist::{ALLOWLIST_FILE, Allowlist};
use expects::{Collected, DisableRules, ExpectCollector, Target};
use lints::{LINTS_FILE, LintCatalogue, individually_configured};
use script::{ScriptRules, repository_script_rows, script_rows};

pub(crate) const RUST_EXPECT_KIND: &str = "rust-expect";

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Rule {
    LintGroup,
    NotDisableable,
    Placement(Target),
    Unparsable,
}

const fn placement_name(target: Target) -> &'static str {
    match target {
        Target::Function => "function",
        Target::Statement => "statement",
        Target::Field => "field",
        Target::Variant => "variant",
        Target::Item => "item",
        Target::Module => "module",
        Target::Impl => "impl",
        Target::Trait => "trait",
        Target::ForeignBlock => "foreign-block",
        Target::Macro => "macro",
        Target::Block => "block",
        Target::Inner => "inner",
        Target::Other => "other",
        Target::NestedItem => "nested-item",
    }
}

impl fmt::Display for Rule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LintGroup => f.write_str("disable:lint-group"),
            Self::NotDisableable => f.write_str("disable:not-disableable"),
            Self::Placement(target) => write!(f, "disable:placement:{}", placement_name(*target)),
            Self::Unparsable => f.write_str("disable:unparsable"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Finding {
    pub(crate) path: String,
    pub(crate) line: usize,
    pub(crate) rule: Rule,
    pub(crate) lint: String,
}

impl fmt::Display for Finding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{}: {} {}",
            self.path, self.line, self.rule, self.lint
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Row {
    pub(crate) kind: &'static str,
    pub(crate) path: String,
    pub(crate) line: usize,
    pub(crate) rule: String,
    pub(crate) reason: String,
}

#[derive(Debug, Clone)]
pub(crate) struct Policy {
    rules: DisableRules,
    catalogue: LintCatalogue,
    individual: BTreeSet<String>,
    allowlist: Allowlist,
}

pub(crate) const RULES_FILE: &str = "harness/disable-control/rules.toml";

const RULES_KEYS: [&str; 2] = ["attribute-roots", "block-keywords"];

fn rule_names(table: &toml::Table, key: &str) -> Result<BTreeSet<String>, SourceFailure> {
    table
        .get(key)
        .and_then(toml::Value::as_array)
        .ok_or_else(|| SourceFailure(format!("{RULES_FILE}: {key} 목록이 없다")))?
        .iter()
        .map(|value| {
            value
                .as_str()
                .filter(|name| !name.trim().is_empty())
                .map(String::from)
                .ok_or_else(|| SourceFailure(format!("{RULES_FILE}: {key}의 원소는 문자열이다")))
        })
        .collect()
}

pub(crate) fn load_disable_rules(table: &toml::Table) -> Result<DisableRules, SourceFailure> {
    if let Some(unknown) = table.keys().find(|key| !RULES_KEYS.contains(&key.as_str())) {
        return Err(SourceFailure(format!("{RULES_FILE}: 모르는 키 {unknown}")));
    }
    Ok(DisableRules {
        attribute_roots: rule_names(table, "attribute-roots")?,
        block_keywords: rule_names(table, "block-keywords")?,
    })
}

fn read_table(root: &Path, file: &str) -> Result<toml::Table, SourceFailure> {
    let path = root.join(file);
    let text = fs::read_to_string(&path).map_err(|error| io_failure(&path, &error))?;
    text.parse()
        .map_err(|error: toml::de::Error| SourceFailure(format!("{file}: {error}")))
}

impl Policy {
    pub(crate) fn from_tables(
        root: &Path,
        lints: &toml::Table,
        allowlist: &toml::Table,
        rules: &toml::Table,
    ) -> Result<Self, SourceFailure> {
        Ok(Self {
            rules: load_disable_rules(rules)?,
            catalogue: LintCatalogue::parse(lints)?,
            individual: individually_configured(&read_table(root, "Cargo.toml")?),
            allowlist: Allowlist::parse(root, allowlist)?,
        })
    }

    pub(crate) fn load(root: &Path) -> Result<Self, SourceFailure> {
        Self::from_tables(
            root,
            &read_table(root, LINTS_FILE)?,
            &read_table(root, ALLOWLIST_FILE)?,
            &read_table(root, RULES_FILE)?,
        )
    }

    fn disableable(&self, lint: &str) -> Option<()> {
        (self.catalogue.disableable.contains(lint) && !self.individual.contains(lint)).then_some(())
    }

    fn lint_findings(&self, path: &str, collected: &Collected) -> Vec<Finding> {
        let finding = |rule: Rule, lint: &str| Finding {
            path: String::from(path),
            line: collected.line,
            rule,
            lint: String::from(lint),
        };
        let mut findings: Vec<Finding> = match collected.target {
            Target::Function
            | Target::Statement
            | Target::Field
            | Target::Variant
            | Target::Item => Vec::new(),
            Target::Module
            | Target::Impl
            | Target::Trait
            | Target::ForeignBlock
            | Target::Macro
            | Target::Block
            | Target::Inner
            | Target::Other
            | Target::NestedItem => vec![finding(
                Rule::Placement(collected.target),
                &collected.lints.join(","),
            )],
        };
        for lint in &collected.lints {
            if self.catalogue.groups.contains(lint) {
                findings.push(finding(Rule::LintGroup, lint));
            } else if self.disableable(lint).is_none()
                && self
                    .allowlist
                    .permits(path, collected.statement.as_deref(), lint)
                    .is_none()
            {
                findings.push(finding(Rule::NotDisableable, lint));
            }
        }
        findings
    }
}

pub(crate) fn collect(source: &str, rules: &DisableRules) -> Option<Vec<Collected>> {
    let file = syn::parse_file(source).ok()?;
    let mut collector = ExpectCollector::new(rules);
    collector.visit_file(&file);
    Some(collector.found)
}

pub(crate) fn check_source(path: &str, source: &str, policy: &Policy) -> BTreeSet<Finding> {
    collect(source, &policy.rules).map_or_else(
        || {
            BTreeSet::from([Finding {
                path: String::from(path),
                line: 0,
                rule: Rule::Unparsable,
                lint: String::new(),
            }])
        },
        |found| {
            found
                .iter()
                .flat_map(|collected| policy.lint_findings(path, collected))
                .collect()
        },
    )
}

pub(crate) fn rows(path: &str, source: &str, rules: &DisableRules) -> Vec<Row> {
    collect(source, rules)
        .into_iter()
        .flatten()
        .flat_map(|collected| {
            let lints = if collected.lints.is_empty() {
                vec![String::new()]
            } else {
                collected.lints.clone()
            };
            lints.into_iter().map(move |lint| Row {
                kind: RUST_EXPECT_KIND,
                path: String::from(path),
                line: collected.line,
                rule: lint,
                reason: collected.reason.clone(),
            })
        })
        .collect()
}

pub(crate) fn repository_sources(
    root: Option<&Path>,
) -> Result<Vec<(String, String)>, SourceFailure> {
    let manifest = root.map(|directory| directory.join("Cargo.toml"));
    let members = load_members(manifest.as_deref())?;
    member_rust_files(&members)?
        .into_iter()
        .map(|path| {
            fs::read_to_string(&path)
                .map(|text| (relative_text(&members.root, &path), text))
                .map_err(|error| io_failure(&path, &error))
        })
        .collect()
}

pub(crate) fn check_repository(
    root: &Path,
    policy: &Policy,
) -> Result<BTreeSet<Finding>, SourceFailure> {
    Ok(repository_sources(Some(root))?
        .iter()
        .flat_map(|(path, source)| check_source(path, source, policy))
        .collect())
}

pub(crate) fn repository_rows(root: &Path) -> Result<Vec<Row>, SourceFailure> {
    let rules = load_disable_rules(&read_table(root, RULES_FILE)?)?;
    let mut found: Vec<Row> = repository_sources(Some(root))?
        .iter()
        .flat_map(|(path, source)| rows(path, source, &rules))
        .collect();
    found.sort();
    Ok(found)
}

fn escaped(text: &str) -> String {
    text.replace('|', "\\|").replace('\n', " ")
}

pub(crate) fn render_report(found: &[Row]) -> String {
    let mut lines = vec![
        String::from("| 종류 | 파일 | 줄 | 규칙 | 이유 |"),
        String::from("|---|---|---|---|---|"),
    ];
    lines.extend(found.iter().map(|row| {
        format!(
            "| {} | {} | {} | {} | {} |",
            row.kind,
            escaped(&row.path),
            row.line,
            escaped(&row.rule),
            escaped(&row.reason)
        )
    }));
    let kinds: BTreeSet<&str> = found.iter().map(|row| row.kind).collect();
    let counts: Vec<String> = kinds
        .iter()
        .map(|kind| {
            let count = found.iter().filter(|row| row.kind == *kind).count();
            format!("{kind} {count}건")
        })
        .collect();
    lines.push(format!(
        "disable-report: 끄기 {}건 ({})",
        found.len(),
        counts.join(", ")
    ));
    lines.join("\n")
}

fn repository_root() -> Result<std::path::PathBuf, SourceFailure> {
    load_members(None).map(|members| members.root)
}

pub(crate) fn check_command() -> ExitCode {
    let outcome = repository_root().and_then(|root| {
        let policy = Policy::load(&root)?;
        check_repository(&root, &policy)
    });
    let written = match outcome {
        Ok(findings) if findings.is_empty() => {
            writeln!(io::stdout().lock(), "disable-check: 위반 없음").map(|()| ExitCode::SUCCESS)
        }
        Ok(findings) => write_findings(&findings),
        Err(failure) => {
            writeln!(io::stderr().lock(), "disable-check: {failure}").map(|()| ExitCode::FAILURE)
        }
    };
    match written {
        Ok(code) => code,
        Err(_) => ExitCode::FAILURE,
    }
}

fn write_findings(findings: &BTreeSet<Finding>) -> io::Result<ExitCode> {
    let mut output = io::stderr().lock();
    for finding in findings {
        writeln!(output, "{finding}")?;
    }
    writeln!(output, "disable-check: 위반 {}건", findings.len())?;
    Ok(ExitCode::FAILURE)
}

pub(crate) fn all_rows(root: &Path) -> Result<Vec<Row>, SourceFailure> {
    let mut found = repository_rows(root)?;
    found.extend(repository_script_rows(
        root,
        &ScriptRules::load(root)?,
        &Targets::load(root)?,
    )?);
    found.sort();
    Ok(found)
}

pub(crate) fn language_counts(found: &[Row]) -> (usize, usize) {
    let rust = found
        .iter()
        .filter(|row| row.kind == RUST_EXPECT_KIND)
        .count();
    (rust, found.len().saturating_sub(rust))
}

pub(crate) fn render_languages(found: &[Row]) -> String {
    let (rust, script) = language_counts(found);
    format!("disable-report: Rust {rust}건, TS {script}건")
}

fn base_blob(root: &Path, base: &str, path: &str) -> Result<Option<String>, SourceFailure> {
    let spec = format!("{base}:{path}");
    let mut exists = git_base::git(root);
    exists.args(["cat-file", "-e", &spec]);
    let present = exists
        .status()
        .map_err(|error| SourceFailure(format!("git cat-file: {error}")))?;
    if !present.success() {
        return Ok(None);
    }
    let mut blob = git_base::git(root);
    blob.args(["cat-file", "blob", &spec]);
    git_base::output(blob, "cat-file")
        .map(|bytes| Some(String::from_utf8_lossy(&bytes).into_owned()))
        .map_err(SourceFailure)
}

fn base_rows(root: &Path, base: &str, path: &str) -> Result<Vec<Row>, SourceFailure> {
    let Some(text) = base_blob(root, base, path)? else {
        return Ok(Vec::new());
    };
    let rules = load_disable_rules(&read_table(root, RULES_FILE)?)?;
    let targets = Targets::load(root)?;
    let coverage = |candidate: &str| targets.eslint_coverage(root, candidate);
    let mut found = rows(path, &text, &rules);
    found.extend(script_rows(
        path,
        &text,
        &ScriptRules::load(root)?,
        &coverage,
    ));
    Ok(found)
}

pub(crate) fn unmatched(current: Vec<Row>, previous: Vec<Row>) -> Vec<Row> {
    let same = |left: &Row, right: &Row| {
        left.kind == right.kind
            && left.path == right.path
            && left.rule == right.rule
            && left.reason == right.reason
    };
    let mut pool: Vec<Option<Row>> = previous.into_iter().map(Some).collect();
    current
        .into_iter()
        .filter(|row| {
            pool.iter_mut()
                .find(|slot| slot.as_ref().is_some_and(|candidate| same(candidate, row)))
                .and_then(Option::take)
                .is_none()
        })
        .collect()
}

pub(crate) fn new_rows(root: &Path, base: &str) -> Result<(String, Vec<Row>), SourceFailure> {
    let resolved = git_base::resolve_base(root, base).map_err(SourceFailure)?;
    let changed = git_base::changed_paths(root, &resolved).map_err(SourceFailure)?;
    let current: Vec<Row> = all_rows(root)?
        .into_iter()
        .filter(|row| changed.contains(&row.path))
        .collect();
    let paths: BTreeSet<String> = current.iter().map(|row| row.path.clone()).collect();
    let mut previous = Vec::new();
    for path in &paths {
        previous.extend(base_rows(root, &resolved, path)?);
    }
    Ok((resolved, unmatched(current, previous)))
}

pub(crate) fn render_new(base: &str, found: &[Row]) -> String {
    let table = render_report(found);
    let mut lines: Vec<&str> = table.lines().collect();
    lines.pop();
    let (rust, script) = language_counts(found);
    let summary = format!(
        "disable-report: {base} 이후 새 끄기 {}건 (Rust {rust}건, TS {script}건)",
        found.len()
    );
    lines.push(&summary);
    lines.join("\n")
}

fn report_text(arguments: &[String]) -> Result<String, SourceFailure> {
    let root = repository_root()?;
    match arguments {
        [] => {
            let found = all_rows(&root)?;
            Ok(format!(
                "{}\n{}",
                render_report(&found),
                render_languages(&found)
            ))
        }
        [flag, base] if flag == "--since" => {
            let (resolved, found) = new_rows(&root, base)?;
            Ok(render_new(&resolved, &found))
        }
        _ => Err(SourceFailure(String::from(
            "사용법: xtask disable-report [--since <HEAD|40자리 커밋 ID>]",
        ))),
    }
}

pub(crate) fn report_command(arguments: &[String]) -> ExitCode {
    let written = match report_text(arguments) {
        Ok(text) => writeln!(io::stdout().lock(), "{text}").map(|()| ExitCode::SUCCESS),
        Err(failure) => {
            writeln!(io::stderr().lock(), "disable-report: {failure}").map(|()| ExitCode::FAILURE)
        }
    };
    match written {
        Ok(code) => code,
        Err(_) => ExitCode::FAILURE,
    }
}
