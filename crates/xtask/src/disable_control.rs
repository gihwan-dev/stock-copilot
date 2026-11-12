mod allowlist;
mod expects;
mod lints;

#[cfg(test)]
mod tests;

use std::collections::BTreeSet;
use std::fmt;
use std::fs;
use std::io::{self, Write};
use std::path::Path;
use std::process::ExitCode;

use syn::visit::Visit;

use crate::workspace_sources::{
    SourceFailure, io_failure, load_members, member_rust_files, relative_text,
};
use allowlist::{ALLOWLIST_FILE, Allowlist};
use expects::{Collected, ExpectCollector, Target};
use lints::{LINTS_FILE, LintCatalogue, individually_configured};

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
    catalogue: LintCatalogue,
    individual: BTreeSet<String>,
    allowlist: Allowlist,
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
    ) -> Result<Self, SourceFailure> {
        Ok(Self {
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
            | Target::Other => vec![finding(
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

pub(crate) fn collect(source: &str) -> Option<Vec<Collected>> {
    let file = syn::parse_file(source).ok()?;
    let mut collector = ExpectCollector::default();
    collector.visit_file(&file);
    Some(collector.found)
}

pub(crate) fn check_source(path: &str, source: &str, policy: &Policy) -> BTreeSet<Finding> {
    collect(source).map_or_else(
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

pub(crate) fn rows(path: &str, source: &str) -> Vec<Row> {
    collect(source)
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
    let mut found: Vec<Row> = repository_sources(Some(root))?
        .iter()
        .flat_map(|(path, source)| rows(path, source))
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

pub(crate) fn report_command() -> ExitCode {
    let written =
        match repository_root().and_then(|root| repository_rows(&root)) {
            Ok(found) => writeln!(io::stdout().lock(), "{}", render_report(&found))
                .map(|()| ExitCode::SUCCESS),
            Err(failure) => writeln!(io::stderr().lock(), "disable-report: {failure}")
                .map(|()| ExitCode::FAILURE),
        };
    match written {
        Ok(code) => code,
        Err(_) => ExitCode::FAILURE,
    }
}
