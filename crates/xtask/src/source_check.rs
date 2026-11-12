mod allowlist;
mod comments;
mod failure;
mod finding;
mod manifest;
mod tokens;

#[cfg(test)]
mod tests;

use std::collections::BTreeSet;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use cargo_metadata::{Metadata, MetadataCommand};
use proc_macro2::TokenStream;

use allowlist::{Allowlists, Membership};
use comments::comment_lines;
use failure::CheckFailure;
use finding::{Finding, Rule};
use manifest::{ManifestRole, manifest_findings};
use tokens::{ClockScope, FloatScope, Scope, token_rules};

const SKIPPED_DIRECTORIES: [&str; 3] = ["target", "node_modules", ".git"];

const SKIPPED_TOP_LEVEL_DIRECTORIES: [&str; 1] = [".claude"];

const CARGO_CONFIG_DIRECTORY: &str = ".cargo";

const NESTED_CONFIG_FILES: [&str; 6] = [
    "clippy.toml",
    ".clippy.toml",
    "rustfmt.toml",
    ".rustfmt.toml",
    "rust-toolchain",
    "rust-toolchain.toml",
];

const BUILD_SCRIPT_FILE: &str = "build.rs";

const DOMAIN_DIRECTORY: &str = "crates/domain/";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EntryKind {
    File,
    CargoDirectory,
}

#[derive(Debug)]
struct Entry {
    relative: String,
    name: String,
    kind: EntryKind,
}

pub(crate) fn check_source(path: &str, source: &str, allowlists: &Allowlists) -> Vec<Finding> {
    if allowlists.generated(path) == Membership::Listed {
        return Vec::new();
    }
    let mut findings: Vec<Finding> = comment_lines(source)
        .into_iter()
        .map(|line| Finding::at_line(path, line, Rule::Comment))
        .collect();
    let scope = Scope {
        float: if path.starts_with(DOMAIN_DIRECTORY) {
            FloatScope::Domain
        } else {
            FloatScope::Elsewhere
        },
        clock: match allowlists.clock_environment(path) {
            Membership::Listed => ClockScope::Allowed,
            Membership::Unlisted => ClockScope::Forbidden,
        },
    };
    match source.parse::<TokenStream>() {
        Ok(stream) => findings.extend(
            token_rules(stream, scope)
                .into_iter()
                .map(|(line, rule)| Finding::at_line(path, line, rule)),
        ),
        Err(_) => findings.push(Finding::whole_file(path, Rule::Unparsable)),
    }
    findings
}

fn relative_path(root: &Path, path: &Path) -> Result<String, CheckFailure> {
    path.strip_prefix(root)
        .ok()
        .and_then(Path::to_str)
        .map(String::from)
        .ok_or_else(|| CheckFailure::OutsideRoot(path.to_path_buf()))
}

fn walk(root: &Path, directory: &Path, entries: &mut Vec<Entry>) -> Result<(), CheckFailure> {
    let listing = fs::read_dir(directory)
        .map_err(|error| CheckFailure::Io(directory.to_path_buf(), error))?;
    for listed in listing {
        let entry = listed.map_err(|error| CheckFailure::Io(directory.to_path_buf(), error))?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        let file_type = entry
            .file_type()
            .map_err(|error| CheckFailure::Io(path.clone(), error))?;
        let relative = relative_path(root, &path)?;
        if name == CARGO_CONFIG_DIRECTORY {
            entries.push(Entry {
                relative,
                name,
                kind: EntryKind::CargoDirectory,
            });
        } else if !file_type.is_dir() {
            entries.push(Entry {
                relative,
                name,
                kind: EntryKind::File,
            });
        } else if !SKIPPED_DIRECTORIES.contains(&name.as_str())
            && (directory != root || !SKIPPED_TOP_LEVEL_DIRECTORIES.contains(&name.as_str()))
        {
            walk(root, &path, entries)?;
        }
    }
    Ok(())
}

fn workspace_metadata(manifest: Option<&Path>) -> Result<Metadata, CheckFailure> {
    let mut command = MetadataCommand::new();
    if let Some(path) = manifest {
        command.manifest_path(path);
    }
    command
        .no_deps()
        .other_options(vec![String::from("--locked")])
        .exec()
        .map_err(CheckFailure::Metadata)
}

