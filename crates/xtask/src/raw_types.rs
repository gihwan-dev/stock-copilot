mod crates;
mod resolve;
mod rules;
mod scan;
mod visitor;

#[cfg(test)]
mod tests;

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use syn::visit::Visit;

use crate::workspace_sources::{Member, SourceFailure, io_failure, load_members, relative_text};
use crates::{CRATES_FILE, CrateRules, Scope};
use resolve::FileContext;
use rules::{RULES_FILE, RawRules};
use scan::{Definitions, ModuleScan, Newtypes, UseScan};
use visitor::RawTypes;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Rule {
    RawType(String),
    TypeAlias,
    OpaqueItemMacro,
    Unparsable,
    Unit,
    GlobImport,
    UseRename,
}

impl fmt::Display for Rule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RawType(name) => write!(f, "raw-type:{name}"),
            Self::TypeAlias => f.write_str("raw-type-alias"),
            Self::OpaqueItemMacro => f.write_str("raw-type-opaque-macro"),
            Self::Unparsable => f.write_str("raw-type-unparsable"),
            Self::Unit => f.write_str("raw-type-unit"),
            Self::GlobImport => f.write_str("raw-type-glob-import"),
            Self::UseRename => f.write_str("raw-type-use-rename"),
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

struct CrateSources {
    files: Vec<ParsedFile>,
    definitions: Definitions,
    findings: BTreeSet<Finding>,
}

fn file_findings(
    parsed: &ParsedFile,
    rules: &RawRules,
    local: &Definitions,
    dependencies: &BTreeMap<String, Definitions>,
    findings: &mut BTreeSet<Finding>,
) {
    let mut uses = UseScan::default();
    uses.visit_file(&parsed.file);
    let context = FileContext {
        rules,
        local,
        dependencies,
        imports: &uses.imports,
    };
    let mut newtypes = Newtypes::new(rules);
    newtypes.visit_file(&parsed.file);
    let mut raw = RawTypes::new(&context, newtypes.fields);
    raw.visit_file(&parsed.file);
    findings.extend(
        uses.found
            .into_iter()
            .chain(raw.found)
            .map(|(line, rule)| Finding {
                path: parsed.label.clone(),
                line,
                rule,
            }),
    );
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

#[derive(Debug, Clone, Copy)]
pub(crate) struct Replacement<'text> {
    pub(crate) label: &'text str,
    pub(crate) text: &'text str,
}

fn crate_sources(root: &Path, roots: &[PathBuf]) -> Result<CrateSources, SourceFailure> {
    crate_sources_with(root, roots, None)
}

fn crate_sources_with(
    root: &Path,
    roots: &[PathBuf],
    replacement: Option<Replacement<'_>>,
) -> Result<CrateSources, SourceFailure> {
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
    let mut sources = CrateSources {
        files: Vec::new(),
        definitions: Definitions::default(),
        findings: BTreeSet::new(),
    };
    while let Some((path, children)) = pending.pop() {
        if !seen.insert(path.clone()) {
            continue;
        }
        let label = relative_text(root, &path);
        let text = match replacement {
            Some(replaced) if replaced.label == label => String::from(replaced.text),
            Some(_) | None => {
                fs::read_to_string(&path).map_err(|error| io_failure(&path, &error))?
            }
        };
        let Ok(file) = syn::parse_file(&text) else {
            sources.findings.insert(Finding {
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
        sources.definitions.extend(&modules.definitions);
        sources.files.push(ParsedFile { label, file });
    }
    Ok(sources)
}

pub(crate) fn check_crate(
    root: &Path,
    roots: &[PathBuf],
    rules: &RawRules,
    dependencies: &BTreeMap<String, Definitions>,
    findings: &mut BTreeSet<Finding>,
) -> Result<(), SourceFailure> {
    let sources = crate_sources(root, roots)?;
    findings.extend(sources.findings);
    for file in &sources.files {
        file_findings(file, rules, &sources.definitions, dependencies, findings);
    }
    Ok(())
}

fn read_table(root: &Path, file: &str) -> Result<toml::Table, SourceFailure> {
    let path = root.join(file);
    let text = fs::read_to_string(&path).map_err(|error| io_failure(&path, &error))?;
    text.parse()
        .map_err(|error: toml::de::Error| SourceFailure(format!("{file}: {error}")))
}

pub(crate) fn load_crate_rules(root: &Path) -> Result<CrateRules, SourceFailure> {
    CrateRules::parse(&read_table(root, CRATES_FILE)?)
}

pub(crate) fn load_raw_rules(root: &Path) -> Result<RawRules, SourceFailure> {
    RawRules::parse(&read_table(root, RULES_FILE)?)
}

fn dependency_definitions(
    root: &Path,
    member: &Member,
    members: &[Member],
) -> Result<BTreeMap<String, Definitions>, SourceFailure> {
    members
        .iter()
        .filter(|candidate| member.dependencies.contains(&candidate.name))
        .map(|dependency| {
            crate_sources(root, &dependency.roots)
                .map(|sources| (dependency.name.replace('-', "_"), sources.definitions))
        })
        .collect()
}

pub(crate) fn check_members(
    root: &Path,
    members: &[Member],
    crates: &CrateRules,
    rules: &RawRules,
) -> Result<BTreeSet<Finding>, SourceFailure> {
    let names: BTreeSet<String> = members.iter().map(|member| member.name.clone()).collect();
    let unclassified = crates.classification_failures(&names);
    if !unclassified.is_empty() {
        return Err(SourceFailure(unclassified.join("\n")));
    }
    let mut findings = BTreeSet::new();
    for member in members {
        if crates.scope(&member.name) == Scope::Checked {
            let dependencies = dependency_definitions(root, member, members)?;
            check_crate(root, &member.roots, rules, &dependencies, &mut findings)?;
        }
    }
    Ok(findings)
}

pub(crate) fn check_file_text(
    members: &crate::workspace_sources::Members,
    member: &Member,
    replacement: Replacement<'_>,
) -> Result<Option<BTreeSet<Finding>>, SourceFailure> {
    let crates = load_crate_rules(&members.root)?;
    let names: BTreeSet<String> = members
        .packages
        .iter()
        .map(|candidate| candidate.name.clone())
        .collect();
    let unclassified = crates.classification_failures(&names);
    if !unclassified.is_empty() {
        return Err(SourceFailure(unclassified.join("\n")));
    }
    if crates.scope(&member.name) != Scope::Checked {
        return Ok(None);
    }
    let rules = load_raw_rules(&members.root)?;
    let sources = crate_sources_with(&members.root, &member.roots, Some(replacement))?;
    let in_tree = sources
        .files
        .iter()
        .any(|file| file.label == replacement.label)
        || sources
            .findings
            .iter()
            .any(|finding| finding.path == replacement.label);
    if !in_tree {
        return Ok(None);
    }
    let dependencies = dependency_definitions(&members.root, member, &members.packages)?;
    let mut findings: BTreeSet<Finding> = sources.findings;
    for file in &sources.files {
        file_findings(
            file,
            &rules,
            &sources.definitions,
            &dependencies,
            &mut findings,
        );
    }
    Ok(Some(
        findings
            .into_iter()
            .filter(|finding| finding.path == replacement.label)
            .collect(),
    ))
}

fn run() -> Result<BTreeSet<Finding>, SourceFailure> {
    let members = load_members(None)?;
    let crates = load_crate_rules(&members.root)?;
    let rules = load_raw_rules(&members.root)?;
    check_members(&members.root, &members.packages, &crates, &rules)
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
