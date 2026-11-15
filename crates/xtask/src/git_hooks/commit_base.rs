mod tree_base;

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use crate::git_base;

pub(crate) const BASE_BRANCH: &str = "refs/heads/master";

fn first_line(root: &Path, arguments: &[&str], label: &str) -> Result<String, String> {
    let mut command = git_base::git(root);
    command.args(arguments);
    let text = String::from_utf8_lossy(&git_base::output(command, label)?).into_owned();
    text.lines()
        .next()
        .map(|line| String::from(line.trim()))
        .filter(|line| !line.is_empty())
        .ok_or_else(|| format!("{label}: 출력이 없다"))
}

pub(crate) fn commit_base(root: &Path) -> Result<String, String> {
    let branch = format!("{BASE_BRANCH}^{{commit}}");
    first_line(
        root,
        &["rev-parse", "--verify", "--quiet", &branch],
        "rev-parse master",
    )
    .map_err(|failure| {
        format!(
            "기준 브랜치 master가 없다. 커밋 훅은 master와의 분기점을 기준으로 검사한다 ({failure})"
        )
    })?;
    if let MergeBase::IncomingDefault(base) = merge_base_kind(root)? {
        return Ok(base);
    }
    first_line(
        root,
        &["merge-base", "HEAD", BASE_BRANCH],
        "merge-base HEAD master",
    )
}

fn toplevel() -> Result<PathBuf, String> {
    first_line(
        Path::new("."),
        &["rev-parse", "--show-toplevel"],
        "rev-parse --show-toplevel",
    )
    .map(PathBuf::from)
}

pub(crate) fn command() -> ExitCode {
    let written = match toplevel().and_then(|root| commit_base(&root)) {
        Ok(base) => writeln!(io::stdout().lock(), "{base}").map(|()| ExitCode::SUCCESS),
        Err(failure) => {
            writeln!(io::stderr().lock(), "commit-base: {failure}").map(|()| ExitCode::FAILURE)
        }
    };
    match written {
        Ok(code) => code,
        Err(_) => ExitCode::FAILURE,
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum MergeBase {
    ForkPoint,
    IncomingDefault(String),
}

pub(crate) fn merge_base_kind(root: &Path) -> Result<MergeBase, String> {
    let merge_path = first_line(
        root,
        &[
            "rev-parse",
            "--path-format=absolute",
            "--git-path",
            "MERGE_HEAD",
        ],
        "MERGE_HEAD 경로",
    )?;
    let text = match fs::read_to_string(merge_path) {
        Ok(text) => text,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return automatic_base(root);
        }
        Err(error) => return Err(format!("MERGE_HEAD: {error}")),
    };
    let heads: Vec<&str> = text.lines().collect();
    let [incoming] = heads.as_slice() else {
        return Ok(MergeBase::ForkPoint);
    };
    if !git_base::is_commit_id(incoming) {
        return Ok(MergeBase::ForkPoint);
    }
    if work_branch(root)? != WorkBranch::NamedWork {
        return Ok(MergeBase::ForkPoint);
    }
    let tip = first_line(root, &["rev-parse", "--verify", BASE_BRANCH], "master 끝")?;
    if *incoming == tip {
        Ok(MergeBase::IncomingDefault(tip))
    } else {
        Ok(MergeBase::ForkPoint)
    }
}

fn automatic_base(root: &Path) -> Result<MergeBase, String> {
    if work_branch(root)? != WorkBranch::NamedWork {
        return Ok(MergeBase::ForkPoint);
    }
    let tip = first_line(root, &["rev-parse", "--verify", BASE_BRANCH], "master 끝")?;
    match tree_base::tree_match(root, &tip)? {
        tree_base::TreeMatch::Equal => Ok(MergeBase::IncomingDefault(tip)),
        tree_base::TreeMatch::Different => Ok(MergeBase::ForkPoint),
        tree_base::TreeMatch::Unavailable => {
            writeln!(io::stderr().lock(),
                "commit-base: 깨끗한 병합 트리를 확인할 수 없어 기존 분기점을 사용한다 (Git 2.38 이상 필요)"
            ).map_err(|error| format!("commit-base 진단 출력: {error}"))?;
            Ok(MergeBase::ForkPoint)
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum WorkBranch {
    NamedWork,
    Other,
}

pub(crate) fn work_branch(root: &Path) -> Result<WorkBranch, String> {
    let mut symbolic = git_base::git(root);
    symbolic.args(["symbolic-ref", "--quiet", "HEAD"]);
    let finished = symbolic
        .output()
        .map_err(|error| format!("symbolic-ref HEAD: {error}"))?;
    if finished.status.code() == Some(1_i32) {
        return Ok(WorkBranch::Other);
    }
    if !finished.status.success() {
        return Err(String::from("symbolic-ref HEAD 실패"));
    }
    let branch = String::from_utf8_lossy(&finished.stdout);
    if branch.trim().starts_with("refs/heads/") && !branch.trim().eq_ignore_ascii_case(BASE_BRANCH)
    {
        Ok(WorkBranch::NamedWork)
    } else {
        Ok(WorkBranch::Other)
    }
}

pub(crate) fn validate_command(base: &str, base_use: git_base::BaseUse) -> ExitCode {
    match toplevel().and_then(|root| git_base::resolve_base_for(&root, base, base_use)) {
        Ok(_) => ExitCode::SUCCESS,
        Err(failure) => {
            let written = writeln!(io::stderr().lock(), "require-base: {failure}")
                .map(|()| ExitCode::FAILURE);
            match written {
                Ok(code) => code,
                Err(_) => ExitCode::FAILURE,
            }
        }
    }
}

pub(crate) fn recipe_command() -> ExitCode {
    let selected = toplevel().and_then(|root| work_branch(&root));
    let recipe = match selected {
        Ok(WorkBranch::NamedWork) => "check-fast",
        Ok(WorkBranch::Other) | Err(_) => "check",
    };
    match writeln!(io::stdout().lock(), "{recipe}") {
        Ok(()) => ExitCode::SUCCESS,
        Err(_) => ExitCode::FAILURE,
    }
}