fn read_manifest(root: &Path, relative: &str) -> Result<toml::Table, CheckFailure> {
    let path = root.join(relative);
    let text = fs::read_to_string(&path).map_err(|error| CheckFailure::Io(path, error))?;
    text.parse()
        .map_err(|error| CheckFailure::Manifest(String::from(relative), error))
}

fn package_findings(
    root: &Path,
    metadata: &Metadata,
    allowlists: &Allowlists,
) -> Result<(Vec<String>, Vec<Finding>), CheckFailure> {
    let mut members = Vec::new();
    let mut findings = manifest_findings(
        "Cargo.toml",
        &read_manifest(root, "Cargo.toml")?,
        ManifestRole::WorkspaceRoot,
    );
    for package in metadata.workspace_packages() {
        let manifest = relative_path(root, package.manifest_path.as_std_path())?;
        findings.extend(manifest_findings(
            &manifest,
            &read_manifest(root, &manifest)?,
            ManifestRole::Member,
        ));
        for target in &package.targets {
            let source = relative_path(root, target.src_path.as_std_path())?;
            if target.is_custom_build() && allowlists.build_script(&source) == Membership::Unlisted
            {
                findings.push(Finding::whole_file(&source, Rule::BuildScript));
            }
            if target.is_proc_macro() {
                findings.push(Finding::whole_file(&manifest, Rule::ProcMacro));
            }
        }
        let directory = manifest.strip_suffix("Cargo.toml").unwrap_or(&manifest);
        members.push(String::from(directory));
    }
    Ok((members, findings))
}

fn entry_findings(
    root: &Path,
    entry: &Entry,
    members: &[String],
    allowlists: &Allowlists,
) -> Result<Vec<Finding>, CheckFailure> {
    let in_member = members
        .iter()
        .any(|member| entry.relative.starts_with(member.as_str()));
    let path = entry.relative.as_str();
    let findings = match entry.kind {
        EntryKind::CargoDirectory => vec![Finding::whole_file(path, Rule::CargoConfig)],
        EntryKind::File
            if NESTED_CONFIG_FILES.contains(&entry.name.as_str()) && path.contains('/') =>
        {
            vec![Finding::whole_file(
                path,
                Rule::NestedConfig(entry.name.clone()),
            )]
        }
        EntryKind::File
            if in_member
                && Path::new(path)
                    .extension()
                    .is_some_and(|extension| extension == "rs") =>
        {
            let full = root.join(path);
            let source =
                fs::read_to_string(&full).map_err(|error| CheckFailure::Io(full, error))?;
            let mut found = check_source(path, &source, allowlists);
            if entry.name == BUILD_SCRIPT_FILE
                && allowlists.build_script(path) == Membership::Unlisted
            {
                found.push(Finding::whole_file(path, Rule::BuildScript));
            }
            found
        }
        EntryKind::File => Vec::new(),
    };
    Ok(findings)
}

pub(crate) fn check_repository(
    root: &Path,
    allowlists: &Allowlists,
) -> Result<BTreeSet<Finding>, CheckFailure> {
    let metadata = workspace_metadata(Some(&root.join("Cargo.toml")))?;
    let (members, mut findings) = package_findings(root, &metadata, allowlists)?;
    let mut entries = Vec::new();
    walk(root, root, &mut entries)?;
    for entry in &entries {
        findings.extend(entry_findings(root, entry, &members, allowlists)?);
    }
    Ok(findings.into_iter().collect())
}

fn repository_root() -> Result<PathBuf, CheckFailure> {
    workspace_metadata(None).map(|metadata| metadata.workspace_root.into_std_path_buf())
}

pub(crate) fn command() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(_) => ExitCode::FAILURE,
    }
}

fn run() -> io::Result<ExitCode> {
    let outcome = repository_root().and_then(|root| {
        let allowlists = Allowlists::load(&root)?;
        check_repository(&root, &allowlists)
    });
    match outcome {
        Ok(findings) if findings.is_empty() => {
            writeln!(io::stdout().lock(), "source-check: 위반 없음")?;
            Ok(ExitCode::SUCCESS)
        }
        Ok(findings) => {
            let mut output = io::stderr().lock();
            for finding in &findings {
                writeln!(output, "{finding}")?;
            }
            writeln!(output, "source-check: 위반 {}건", findings.len())?;
            Ok(ExitCode::FAILURE)
        }
        Err(failure) => {
            writeln!(io::stderr().lock(), "source-check: {failure}")?;
            Ok(ExitCode::FAILURE)
        }
    }
}
