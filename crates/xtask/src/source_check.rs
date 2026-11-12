mod allowlist;
pub(crate) mod cargo_config;
mod comments;
mod failure;
mod finding;
mod manifest;
mod test_impls;
mod tokens;

#[cfg(test)]
mod cargo_config_tests;
#[cfg(test)]
mod tests;

use std::collections::BTreeSet;
use std::fs;
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};
use std::process::ExitCode;

use cargo_metadata::{Metadata, MetadataCommand};
use proc_macro2::TokenStream;

use allowlist::{Allowlists, Membership};
use cargo_config::{ConfigFile, config_problems};
use comments::comment_lines;
use failure::CheckFailure;
use finding::{Finding, Rule};
use manifest::{ManifestRole, manifest_findings};
use test_impls::test_module_impl_lines;
use tokens::{AttributeScope, ClockScope, FloatScope, Scope, token_rules};

const SKIPPED_DIRECTORIES: [&str; 4] = [
    ".git",
    ".claude",
    "node_modules",
    "apps/desktop/node_modules",
];

const CARGO_CONFIG_DIRECTORY: &str = ".cargo";

pub(crate) const NESTED_CONFIG_FILES: [&str; 6] = [
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
    CargoSetting(ConfigFile),
    Symlink,
}

#[derive(Debug)]
struct Entry {
    relative: String,
    name: String,
    kind: EntryKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Exemption {
    Checked,
    Exempt,
}

fn generated_exempt(scope: AttributeScope, rule: &Rule, allowlists: &Allowlists) -> Exemption {
    match (scope, rule) {
        (AttributeScope::Generated, Rule::Attribute(name))
            if allowlists.generated_attribute(name) == Membership::Listed =>
        {
            Exemption::Exempt
        }
        _ => Exemption::Checked,
    }
}

pub(crate) fn check_source(path: &str, source: &str, allowlists: &Allowlists) -> Vec<Finding> {
    let attributes = match allowlists.generated(path) {
        Membership::Listed => AttributeScope::Generated,
        Membership::Unlisted => AttributeScope::Checked,
    };
    let mut findings: Vec<Finding> = match attributes {
        AttributeScope::Checked => comment_lines(source)
            .into_iter()
            .map(|line| Finding::at_line(path, line, Rule::Comment))
            .collect(),
        AttributeScope::Generated => Vec::new(),
    };
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
                .filter(|(_, rule)| {
                    generated_exempt(attributes, rule, allowlists) == Exemption::Checked
                })
                .filter(|(_, rule)| {
                    allowlists
                        .clock_names(path)
                        .is_none_or(|allowed| !allowed.contains(&rule.to_string()))
                })
                .map(|(line, rule)| Finding::at_line(path, line, rule)),
        ),
        Err(_) => findings.push(Finding::whole_file(path, Rule::Unparsable)),
    }
    findings.extend(
        test_module_impl_lines(path, source)
            .into_iter()
            .map(|line| Finding::at_line(path, line, Rule::TestModuleImpl)),
    );
    findings
}

fn relative_path(root: &Path, path: &Path) -> Result<String, CheckFailure> {
    path.strip_prefix(root)
        .ok()
        .and_then(Path::to_str)
        .map(String::from)
        .ok_or_else(|| CheckFailure::OutsideRoot(path.to_path_buf()))
}

fn lexically_normalized(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::ParentDir => {
                normalized.pop();
            }
            Component::CurDir => {}
            Component::Prefix(_) | Component::RootDir | Component::Normal(_) => {
                normalized.push(component);
            }
        }
    }
    normalized
}

fn walk(
    root: &Path,
    directory: &Path,
    skipped: &BTreeSet<String>,
    entries: &mut Vec<Entry>,
) -> Result<(), CheckFailure> {
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
        if file_type.is_symlink() {
            entries.push(Entry {
                relative,
                name,
                kind: EntryKind::Symlink,
            });
        } else if name == CARGO_CONFIG_DIRECTORY
            && relative == CARGO_CONFIG_DIRECTORY
            && file_type.is_dir()
        {
            cargo_settings(root, &path, entries)?;
        } else if name == CARGO_CONFIG_DIRECTORY {
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
        } else if !skipped.contains(&relative) {
            walk(root, &path, skipped, entries)?;
        }
    }
    Ok(())
}

fn cargo_settings(
    root: &Path,
    directory: &Path,
    entries: &mut Vec<Entry>,
) -> Result<(), CheckFailure> {
    let listing = fs::read_dir(directory)
        .map_err(|error| CheckFailure::Io(directory.to_path_buf(), error))?;
    for listed in listing {
        let entry = listed.map_err(|error| CheckFailure::Io(directory.to_path_buf(), error))?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        let file_type = entry
            .file_type()
            .map_err(|error| CheckFailure::Io(path.clone(), error))?;
        let kind = if file_type.is_symlink() {
            EntryKind::Symlink
        } else {
            match ConfigFile::named(&name) {
                Some(file) if !file_type.is_dir() => EntryKind::CargoSetting(file),
                Some(_) | None => EntryKind::CargoDirectory,
            }
        };
        entries.push(Entry {
            relative: relative_path(root, &path)?,
            name,
            kind,
        });
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
        let directory = manifest.strip_suffix("Cargo.toml").unwrap_or(&manifest);
        for target in &package.targets {
            let source = relative_path(root, &lexically_normalized(target.src_path.as_std_path()))?;
            if !source.starts_with(directory) {
                findings.push(Finding::whole_file(&source, Rule::TargetOutsideMember));
            }
            if target.is_custom_build() && allowlists.build_script(&source) == Membership::Unlisted
            {
                findings.push(Finding::whole_file(&source, Rule::BuildScript));
            }
            if target.is_proc_macro() {
                findings.push(Finding::whole_file(&manifest, Rule::ProcMacro));
            }
        }
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
        EntryKind::CargoSetting(file) => {
            let full = root.join(path);
            let text = fs::read_to_string(&full).map_err(|error| CheckFailure::Io(full, error))?;
            if config_problems(file, &text).is_empty() {
                Vec::new()
            } else {
                vec![Finding::whole_file(path, Rule::CargoConfig)]
            }
        }
        EntryKind::Symlink => vec![Finding::whole_file(path, Rule::Symlink)],
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
    let mut skipped: BTreeSet<String> = SKIPPED_DIRECTORIES.into_iter().map(String::from).collect();
    if let Ok(target) = metadata.target_directory.as_std_path().strip_prefix(root)
        && let Some(relative) = target.to_str()
    {
        skipped.insert(String::from(relative));
    }
    let mut entries = Vec::new();
    walk(root, root, &skipped, &mut entries)?;
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
