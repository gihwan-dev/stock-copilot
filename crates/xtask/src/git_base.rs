#[cfg(test)]
mod tests;

use std::collections::BTreeSet;
use std::path::Path;
use std::process::{Command, Stdio};

pub(crate) const HEAD: &str = "HEAD";

const COMMIT_WIDTH: usize = 40;

pub(crate) const REDIRECTING_VARIABLES: [&str; 13] = [
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_INDEX_FILE",
    "GIT_OBJECT_DIRECTORY",
    "GIT_ALTERNATE_OBJECT_DIRECTORIES",
    "GIT_COMMON_DIR",
    "GIT_SHALLOW_FILE",
    "GIT_CONFIG_GLOBAL",
    "GIT_CONFIG_SYSTEM",
    "GIT_CONFIG_COUNT",
    "GIT_CONFIG_PARAMETERS",
    "GIT_DIFF_OPTS",
    "GIT_ATTR_SOURCE",
];

pub(crate) const GRAFT_FILE: (&str, &str) = ("GIT_GRAFT_FILE", "/dev/null");

pub(crate) fn git_with(root: &Path, inherited: &[(String, String)]) -> Command {
    let mut command = Command::new("git");
    command.envs(inherited.iter().map(|(name, value)| (name, value)));
    for variable in REDIRECTING_VARIABLES {
        command.env_remove(variable);
    }
    let (graft_variable, graft_value) = GRAFT_FILE;
    command
        .env(graft_variable, graft_value)
        .arg("-C")
        .arg(root)
        .args([
            "--no-replace-objects",
            "-c",
            "core.fsmonitor=false",
            "-c",
            "advice.graftFileDeprecated=false",
        ])
        .stdin(Stdio::null());
    command
}

pub(crate) fn git(root: &Path) -> Command {
    git_with(root, &[])
}

pub(crate) fn output(mut command: Command, label: &str) -> Result<Vec<u8>, String> {
    let finished = command
        .stderr(Stdio::piped())
        .output()
        .map_err(|error| format!("git {label}: {error}"))?;
    if finished.status.success() {
        Ok(finished.stdout)
    } else {
        Err(format!(
            "git {label}: {}",
            String::from_utf8_lossy(&finished.stderr).trim()
        ))
    }
}

fn is_commit_id(text: &str) -> bool {
    text.len() == COMMIT_WIDTH
        && text
            .chars()
            .all(|next| next.is_ascii_digit() || ('a'..='f').contains(&next))
}

pub(crate) fn resolve_base(root: &Path, base: &str) -> Result<String, String> {
    if base != HEAD && !is_commit_id(base) {
        return Err(format!(
            "기준 커밋은 HEAD 또는 40자리 커밋 ID여야 한다(브랜치·태그 이름은 받지 않는다): {base}"
        ));
    }
    let mut command = git(root);
    command
        .args(["rev-parse", "--verify", "--quiet", "--end-of-options"])
        .arg(format!("{base}^{{commit}}"));
    let found = output(command, "rev-parse")
        .map_err(|failure| format!("기준 커밋을 찾지 못했다: {base} ({failure})"))?;
    let resolved = String::from_utf8_lossy(&found).trim().to_owned();
    if !is_commit_id(&resolved) {
        return Err(format!("기준 커밋을 찾지 못했다: {base}"));
    }
    let mut ancestry = git(root);
    ancestry.args(["merge-base", "--is-ancestor", &resolved, HEAD]);
    let status = ancestry
        .status()
        .map_err(|error| format!("git merge-base: {error}"))?;
    if status.success() {
        Ok(resolved)
    } else {
        Err(format!("기준 커밋 {resolved}이 HEAD의 조상이 아니다"))
    }
}

pub(crate) fn changed_paths(root: &Path, base: &str) -> Result<BTreeSet<String>, String> {
    let mut tracked = git(root);
    tracked.args(["diff", "--name-only", "-z", "--no-renames", base, "--"]);
    let mut untracked = git(root);
    untracked.args(["ls-files", "-z", "--others", "--exclude-standard"]);
    let mut paths = BTreeSet::new();
    for listing in [output(tracked, "diff")?, output(untracked, "ls-files")?] {
        paths.extend(
            listing
                .split(|byte| *byte == 0)
                .filter(|name| !name.is_empty())
                .map(|name| String::from_utf8_lossy(name).into_owned()),
        );
    }
    Ok(paths)
}
