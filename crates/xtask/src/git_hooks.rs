pub(crate) mod commit_base;

#[cfg(test)]
mod commit_base_tests;
#[cfg(test)]
mod tests;

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::io::{self, Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};

use crate::git_base::{self, REDIRECTING_VARIABLES};
use crate::workspace_sources::load_members;

pub(crate) const HOOK_RULES_FILE: &str = "harness/git-hooks/lefthook.toml";

pub(crate) const LEFTHOOK_CONFIG: &str = "lefthook.yml";

const HOOK_RULES_KEYS: [&str; 2] = ["foreign-configs", "version"];

const LEFTHOOK_VARIABLES: [&str; 6] = [
    "LEFTHOOK",
    "LEFTHOOK_BIN",
    "LEFTHOOK_CONFIG",
    "LEFTHOOK_EXCLUDE",
    "LEFTHOOK_QUIET",
    "LEFTHOOK_VERBOSE",
];

const ISOLATED_GIT: [(&str, &str); 2] = [
    ("GIT_CONFIG_GLOBAL", "/dev/null"),
    ("GIT_CONFIG_NOSYSTEM", "1"),
];

const SAMPLE_SUFFIX: &str = ".sample";

const EXECUTABLE_BITS: u32 = 0o111;

const ZERO_COMMIT: &str = "0000000000000000000000000000000000000000";

const EXPECTED_DIRECTORY: &str = "git-hooks-expected";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HookRules {
    pub(crate) version: String,
    pub(crate) foreign_configs: BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum HookFinding {
    LefthookVersion(String),
    HooksPath(String),
    Missing(String),
    Modified(String),
    NotExecutable(String),
    ForeignConfig(String),
}

impl fmt::Display for HookFinding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LefthookVersion(found) => write!(
                f,
                "hooks:lefthook-version: lefthook {found}, 고정 버전과 다르다 ({HOOK_RULES_FILE})"
            ),
            Self::HooksPath(found) => write!(
                f,
                "hooks:hooks-path: git이 {found}의 훅을 쓴다. core.hooksPath를 지운다"
            ),
            Self::Missing(name) => write!(
                f,
                "hooks:missing: {name} 훅이 없다. 저장소 루트에서 lefthook install을 한 번 실행한다"
            ),
            Self::Modified(name) => write!(
                f,
                "hooks:modified: {name} 훅이 lefthook이 만드는 내용과 다르다. lefthook install을 다시 실행한다"
            ),
            Self::NotExecutable(name) => {
                write!(f, "hooks:not-executable: {name} 훅에 실행 권한이 없다")
            }
            Self::ForeignConfig(name) => write!(
                f,
                "hooks:foreign-config: {name}가 있다. lefthook 설정은 {LEFTHOOK_CONFIG} 하나만 둔다"
            ),
        }
    }
}

fn rule_failure(message: &str) -> String {
    format!("{HOOK_RULES_FILE}: {message}")
}

