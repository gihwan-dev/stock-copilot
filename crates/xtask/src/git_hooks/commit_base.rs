mod tree_base;

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use crate::git_base;

pub(crate) const BASE_BRANCH: &str = "refs/heads/master";
const REMOTE_BASE_BRANCH: &str = "refs/remotes/origin/master";

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

fn is_strict_ancestor(root: &Path, older: &str, newer: &str) -> bool {
    if older == newer {
        return false;
    }
    let mut command = git_base::git(root);
    command.args(["merge-base", "--is-ancestor", older, newer]);
    command.status().is_ok_and(|status| status.success())
}

fn effective_base(root: &Path) -> Result<(&'static str, String), String> {
    let branch = format!("{BASE_BRANCH}^{{commit}}");
    let local_tip = first_line(
        root,
        &["rev-parse", "--verify", "--quiet", &branch],
        "rev-parse master",
    )
    .map_err(|failure| {
        format!(
            "기준 브랜치 master가 없다. 커밋 훅은 master와의 분기점을 기준으로 검사한다 ({failure})"
        )
    })?;
    let remote = format!("{REMOTE_BASE_BRANCH}^{{commit}}");
    if let Ok(remote_tip) = first_line(
        root,
        &["rev-parse", "--verify", "--quiet", &remote],
        "rev-parse origin/master",
    ) && git_base::is_commit_id(&remote_tip)
        && is_strict_ancestor(root, &local_tip, &remote_tip)
    {
        return Ok((REMOTE_BASE_BRANCH, remote_tip));
    }
    Ok((BASE_BRANCH, local_tip))
}

pub(crate) fn commit_base(root: &Path) -> Result<String, String> {
    let (base_ref, _) = effective_base(root)?;
    if let MergeBase::IncomingDefault(base) = merge_base_kind(root)? {
        return Ok(base);
    }
    first_line(
        root,
        &["merge-base", "HEAD", base_ref],
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
    let (_, tip) = effective_base(root)?;
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
    let (_, tip) = effective_base(root)?;
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

#[derive(Clone, Copy)]
enum BranchPolicy {
    Comparison,
    Fast,
}

pub(crate) fn work_branch(root: &Path) -> Result<WorkBranch, String> {
    classified_branch(root, BranchPolicy::Comparison)
}

pub(crate) fn fast_branch(root: &Path) -> Result<WorkBranch, String> {
    classified_branch(root, BranchPolicy::Fast)
}

fn classified_branch(root: &Path, policy: BranchPolicy) -> Result<WorkBranch, String> {
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
    let known_name = match policy {
        BranchPolicy::Comparison => true,
        BranchPolicy::Fast => branch.trim().is_ascii(),
    };
    if known_name
        && branch.trim().starts_with("refs/heads/")
        && !branch.trim().eq_ignore_ascii_case(BASE_BRANCH)
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
    let selected = toplevel().and_then(|root| fast_branch(&root));
    let recipe = recipe_for(&selected);
    match writeln!(io::stdout().lock(), "{recipe}") {
        Ok(()) => ExitCode::SUCCESS,
        Err(_) => ExitCode::FAILURE,
    }
}

fn recipe_for(selected: &Result<WorkBranch, String>) -> &'static str {
    match selected {
        Ok(WorkBranch::NamedWork) => "check-fast",
        Ok(WorkBranch::Other) | Err(_) => "check",
    }
}

#[cfg(test)]
mod recipe_tests {
    #[test]
    fn errors_select_full_check() {
        assert_eq!(super::recipe_for(&Err(String::from("unknown"))), "check");
        assert_eq!(super::recipe_for(&Ok(super::WorkBranch::Other)), "check");
        assert_eq!(
            super::recipe_for(&Ok(super::WorkBranch::NamedWork)),
            "check-fast"
        );
    }
}
