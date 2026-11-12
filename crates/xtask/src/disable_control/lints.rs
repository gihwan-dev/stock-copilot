use std::collections::BTreeSet;

use crate::workspace_sources::SourceFailure;

pub(crate) const LINTS_FILE: &str = "harness/disable-control/lints.toml";

const LINTS_KEYS: [&str; 3] = ["clippy", "groups", "disableable"];

const CLIPPY_PREFIX: &str = "clippy::";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LintCatalogue {
    pub(crate) clippy: String,
    pub(crate) groups: BTreeSet<String>,
    pub(crate) disableable: BTreeSet<String>,
}

fn failure(file: &str, message: &str) -> SourceFailure {
    SourceFailure(format!("{file}: {message}"))
}

fn names(table: &toml::Table, key: &str) -> Result<BTreeSet<String>, SourceFailure> {
    let entries = table
        .get(key)
        .and_then(toml::Value::as_array)
        .ok_or_else(|| failure(LINTS_FILE, &format!("{key} 목록이 없다")))?;
    let listed: Vec<String> = entries
        .iter()
        .map(|entry| {
            entry
                .as_str()
                .filter(|name| !name.trim().is_empty())
                .map(String::from)
                .ok_or_else(|| failure(LINTS_FILE, &format!("{key}의 원소는 린트 이름이다")))
        })
        .collect::<Result<_, _>>()?;
    let unique: BTreeSet<String> = listed.iter().cloned().collect();
    if unique.len() == listed.len() {
        Ok(unique)
    } else {
        Err(failure(
            LINTS_FILE,
            &format!("{key}에 같은 이름이 두 번 있다"),
        ))
    }
}

impl LintCatalogue {
    pub(crate) fn parse(table: &toml::Table) -> Result<Self, SourceFailure> {
        if let Some(unknown) = table.keys().find(|key| !LINTS_KEYS.contains(&key.as_str())) {
            return Err(failure(LINTS_FILE, &format!("모르는 키 {unknown}")));
        }
        let clippy = table
            .get("clippy")
            .and_then(toml::Value::as_str)
            .filter(|version| !version.trim().is_empty())
            .map(String::from)
            .ok_or_else(|| failure(LINTS_FILE, "clippy 버전이 없다"))?;
        Ok(Self {
            clippy,
            groups: names(table, "groups")?,
            disableable: names(table, "disableable")?,
        })
    }
}

pub(crate) fn individually_configured(manifest: &toml::Table) -> BTreeSet<String> {
    let lints = manifest
        .get("workspace")
        .and_then(|workspace| workspace.get("lints"))
        .and_then(toml::Value::as_table);
    let tool = |name: &str| {
        lints
            .and_then(|table| table.get(name))
            .and_then(toml::Value::as_table)
            .map(|table| table.keys().cloned().collect::<Vec<_>>())
            .into_iter()
            .flatten()
    };
    tool("rust")
        .chain(tool("clippy").map(|name| format!("{CLIPPY_PREFIX}{name}")))
        .collect()
}
