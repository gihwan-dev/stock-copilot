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
