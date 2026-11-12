mod graph;
mod metadata;
pub(crate) mod rules;
mod source;
mod tokens;

use std::collections::BTreeSet;
use std::fmt;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

pub(crate) use graph::check_graph;
pub(crate) use metadata::{Workspace, load_workspace};
pub(crate) use rules::{Rules, load_rules};
pub(crate) use source::check_sources;

pub(crate) const RULES_PATH: &str = "harness/arch-check/rules.toml";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Rule {
    UnknownCrate,
    PathPackage,
    AllowedEdge,
    DevEdge,
    DirectDependent,
    TransitiveDependency,
    ProductBinGraph,
    ExternalLocation,
    ExternalAllowlist,
    ExternalFeature,
    Reexport,
    UserPresenceImpl,
    UserPresenceAlias,
    UserPresenceReexport,
    TossOrderWrite,
    TossHost,
    HttpWriteMeans,
    ProcessSpawn,
    SourceParse,
}

impl Rule {
    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::UnknownCrate => "unknown-crate",
            Self::PathPackage => "path-package",
            Self::AllowedEdge => "allowed-edge",
            Self::DevEdge => "dev-edge",
            Self::DirectDependent => "direct-dependent",
            Self::TransitiveDependency => "transitive-dependency",
            Self::ProductBinGraph => "product-bin-graph",
            Self::ExternalLocation => "external-location",
            Self::ExternalAllowlist => "external-allowlist",
            Self::ExternalFeature => "external-feature",
            Self::Reexport => "reexport",
            Self::UserPresenceImpl => "user-presence-impl",
            Self::UserPresenceAlias => "user-presence-alias",
            Self::UserPresenceReexport => "user-presence-reexport",
            Self::TossOrderWrite => "toss-order-write",
            Self::TossHost => "toss-host",
            Self::HttpWriteMeans => "http-write-means",
            Self::ProcessSpawn => "process-spawn",
            Self::SourceParse => "source-parse",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Finding {
    pub(crate) rule: Rule,
    pub(crate) subject: String,
}

impl Finding {
    pub(crate) const fn new(rule: Rule, subject: String) -> Self {
        Self { rule, subject }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct CrateName(String);

impl CrateName {
    pub(crate) fn new(name: &str) -> Self {
        Self(String::from(name))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for CrateName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug)]
pub(crate) enum CheckFailure {
    Metadata(cargo_metadata::Error),
    Io { path: PathBuf, error: io::Error },
    Rules(String),
    Graph(String),
}

impl fmt::Display for CheckFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Metadata(error) => write!(f, "cargo metadata 실패: {error}"),
            Self::Io { path, error } => write!(f, "{}: {error}", path.display()),
            Self::Rules(problem) => write!(f, "{RULES_PATH}: {problem}"),
            Self::Graph(problem) => write!(f, "의존 그래프: {problem}"),
        }
    }
}

pub(crate) fn check_workspace(
    workspace: &Workspace,
    rules: &Rules,
) -> Result<BTreeSet<Finding>, CheckFailure> {
    let mut findings = check_graph(&rules.graph, &workspace.graph);
    findings.extend(check_sources(&rules.source, workspace)?);
    Ok(findings)
}

fn run(manifest: &Path) -> Result<BTreeSet<Finding>, CheckFailure> {
    let workspace = load_workspace(manifest, &[])?;
    let rules = load_rules(&workspace.root.join(RULES_PATH))?;
    check_workspace(&workspace, &rules)
}

fn report(outcome: &Result<BTreeSet<Finding>, CheckFailure>) -> io::Result<ExitCode> {
    let mut stderr = io::stderr().lock();
    match outcome {
        Ok(findings) if findings.is_empty() => {
            writeln!(stderr, "arch-check: 위반 0건")?;
            Ok(ExitCode::SUCCESS)
        }
        Ok(findings) => {
            writeln!(stderr, "arch-check: 위반 {}건", findings.len())?;
            for finding in findings {
                writeln!(stderr, "  {}: {}", finding.rule.name(), finding.subject)?;
            }
            Ok(ExitCode::FAILURE)
        }
        Err(failure) => {
            writeln!(stderr, "arch-check: {failure}")?;
            Ok(ExitCode::FAILURE)
        }
    }
}

pub(crate) fn command(manifest: &Path) -> ExitCode {
    match report(&run(manifest)) {
        Ok(code) => code,
        Err(_) => ExitCode::FAILURE,
    }
}

pub(crate) fn usage() -> ExitCode {
    match writeln!(io::stderr().lock(), "사용법: xtask arch-check") {
        Ok(()) | Err(_) => ExitCode::FAILURE,
    }
}
