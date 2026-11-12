use std::path::Path;
use std::process::{Command, Stdio};

pub(crate) const HEAD: &str = "HEAD";

const COMMIT_WIDTH: usize = 40;

const REDIRECTING_VARIABLES: [&str; 22] = [
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_INDEX_FILE",
    "GIT_OBJECT_DIRECTORY",
    "GIT_ALTERNATE_OBJECT_DIRECTORIES",
    "GIT_COMMON_DIR",
    "GIT_NAMESPACE",
    "GIT_CEILING_DIRECTORIES",
    "GIT_DISCOVERY_ACROSS_FILESYSTEM",
    "GIT_REPLACE_REF_BASE",
    "GIT_NO_REPLACE_OBJECTS",
    "GIT_GRAFT_FILE",
    "GIT_SHALLOW_FILE",
    "GIT_CONFIG",
    "GIT_CONFIG_GLOBAL",
    "GIT_CONFIG_SYSTEM",
    "GIT_CONFIG_NOSYSTEM",
    "GIT_CONFIG_COUNT",
    "GIT_CONFIG_PARAMETERS",
    "GIT_EXTERNAL_DIFF",
    "GIT_DIFF_OPTS",
    "GIT_ATTR_SOURCE",
];

pub(crate) fn git(root: &Path) -> Command {
    let mut command = Command::new("git");
    for variable in REDIRECTING_VARIABLES {
        command.env_remove(variable);
    }
    command
        .arg("-C")
        .arg(root)
        .args(["--no-replace-objects", "-c", "core.fsmonitor=false"])
        .stdin(Stdio::null());
    command
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
    if is_commit_id(&resolved) {
        Ok(resolved)
    } else {
        Err(format!("기준 커밋을 찾지 못했다: {base}"))
    }
}

pub(crate) fn working_tree_diff(root: &Path, base: &str) -> Result<String, String> {
    let mut tracked = git(root);
    tracked.args([
        "diff",
        "--no-ext-diff",
        "--no-textconv",
        "--no-color",
        "--no-renames",
        base,
        "--",
    ]);
    let mut text = String::from_utf8_lossy(&output(tracked, "diff")?).into_owned();
    let mut untracked = git(root);
    untracked.args(["ls-files", "-z", "--others", "--exclude-standard"]);
    let listing = output(untracked, "ls-files")?;
    for name in listing
        .split(|byte| *byte == 0)
        .filter(|name| !name.is_empty())
    {
        let path = String::from_utf8_lossy(name).into_owned();
        let mut added = git(root);
        added.args([
            "diff",
            "--no-index",
            "--no-ext-diff",
            "--no-textconv",
            "--no-color",
            "--",
            "/dev/null",
            &path,
        ]);
        let finished = added
            .output()
            .map_err(|error| format!("git diff --no-index: {error}"))?;
        text.push_str(&String::from_utf8_lossy(&finished.stdout));
    }
    Ok(text)
}
