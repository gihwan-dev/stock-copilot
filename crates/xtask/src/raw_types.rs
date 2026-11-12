mod catalogue;
mod crates;
mod scan;
mod visitor;

#[cfg(test)]
mod tests;

use std::collections::BTreeSet;
use std::fmt;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use syn::visit::Visit;

use crate::workspace_sources::{Member, SourceFailure, io_failure, load_members, relative_text};
use crates::{CRATES_FILE, CrateRules, Scope};
use scan::{ModuleScan, Newtypes};
use visitor::RawTypes;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Rule {
    RawType(String),
    TypeAlias,
    OpaqueItemMacro,
    Unparsable,
}

impl fmt::Display for Rule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RawType(name) => write!(f, "raw-type:{name}"),
            Self::TypeAlias => f.write_str("raw-type-alias"),
            Self::OpaqueItemMacro => f.write_str("raw-type-opaque-macro"),
            Self::Unparsable => f.write_str("raw-type-unparsable"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Finding {
    pub(crate) path: String,
    pub(crate) line: usize,
    pub(crate) rule: Rule,
}

impl fmt::Display for Finding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}: {}", self.path, self.line, self.rule)
    }
}

struct ParsedFile {
    label: String,
    file: syn::File,
}

fn file_findings(
    parsed: &ParsedFile,
    local_traits: &BTreeSet<String>,
    findings: &mut BTreeSet<Finding>,
) {
    let mut newtypes = Newtypes::default();
    newtypes.visit_file(&parsed.file);
    let mut raw = RawTypes::new(local_traits, newtypes.names);
    raw.visit_file(&parsed.file);
    findings.extend(raw.found.into_iter().map(|(line, rule)| Finding {
        path: parsed.label.clone(),
        line,
        rule,
    }));
}

fn module_file(directory: &Path, inline: &[String], name: &str) -> Option<(PathBuf, PathBuf)> {
    let base = inline
        .iter()
        .fold(directory.to_path_buf(), |path, part| path.join(part));
    let children = base.join(name);
    let flat = base.join(format!("{name}.rs"));
    let nested = children.join("mod.rs");
    if flat.is_file() {
        Some((flat, children))
    } else if nested.is_file() {
        Some((nested, children))
    } else {
        None
    }
}

pub(crate) fn check_crate(
    root: &Path,
    roots: &[PathBuf],
    findings: &mut BTreeSet<Finding>,
) -> Result<(), SourceFailure> {
    let mut pending: Vec<(PathBuf, PathBuf)> = roots
        .iter()
        .map(|path| {
            (
                path.clone(),
                path.parent().map_or_else(PathBuf::new, Path::to_path_buf),
            )
        })
        .collect();
    let mut seen = BTreeSet::new();
    let mut parsed = Vec::new();
    let mut traits = BTreeSet::new();
    while let Some((path, children)) = pending.pop() {
        if !seen.insert(path.clone()) {
            continue;
        }
        let label = relative_text(root, &path);
        let text = fs::read_to_string(&path).map_err(|error| io_failure(&path, &error))?;
        let Ok(file) = syn::parse_file(&text) else {
            findings.insert(Finding {
                path: label,
                line: 0,
                rule: Rule::Unparsable,
            });
            continue;
        };
        let mut modules = ModuleScan::default();
        modules.visit_file(&file);
        for (inline, name) in &modules.declared {
            let found = module_file(&children, inline, name).ok_or_else(|| {
                SourceFailure(format!("{label}: 모듈 {name}의 파일을 찾지 못했다"))
            })?;
            pending.push(found);
        }
        traits.extend(modules.traits);
        parsed.push(ParsedFile { label, file });
    }
    for file in &parsed {
        file_findings(file, &traits, findings);
    }
    Ok(())
}

pub(crate) fn load_crate_rules(root: &Path) -> Result<CrateRules, SourceFailure> {
    let path = root.join(CRATES_FILE);
    let text = fs::read_to_string(&path).map_err(|error| io_failure(&path, &error))?;
    let table: toml::Table = text
        .parse()
        .map_err(|error: toml::de::Error| SourceFailure(format!("{CRATES_FILE}: {error}")))?;
    CrateRules::parse(&table)
}

pub(crate) fn check_members(
    root: &Path,
    members: &[Member],
    rules: &CrateRules,
) -> Result<BTreeSet<Finding>, SourceFailure> {
    let names: BTreeSet<String> = members.iter().map(|member| member.name.clone()).collect();
    let unclassified = rules.classification_failures(&names);
    if !unclassified.is_empty() {
        return Err(SourceFailure(unclassified.join("\n")));
    }
    let mut findings = BTreeSet::new();
    for member in members {
        if rules.scope(&member.name) == Scope::Checked {
            check_crate(root, &member.roots, &mut findings)?;
        }
    }
    Ok(findings)
}

fn run() -> Result<BTreeSet<Finding>, SourceFailure> {
    let members = load_members(None)?;
    let rules = load_crate_rules(&members.root)?;
    check_members(&members.root, &members.packages, &rules)
}

pub(crate) fn command() -> ExitCode {
    match report(run()) {
        Ok(code) => code,
        Err(_) => ExitCode::FAILURE,
    }
}

fn report(outcome: Result<BTreeSet<Finding>, SourceFailure>) -> io::Result<ExitCode> {
    match outcome {
        Ok(findings) if findings.is_empty() => {
            writeln!(io::stdout().lock(), "raw-types: 위반 없음")?;
            Ok(ExitCode::SUCCESS)
        }
        Ok(findings) => {
            let mut output = io::stderr().lock();
            for finding in &findings {
                writeln!(output, "{finding}")?;
            }
            writeln!(output, "raw-types: 위반 {}건", findings.len())?;
            Ok(ExitCode::FAILURE)
        }
        Err(failure) => {
            writeln!(io::stderr().lock(), "raw-types: {failure}")?;
            Ok(ExitCode::FAILURE)
        }
    }
}
