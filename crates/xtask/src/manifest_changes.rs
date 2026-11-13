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

pub(crate) fn verdict(before: Option<&str>, after: Option<&str>) -> Verdict {
    match (before, after) {
        (None, None) => Verdict::DependenciesOnly,
        (Some(left), Some(right)) if left == right => Verdict::DependenciesOnly,
        (Some(left), Some(right)) => {
            match (outside_dependencies(left), outside_dependencies(right)) {
                (Ok(kept_left), Ok(kept_right)) if kept_left == kept_right => {
                    Verdict::DependenciesOnly
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
    Ok(verdict(before.as_deref(), after.as_deref()))
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
