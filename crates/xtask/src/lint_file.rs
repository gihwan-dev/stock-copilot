pub(crate) mod targets;

#[cfg(test)]
mod tests;

use std::fmt;
use std::fs;
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};

use crate::disable_control;
use crate::raw_types::{self, Replacement};
use crate::source_check;
use crate::sql_check;
use crate::workspace_sources::{Member, Members, load_members, relative_text};
use targets::{Coverage, Targets, under};

const RUSTFMT_CONFIG: &str = "rustfmt.toml";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Linter {
    Rustfmt,
    SourceCheck,
    RawTypes,
    DisableCheck,
    SqlCheck,
    Prettier,
    Eslint,
}

impl Linter {
    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::Rustfmt => "rustfmt",
            Self::SourceCheck => "source-check",
            Self::RawTypes => "raw-types",
            Self::DisableCheck => "disable-check",
            Self::SqlCheck => "sql-check",
            Self::Prettier => "prettier",
            Self::Eslint => "eslint",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Outcome {
    Passed,
    Failed(Vec<String>),
    Skipped(&'static str),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CheckResult {
    pub(crate) check: Linter,
    pub(crate) outcome: Outcome,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PathProblem {
    Missing,
    Directory,
    Symlink,
    Outside,
    NotRegular,
    Unreadable(String),
}

impl fmt::Display for PathProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing => f.write_str("없는 파일이다"),
            Self::Directory => f.write_str("폴더다. 파일 하나를 준다"),
            Self::Symlink => f.write_str("심볼릭 링크이거나 심볼릭 링크를 거친다"),
            Self::Outside => f.write_str("저장소 밖의 경로다"),
            Self::NotRegular => f.write_str("일반 파일이 아니다"),
            Self::Unreadable(error) => write!(f, "경로를 읽지 못했다 ({error})"),
        }
    }
}

#[derive(Debug)]
pub(crate) struct Context {
    pub(crate) members: Members,
    pub(crate) targets: Targets,
}

fn normalized(path: &Path) -> PathBuf {
    let mut result = PathBuf::new();
    for component in path.components() {
        match component {
            Component::ParentDir => {
                result.pop();
            }
            Component::CurDir => {}
            Component::Prefix(_) | Component::RootDir | Component::Normal(_) => {
                result.push(component);
            }
        }
    }
    result
}

fn relative_under(path: &Path, roots: &[&Path]) -> Option<String> {
    roots.iter().find_map(|root| {
        path.strip_prefix(root)
            .ok()
            .filter(|inside| inside.components().next().is_some())
            .map(|inside| relative_text(Path::new(""), inside))
    })
}

fn unreadable(error: &io::Error) -> PathProblem {
    if error.kind() == io::ErrorKind::NotFound {
        PathProblem::Missing
    } else {
        PathProblem::Unreadable(error.to_string())
    }
}

pub(crate) fn resolve(root: &Path, given: &Path) -> Result<String, PathProblem> {
    let absolute = std::path::absolute(given).map_err(|error| unreadable(&error))?;
    let lexical = normalized(&absolute);
    let canonical_root = root.canonicalize().map_err(|error| unreadable(&error))?;
    let relative =
        relative_under(&lexical, &[root, &canonical_root]).ok_or(PathProblem::Outside)?;
    let metadata = fs::symlink_metadata(&lexical).map_err(|error| unreadable(&error))?;
    if metadata.file_type().is_symlink() {
        return Err(PathProblem::Symlink);
    }
    if metadata.is_dir() {
        return Err(PathProblem::Directory);
    }
    if !metadata.is_file() {
        return Err(PathProblem::NotRegular);
    }
    let canonical = lexical.canonicalize().map_err(|error| unreadable(&error))?;
    if canonical != canonical_root.join(&relative) {
        return Err(PathProblem::Symlink);
    }
    Ok(relative)
}

