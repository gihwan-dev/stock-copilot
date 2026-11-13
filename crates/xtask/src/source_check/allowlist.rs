use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use super::decimal_methods::DecimalRules;
use super::failure::CheckFailure;
use super::finding::Finding;
use proc_macro2::TokenStream;

pub(crate) const ALLOWLIST_FILE: &str = "harness/source-checks/allowlists.toml";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Membership {
    Listed,
    Unlisted,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Allowlists {
    decimal: DecimalRules,
    generated: BTreeSet<String>,
    clock_environment: BTreeSet<String>,
    clock_names: BTreeMap<String, BTreeSet<String>>,
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

fn named_paths(
    table: &toml::Table,
    key: &str,
) -> Result<BTreeMap<String, BTreeSet<String>>, CheckFailure> {
    let entries = table
        .get(key)
        .and_then(toml::Value::as_array)
        .map_or(&[][..], Vec::as_slice);
    let mut named = BTreeMap::new();
    for entry in entries {
        let Some(listed) = entry.get("names") else {
            continue;
        };
        let rule_names: Option<BTreeSet<String>> = listed.as_array().and_then(|items| {
            items
                .iter()
                .map(|item| item.as_str().map(String::from))
                .collect()
        });
        match (entry.get("path").and_then(toml::Value::as_str), rule_names) {
            (Some(path), Some(set)) if !set.is_empty() => {
                named.insert(String::from(path), set);
            }
            (Some(_) | None, Some(_) | None) => {
                return Err(CheckFailure::Allowlist(format!(
                    "{ALLOWLIST_FILE}: [[{key}]] names는 비어 있지 않은 규칙 이름 목록이어야 한다"
                )));
            }
        }
    }
    Ok(named)
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
            decimal: DecimalRules::parse(&table)?,
            generated: listed_paths(root, &table, "generated")?,
            clock_environment: listed_paths(root, &table, "clock-environment")?,
            clock_names: named_paths(&table, "clock-environment")?,
            build_scripts: listed_paths(root, &table, "build-script")?,
            generated_attributes: match table.get("generated-attribute") {
                Some(_) => listed_names(&table, "generated-attribute")?,
                None => BTreeSet::new(),
            },
        })
    }

    pub(crate) fn decimal_findings(&self, path: &str, stream: TokenStream) -> Vec<Finding> {
        self.decimal.findings(path, stream)
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

    pub(crate) fn clock_names(&self, path: &str) -> Option<&BTreeSet<String>> {
        self.clock_names.get(path)
    }

    pub(crate) fn clock_environment(&self, path: &str) -> Membership {
        if self.clock_environment.contains(path) && !self.clock_names.contains_key(path) {
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
