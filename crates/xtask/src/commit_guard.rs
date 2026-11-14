use std::collections::BTreeSet;
use std::fmt;
use std::fs;
use std::io::{self, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};
use std::thread;

use crate::git_base::{self, git, output};

const SUBMODULE_MODE: &str = "160000";
const SYMLINK_MODE: &str = "120000";
const EXECUTABLE_MODE: &str = "100755";
const EXECUTABLE_BITS: u32 = 0o111;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum GuardFinding {
    Flagged(String),
    Changed(String),
    Missing(String),
    Unmerged(String),
    Submodule(String),
    Untracked(String),
}

impl fmt::Display for GuardFinding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Flagged(path) => write!(
                f,
                "commit-guard:flagged: {path}에 assume-unchanged 또는 skip-worktree 표시가 있다. 커밋 훅은 작업 사본을 검사하므로 표시를 지운다"
            ),
            Self::Changed(path) => write!(
                f,
                "commit-guard:changed: {path}의 작업 사본 내용이 커밋할 인덱스 내용과 다르다. 스테이징하거나 되돌린다"
            ),
            Self::Missing(path) => {
                write!(
                    f,
                    "commit-guard:missing: 인덱스의 {path}가 작업 사본에 없다"
                )
            }
            Self::Unmerged(path) => write!(f, "commit-guard:unmerged: {path}의 충돌이 남아 있다"),
            Self::Submodule(path) => {
                write!(
                    f,
                    "commit-guard:submodule: {path}는 서브모듈이라 검사할 수 없다"
                )
            }
            Self::Untracked(path) => write!(
                f,
                "commit-guard:untracked: {path}는 추적하지 않는 파일이다(루트 .gitignore만 무시 규칙으로 쓴다). 커밋하거나 지운다"
            ),
        }
    }
}

fn indexed(root: &Path, index: Option<&Path>) -> Command {
    let mut command = git(root);
    if let Some(file) = index {
        command.env("GIT_INDEX_FILE", file);
    }
    command
}

fn entries(listing: &[u8]) -> Vec<String> {
    listing
        .split(|byte| *byte == 0)
        .filter(|entry| !entry.is_empty())
        .map(|entry| String::from_utf8_lossy(entry).into_owned())
        .collect()
}

#[derive(Debug)]
struct Staged {
    mode: String,
    object: String,
    stage: String,
    path: String,
}

fn staged(entry: &str) -> Option<Staged> {
    let (meta, path) = entry.split_once('\t')?;
    let mut fields = meta.split(' ');
    Some(Staged {
        mode: String::from(fields.next()?),
        object: String::from(fields.next()?),
        stage: String::from(fields.next()?),
        path: String::from(path),
    })
}

pub(crate) fn hashed(
    root: &Path,
    mut command: Command,
    input: &[u8],
) -> Result<Vec<String>, String> {
    let mut child = command
        .current_dir(root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("git hash-object: {error}"))?;
    let pipe = child.stdin.take();
    let (written, waited) = thread::scope(|scope| {
        let writer = scope.spawn(move || match pipe {
            Some(mut stdin) => stdin.write_all(input),
            None => Ok(()),
        });
        let waited = child.wait_with_output();
        (writer.join(), waited)
    });
    let finished = waited.map_err(|error| format!("git hash-object: {error}"))?;
    if !finished.status.success() {
        return Err(format!(
            "git hash-object: {}",
            String::from_utf8_lossy(&finished.stderr).trim()
        ));
    }
    match written {
        Ok(Ok(())) => {}
        Ok(Err(error)) => return Err(format!("git hash-object: {error}")),
        Err(_) => return Err(String::from("git hash-object: 입력 쓰기 스레드가 실패했다")),
    }
    Ok(String::from_utf8_lossy(&finished.stdout)
        .lines()
        .map(String::from)
        .collect())
}

fn link_hash(root: &Path, path: &Path) -> Result<Option<String>, String> {
    let Ok(target) = fs::read_link(path) else {
        return Ok(None);
    };
    let mut command = git(root);
    command.args(["hash-object", "--stdin"]);
    let found = hashed(root, command, target.as_os_str().as_encoded_bytes())?;
    Ok(found.into_iter().next())
}

