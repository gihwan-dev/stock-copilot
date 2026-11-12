use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use super::failure::CheckFailure;

pub(crate) const ALLOWLIST_FILE: &str = "harness/source-checks/allowlists.toml";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Membership {
    Listed,
    Unlisted,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Allowlists {
    generated: BTreeSet<String>,
    clock_environment: BTreeSet<String>,
    build_scripts: BTreeSet<String>,
    generated_attributes: BTreeSet<String>,
}

fn listed_paths(
    root: &Path,
    table: &toml::Table,
    key: &str,
) -> Result<BTreeSet<String>, CheckFailure> {
    let entries = table
        .get(key)
        .and_then(toml::Value::as_array)
        .ok_or_else(|| CheckFailure::Allowlist(format!("{ALLOWLIST_FILE}: [[{key}]] 표가 없다")))?;
    entries
        .iter()
        .map(|entry| {
            let path = entry.get("path").and_then(toml::Value::as_str);
            let reason = entry.get("reason").and_then(toml::Value::as_str);
            match (path, reason) {
                (Some(listed), Some(why)) if !listed.is_empty() && !why.trim().is_empty() => {
                    if root.join(listed).exists() {
                        Ok(String::from(listed))
                    } else {
                        Err(CheckFailure::Allowlist(format!(
                            "{ALLOWLIST_FILE}: [[{key}]] {listed}가 없다"
                        )))
                    }
                }
                (Some(_) | None, Some(_) | None) => Err(CheckFailure::Allowlist(format!(
                    "{ALLOWLIST_FILE}: [[{key}]] 항목마다 path와 reason이 있어야 한다"
                ))),
            }
        })
        .collect()
}

fn listed_names(table: &toml::Table, key: &str) -> Result<BTreeSet<String>, CheckFailure> {
    let entries = table
        .get(key)
        .and_then(toml::Value::as_array)
        .ok_or_else(|| CheckFailure::Allowlist(format!("{ALLOWLIST_FILE}: [[{key}]] 표가 없다")))?;
    entries
        .iter()
        .map(|entry| {
            let name = entry.get("name").and_then(toml::Value::as_str);
            let reason = entry.get("reason").and_then(toml::Value::as_str);
            match (name, reason) {
                (Some(listed), Some(why)) if !listed.is_empty() && !why.trim().is_empty() => {
                    Ok(String::from(listed))
                }
                (Some(_) | None, Some(_) | None) => Err(CheckFailure::Allowlist(format!(
                    "{ALLOWLIST_FILE}: [[{key}]] 항목마다 name과 reason이 있어야 한다"
                ))),
            }
        })
        .collect()
}

impl Allowlists {
    pub(crate) fn load(root: &Path) -> Result<Self, CheckFailure> {
        let path = root.join(ALLOWLIST_FILE);
        let text =
            fs::read_to_string(&path).map_err(|error| CheckFailure::Io(path.clone(), error))?;
        let table: toml::Table = text.parse().map_err(|error: toml::de::Error| {
            CheckFailure::Allowlist(format!("{ALLOWLIST_FILE}: {error}"))
        })?;
        Ok(Self {
            generated: listed_paths(root, &table, "generated")?,
            clock_environment: listed_paths(root, &table, "clock-environment")?,
            build_scripts: listed_paths(root, &table, "build-script")?,
            generated_attributes: match table.get("generated-attribute") {
                Some(_) => listed_names(&table, "generated-attribute")?,
                None => BTreeSet::new(),
            },
        })
    }

    pub(crate) fn generated(&self, path: &str) -> Membership {
        if self.generated.iter().any(|prefix| {
            path.strip_prefix(prefix.as_str())
                .is_some_and(|rest| rest.starts_with('/'))
        }) {
            Membership::Listed
        } else {
            Membership::Unlisted
        }
    }

    pub(crate) fn clock_environment(&self, path: &str) -> Membership {
        if self.clock_environment.contains(path) {
            Membership::Listed
        } else {
            Membership::Unlisted
        }
    }

    pub(crate) fn generated_attribute(&self, name: &str) -> Membership {
        if self.generated_attributes.contains(name) {
            Membership::Listed
        } else {
            Membership::Unlisted
        }
    }

    pub(crate) fn build_script(&self, path: &str) -> Membership {
        if self.build_scripts.contains(path) {
            Membership::Listed
        } else {
            Membership::Unlisted
        }
    }
}
