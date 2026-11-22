mod catalog;
mod daily;
mod environment;
mod scope;
mod test_packages;
use scope::{diff_scope, jobs_for};
#[cfg(test)]
mod tests;
use std::collections::BTreeSet;
use std::fmt;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, ExitCode, Stdio};

use crate::check_support::{Finding, read_text, report, workspace_root};
use crate::git_base::changed_paths;
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
    Files(BTreeSet<String>),
    DiffSharded(BTreeSet<String>, String),
    DailyFiles(BTreeSet<String>),
    DailySharded(BTreeSet<String>, String),
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

fn partial_findings(
    directory: &Path,
    survivors: &[Survivor],
) -> Result<BTreeSet<Finding<Rule>>, String> {
    evaluate(
        &Outcomes {
            missed: read_outcome(directory, MISSED_FILE, Written::WhenTested)?,
            timeouts: read_outcome(directory, TIMEOUT_FILE, Written::WhenTested)?,
        },
        survivors,
    )
}

fn bounded_status(
    command: Command,
    workspace: &Path,
    results: &Path,
) -> Result<std::process::ExitStatus, String> {
    match environment::run_bounded(command) {
        Err(failure) if failure == environment::DEADLINE => {
            let survivors = load_survivors(workspace, SURVIVORS_FILE)?;
            let findings = partial_findings(results, &survivors)?;
            if findings.is_empty() {
                return Err(failure);
            }
            let details = findings
                .iter()
                .map(|finding| finding.rule.to_string())
                .collect::<Vec<_>>()
                .join("\n");
            Err(format!("변경분 변이 제한 전 발견:\n{details}"))
        }
        result => result,
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
    if let Scope::Files(files)
    | Scope::DiffSharded(files, _)
    | Scope::DailyFiles(files)
    | Scope::DailySharded(files, _) = scope
    {
        return files.iter().cloned().collect();
    }
    globs
        .iter()
        .filter(|_| match scope {
            Scope::Everything | Scope::Sharded(_) => true,
            Scope::Files(_)
            | Scope::DiffSharded(_, _)
            | Scope::DailyFiles(_)
            | Scope::DailySharded(_, _) => false,
        })
        .cloned()
        .collect()
}

fn workspace_support(path: &str) -> bool {
    [
        "Cargo.toml",
        "Cargo.lock",
        "rust-toolchain.toml",
        "justfile",
        "lefthook.yml",
    ]
    .contains(&path)
        || [
            ".cargo/",
            ".config/",
            "harness/",
            "crates/xtask/",
            ".github/",
        ]
        .iter()
        .any(|prefix| path.starts_with(prefix))
}

pub(crate) fn touched_crates(globs: &[String], changed: &BTreeSet<String>) -> BTreeSet<String> {
    globs
        .iter()
        .map(|glob| crate_of(glob))
        .filter(|directory| {
            let prefix = format!("{directory}/");
            changed
                .iter()
                .any(|path| path.starts_with(&prefix) || workspace_support(path))
        })
        .chain(test_packages::companion_sources(globs, changed))
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
    let multiplier = 5_usize;
    let test_threads = cores.checked_div(jobs).unwrap_or(1).clamp(1, 4);
    let mut files = BTreeSet::new();
    for directory in globs.iter().map(|glob| crate_of(glob)) {
        let path = workspace.join(directory).join("src");
        if path.exists() {
            selected_files(workspace, &path, globs, &mut files)?;
        }
    }
    writeln!(
        std::io::stdout(),
        "mutants: jobs={jobs}, test-threads={test_threads}, timeout-multiplier={multiplier}, files={}",
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
        .arg(multiplier.to_string())
        .arg(format!("--cargo-test-arg=--test-threads={test_threads}"));
    Ok(())
}

pub(crate) fn run(
    workspace: &Path,
    scope: &Scope,
    ignore: IgnoreFiles,
) -> Result<Outcomes, String> {
    run_with_diff(workspace, scope, ignore, None)
}

fn configure_profile(command: &mut Command) {
    command
        .env_remove("STOCK_COPILOT_PROPTEST_CASES")
        .env("CARGO_PROFILE_DEV_DEBUG", "0")
        .env("CARGO_PROFILE_TEST_DEBUG", "0")
        .env("CARGO_PROFILE_DEV_OPT_LEVEL", "1")
        .env("CARGO_PROFILE_TEST_OPT_LEVEL", "1")
        .env("CARGO_PROFILE_DEV_DEBUG_ASSERTIONS", "true")
        .env("CARGO_PROFILE_TEST_DEBUG_ASSERTIONS", "true")
        .env("CARGO_PROFILE_DEV_OVERFLOW_CHECKS", "true")
        .env("CARGO_PROFILE_TEST_OVERFLOW_CHECKS", "true");
}

fn prepare_output(
    root: &Path,
    scope: &Scope,
    ignore: IgnoreFiles,
    globs: &[String],
) -> Result<std::path::PathBuf, String> {
    let output = root.join(OUTPUT_DIRECTORY);
    fs::create_dir_all(&output).map_err(|error| error.to_string())?;
    let results = output.join("mutants.out");
    if results.exists() {
        fs::remove_dir_all(results).map_err(|error| error.to_string())?;
    }
    match scope {
        Scope::Files(_)
        | Scope::DiffSharded(_, _)
        | Scope::DailyFiles(_)
        | Scope::DailySharded(_, _) => {}
        Scope::Everything | Scope::Sharded(_) => catalog::prepare(root, globs, ignore, &output)?,
    }
    Ok(output)
}

fn configure_selection(
    command: &mut Command,
    scope: &Scope,
    ignore: IgnoreFiles,
    globs: &[String],
    patch: Option<&Path>,
) {
    if let Scope::Sharded(shard) | Scope::DiffSharded(_, shard) | Scope::DailySharded(_, shard) =
        scope
    {
        command.arg("--shard").arg(shard);
    }
    for glob in globs {
        command.arg("--file").arg(glob);
    }
    if ignore == IgnoreFiles::Disregarded {
        command.arg("--gitignore=false");
    }
    if let Some(file) = patch {
        command.arg("--in-diff").arg(file);
    }
}

fn run_with_diff(
    workspace: &Path,
    scope: &Scope,
    ignore: IgnoreFiles,
    patch: Option<&Path>,
) -> Result<Outcomes, String> {
    let globs = selected_globs(&target_globs(workspace)?, scope);
    if globs.is_empty() {
        return match scope {
            Scope::Files(_)
            | Scope::DiffSharded(_, _)
            | Scope::DailyFiles(_)
            | Scope::DailySharded(_, _) => Ok(Outcomes::default()),
            Scope::Everything | Scope::Sharded(_) => Err(String::from("밤 전체 변이 대상이 없다")),
        };
    }
    let output = prepare_output(workspace, scope, ignore, &globs)?;
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
            "--cargo-arg=--build-jobs=1",
        ])
        .arg("--dir")
        .arg(workspace)
        .arg("--output")
        .arg(&output)
        .stdin(Stdio::null());
    test_packages::configure(&mut command, &globs)?;
    configure_profile(&mut command);
    configure_parallelism(&mut command, workspace, &globs)?;
    configure_selection(&mut command, scope, ignore, &globs, patch);
    let results = output.join("mutants.out");
    let status = match scope {
        Scope::Files(_) | Scope::DiffSharded(_, _) => bounded_status(command, workspace, &results)?,
        Scope::Everything
        | Scope::Sharded(_)
        | Scope::DailyFiles(_)
        | Scope::DailySharded(_, _) => command.status().map_err(|error| error.to_string())?,
    };
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

fn scope_for(
    root: &Path,
    arguments: &[String],
    base_use: crate::git_base::BaseUse,
) -> Result<Scope, String> {
    match arguments {
        [] => Ok(Scope::Everything),
        [flag, shard]
            if flag == "--shard" && (0_u16..128).any(|index| shard == &format!("{index}/128")) =>
        {
            Ok(Scope::Sharded(shard.clone()))
        }
        [flag, base] if flag == "--in-diff" || flag == "--in-diff-plan" => {
            let commit = crate::git_base::resolve_base_for(root, base, base_use)?;
            let changed = changed_paths(root, &commit)?;
            Ok(diff_scope(root, &commit, &target_globs(root)?, &changed))
        }
        [flag, base, option, shard]
            if flag == "--in-diff"
                && option == "--shard"
                && [4_u16, 8, 16, 32, 64, 128]
                    .iter()
                    .any(|parts| (0..*parts).any(|index| shard == &format!("{index}/{parts}"))) =>
        {
            let commit = crate::git_base::resolve_base_for(root, base, base_use)?;
            let changed = changed_paths(root, &commit)?;
            let Scope::Files(files) = diff_scope(root, &commit, &target_globs(root)?, &changed)
            else {
                return Err(String::from("변경분 변이 파일 선택 실패"));
            };
            Ok(Scope::DiffSharded(files, shard.clone()))
        }
        _ => Err(String::from(
            "사용법: xtask mutants [--in-diff <HEAD|커밋 ID> | --shard <0/128..127/128> | --daily <커밋 ID|initial>]",
        )),
    }
}

fn checked_scope(
    root: &Path,
    arguments: &[String],
    base_use: crate::git_base::BaseUse,
) -> Result<Scope, String> {
    let scope = scope_for(root, arguments, base_use)?;
    if let Some([flag, base, ..]) = arguments.get(..)
        && (flag == "--in-diff" || flag == "--in-diff-plan")
    {
        let findings = crate::test_diff::check_for(root, base, base_use)?;
        if !findings.is_empty() {
            let details = findings
                .iter()
                .map(|finding| finding.rule.to_string())
                .collect::<Vec<_>>()
                .join("\n");
            return Err(format!("test-diff 실패: {details}"));
        }
    }
    Ok(scope)
}

fn list_changed_mutants(
    root: &Path,
    scope: &Scope,
    patch: &Path,
) -> Result<BTreeSet<String>, String> {
    let globs = selected_globs(&target_globs(root)?, scope);
    if globs.is_empty() {
        daily::catalog(root, b"[]")?;
        return Ok(BTreeSet::new());
    }
    let mut command = Command::new("cargo");
    command
        .current_dir(root)
        .args([
            "mutants",
            "--no-config",
            "--no-shuffle",
            "--list",
            "--json",
            "--in-diff",
        ])
        .arg(patch);
    for file in globs {
        command.arg("--file").arg(file);
    }
    let output = command.output().map_err(|error| error.to_string())?;
    if !output.status.success() {
        return Err(format!(
            "변이 열거 실패: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    if output.stdout.is_empty()
        && String::from_utf8_lossy(&output.stderr).contains("No mutants to filter")
    {
        daily::catalog(root, b"[]")?;
        writeln!(std::io::stdout().lock(), "mutants: 선택 변이 0개")
            .map_err(|error| error.to_string())?;
        return Ok(BTreeSet::new());
    }
    let parsed: serde_json::Value =
        serde_json::from_slice(&output.stdout).map_err(|error| error.to_string())?;
    let entries = parsed.as_array().ok_or("변이 열거 목록이 아니다")?;
    daily::catalog(root, &output.stdout)?;
    writeln!(
        std::io::stdout().lock(),
        "mutants: 선택 변이 {}개",
        entries.len()
    )
    .map_err(|error| error.to_string())?;
    entries
        .iter()
        .map(|entry| {
            entry
                .get("file")
                .and_then(serde_json::Value::as_str)
                .map(crate_of)
                .ok_or_else(|| String::from("변이 열거에 file이 없다"))
        })
        .collect()
}

fn report_nightly_targets(
    root: &Path,
    base: &str,
    mutated: &BTreeSet<String>,
) -> Result<(), String> {
    let changed = changed_paths(root, base)?;
    let globs = target_globs(root)?;
    for directory in touched_crates(&globs, &changed).difference(mutated) {
        writeln!(
            std::io::stdout().lock(),
            "mutants: {directory}는 주간 전체 실행 대상"
        )
        .map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn diff_patch(root: &Path, base: &str) -> Result<std::path::PathBuf, String> {
    let mut command = crate::git_base::git(root);
    command.args([
        "diff",
        "--no-ext-diff",
        "--no-textconv",
        "--no-renames",
        "--unified=0",
        base,
        "--",
    ]);
    let mut patch = crate::git_base::output(command, "diff")?;
    for file in crate::git_base::untracked_paths(root)? {
        if Path::new(&file).extension().is_some_and(|ext| ext == "rs") {
            let mut added = crate::git_base::git(root);
            added.args([
                "diff",
                "--no-index",
                "--no-ext-diff",
                "--no-textconv",
                "--unified=0",
                "--",
                "/dev/null",
                &file,
            ]);
            let result = added.output().map_err(|error| error.to_string())?;
            if result.status.code() != Some(1_i32) {
                return Err(format!("새 파일 diff 실패: {file}"));
            }
            patch.extend(result.stdout);
        }
    }
    let directory = root.join(OUTPUT_DIRECTORY);
    fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
    let destination = directory.join("changes.diff");
    fs::write(&destination, patch).map_err(|error| error.to_string())?;
    Ok(destination)
}

pub(crate) fn command(arguments: &[String], base_use: crate::git_base::BaseUse) -> ExitCode {
    let result = workspace_root().and_then(|root| {
        if arguments
            .first()
            .is_some_and(|flag| flag.starts_with("--daily"))
        {
            return daily::dispatch(&root, arguments, base_use);
        }
        let survivors = load_survivors(&root, SURVIVORS_FILE)?;
        let scope = checked_scope(&root, arguments, base_use)?;
        let outcomes = if let Some([flag, base, ..]) = arguments.get(..)
            && (flag == "--in-diff" || flag == "--in-diff-plan")
        {
            let patch = diff_patch(&root, base)?;
            let mutated = list_changed_mutants(&root, &scope, &patch)?;
            report_nightly_targets(&root, base, &mutated)?;
            if flag == "--in-diff-plan" || mutated.is_empty() {
                Outcomes::default()
            } else {
                run_with_diff(&root, &scope, IgnoreFiles::Respected, Some(&patch))?
            }
        } else {
            run(&root, &scope, IgnoreFiles::Respected)?
        };
        evaluate(&outcomes, &survivors)
    });
    if result
        .as_ref()
        .is_err_and(|failure| failure == environment::DEADLINE)
    {
        let _reported = report("mutants", result);
        ExitCode::from(124)
    } else {
        report("mutants", result)
    }
}
