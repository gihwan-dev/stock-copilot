mod functions;
mod lcov;

#[cfg(test)]
mod tests;

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};

use crate::check_support::{Finding, read_text, report, workspace_root};
use crate::nextest_list::list_tests;
use crate::workspace_sources::relative_text;
use functions::{function_lines, functions};
use lcov::{LineCounts, parse_lcov};

pub(crate) const POLICY_FILE: &str = "harness/coverage/policy.toml";
pub(crate) const EXCEPTIONS_FILE: &str = "harness/coverage/exceptions.toml";
const REPORT_FILE: &str = "target/coverage/lcov.info";
const FULL_PERCENT: u32 = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Rule {
    BelowThreshold,
    UncoveredLine,
    UntestedFile,
}

impl fmt::Display for Rule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::BelowThreshold => "coverage:below-threshold",
            Self::UncoveredLine => "coverage:uncovered-line",
            Self::UntestedFile => "coverage:untested-file",
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Threshold {
    pub(crate) path: String,
    pub(crate) percent: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Policy {
    pub(crate) packages: Vec<String>,
    pub(crate) thresholds: Vec<Threshold>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Exception {
    pub(crate) path: String,
    pub(crate) function: String,
}

fn reasoned_entries<'table>(
    table: &'table toml::Table,
    key: &str,
    file: &str,
) -> Result<&'table [toml::Value], String> {
    let entries = table
        .get(key)
        .and_then(toml::Value::as_array)
        .ok_or_else(|| format!("{file}: {key} 목록이 없다"))?;
    let reasoned = entries.iter().all(|entry| {
        entry
            .get("reason")
            .and_then(toml::Value::as_str)
            .is_some_and(|reason| !reason.trim().is_empty())
    });
    if reasoned {
        Ok(entries.as_slice())
    } else {
        Err(format!("{file}: {key} 항목마다 reason이 있어야 한다"))
    }
}

fn text_of(entry: &toml::Value, key: &str, file: &str) -> Result<String, String> {
    entry
        .get(key)
        .and_then(toml::Value::as_str)
        .filter(|text| !text.trim().is_empty())
        .map(String::from)
        .ok_or_else(|| format!("{file}: 항목마다 {key}가 있어야 한다"))
}

