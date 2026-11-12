mod sha256;

#[cfg(test)]
mod tests;

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::path::Path;
use std::process::ExitCode;

use crate::check_support::{Finding, report, workspace_root};
use sha256::sha256_hex;

pub(crate) const STORE_DIRECTORY: &str = "crates/adapters/store-sqlite";
const MIGRATIONS_DIRECTORY: &str = "migrations";
const LOCK_FILE: &str = "migrations.lock";
const LOCK_KEY: &str = "migration";
const ENTRY_KEYS: [&str; 3] = ["number", "file", "sha256"];
const NUMBER_WIDTH: usize = 4;
const HASH_WIDTH: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Rule {
    Name,
    DuplicateNumber,
    Gap,
    Empty,
    Modified,
    Removed,
    LockFormat,
    LockDuplicate,
    LockOrder,
}

impl fmt::Display for Rule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Name => "migration:name",
            Self::DuplicateNumber => "migration:duplicate-number",
            Self::Gap => "migration:gap",
            Self::Empty => "migration:empty",
            Self::Modified => "migration:modified",
            Self::Removed => "migration:removed",
            Self::LockFormat => "migration:lock-format",
            Self::LockDuplicate => "migration:lock-duplicate",
            Self::LockOrder => "migration:lock-order",
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LockEntry {
    number: u32,
    file: String,
    sha256: String,
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

fn lock_entry(value: &toml::Value) -> Option<LockEntry> {
    let table = value.as_table()?;
    let known = table.len() == ENTRY_KEYS.len()
        && table.keys().all(|key| ENTRY_KEYS.contains(&key.as_str()));
    let number = u32::try_from(table.get("number")?.as_integer()?).ok()?;
    let file = table.get("file")?.as_str()?;
    let sha256 = table.get("sha256")?.as_str()?;
    let hashed = sha256.len() == HASH_WIDTH
        && sha256
            .chars()
            .all(|next| next.is_ascii_digit() || ('a'..='f').contains(&next));
    let named = migration_number(file) == Some(number);
    (known && hashed && named).then(|| LockEntry {
        number,
        file: String::from(file),
        sha256: String::from(sha256),
    })
}

fn read_lock(store: &Path) -> Option<Vec<LockEntry>> {
    let table: toml::Table = fs::read_to_string(store.join(LOCK_FILE))
        .ok()?
        .parse()
        .ok()?;
    if table.len() != 1 {
        return None;
    }
    table
        .get(LOCK_KEY)?
        .as_array()?
        .iter()
        .map(lock_entry)
        .collect()
}

fn lock_findings(entries: &[LockEntry], lock_path: &str) -> Vec<Finding<Rule>> {
    let numbers: BTreeSet<u32> = entries.iter().map(|entry| entry.number).collect();
    let files: BTreeSet<&str> = entries.iter().map(|entry| entry.file.as_str()).collect();
    if numbers.len() != entries.len() || files.len() != entries.len() {
        return vec![Finding::whole_file(lock_path, Rule::LockDuplicate)];
    }
    let in_order = (1..)
        .zip(entries)
        .all(|(expected, entry): (u32, &LockEntry)| entry.number == expected);
    if in_order {
        Vec::new()
    } else {
        vec![Finding::whole_file(lock_path, Rule::LockOrder)]
    }
}

fn working_files(store: &Path) -> Result<BTreeMap<String, Option<u32>>, String> {
    let directory = store.join(MIGRATIONS_DIRECTORY);
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
        files.insert(name, number);
    }
    Ok(files)
}

fn migration_path(name: &str) -> String {
    format!("{STORE_DIRECTORY}/{MIGRATIONS_DIRECTORY}/{name}")
}

fn sequence_findings(files: &BTreeMap<String, Option<u32>>) -> Vec<Finding<Rule>> {
    let mut findings = Vec::new();
    let mut by_number: BTreeMap<u32, Vec<String>> = BTreeMap::new();
    for (name, number) in files {
        match number {
            Some(value) => by_number
                .entry(*value)
                .or_default()
                .push(migration_path(name)),
            None => findings.push(Finding::whole_file(&migration_path(name), Rule::Name)),
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

fn content_findings(
    store: &Path,
    files: &BTreeMap<String, Option<u32>>,
    entries: &[LockEntry],
) -> Result<Vec<Finding<Rule>>, String> {
    let mut findings = Vec::new();
    for (name, number) in files {
        if number.is_some() {
            let path = store.join(MIGRATIONS_DIRECTORY).join(name);
            let bytes = fs::read(&path).map_err(|error| format!("{}: {error}", path.display()))?;
            if bytes.is_empty() {
                findings.push(Finding::whole_file(&migration_path(name), Rule::Empty));
            }
            if let Some(entry) = entries.iter().find(|entry| entry.file == *name)
                && sha256_hex(&bytes) != entry.sha256
            {
                findings.push(Finding::whole_file(&migration_path(name), Rule::Modified));
            }
        }
    }
    for entry in entries {
        if files.get(&entry.file).copied().flatten().is_none() {
            findings.push(Finding::whole_file(
                &migration_path(&entry.file),
                Rule::Removed,
            ));
        }
    }
    Ok(findings)
}

pub(crate) fn check_store(store: &Path) -> Result<BTreeSet<Finding<Rule>>, String> {
    let lock_path = format!("{STORE_DIRECTORY}/{LOCK_FILE}");
    let files = working_files(store)?;
    let mut findings: BTreeSet<Finding<Rule>> = sequence_findings(&files).into_iter().collect();
    let entries = read_lock(store);
    if let Some(locked) = &entries {
        findings.extend(lock_findings(locked, &lock_path));
    } else {
        findings.insert(Finding::whole_file(&lock_path, Rule::LockFormat));
    }
    findings.extend(content_findings(
        store,
        &files,
        entries.as_deref().map_or(&[], |locked| locked),
    )?);
    Ok(findings)
}

pub(crate) fn command() -> ExitCode {
    report(
        "migration-check",
        workspace_root().and_then(|root| check_store(&root.join(STORE_DIRECTORY))),
    )
}
