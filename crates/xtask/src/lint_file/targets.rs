use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::workspace_sources::{SourceFailure, io_failure};

pub(crate) const TARGETS_FILE: &str = "harness/lint-file/targets.toml";

const TARGETS_KEYS: [&str; 4] = ["eslint", "eslint-binary", "prettier", "prettier-binary"];

const ESLINT_KEYS: [&str; 3] = ["config", "directory", "paths"];

const UNCONFIGURED: &str = "undefined";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Coverage {
    Linted,
    Unlinted,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EslintTarget {
    pub(crate) directory: String,
    pub(crate) config: String,
    pub(crate) paths: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Targets {
    pub(crate) prettier_binary: String,
    pub(crate) eslint_binary: String,
    pub(crate) prettier: Vec<String>,
    pub(crate) eslint: Vec<EslintTarget>,
}

pub(crate) fn under(path: &str, prefix: &str) -> bool {
    path == prefix
        || path
            .strip_prefix(prefix)
            .is_some_and(|rest| rest.starts_with('/'))
}

fn failure(message: &str) -> SourceFailure {
    SourceFailure(format!("{TARGETS_FILE}: {message}"))
}

fn text(table: &toml::Table, key: &str) -> Result<String, SourceFailure> {
    table
        .get(key)
        .and_then(toml::Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(String::from)
        .ok_or_else(|| failure(&format!("{key}는 빈 값이 아닌 문자열이다")))
}

fn texts(table: &toml::Table, key: &str) -> Result<Vec<String>, SourceFailure> {
    let listed = table
        .get(key)
        .and_then(toml::Value::as_array)
        .ok_or_else(|| failure(&format!("{key} 목록이 없다")))?;
    let values = listed
        .iter()
        .map(|value| {
            value
                .as_str()
                .filter(|item| !item.trim().is_empty())
                .map(String::from)
                .ok_or_else(|| failure(&format!("{key}의 원소는 문자열이다")))
        })
        .collect::<Result<Vec<String>, SourceFailure>>()?;
    if values.is_empty() {
        return Err(failure(&format!("{key} 목록이 비어 있다")));
    }
    Ok(values)
}

fn unknown_key(table: &toml::Table, known: &[&str]) -> Result<(), SourceFailure> {
    table
        .keys()
        .find(|key| !known.contains(&key.as_str()))
        .map_or(Ok(()), |key| Err(failure(&format!("모르는 키 {key}"))))
}

impl Targets {
    pub(crate) fn parse(table: &toml::Table) -> Result<Self, SourceFailure> {
        unknown_key(table, &TARGETS_KEYS)?;
        let entries = table
            .get("eslint")
            .and_then(toml::Value::as_array)
            .ok_or_else(|| failure("[[eslint]] 표가 없다"))?;
        let eslint = entries
            .iter()
            .map(|entry| {
                let block = entry
                    .as_table()
                    .ok_or_else(|| failure("[[eslint]] 항목은 표다"))?;
                unknown_key(block, &ESLINT_KEYS)?;
                Ok(EslintTarget {
                    directory: text(block, "directory")?,
                    config: text(block, "config")?,
                    paths: texts(block, "paths")?,
                })
            })
            .collect::<Result<Vec<EslintTarget>, SourceFailure>>()?;
        if eslint.is_empty() {
            return Err(failure("[[eslint]] 표가 비어 있다"));
        }
        Ok(Self {
            prettier_binary: text(table, "prettier-binary")?,
            eslint_binary: text(table, "eslint-binary")?,
            prettier: texts(table, "prettier")?,
            eslint,
        })
    }

    pub(crate) fn load(root: &Path) -> Result<Self, SourceFailure> {
        let path = root.join(TARGETS_FILE);
        let text = fs::read_to_string(&path).map_err(|error| io_failure(&path, &error))?;
        let table: toml::Table = text
            .parse()
            .map_err(|error: toml::de::Error| failure(&error.to_string()))?;
        Self::parse(&table)
    }

    pub(crate) fn prettier_applies(&self, path: &str) -> Option<()> {
        self.prettier
            .iter()
            .any(|prefix| under(path, prefix))
            .then_some(())
    }

    pub(crate) fn eslint_for(&self, path: &str) -> Option<&EslintTarget> {
        self.eslint
            .iter()
            .find(|target| target.paths.iter().any(|prefix| under(path, prefix)))
    }

    pub(crate) fn eslint_command(&self, root: &Path, target: &EslintTarget) -> Command {
        let mut command = Command::new(root.join(&self.eslint_binary));
        command
            .current_dir(target.directory_in(root))
            .arg("--config")
            .arg(root.join(&target.config))
            .stdin(Stdio::null());
        command
    }

    pub(crate) fn eslint_configured(
        &self,
        root: &Path,
        target: &EslintTarget,
        path: &str,
    ) -> Result<Coverage, String> {
        let mut command = self.eslint_command(root, target);
        command.arg("--print-config").arg(root.join(path));
        let finished = command
            .output()
            .map_err(|error| format!("eslint --print-config: {error}"))?;
        if !finished.status.success() {
            return Err(format!(
                "eslint --print-config {path}: {}",
                String::from_utf8_lossy(&finished.stderr).trim()
            ));
        }
        if String::from_utf8_lossy(&finished.stdout).trim() == UNCONFIGURED {
            Ok(Coverage::Unlinted)
        } else {
            Ok(Coverage::Linted)
        }
    }
}

impl EslintTarget {
    pub(crate) fn directory_in(&self, root: &Path) -> PathBuf {
        if self.directory == "." {
            root.to_path_buf()
        } else {
            root.join(&self.directory)
        }
    }

    pub(crate) fn relative_to_directory(&self, path: &str) -> String {
        if self.directory == "." {
            return String::from(path);
        }
        path.strip_prefix(self.directory.as_str())
            .and_then(|rest| rest.strip_prefix('/'))
            .map_or_else(|| String::from(path), String::from)
    }
}
