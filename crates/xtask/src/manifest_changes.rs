#[cfg(test)]
mod tests;

use std::io::{self, Write};
use std::path::Path;
use std::process::ExitCode;

use crate::git_base::{git, is_commit_id, output};

const DEPENDENCY_TABLES: [&str; 5] = [
    "dependencies",
    "dev-dependencies",
    "build-dependencies",
    "dev_dependencies",
    "build_dependencies",
];
const WORKSPACE_TABLE: &str = "workspace";
const PLAIN_KEYS: [&str; 8] = [
    "version",
    "features",
    "default-features",
    "default_features",
    "optional",
    "package",
    "workspace",
    "path",
];
const VERSION_KEY: &str = "version";
const PATH_KEY: &str = "path";
const WORKSPACE_KEY: &str = "workspace";
const FORBIDDEN_IN_VERSION: [char; 3] = [':', '/', '\\'];
const BLOB_MODES: [&str; 2] = ["100644", "100755"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Verdict {
    DependenciesOnly,
    Outside,
}

fn outside_dependencies(text: &str) -> Result<toml::Table, String> {
    let mut table: toml::Table = text.parse().map_err(|error| format!("{error}"))?;
    for name in DEPENDENCY_TABLES {
        table.remove(name);
    }
    if let Some(toml::Value::Table(workspace)) = table.get_mut(WORKSPACE_TABLE) {
        workspace.remove(DEPENDENCY_TABLES[0]);
    }
    Ok(table)
}

fn inside_repository(directory: &str, relative: &str) -> Verdict {
    if relative.starts_with('/') || relative.contains(['\\', ':']) {
        return Verdict::Outside;
    }
    let mut parts: Vec<&str> = directory
        .split('/')
        .filter(|part| !part.is_empty())
        .collect();
    for part in relative.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                if parts.pop().is_none() {
                    return Verdict::Outside;
                }
            }
            name => parts.push(name),
        }
    }
    Verdict::DependenciesOnly
}

fn plain_version(value: &toml::Value) -> Verdict {
    match value.as_str() {
        Some(text) if !text.contains(FORBIDDEN_IN_VERSION) => Verdict::DependenciesOnly,
        Some(_) | None => Verdict::Outside,
    }
}

fn plain_entry(directory: &str, value: &toml::Value) -> Verdict {
    let Some(entry) = value.as_table() else {
        return plain_version(value);
    };
    let unknown = entry.keys().any(|key| !PLAIN_KEYS.contains(&key.as_str()));
    let version = entry.get(VERSION_KEY).map(plain_version);
    let path = entry.get(PATH_KEY).map(|path| match path.as_str() {
        Some(text) => inside_repository(directory, text),
        None => Verdict::Outside,
    });
    let workspace = entry.get(WORKSPACE_KEY).map(|flag| match flag.as_bool() {
        Some(true) => Verdict::DependenciesOnly,
        Some(false) | None => Verdict::Outside,
    });
    if unknown || [version, path, workspace].contains(&Some(Verdict::Outside)) {
        Verdict::Outside
    } else {
        Verdict::DependenciesOnly
    }
}

fn dependency_entries(table: &toml::Table) -> Vec<&toml::Value> {
    let workspace = table
        .get(WORKSPACE_TABLE)
        .and_then(toml::Value::as_table)
        .and_then(|workspace| workspace.get(DEPENDENCY_TABLES[0]));
    DEPENDENCY_TABLES
        .iter()
        .filter_map(|name| table.get(*name))
        .chain(workspace)
        .flat_map(|section| match section.as_table() {
            Some(entries) => entries.values().collect(),
            None => vec![section],
        })
        .collect()
}

fn plain_dependencies(directory: &str, text: &str) -> Verdict {
    match text.parse::<toml::Table>() {
        Ok(table) => {
            if dependency_entries(&table)
                .into_iter()
                .any(|entry| plain_entry(directory, entry) == Verdict::Outside)
            {
                Verdict::Outside
            } else {
                Verdict::DependenciesOnly
            }
        }
        Err(_) => Verdict::Outside,
    }
}

pub(crate) fn verdict(directory: &str, before: Option<&str>, after: Option<&str>) -> Verdict {
    match (before, after) {
        (None, None) => Verdict::DependenciesOnly,
        (Some(left), Some(right)) if left == right => Verdict::DependenciesOnly,
        (Some(left), Some(right)) => {
            match (outside_dependencies(left), outside_dependencies(right)) {
                (Ok(kept_left), Ok(kept_right)) if kept_left == kept_right => {
                    plain_dependencies(directory, right)
                }
                _ => Verdict::Outside,
            }
        }
        _ => Verdict::Outside,
    }
}

fn blob(root: &Path, commit: &str, path: &str) -> Result<Option<String>, String> {
    let mut listing = git(root);
    listing.args(["ls-tree", "-z", "--full-tree", commit, "--", path]);
    let entry = String::from_utf8_lossy(&output(listing, "ls-tree")?).into_owned();
    let Some((meta, _)) = entry.split_once('\t') else {
        return Ok(None);
    };
    let fields: Vec<&str> = meta.split(' ').collect();
    let [mode, _, object] = fields.as_slice() else {
        return Err(format!("{path}: ls-tree 줄을 읽지 못했다"));
    };
    if !BLOB_MODES.contains(mode) {
        return Err(format!("{path}: 일반 파일이 아니다({mode})"));
    }
    let mut reading = git(root);
    reading.args(["cat-file", "blob", object]);
    String::from_utf8(output(reading, "cat-file")?)
        .map(Some)
        .map_err(|error| format!("{path}: {error}"))
}

pub(crate) fn compare(
    root: &Path,
    parent: &str,
    commit: &str,
    path: &str,
) -> Result<Verdict, String> {
    if !(is_commit_id(parent) && is_commit_id(commit)) {
        return Err(String::from("manifest-changes는 40자리 커밋 ID만 받는다"));
    }
    let before = blob(root, parent, path)?;
    let after = blob(root, commit, path)?;
    let directory = path.rsplit_once('/').map_or("", |(folder, _)| folder);
    Ok(verdict(directory, before.as_deref(), after.as_deref()))
}

pub(crate) fn command(arguments: &[String]) -> ExitCode {
    let outcome = match arguments {
        [root, parent, commit, path] => compare(Path::new(root), parent, commit, path),
        _ => Err(String::from(
            "사용법: xtask manifest-changes <저장소> <부모 커밋> <커밋> <경로>",
        )),
    };
    let (line, code) = match outcome {
        Ok(Verdict::DependenciesOnly) => (String::from("의존성 절만 바뀌었다"), ExitCode::SUCCESS),
        Ok(Verdict::Outside) => (String::from("의존성 절 밖이 바뀌었다"), ExitCode::FAILURE),
        Err(failure) => (failure, ExitCode::FAILURE),
    };
    match writeln!(io::stdout().lock(), "manifest-changes: {line}") {
        Ok(()) => code,
        Err(_) => ExitCode::FAILURE,
    }
}
