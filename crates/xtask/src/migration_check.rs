#[cfg(test)]
mod tests;

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::path::Path;
use std::process::{Command, ExitCode};

use crate::check_support::{Finding, report, workspace_root};

pub(crate) const MIGRATIONS_DIRECTORY: &str = "crates/adapters/store-sqlite/migrations";

pub(crate) const BASE_BRANCHES: [&str; 2] = ["main", "master"];

pub(crate) const GIT_LOCATION_VARIABLES: [&str; 7] = [
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_INDEX_FILE",
    "GIT_OBJECT_DIRECTORY",
    "GIT_ALTERNATE_OBJECT_DIRECTORIES",
    "GIT_COMMON_DIR",
    "GIT_NAMESPACE",
];

const NUMBER_WIDTH: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Rule {
    Name,
    DuplicateNumber,
    Gap,
    Modified,
    Removed,
    ModifiedInHistory,
}

impl fmt::Display for Rule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Name => "migration:name",
            Self::DuplicateNumber => "migration:duplicate-number",
            Self::Gap => "migration:gap",
            Self::Modified => "migration:modified",
            Self::Removed => "migration:removed",
            Self::ModifiedInHistory => "migration:modified-in-history",
        })
    }
}

pub(crate) fn git(root: &Path, arguments: &[&str]) -> Result<Vec<u8>, String> {
    let mut command = Command::new("git");
    for variable in GIT_LOCATION_VARIABLES {
        command.env_remove(variable);
    }
    let output = command
        .arg("-C")
        .arg(root)
        .args(arguments)
        .output()
        .map_err(|error| format!("git {}: {error}", arguments.join(" ")))?;
    if output.status.success() {
        Ok(output.stdout)
    } else {
        Err(format!(
            "git {}: {}",
            arguments.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

fn git_text(root: &Path, arguments: &[&str]) -> Result<String, String> {
    git(root, arguments).map(|bytes| String::from_utf8_lossy(&bytes).trim().to_owned())
}

fn base_commit(root: &Path) -> Result<String, String> {
    let branch = BASE_BRANCHES
        .iter()
        .find_map(|branch| {
            git_text(
                root,
                &[
                    "rev-parse",
                    "--verify",
                    "--quiet",
                    &format!("{branch}^{{commit}}"),
                ],
            )
            .ok()
        })
        .ok_or_else(|| format!("기준 브랜치 {BASE_BRANCHES:?}를 찾을 수 없다"))?;
    git_text(root, &["merge-base", "HEAD", &branch])
}

fn applied_paths(root: &Path, base: &str) -> Result<BTreeSet<String>, String> {
    let listing = git_text(
        root,
        &[
            "ls-tree",
            "--name-only",
            base,
            "--",
            &format!("{MIGRATIONS_DIRECTORY}/"),
        ],
    )?;
    Ok(listing
        .lines()
        .filter(|line| !line.is_empty())
        .map(String::from)
        .collect())
}

fn history_changes(root: &Path, base: &str) -> Result<BTreeSet<String>, String> {
    let listing = git_text(
        root,
        &[
            "log",
            "--no-renames",
            "--diff-filter=MDT",
            "--format=",
            "--name-only",
            base,
            "--",
            MIGRATIONS_DIRECTORY,
        ],
    )?;
    Ok(listing
        .lines()
        .filter(|line| !line.is_empty())
        .map(String::from)
        .collect())
}

pub(crate) fn migration_number(name: &str) -> Option<u32> {
    let stem = name.strip_suffix(".sql")?;
    let (number, label) = stem.split_once('_')?;
    let well_formed = number.len() == NUMBER_WIDTH
        && number.chars().all(|next| next.is_ascii_digit())
        && !label.is_empty()
        && !label.starts_with('_')
        && !label.ends_with('_')
        && !label.contains("__")
        && label
            .chars()
            .all(|next| next.is_ascii_lowercase() || next.is_ascii_digit() || next == '_');
    if well_formed {
        number.parse().ok()
    } else {
        None
    }
}

fn working_files(root: &Path) -> Result<BTreeMap<String, Option<u32>>, String> {
    let directory = root.join(MIGRATIONS_DIRECTORY);
    if !directory.exists() {
        return Ok(BTreeMap::new());
    }
    let listing =
        fs::read_dir(&directory).map_err(|error| format!("{}: {error}", directory.display()))?;
    let mut files = BTreeMap::new();
    for listed in listing {
        let entry = listed.map_err(|error| format!("{}: {error}", directory.display()))?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let file_type = entry
            .file_type()
            .map_err(|error| format!("{name}: {error}"))?;
        let number =
            migration_number(&name).filter(|_| !file_type.is_dir() && !file_type.is_symlink());
        files.insert(format!("{MIGRATIONS_DIRECTORY}/{name}"), number);
    }
    Ok(files)
}

fn sequence_findings(files: &BTreeMap<String, Option<u32>>) -> Vec<Finding<Rule>> {
    let mut findings = Vec::new();
    let mut by_number: BTreeMap<u32, Vec<&String>> = BTreeMap::new();
    for (path, number) in files {
        match number {
            Some(value) => by_number.entry(*value).or_default().push(path),
            None => findings.push(Finding::whole_file(path, Rule::Name)),
        }
    }
    let mut expected: u32 = 1;
    for (number, paths) in &by_number {
        if paths.len() > 1 {
            findings.extend(
                paths
                    .iter()
                    .map(|path| Finding::whole_file(path, Rule::DuplicateNumber)),
            );
        }
        if *number != expected {
            findings.extend(
                paths
                    .iter()
                    .map(|path| Finding::whole_file(path, Rule::Gap)),
            );
        }
        expected = number.saturating_add(1);
    }
    findings
}

fn applied_findings(
    root: &Path,
    base: &str,
    files: &BTreeMap<String, Option<u32>>,
) -> Result<Vec<Finding<Rule>>, String> {
    let mut findings: Vec<Finding<Rule>> = history_changes(root, base)?
        .iter()
        .map(|path| Finding::whole_file(path, Rule::ModifiedInHistory))
        .collect();
    for path in applied_paths(root, base)? {
        if !files.contains_key(&path) {
            findings.push(Finding::whole_file(&path, Rule::Removed));
            continue;
        }
        let applied = git(root, &["cat-file", "blob", &format!("{base}:{path}")])?;
        let current = fs::read(root.join(&path)).map_err(|error| format!("{path}: {error}"))?;
        if applied != current {
            findings.push(Finding::whole_file(&path, Rule::Modified));
        }
    }
    Ok(findings)
}

pub(crate) fn check_repository(root: &Path) -> Result<BTreeSet<Finding<Rule>>, String> {
    let base = base_commit(root)?;
    let files = working_files(root)?;
    let mut findings: BTreeSet<Finding<Rule>> = sequence_findings(&files).into_iter().collect();
    findings.extend(applied_findings(root, &base, &files)?);
    Ok(findings)
}

pub(crate) fn command() -> ExitCode {
    report(
        "migration-check",
        workspace_root().and_then(|root| check_repository(&root)),
    )
}