fn extension_is(path: &str, wanted: &str) -> bool {
    Path::new(path)
        .extension()
        .is_some_and(|extension| extension == wanted)
}

impl Context {
    pub(crate) fn load(root: Option<&Path>) -> Result<Self, String> {
        let manifest = root.map(|directory| directory.join("Cargo.toml"));
        let members = load_members(manifest.as_deref()).map_err(|failure| failure.0)?;
        let targets = Targets::load(&members.root).map_err(|failure| failure.0)?;
        Ok(Self { members, targets })
    }

    fn root(&self) -> &Path {
        &self.members.root
    }

    fn member_of(&self, path: &str) -> Option<&Member> {
        let target = relative_text(self.root(), &self.members.target_directory);
        if under(path, &target) {
            return None;
        }
        self.members
            .packages
            .iter()
            .filter(|member| {
                let directory = relative_text(self.root(), &member.directory);
                directory.is_empty() || under(path, &directory)
            })
            .max_by_key(|member| member.directory.components().count())
    }
}

fn run_with_input(mut command: Command, input: &str, label: &str) -> Result<Finished, String> {
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("{label}: {error}"))?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(input.as_bytes())
            .map_err(|error| format!("{label}: {error}"))?;
    }
    let finished = child
        .wait_with_output()
        .map_err(|error| format!("{label}: {error}"))?;
    Ok(Finished {
        success: if finished.status.success() {
            Status::Success
        } else {
            Status::Failure(finished.status.code())
        },
        stdout: String::from_utf8_lossy(&finished.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&finished.stderr).into_owned(),
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Status {
    Success,
    Failure(Option<i32>),
}

#[derive(Debug)]
struct Finished {
    success: Status,
    stdout: String,
    stderr: String,
}

fn message_lines(text: &str) -> Vec<String> {
    text.lines()
        .map(str::trim_end)
        .filter(|line| !line.trim().is_empty())
        .map(String::from)
        .collect()
}

fn rustfmt(root: &Path, text: &str) -> Result<Outcome, String> {
    let mut command = Command::new("rustfmt");
    command
        .current_dir(root)
        .args(["--edition", "2024", "--emit", "stdout", "--config-path"])
        .arg(root.join(RUSTFMT_CONFIG));
    let finished = run_with_input(command, text, "rustfmt")?;
    Ok(match finished.success {
        Status::Success if finished.stdout == text => Outcome::Passed,
        Status::Success => Outcome::Failed(vec![String::from(
            "rustfmt 형식과 다르다 (cargo fmt로 고친다)",
        )]),
        Status::Failure(_) => Outcome::Failed(message_lines(&finished.stderr)),
    })
}

fn findings_outcome(findings: Vec<String>) -> Outcome {
    if findings.is_empty() {
        Outcome::Passed
    } else {
        Outcome::Failed(findings)
    }
}

fn rust_checks(context: &Context, path: &str, text: &str) -> Result<Vec<CheckResult>, String> {
    let mut results = Vec::new();
    let root = context.root();
    let member = context.member_of(path);
    let rust = extension_is(path, "rs");
    if let (Some(_), true) = (member, rust) {
        results.push(CheckResult {
            check: Linter::Rustfmt,
            outcome: rustfmt(root, text)?,
        });
    }
    if let Some(found) =
        source_check::file_findings(root, path, text).map_err(|failure| failure.to_string())?
    {
        results.push(CheckResult {
            check: Linter::SourceCheck,
            outcome: findings_outcome(found.iter().map(ToString::to_string).collect()),
        });
    }
    let Some(owner) = member else {
        return Ok(results);
    };
    if rust {
        let replacement = Replacement { label: path, text };
        let outcome = raw_types::check_file_text(&context.members, owner, replacement)
            .map_err(|failure| failure.0)?
            .map_or(
                Outcome::Skipped("raw-types 대상 crate가 아니거나 그 모듈 트리 밖"),
                |found| findings_outcome(found.iter().map(ToString::to_string).collect()),
            );
        results.push(CheckResult {
            check: Linter::RawTypes,
            outcome,
        });
        let policy = disable_control::Policy::load(root).map_err(|failure| failure.0)?;
        results.push(CheckResult {
            check: Linter::DisableCheck,
            outcome: findings_outcome(
                disable_control::check_source(path, text, &policy)
                    .iter()
                    .map(ToString::to_string)
                    .collect(),
            ),
        });
    }
    if rust || extension_is(path, "sql") {
        results.push(CheckResult {
            check: Linter::SqlCheck,
            outcome: findings_outcome(sql_check::text_findings(root, path, text)?),
        });
    }
    Ok(results)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PrettierInfo {
    Ignored,
    Unsupported,
    Supported,
}

fn compact(text: &str) -> String {
    text.chars().filter(|next| !next.is_whitespace()).collect()
}

fn prettier_info(context: &Context, path: &str) -> Result<PrettierInfo, String> {
    let root = context.root();
    let finished = Command::new(root.join(&context.targets.prettier_binary))
        .current_dir(root)
        .arg("--file-info")
        .arg(root.join(path))
        .stdin(Stdio::null())
        .output()
        .map_err(|error| format!("prettier --file-info: {error}"))?;
    let info = compact(&String::from_utf8_lossy(&finished.stdout));
    if !finished.status.success() {
        return Err(format!(
            "prettier --file-info: {}",
            String::from_utf8_lossy(&finished.stderr).trim()
        ));
    }
    if info.contains("\"ignored\":true") {
        Ok(PrettierInfo::Ignored)
    } else if info.contains("\"inferredParser\":null") {
        Ok(PrettierInfo::Unsupported)
    } else if info.contains("\"ignored\":false") {
        Ok(PrettierInfo::Supported)
    } else {
        Err(format!("prettier --file-info: 모르는 출력 {info}"))
    }
}

fn prettier(context: &Context, path: &str, text: &str) -> Result<Outcome, String> {
    match prettier_info(context, path)? {
        PrettierInfo::Ignored => return Ok(Outcome::Skipped(".prettierignore가 제외한 파일")),
        PrettierInfo::Unsupported => return Ok(Outcome::Skipped("prettier가 다루지 않는 형식")),
        PrettierInfo::Supported => {}
    }
    let root = context.root();
    let mut command = Command::new(root.join(&context.targets.prettier_binary));
    command
        .current_dir(root)
        .args(["--check", "--stdin-filepath"])
        .arg(path);
    let finished = run_with_input(command, text, "prettier")?;
    Ok(match finished.success {
        Status::Success => Outcome::Passed,
        Status::Failure(_) => {
            let mut lines = message_lines(&finished.stderr);
            lines.extend(message_lines(&finished.stdout));
            lines.push(String::from(
                "prettier 형식과 다르다 (prettier --write로 고친다)",
            ));
            Outcome::Failed(lines)
        }
    })
}

fn eslint(context: &Context, path: &str, text: &str) -> Result<Option<Outcome>, String> {
    let Some(target) = context.targets.eslint_for(path) else {
        return Ok(None);
    };
    let root = context.root();
    if context.targets.eslint_configured(root, target, path)? == Coverage::Unlinted {
        return Ok(Some(Outcome::Skipped("ESLint 설정이 다루지 않는 파일")));
    }
    let mut command = context.targets.eslint_command(root, target);
    command
        .args(["--max-warnings", "0", "--stdin", "--stdin-filename"])
        .arg(target.relative_to_directory(path));
    let finished = run_with_input(command, text, "eslint")?;
    Ok(Some(match finished.success {
        Status::Success => Outcome::Passed,
        Status::Failure(Some(1)) => Outcome::Failed(message_lines(&finished.stdout)),
        Status::Failure(_) => {
            return Err(format!(
                "eslint가 실행되지 않았다: {}{}",
                finished.stdout.trim(),
                finished.stderr.trim()
            ));
        }
    }))
}

pub(crate) fn lint_text(
    context: &Context,
    path: &str,
    text: &str,
) -> Result<Vec<CheckResult>, String> {
    let mut results = rust_checks(context, path, text)?;
    if context.targets.prettier_applies(path).is_some() {
        results.push(CheckResult {
            check: Linter::Prettier,
            outcome: prettier(context, path, text)?,
        });
    }
    if let Some(outcome) = eslint(context, path, text)? {
        results.push(CheckResult {
            check: Linter::Eslint,
            outcome,
        });
    }
    Ok(results)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Verdict {
    Passed,
    Failed,
    NothingApplied,
}

const fn failed(outcome: &Outcome) -> bool {
    match outcome {
        Outcome::Failed(_) => true,
        Outcome::Passed | Outcome::Skipped(_) => false,
    }
}

const fn skipped(outcome: &Outcome) -> bool {
    match outcome {
        Outcome::Skipped(_) => true,
        Outcome::Passed | Outcome::Failed(_) => false,
    }
}

pub(crate) fn verdict(results: &[CheckResult]) -> Verdict {
    if results.iter().any(|result| failed(&result.outcome)) {
        Verdict::Failed
    } else if results
        .iter()
        .any(|result| result.outcome == Outcome::Passed)
    {
        Verdict::Passed
    } else {
        Verdict::NothingApplied
    }
}

pub(crate) fn render(path: &str, results: &[CheckResult]) -> String {
    let mut lines = Vec::new();
    for result in results {
        match &result.outcome {
            Outcome::Passed => lines.push(format!("{}: 통과", result.check.name())),
            Outcome::Skipped(why) => {
                lines.push(format!("{}: 적용하지 않음 ({why})", result.check.name()));
            }
            Outcome::Failed(messages) => {
                lines.push(format!("{}: 위반", result.check.name()));
                lines.extend(messages.iter().map(|message| format!("  {message}")));
            }
        }
    }
    let applied: Vec<&str> = results
        .iter()
        .filter(|result| !skipped(&result.outcome))
        .map(|result| result.check.name())
        .collect();
    lines.push(match verdict(results) {
        Verdict::Passed => format!("lint-file: {path}: 통과 ({})", applied.join(", ")),
        Verdict::Failed => format!("lint-file: {path}: 위반 ({})", applied.join(", ")),
        Verdict::NothingApplied => format!("lint-file: {path}: 적용할 검사 없음"),
    });
    lines.join("\n")
}

fn lint(given: &str) -> Result<(String, Verdict), String> {
    let context = Context::load(None)?;
    let path = resolve(context.root(), Path::new(given))
        .map_err(|problem| format!("{given}: {problem}"))?;
    let full = context.root().join(&path);
    let text = fs::read_to_string(&full)
        .map_err(|error| format!("{path}: 텍스트 파일로 읽지 못했다 ({error})"))?;
    let results = lint_text(&context, &path, &text)?;
    Ok((render(&path, &results), verdict(&results)))
}

pub(crate) fn command(arguments: &[String]) -> ExitCode {
    let written = match arguments {
        [given] => match lint(given) {
            Ok((report, Verdict::Failed)) => {
                writeln!(io::stderr().lock(), "{report}").map(|()| ExitCode::FAILURE)
            }
            Ok((report, Verdict::Passed | Verdict::NothingApplied)) => {
                writeln!(io::stdout().lock(), "{report}").map(|()| ExitCode::SUCCESS)
            }
            Err(failure) => {
                writeln!(io::stderr().lock(), "lint-file: {failure}").map(|()| ExitCode::FAILURE)
            }
        },
        [] | [_, _, ..] => writeln!(
            io::stderr().lock(),
            "lint-file: 경로 하나가 필요하다: xtask lint-file <경로>"
        )
        .map(|()| ExitCode::FAILURE),
    };
    match written {
        Ok(code) => code,
        Err(_) => ExitCode::FAILURE,
    }
}