impl HookRules {
    pub(crate) fn parse(table: &toml::Table) -> Result<Self, String> {
        if let Some(unknown) = table
            .keys()
            .find(|key| !HOOK_RULES_KEYS.contains(&key.as_str()))
        {
            return Err(rule_failure(&format!("모르는 키 {unknown}")));
        }
        let version = table
            .get("version")
            .and_then(toml::Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .map(String::from)
            .ok_or_else(|| rule_failure("version이 없다"))?;
        let foreign_configs = table
            .get("foreign-configs")
            .and_then(toml::Value::as_array)
            .ok_or_else(|| rule_failure("foreign-configs 목록이 없다"))?
            .iter()
            .map(|value| {
                value
                    .as_str()
                    .filter(|name| !name.trim().is_empty() && *name != LEFTHOOK_CONFIG)
                    .map(String::from)
                    .ok_or_else(|| rule_failure("foreign-configs의 원소는 다른 설정 파일 이름이다"))
            })
            .collect::<Result<BTreeSet<String>, String>>()?;
        Ok(Self {
            version,
            foreign_configs,
        })
    }

    pub(crate) fn load(root: &Path) -> Result<Self, String> {
        let path = root.join(HOOK_RULES_FILE);
        let text =
            fs::read_to_string(&path).map_err(|error| format!("{}: {error}", path.display()))?;
        let table: toml::Table = text
            .parse()
            .map_err(|error: toml::de::Error| rule_failure(&error.to_string()))?;
        Self::parse(&table)
    }
}

fn tool(program: &str, directory: &Path, environment: &[(String, String)]) -> Command {
    let mut command = Command::new(program);
    command.current_dir(directory).stdin(Stdio::null());
    command.envs(environment.iter().map(|(name, value)| (name, value)));
    command
}

fn isolated(program: &str, directory: &Path) -> Command {
    let mut command = Command::new(program);
    command.current_dir(directory).stdin(Stdio::null());
    for variable in REDIRECTING_VARIABLES.into_iter().chain(LEFTHOOK_VARIABLES) {
        command.env_remove(variable);
    }
    command.envs(ISOLATED_GIT);
    command
}

fn stdout_of(mut command: Command, label: &str) -> Result<String, String> {
    let finished = command
        .stderr(Stdio::piped())
        .output()
        .map_err(|error| format!("{label}: {error}"))?;
    if finished.status.success() {
        Ok(String::from_utf8_lossy(&finished.stdout).trim().to_owned())
    } else {
        Err(format!(
            "{label}: {}{}",
            String::from_utf8_lossy(&finished.stdout).trim(),
            String::from_utf8_lossy(&finished.stderr).trim()
        ))
    }
}

fn comparable(path: &Path) -> PathBuf {
    match path.canonicalize() {
        Ok(resolved) => resolved,
        Err(_) => path.to_path_buf(),
    }
}

fn expected_hooks(repository: &Path, scratch: &Path) -> Result<BTreeMap<String, Vec<u8>>, String> {
    if scratch.exists() {
        fs::remove_dir_all(scratch).map_err(|error| format!("{}: {error}", scratch.display()))?;
    }
    fs::create_dir_all(scratch).map_err(|error| format!("{}: {error}", scratch.display()))?;
    fs::copy(
        repository.join(LEFTHOOK_CONFIG),
        scratch.join(LEFTHOOK_CONFIG),
    )
    .map_err(|error| format!("{LEFTHOOK_CONFIG}: {error}"))?;
    let mut init = isolated("git", scratch);
    init.args(["init", "--quiet"]);
    stdout_of(init, "git init")?;
    let mut install = isolated("lefthook", scratch);
    install.arg("install");
    stdout_of(install, "lefthook install")?;
    let directory = scratch.join(".git").join("hooks");
    let listing =
        fs::read_dir(&directory).map_err(|error| format!("{}: {error}", directory.display()))?;
    let mut hooks = BTreeMap::new();
    for listed in listing {
        let entry = listed.map_err(|error| format!("{}: {error}", directory.display()))?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.ends_with(SAMPLE_SUFFIX) {
            continue;
        }
        let bytes =
            fs::read(entry.path()).map_err(|error| format!("{}: {error}", name.as_str()))?;
        hooks.insert(name, bytes);
    }
    if hooks.is_empty() {
        return Err(format!(
            "{LEFTHOOK_CONFIG}로 lefthook install이 훅을 만들지 않았다"
        ));
    }
    Ok(hooks)
}

fn hook_findings(
    directory: &Path,
    expected: &BTreeMap<String, Vec<u8>>,
) -> Result<BTreeSet<HookFinding>, String> {
    let mut findings = BTreeSet::new();
    for (name, bytes) in expected {
        let path = directory.join(name);
        let Ok(metadata) = fs::symlink_metadata(&path) else {
            findings.insert(HookFinding::Missing(name.clone()));
            continue;
        };
        let actual = if metadata.is_file() {
            fs::read(&path).map_err(|error| format!("{}: {error}", path.display()))?
        } else {
            Vec::new()
        };
        if actual != *bytes {
            findings.insert(HookFinding::Modified(name.clone()));
        } else if metadata.permissions().mode() & EXECUTABLE_BITS == 0 {
            findings.insert(HookFinding::NotExecutable(name.clone()));
        }
    }
    Ok(findings)
}

pub(crate) fn installed_findings(
    rules: &HookRules,
    repository: &Path,
    scratch: &Path,
    environment: &[(String, String)],
) -> Result<BTreeSet<HookFinding>, String> {
    let mut findings = BTreeSet::new();
    let mut version = tool("lefthook", repository, environment);
    version.arg("version");
    let found = stdout_of(version, "lefthook version")?;
    if found != rules.version {
        findings.insert(HookFinding::LefthookVersion(found));
    }
    for name in &rules.foreign_configs {
        if fs::symlink_metadata(repository.join(name)).is_ok() {
            findings.insert(HookFinding::ForeignConfig(name.clone()));
        }
    }
    let mut common = git_base::git(repository);
    common.args(["rev-parse", "--path-format=absolute", "--git-common-dir"]);
    let common_directory = PathBuf::from(
        String::from_utf8_lossy(&git_base::output(common, "rev-parse")?)
            .trim()
            .to_owned(),
    );
    let mut hooks = tool("git", repository, environment);
    hooks.args(["rev-parse", "--path-format=absolute", "--git-path", "hooks"]);
    let hooks_directory = PathBuf::from(stdout_of(hooks, "git rev-parse --git-path hooks")?);
    let expected_directory = common_directory.join("hooks");
    if comparable(&hooks_directory) != comparable(&expected_directory) {
        findings.insert(HookFinding::HooksPath(
            hooks_directory.display().to_string(),
        ));
        return Ok(findings);
    }
    let expected = expected_hooks(repository, scratch)?;
    findings.extend(hook_findings(&expected_directory, &expected)?);
    Ok(findings)
}

fn installed_report() -> Result<BTreeSet<HookFinding>, String> {
    let members = load_members(None).map_err(|failure| failure.0)?;
    let rules = HookRules::load(&members.root)?;
    let scratch = members.target_directory.join(EXPECTED_DIRECTORY);
    installed_findings(&rules, &members.root, &scratch, &[])
}

fn write_installed(outcome: Result<BTreeSet<HookFinding>, String>) -> io::Result<ExitCode> {
    match outcome {
        Ok(findings) if findings.is_empty() => {
            writeln!(io::stdout().lock(), "hooks-installed: 위반 없음")?;
            Ok(ExitCode::SUCCESS)
        }
        Ok(findings) => {
            let mut output = io::stderr().lock();
            for finding in &findings {
                writeln!(output, "{finding}")?;
            }
            writeln!(output, "hooks-installed: 위반 {}건", findings.len())?;
            Ok(ExitCode::FAILURE)
        }
        Err(failure) => {
            writeln!(io::stderr().lock(), "hooks-installed: {failure}")?;
            Ok(ExitCode::FAILURE)
        }
    }
}

pub(crate) fn installed_command() -> ExitCode {
    match write_installed(installed_report()) {
        Ok(code) => code,
        Err(_) => ExitCode::FAILURE,
    }
}

fn single_line(command: Command, label: &str) -> Result<Vec<String>, String> {
    Ok(String::from_utf8_lossy(&git_base::output(command, label)?)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(String::from)
        .collect())
}

fn new_branch_base(root: &Path, local: &str) -> Result<String, String> {
    let mut walk = git_base::git(root);
    walk.args(["rev-list", "--boundary", local, "--not", "--remotes"]);
    let listed = single_line(walk, "rev-list")?;
    if listed.is_empty() {
        return Ok(String::from(local));
    }
    let boundaries: BTreeSet<&str> = listed
        .iter()
        .filter_map(|line| line.strip_prefix('-'))
        .collect();
    match boundaries.iter().collect::<Vec<_>>().as_slice() {
        [only] => Ok(String::from(**only)),
        [] => {
            let mut roots = git_base::git(root);
            roots.args(["rev-list", "--max-parents=0", local]);
            match single_line(roots, "rev-list")?.as_slice() {
                [first] => Ok(first.clone()),
                [] | [_, _, ..] => Err(String::from(
                    "원격과 공통 커밋이 없고 첫 커밋이 하나가 아니다. 기준을 정할 수 없다",
                )),
            }
        }
        [_, _, ..] => Err(String::from(
            "새 브랜치가 원격의 여러 커밋에서 갈라진다. 기준을 하나로 정할 수 없다",
        )),
    }
}

fn update_base(root: &Path, head: &str, line: &str) -> Result<Option<String>, String> {
    let fields: Vec<&str> = line.split_whitespace().collect();
    let [local_ref, local, _, remote] = fields.as_slice() else {
        return Err(format!("pre-push 입력 줄의 형식이 다르다: {line}"));
    };
    if *local == ZERO_COMMIT {
        return Ok(None);
    }
    if local != &head {
        return Err(format!(
            "{local_ref}({local})는 HEAD가 아니다. 커밋 훅·푸시 훅은 작업 사본을 검사하므로 HEAD만 푸시한다"
        ));
    }
    let base = if *remote == ZERO_COMMIT {
        new_branch_base(root, local)?
    } else {
        String::from(*remote)
    };
    git_base::resolve_base(root, &base).map(Some)
}

pub(crate) fn push_base(root: &Path, input: &str) -> Result<String, String> {
    let head = git_base::resolve_base(root, git_base::HEAD)?;
    let mut bases = BTreeSet::new();
    for line in input.lines().filter(|line| !line.trim().is_empty()) {
        if let Some(base) = update_base(root, &head, line)? {
            bases.insert(base);
        }
    }
    let listed: Vec<String> = bases.into_iter().collect();
    match listed.as_slice() {
        [] => Ok(head),
        [only] => Ok(only.clone()),
        [_, _, ..] => Err(String::from(
            "기준이 다른 여러 브랜치를 한 번에 푸시한다. 브랜치마다 따로 푸시한다",
        )),
    }
}

fn push_base_text() -> Result<String, String> {
    let mut input = String::new();
    io::stdin()
        .read_to_string(&mut input)
        .map_err(|error| format!("표준 입력: {error}"))?;
    let members = load_members(None).map_err(|failure| failure.0)?;
    push_base(&members.root, &input)
}

pub(crate) fn push_base_command() -> ExitCode {
    let written = match push_base_text() {
        Ok(base) => writeln!(io::stdout().lock(), "{base}").map(|()| ExitCode::SUCCESS),
        Err(failure) => {
            writeln!(io::stderr().lock(), "push-base: {failure}").map(|()| ExitCode::FAILURE)
        }
    };
    match written {
        Ok(code) => code,
        Err(_) => ExitCode::FAILURE,
    }
}