fn content_findings(
    root: &Path,
    listed: &[Staged],
    findings: &mut BTreeSet<GuardFinding>,
) -> Result<(), String> {
    let mut regular: Vec<&Staged> = Vec::new();
    for entry in listed {
        let full = root.join(&entry.path);
        let metadata = fs::symlink_metadata(&full);
        let Ok(found) = metadata else {
            findings.insert(GuardFinding::Missing(entry.path.clone()));
            continue;
        };
        if entry.mode == SYMLINK_MODE {
            if link_hash(root, &full)?.as_deref() != Some(entry.object.as_str()) {
                findings.insert(GuardFinding::Changed(entry.path.clone()));
            }
        } else if !found.is_file() || entry.path.contains('\n') {
            findings.insert(GuardFinding::Changed(entry.path.clone()));
        } else {
            let executable = found.permissions().mode() & EXECUTABLE_BITS != 0;
            if executable != (entry.mode == EXECUTABLE_MODE) {
                findings.insert(GuardFinding::Changed(entry.path.clone()));
            }
            regular.push(entry);
        }
    }
    let mut input = String::new();
    for entry in &regular {
        input.push_str(&entry.path);
        input.push('\n');
    }
    let mut command = git(root);
    command.args(["hash-object", "--no-filters", "--stdin-paths"]);
    let hashes = if regular.is_empty() {
        Vec::new()
    } else {
        hashed(root, command, input.as_bytes())?
    };
    if hashes.len() != regular.len() {
        return Err(String::from("git hash-object: 해시 수가 파일 수와 다르다"));
    }
    for (entry, hash) in regular.iter().zip(hashes) {
        if hash != entry.object {
            findings.insert(GuardFinding::Changed(entry.path.clone()));
        }
    }
    Ok(())
}

pub(crate) fn guard_findings(
    root: &Path,
    index: Option<&Path>,
) -> Result<BTreeSet<GuardFinding>, String> {
    let mut findings = BTreeSet::new();
    let mut flags = indexed(root, index);
    flags.args(["ls-files", "-v", "-z"]);
    for entry in entries(&output(flags, "ls-files -v")?) {
        let Some((tag, path)) = entry.split_once(' ') else {
            continue;
        };
        if tag
            .chars()
            .any(|next| next.is_ascii_lowercase() || next == 'S')
        {
            findings.insert(GuardFinding::Flagged(String::from(path)));
        }
    }
    let mut stages = indexed(root, index);
    stages.args(["ls-files", "-s", "-z"]);
    let listed: Vec<Staged> = entries(&output(stages, "ls-files -s")?)
        .iter()
        .filter_map(|entry| staged(entry))
        .collect();
    let mut checked = Vec::new();
    for entry in listed {
        if entry.stage != "0" {
            findings.insert(GuardFinding::Unmerged(entry.path));
        } else if entry.mode == SUBMODULE_MODE {
            findings.insert(GuardFinding::Submodule(entry.path));
        } else {
            checked.push(entry);
        }
    }
    content_findings(root, &checked, &mut findings)?;
    let mut untracked = git_base::untracked_command(root)?;
    if let Some(file) = index {
        untracked.env("GIT_INDEX_FILE", file);
    }
    for path in entries(&output(untracked, "ls-files --others")?) {
        findings.insert(GuardFinding::Untracked(path));
    }
    Ok(findings)
}

fn toplevel() -> Result<PathBuf, String> {
    let mut command = git(Path::new("."));
    command.args(["rev-parse", "--show-toplevel"]);
    Ok(PathBuf::from(
        String::from_utf8_lossy(&output(command, "rev-parse --show-toplevel")?).trim(),
    ))
}

fn run(arguments: &[String]) -> Result<BTreeSet<GuardFinding>, String> {
    let root = toplevel()?;
    let index = match arguments {
        [] => None,
        [given] if given.is_empty() => None,
        [given] => Some(root.join(given)),
        [_, _, ..] => return Err(String::from("사용법: xtask commit-guard [인덱스 파일]")),
    };
    guard_findings(&root, index.as_deref())
}

fn write(outcome: Result<BTreeSet<GuardFinding>, String>) -> io::Result<ExitCode> {
    match outcome {
        Ok(findings) if findings.is_empty() => {
            writeln!(io::stdout().lock(), "commit-guard: 위반 없음")?;
            Ok(ExitCode::SUCCESS)
        }
        Ok(findings) => {
            let mut output = io::stderr().lock();
            for finding in &findings {
                writeln!(output, "{finding}")?;
            }
            writeln!(output, "commit-guard: 위반 {}건", findings.len())?;
            Ok(ExitCode::FAILURE)
        }
        Err(failure) => {
            writeln!(io::stderr().lock(), "commit-guard: {failure}")?;
            Ok(ExitCode::FAILURE)
        }
    }
}

pub(crate) fn command(arguments: &[String]) -> ExitCode {
    match write(run(arguments)) {
        Ok(code) => code,
        Err(_) => ExitCode::FAILURE,
    }
}
