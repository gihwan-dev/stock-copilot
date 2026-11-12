use std::collections::BTreeSet;

use crate::workspace_sources::SourceFailure;

pub(crate) const CRATES_FILE: &str = "harness/raw-types/crates.toml";

const KINDS: [&str; 3] = ["target", "exception", "outside"];

const ENTRY_KEYS: [&str; 2] = ["crate", "reason"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Scope {
    Checked,
    Unchecked,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CrateRules {
    targets: BTreeSet<String>,
    classified: BTreeSet<String>,
}

fn failure(message: &str) -> SourceFailure {
    SourceFailure(format!("{CRATES_FILE}: {message}"))
}

fn entry_name(kind: &str, entry: &toml::Value) -> Result<String, SourceFailure> {
    let table = entry
        .as_table()
        .ok_or_else(|| failure(&format!("[[{kind}]] 항목은 표여야 한다")))?;
    if let Some(unknown) = table.keys().find(|key| !ENTRY_KEYS.contains(&key.as_str())) {
        return Err(failure(&format!("[[{kind}]] 모르는 키 {unknown}")));
    }
    let name = table.get("crate").and_then(toml::Value::as_str);
    let reason = table.get("reason").and_then(toml::Value::as_str);
    match (name, reason) {
        (Some(listed), Some(why)) if !listed.trim().is_empty() && !why.trim().is_empty() => {
            Ok(String::from(listed))
        }
        (Some(_) | None, Some(_) | None) => Err(failure(&format!(
            "[[{kind}]] 항목마다 crate와 reason이 있어야 한다"
        ))),
    }
}

fn kind_names(table: &toml::Table, kind: &str) -> Result<BTreeSet<String>, SourceFailure> {
    let entries = table
        .get(kind)
        .and_then(toml::Value::as_array)
        .ok_or_else(|| failure(&format!("[[{kind}]] 표가 없다")))?;
    let mut names = BTreeSet::new();
    for entry in entries {
        let name = entry_name(kind, entry)?;
        if !names.insert(name.clone()) {
            return Err(failure(&format!("{name}가 두 번 나온다")));
        }
    }
    Ok(names)
}

impl CrateRules {
    pub(crate) fn parse(table: &toml::Table) -> Result<Self, SourceFailure> {
        if let Some(unknown) = table.keys().find(|key| !KINDS.contains(&key.as_str())) {
            return Err(failure(&format!("모르는 표 {unknown}")));
        }
        let mut classified = BTreeSet::new();
        let mut targets = BTreeSet::new();
        for kind in KINDS {
            let names = kind_names(table, kind)?;
            if let Some(repeated) = names.iter().find(|name| classified.contains(*name)) {
                return Err(failure(&format!("{repeated}가 두 번 나온다")));
            }
            classified.extend(names.iter().cloned());
            if kind == "target" {
                targets = names;
            }
        }
        Ok(Self {
            targets,
            classified,
        })
    }

    pub(crate) fn scope(&self, crate_name: &str) -> Scope {
        if self.targets.contains(crate_name) {
            Scope::Checked
        } else {
            Scope::Unchecked
        }
    }

    pub(crate) fn classification_failures(&self, members: &BTreeSet<String>) -> Vec<String> {
        let unlisted = members
            .difference(&self.classified)
            .map(|name| format!("{CRATES_FILE}: 작업 공간 crate {name}가 분류되지 않았다"));
        let stale = self
            .classified
            .difference(members)
            .map(|name| format!("{CRATES_FILE}: {name}는 작업 공간 crate가 아니다"));
        unlisted.chain(stale).collect()
    }
}