pub(crate) fn parse_policy(text: &str) -> Result<Policy, String> {
    let table: toml::Table = text
        .parse()
        .map_err(|error| format!("{POLICY_FILE}: {error}"))?;
    let packages = table
        .get("packages")
        .and_then(toml::Value::as_array)
        .and_then(|items| {
            items
                .iter()
                .map(|item| item.as_str().map(String::from))
                .collect::<Option<Vec<String>>>()
        })
        .filter(|items| !items.is_empty())
        .ok_or_else(|| format!("{POLICY_FILE}: packages는 비어 있지 않은 이름 목록이어야 한다"))?;
    let thresholds = reasoned_entries(&table, "threshold", POLICY_FILE)?
        .iter()
        .map(|entry| {
            let path = text_of(entry, "path", POLICY_FILE)?;
            let percent = entry
                .get("percent")
                .and_then(toml::Value::as_integer)
                .and_then(|value| u32::try_from(value).ok())
                .filter(|value| (1..=FULL_PERCENT).contains(value))
                .ok_or_else(|| format!("{POLICY_FILE}: {path}의 percent는 1~100이어야 한다"))?;
            Ok(Threshold { path, percent })
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(Policy {
        packages,
        thresholds,
    })
}

pub(crate) fn load_exceptions(root: &Path, file: &str) -> Result<Vec<Exception>, String> {
    let table: toml::Table = read_text(&root.join(file))?
        .parse()
        .map_err(|error| format!("{file}: {error}"))?;
    reasoned_entries(&table, "exception", file)?
        .iter()
        .map(|entry| {
            Ok(Exception {
                path: text_of(entry, "path", file)?,
                function: text_of(entry, "function", file)?,
            })
        })
        .collect()
}

pub(crate) fn excepted_lines(
    root: &Path,
    exceptions: &[Exception],
) -> Result<BTreeMap<String, BTreeSet<usize>>, String> {
    let mut lines: BTreeMap<String, BTreeSet<usize>> = BTreeMap::new();
    for exception in exceptions {
        let source = read_text(&root.join(&exception.path))?;
        let spans = function_lines(&source, &exception.function);
        match spans.as_slice() {
            [single] => lines
                .entry(exception.path.clone())
                .or_default()
                .extend(single.clone()),
            [] => {
                return Err(format!(
                    "{}: 커버리지 예외 함수 {}가 없다",
                    exception.path, exception.function
                ));
            }
            [_, _, ..] => {
                return Err(format!(
                    "{}: 커버리지 예외 함수 {}가 여러 개라 어느 것인지 모른다",
                    exception.path, exception.function
                ));
            }
        }
    }
    Ok(lines)
}

fn matches(threshold: &str, path: &str) -> bool {
    path == threshold
        || path
            .strip_prefix(threshold)
            .is_some_and(|rest| rest == ".rs" || rest.starts_with('/'))
}

pub(crate) fn threshold_for(policy: &Policy, path: &str) -> Option<u32> {
    policy
        .thresholds
        .iter()
        .filter(|threshold| matches(&threshold.path, path))
        .map(|threshold| threshold.percent)
        .max()
}

fn meets(covered: usize, total: usize, percent: u32) -> bool {
    let scaled_covered = u128::try_from(covered)
        .ok()
        .and_then(|count| count.checked_mul(u128::from(FULL_PERCENT)));
    let scaled_total = u128::try_from(total)
        .ok()
        .and_then(|count| count.checked_mul(u128::from(percent)));
    match (scaled_covered, scaled_total) {
        (Some(left), Some(right)) => left >= right,
        (Some(_) | None, Some(_) | None) => false,
    }
}

pub(crate) fn evaluate(
    policy: &Policy,
    counts: &BTreeMap<String, LineCounts>,
    excepted: &BTreeMap<String, BTreeSet<usize>>,
) -> BTreeSet<Finding<Rule>> {
    let empty = BTreeSet::new();
    let mut findings = BTreeSet::new();
    for (path, lines) in counts {
        let Some(percent) = threshold_for(policy, path) else {
            continue;
        };
        let allowed = excepted.get(path).unwrap_or(&empty);
        let uncovered: Vec<usize> = lines
            .iter()
            .filter(|(line, count)| **count == 0 && !allowed.contains(*line))
            .map(|(line, _)| *line)
            .collect();
        if percent == FULL_PERCENT {
            findings.extend(
                uncovered
                    .into_iter()
                    .map(|line| Finding::at_line(path, line, Rule::UncoveredLine)),
            );
        } else {
            let total = lines.len();
            let covered = total.saturating_sub(uncovered.len());
            if !meets(covered, total, percent) {
                findings.insert(Finding::whole_file(path, Rule::BelowThreshold));
            }
        }
    }
    findings
}

pub(crate) fn measure(workspace: &Path, packages: &[String]) -> Result<String, String> {
    let output = workspace.join(REPORT_FILE);
    let mut command = Command::new("cargo");
    command
        .current_dir(workspace)
        .args(["llvm-cov", "nextest", "--locked", "--lcov", "--output-path"])
        .arg(&output)
        .arg("--manifest-path")
        .arg(workspace.join("Cargo.toml"))
        .stdin(Stdio::null());
    for package in packages {
        command.arg("--package").arg(package);
    }
    if output.exists() {
        fs::remove_file(&output).map_err(|error| format!("{}: {error}", output.display()))?;
    }
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("{}: {error}", parent.display()))?;
    }
    let status = command
        .status()
        .map_err(|error| format!("cargo llvm-cov: {error}"))?;
    if status.success() {
        read_text(&output)
    } else {
        Err(format!(
            "cargo llvm-cov가 실패했다({status}). llvm-tools가 없으면 rustup component add llvm-tools"
        ))
    }
}

fn rust_files(directory: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
    let listing =
        fs::read_dir(directory).map_err(|error| format!("{}: {error}", directory.display()))?;
    for listed in listing {
        let entry = listed.map_err(|error| format!("{}: {error}", directory.display()))?;
        let path = entry.path();
        let kind = entry
            .file_type()
            .map_err(|error| format!("{}: {error}", path.display()))?;
        if kind.is_dir() {
            rust_files(&path, files)?;
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            files.push(path);
        }
    }
    Ok(())
}

fn untested_findings(workspace: &Path, policy: &Policy) -> Result<BTreeSet<Finding<Rule>>, String> {
    let mut candidates = Vec::new();
    for threshold in &policy.thresholds {
        let base = workspace.join(&threshold.path);
        let file = workspace.join(format!("{}.rs", threshold.path));
        if file.is_file() {
            candidates.push(file);
        }
        if base.is_dir() {
            rust_files(&base, &mut candidates)?;
        }
    }
    candidates.sort();
    candidates.dedup();
    let mut findings = BTreeSet::new();
    for path in candidates {
        if !functions(&read_text(&path)?).is_empty() {
            findings.insert(Finding::whole_file(
                &relative_text(workspace, &path),
                Rule::UntestedFile,
            ));
        }
    }
    Ok(findings)
}

pub(crate) fn check_workspace(
    workspace: &Path,
    policy: &Policy,
    exceptions: &[Exception],
) -> Result<BTreeSet<Finding<Rule>>, String> {
    let excepted = excepted_lines(workspace, exceptions)?;
    if list_tests(workspace, &policy.packages, None)?.is_empty() {
        return untested_findings(workspace, policy);
    }
    let report_text = measure(workspace, &policy.packages)?;
    Ok(evaluate(
        policy,
        &parse_lcov(&report_text, workspace),
        &excepted,
    ))
}

pub(crate) fn command() -> ExitCode {
    report(
        "coverage-check",
        workspace_root().and_then(|root| {
            let policy = parse_policy(&read_text(&root.join(POLICY_FILE))?)?;
            let exceptions = load_exceptions(&root, EXCEPTIONS_FILE)?;
            check_workspace(&root, &policy, &exceptions)
        }),
    )
}
