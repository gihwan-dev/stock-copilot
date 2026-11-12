#[cfg(test)]
mod tests;

use std::collections::BTreeSet;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};

use crate::check_support::{Finding, read_text, report, workspace_root};
use crate::git_base::{resolve_base, working_tree_diff};

pub(crate) const SURVIVORS_FILE: &str = "harness/mutants/survivors.toml";
const OUTPUT_DIRECTORY: &str = "target/mutants";
const DIFF_FILE: &str = "target/mutants/base.diff";
const MISSED_FILE: &str = "missed.txt";
const TIMEOUT_FILE: &str = "timeout.txt";
const PASSED: i32 = 0;
const FOUND_SURVIVORS: i32 = 2;
const FOUND_TIMEOUTS: i32 = 3;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Rule {
    Missed(String),
    Timeout(String),
}

impl fmt::Display for Rule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missed(mutant) => write!(f, "mutants:missed:{mutant}"),
            Self::Timeout(mutant) => write!(f, "mutants:timeout:{mutant}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Outcome {
    pub(crate) file: String,
    pub(crate) line: usize,
    pub(crate) mutant: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Survivor {
    pub(crate) file: String,
    pub(crate) mutant: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Scope {
    Everything,
    InDiff(PathBuf),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum IgnoreFiles {
    Respected,
    Disregarded,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct Outcomes {
    pub(crate) missed: String,
    pub(crate) timeouts: String,
}

fn outcome(line: &str) -> Option<Outcome> {
    let (location, mutant) = line.split_once(": ")?;
    let mut parts = location.rsplitn(3, ':');
    let column = parts.next()?;
    let number: usize = parts.next()?.parse().ok()?;
    let file = parts.next()?;
    column.parse::<usize>().ok().map(|_| Outcome {
        file: String::from(file),
        line: number,
        mutant: String::from(mutant.trim()),
    })
}

pub(crate) fn parse_outcomes(text: &str) -> Result<Vec<Outcome>, String> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|line| {
            outcome(line).ok_or_else(|| format!("cargo-mutants 결과를 읽지 못했다: {line}"))
        })
        .collect()
}

pub(crate) fn load_survivors(root: &Path, file: &str) -> Result<Vec<Survivor>, String> {
    let table: toml::Table = read_text(&root.join(file))?
        .parse()
        .map_err(|error| format!("{file}: {error}"))?;
    let entries = table
        .get("survivor")
        .and_then(toml::Value::as_array)
        .ok_or_else(|| format!("{file}: survivor 목록이 없다"))?;
    entries
        .iter()
        .map(|entry| {
            let field = |key: &str| {
                entry
                    .get(key)
                    .and_then(toml::Value::as_str)
                    .filter(|text| !text.trim().is_empty())
                    .map(String::from)
                    .ok_or_else(|| format!("{file}: 항목마다 file, mutant, reason이 있어야 한다"))
            };
            let survivor = Survivor {
                file: field("file")?,
                mutant: field("mutant")?,
            };
            field("reason")?;
            if root.join(&survivor.file).is_file() {
                Ok(survivor)
            } else {
                Err(format!("{file}: {}가 없다", survivor.file))
            }
        })
        .collect()
}

pub(crate) fn evaluate(
    outcomes: &Outcomes,
    survivors: &[Survivor],
) -> Result<BTreeSet<Finding<Rule>>, String> {
    let allowed: BTreeSet<(&str, &str)> = survivors
        .iter()
        .map(|survivor| (survivor.file.as_str(), survivor.mutant.as_str()))
        .collect();
    let mut findings = BTreeSet::new();
    for missed in parse_outcomes(&outcomes.missed)? {
        if !allowed.contains(&(missed.file.as_str(), missed.mutant.as_str())) {
            findings.insert(Finding::at_line(
                &missed.file,
                missed.line,
                Rule::Missed(missed.mutant.clone()),
            ));
        }
    }
    for timed_out in parse_outcomes(&outcomes.timeouts)? {
        findings.insert(Finding::at_line(
            &timed_out.file,
            timed_out.line,
            Rule::Timeout(timed_out.mutant.clone()),
        ));
    }
    Ok(findings)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Written {
    Always,
    WhenTested,
}

fn read_outcome(directory: &Path, name: &str, written: Written) -> Result<String, String> {
    let path = directory.join(name);
    if path.exists() || written == Written::Always {
        read_text(&path)
    } else {
        Ok(String::new())
    }
}

pub(crate) fn run(
    workspace: &Path,
    scope: &Scope,
    ignore: IgnoreFiles,
) -> Result<Outcomes, String> {
    let output = workspace.join(OUTPUT_DIRECTORY);
    fs::create_dir_all(&output).map_err(|error| format!("{}: {error}", output.display()))?;
    let results = output.join("mutants.out");
    if results.exists() {
        fs::remove_dir_all(&results).map_err(|error| format!("{}: {error}", results.display()))?;
    }
    let mut command = Command::new("cargo");
    command
        .current_dir(workspace)
        .args([
            "mutants",
            "--cap-lints=true",
            "--test-tool=nextest",
            "--no-shuffle",
            "--colors=never",
        ])
        .arg("--dir")
        .arg(workspace)
        .arg("--output")
        .arg(&output)
        .stdin(Stdio::null());
    if ignore == IgnoreFiles::Disregarded {
        command.arg("--gitignore=false");
    }
    if let Scope::InDiff(diff) = scope {
        command.arg("--in-diff").arg(diff);
    }
    let status = command
        .status()
        .map_err(|error| format!("cargo mutants: {error}"))?;
    let written = match status.code() {
        Some(PASSED) => Written::WhenTested,
        Some(FOUND_SURVIVORS | FOUND_TIMEOUTS) => Written::Always,
        Some(_) | None => return Err(format!("cargo mutants가 실패했다({status})")),
    };
    Ok(Outcomes {
        missed: read_outcome(&results, MISSED_FILE, written)?,
        timeouts: read_outcome(&results, TIMEOUT_FILE, written)?,
    })
}

fn scope_for(root: &Path, arguments: &[String]) -> Result<Scope, String> {
    match arguments {
        [] => Ok(Scope::Everything),
        [flag, base] if flag == "--in-diff" => {
            let commit = resolve_base(root, base)?;
            let diff = root.join(DIFF_FILE);
            if let Some(parent) = diff.parent() {
                fs::create_dir_all(parent)
                    .map_err(|error| format!("{}: {error}", parent.display()))?;
            }
            fs::write(&diff, working_tree_diff(root, &commit)?)
                .map_err(|error| format!("{}: {error}", diff.display()))?;
            Ok(Scope::InDiff(diff))
        }
        _ => Err(String::from(
            "사용법: xtask mutants [--in-diff <HEAD|커밋 ID>]",
        )),
    }
}

pub(crate) fn command(arguments: &[String]) -> ExitCode {
    report(
        "mutants",
        workspace_root().and_then(|root| {
            let survivors = load_survivors(&root, SURVIVORS_FILE)?;
            let scope = scope_for(&root, arguments)?;
            let outcomes = run(&root, &scope, IgnoreFiles::Respected)?;
            evaluate(&outcomes, &survivors)
        }),
    )
}
