mod addition_symbols;
#[cfg(test)]
mod addition_tests;
mod additions;
mod environment;
mod graph;
mod macro_tokens;
#[cfg(test)]
mod macros_tests;
mod scope;
use scope::{diff_scope, jobs_for};

#[cfg(test)]
mod scope_tests;

#[cfg(test)]
mod tests;

use std::collections::BTreeSet;
use std::fmt;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, ExitCode, Stdio};

use crate::check_support::{Finding, read_text, report, workspace_root};
use crate::git_base::{changed_paths, resolve_base};
use crate::source_check::cargo_config::examine_globs;

pub(crate) const SURVIVORS_FILE: &str = "harness/mutants/survivors.toml";
const OUTPUT_DIRECTORY: &str = "target/mutants";
const SETTINGS_FILE: &str = ".cargo/mutants.toml";
const CRATE_DEPTH: usize = 2;
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
    Sharded(String),
    Crates(BTreeSet<String>),
    Files(BTreeSet<String>),
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

pub(crate) fn crate_of(glob: &str) -> String {
    glob.split('/')
        .take(CRATE_DEPTH)
        .collect::<Vec<_>>()
        .join("/")
}

pub(crate) fn target_globs(workspace: &Path) -> Result<Vec<String>, String> {
    examine_globs(&read_text(&workspace.join(SETTINGS_FILE))?)
        .ok_or_else(|| format!("{SETTINGS_FILE}: examine_globs가 없다"))
}

pub(crate) fn selected_globs(globs: &[String], scope: &Scope) -> Vec<String> {
    if let Scope::Files(files) = scope {
        return files.iter().cloned().collect();
    }
    globs
        .iter()
        .filter(|glob| match scope {
            Scope::Everything | Scope::Sharded(_) => true,
            Scope::Files(_) => false,
            Scope::Crates(crates) => crates.contains(&crate_of(glob)),
        })
        .cloned()
        .collect()
}

pub(crate) fn touched_crates(globs: &[String], changed: &BTreeSet<String>) -> BTreeSet<String> {
    globs
        .iter()
        .map(|glob| crate_of(glob))
        .filter(|directory| {
            let prefix = format!("{directory}/");
            changed.iter().any(|path| path.starts_with(&prefix))
        })
        .collect()
}

fn selected_files(
    root: &Path,
    directory: &Path,
    globs: &[String],
    files: &mut BTreeSet<String>,
) -> Result<(), String> {
    for listed in fs::read_dir(directory).map_err(|error| error.to_string())? {
        let entry = listed.map_err(|error| error.to_string())?;
        let path = entry.path();
        let kind = entry.file_type().map_err(|error| error.to_string())?;
        if kind.is_dir() {
            selected_files(root, &path, globs, files)?;
        } else {
            let relative = path
                .strip_prefix(root)
                .map_err(|error| error.to_string())?
                .to_str()
                .ok_or("UTF-8 path")?;
            if globs
                .iter()
                .any(|glob| scope::matches_target(glob, relative))
            {
                files.insert(String::from(relative));
            }
        }
    }
    Ok(())
}

fn configure_parallelism(
    command: &mut Command,
    workspace: &Path,
    globs: &[String],
) -> Result<(), String> {
    let cores = std::thread::available_parallelism()
        .map_err(|error| error.to_string())?
        .get();
    let override_jobs = environment::jobs_override()?;
    let jobs = jobs_for(cores, override_jobs.as_deref())?;
    let multiplier = jobs.saturating_mul(5);
    let mut files = BTreeSet::new();
    for directory in globs.iter().map(|glob| crate_of(glob)) {
        let path = workspace.join(directory).join("src");
        if path.exists() {
            selected_files(workspace, &path, globs, &mut files)?;
        }
    }
    writeln!(
        std::io::stdout(),
        "mutants: jobs={jobs}, timeout-multiplier={multiplier}, files={}",
        files.len()
    )
    .map_err(|error| error.to_string())?;
    for file in files {
        writeln!(std::io::stdout(), "mutants: file={file}").map_err(|error| error.to_string())?;
    }
    command
        .arg("--jobs")
        .arg(jobs.to_string())
        .arg("--timeout-multiplier")
        .arg(multiplier.to_string());
    Ok(())
}

pub(crate) fn run(
    workspace: &Path,
    scope: &Scope,
    ignore: IgnoreFiles,
) -> Result<Outcomes, String> {
    let globs = selected_globs(&target_globs(workspace)?, scope);
    if globs.is_empty() {
        return Ok(Outcomes::default());
    }
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
            "--no-config",
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
    configure_parallelism(&mut command, workspace, &globs)?;
    if let Scope::Sharded(shard) = scope {
        command.arg("--shard").arg(shard);
    }
    for glob in &globs {
        command.arg("--file").arg(glob);
    }
    if ignore == IgnoreFiles::Disregarded {
        command.arg("--gitignore=false");
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
        [flag, shard]
            if flag == "--shard" && ["0/4", "1/4", "2/4", "3/4"].contains(&shard.as_str()) =>
        {
            Ok(Scope::Sharded(shard.clone()))
        }
        [flag, base] if flag == "--in-diff" => {
            let commit = resolve_base(root, base)?;
            let changed = changed_paths(root, &commit)?;
            Ok(diff_scope(root, &commit, &target_globs(root)?, &changed))
        }
        _ => Err(String::from(
            "사용법: xtask mutants [--in-diff <HEAD|커밋 ID> | --shard <0/4|1/4|2/4|3/4>]",
        )),
    }
}

fn checked_scope(root: &Path, arguments: &[String]) -> Result<Scope, String> {
    let [flag, base] = arguments else {
        return scope_for(root, arguments);
    };
    if flag != "--in-diff" {
        return scope_for(root, arguments);
    }
    let commit = resolve_base(root, base)?;
    let findings = crate::test_diff::check(root, &commit)?;
    if !findings.is_empty() {
        return Err(format!(
            "추가 전용 범위의 전제인 test-diff 실패:\n{}",
            findings
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n")
        ));
    }
    scope_for(root, &[flag.clone(), commit])
}

pub(crate) fn command(arguments: &[String]) -> ExitCode {
    report(
        "mutants",
        workspace_root().and_then(|root| {
            let survivors = load_survivors(&root, SURVIVORS_FILE)?;
            let scope = checked_scope(&root, arguments)?;
            let outcomes = run(&root, &scope, IgnoreFiles::Respected)?;
            evaluate(&outcomes, &survivors)
        }),
    )
}
