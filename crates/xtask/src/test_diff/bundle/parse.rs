use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;
use std::path::Path;

use crate::git_base;
use crate::test_diff::approval::{valid_hash, valid_path, valid_task};

pub(super) const BUNDLES_DIR: &str = "harness/test-diff/bundles";
pub(super) const DRAFT_REASON: &str = "승인 사유를 적는다";

const BASE_KEYS: [&str; 5] = ["task", "base_commit", "reason", "findings", "file"];
const TARGET_KEYS: [&str; 3] = ["merge_head", "target_commit", "target_tree"];
const FILE_KEYS: [&str; 3] = ["path", "before_sha256", "after_sha256"];
const APPROVABLE_PREFIXES: [&str; 2] = ["test-diff:modified:", "test-diff:support-modified:"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Target {
    MergeHead(String),
    Commit(String),
    Tree(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct FileEntry {
    pub(super) path: String,
    pub(super) before: String,
    pub(super) after: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Bundle {
    pub(super) task: String,
    pub(super) base_commit: String,
    pub(super) target: Target,
    pub(super) reason: String,
    pub(super) findings: BTreeSet<String>,
    pub(super) files: BTreeMap<String, FileEntry>,
}

fn require_text<'a>(table: &'a toml::Table, label: &str, key: &str) -> Result<&'a str, String> {
    table
        .get(key)
        .and_then(toml::Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| format!("{label}: {key}가 없다"))
}

fn valid_finding_text(finding: &str) -> bool {
    APPROVABLE_PREFIXES.iter().any(|prefix| {
        finding
            .strip_prefix(prefix)
            .is_some_and(|rest| !rest.trim().is_empty())
    }) && !finding.contains('*')
        && !finding.contains('\n')
        && !finding.contains('\r')
}

fn parse_target(table: &toml::Table, label: &str, base_commit: &str) -> Result<Target, String> {
    let present: Vec<&str> = TARGET_KEYS
        .into_iter()
        .filter(|key| table.contains_key(*key))
        .collect();
    let [selected] = present.as_slice() else {
        return Err(format!(
            "{label}: merge_head, target_commit, target_tree 중 정확히 하나만 있어야 한다"
        ));
    };
    let value = require_text(table, label, selected)?;
    if !git_base::is_commit_id(value) {
        return Err(format!(
            "{label}: {selected}는 40자리 소문자 16진수여야 한다"
        ));
    }
    match *selected {
        "merge_head" => {
            if value == base_commit {
                return Err(format!("{label}: merge_head는 base_commit과 달라야 한다"));
            }
            Ok(Target::MergeHead(String::from(value)))
        }
        "target_commit" => {
            if value == base_commit {
                return Err(format!(
                    "{label}: target_commit은 base_commit과 달라야 한다"
                ));
            }
            Ok(Target::Commit(String::from(value)))
        }
        "target_tree" => Ok(Target::Tree(String::from(value))),
        _ => Err(format!("{label}: 알 수 없는 대상 키다: {selected}")),
    }
}

fn parse_findings(table: &toml::Table, label: &str) -> Result<BTreeSet<String>, String> {
    let entries = table
        .get("findings")
        .and_then(toml::Value::as_array)
        .ok_or_else(|| format!("{label}: findings 배열이 없다"))?;
    if entries.is_empty() {
        return Err(format!("{label}: findings가 비어 있다"));
    }
    let mut result = BTreeSet::new();
    for entry in entries {
        let text = entry
            .as_str()
            .ok_or_else(|| format!("{label}: findings 항목은 문자열이어야 한다"))?;
        if !valid_finding_text(text) {
            return Err(format!(
                "{label}: 승인할 수 없거나 형식이 잘못된 지적이다: {text}"
            ));
        }
        if !result.insert(String::from(text)) {
            return Err(format!("{label}: 중복 지적이다: {text}"));
        }
    }
    Ok(result)
}

fn parse_files(table: &toml::Table, label: &str) -> Result<BTreeMap<String, FileEntry>, String> {
    let entries = table
        .get("file")
        .and_then(toml::Value::as_array)
        .ok_or_else(|| format!("{label}: file 목록이 없다"))?;
    if entries.is_empty() {
        return Err(format!("{label}: file 목록이 비어 있다"));
    }
    let mut files = BTreeMap::new();
    for entry in entries {
        let row = entry
            .as_table()
            .ok_or_else(|| format!("{label}: file 항목이 표가 아니다"))?;
        if FILE_KEYS.iter().any(|key| !row.contains_key(*key))
            || row.keys().any(|key| !FILE_KEYS.contains(&key.as_str()))
        {
            return Err(format!("{label}: file 필드가 다르다"));
        }
        let path = require_text(row, label, "path")?;
        let before = require_text(row, label, "before_sha256")?;
        let after = require_text(row, label, "after_sha256")?;
        if !valid_path(path)
            || path.starts_with("harness/")
            || !valid_hash(before)
            || !valid_hash(after)
            || before == after
        {
            return Err(format!("{label}: file 값이 잘못됐다: {path}"));
        }
        let item = FileEntry {
            path: String::from(path),
            before: String::from(before),
            after: String::from(after),
        };
        if files.insert(String::from(path), item).is_some() {
            return Err(format!("{label}: 중복 파일 항목이다: {path}"));
        }
    }
    Ok(files)
}

pub(super) fn valid_bundle_relative_path(path: &str) -> bool {
    path.strip_prefix(&format!("{BUNDLES_DIR}/"))
        .and_then(|rest| rest.strip_suffix(".toml"))
        .is_some_and(|stem| !stem.contains('/') && valid_task(stem))
}

fn expected_task_from_path(path: &str) -> Option<&str> {
    Path::new(path)
        .file_name()
        .and_then(std::ffi::OsStr::to_str)
        .and_then(|name| name.strip_suffix(".toml"))
        .filter(|stem| valid_task(stem))
}

pub(super) fn parse(label: &str, contents: &str) -> Result<Bundle, String> {
    let table: toml::Table = contents
        .parse()
        .map_err(|error: toml::de::Error| format!("{label}: {error}"))?;
    if BASE_KEYS.iter().any(|key| !table.contains_key(*key))
        || table
            .keys()
            .any(|key| !BASE_KEYS.contains(&key.as_str()) && !TARGET_KEYS.contains(&key.as_str()))
    {
        return Err(format!("{label}: 묶음 필드가 다르다"));
    }
    let task = require_text(&table, label, "task")?;
    if !valid_task(task) {
        return Err(format!("{label}: task 형식이 잘못됐다: {task}"));
    }
    if let Some(stem) = expected_task_from_path(label)
        && stem != task
    {
        return Err(format!(
            "{label}: 파일 이름({stem}.toml)과 task({task})가 다르다"
        ));
    }
    let base_commit = require_text(&table, label, "base_commit")?;
    if !git_base::is_commit_id(base_commit) {
        return Err(format!(
            "{label}: base_commit은 40자리 소문자 16진수여야 한다"
        ));
    }
    let target = parse_target(&table, label, base_commit)?;
    let reason = require_text(&table, label, "reason")?;
    if reason.trim() == DRAFT_REASON {
        return Err(format!(
            "{label}: 초안 기본 reason(\"{DRAFT_REASON}\")을 실제 승인 사유로 바꿔야 한다"
        ));
    }
    let findings = parse_findings(&table, label)?;
    let files = parse_files(&table, label)?;
    Ok(Bundle {
        task: String::from(task),
        base_commit: String::from(base_commit),
        target,
        reason: String::from(reason),
        findings,
        files,
    })
}

fn quote_toml(value: &str) -> String {
    toml::Value::String(String::from(value)).to_string()
}

pub(super) fn render(bundle: &Bundle) -> Result<String, String> {
    let mut output = String::new();
    let (target_key, target_value) = match &bundle.target {
        Target::MergeHead(sha) => ("merge_head", sha.as_str()),
        Target::Commit(sha) => ("target_commit", sha.as_str()),
        Target::Tree(sha) => ("target_tree", sha.as_str()),
    };
    writeln!(output, "task = {}", quote_toml(&bundle.task)).map_err(|error| error.to_string())?;
    writeln!(output, "base_commit = {}", quote_toml(&bundle.base_commit))
        .map_err(|error| error.to_string())?;
    writeln!(output, "{target_key} = {}", quote_toml(target_value))
        .map_err(|error| error.to_string())?;
    writeln!(output, "reason = {}", quote_toml(&bundle.reason))
        .map_err(|error| error.to_string())?;
    writeln!(output, "findings = [").map_err(|error| error.to_string())?;
    for finding in &bundle.findings {
        writeln!(output, "  {},", quote_toml(finding)).map_err(|error| error.to_string())?;
    }
    writeln!(output, "]").map_err(|error| error.to_string())?;
    for entry in bundle.files.values() {
        writeln!(output, "\n[[file]]").map_err(|error| error.to_string())?;
        writeln!(output, "path = {}", quote_toml(&entry.path))
            .map_err(|error| error.to_string())?;
        writeln!(output, "before_sha256 = {}", quote_toml(&entry.before))
            .map_err(|error| error.to_string())?;
        writeln!(output, "after_sha256 = {}", quote_toml(&entry.after))
            .map_err(|error| error.to_string())?;
    }
    Ok(output)
}
