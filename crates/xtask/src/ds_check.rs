#[cfg(test)]
mod tests;

use std::collections::BTreeSet;
use std::fmt;
use std::fs;
use std::path::Path;
use std::process::ExitCode;

use crate::check_support::{Finding, read_text, report, workspace_root};

pub(crate) const INDEX_FILE: &str = "apps/desktop/src/shared/ds/index.ts";
pub(crate) const LOCK_FILE: &str = "harness/ds-catalog/public-list.json";
pub(crate) const CATALOG_DIR: &str = "apps/desktop/.storybook/catalog";
pub(crate) const BASELINE_DIR: &str = "harness/visual/baselines";
pub(crate) const STRESS_DIR: &str = "apps/desktop/src/shared/ds/testing";
const THEMES: [&str; 2] = ["light", "dark"];

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Rule {
    PublicListChanged(String),
    MissingCatalogEntry(String),
    MissingBaseline(String),
    MissingStateStory(String),
}

impl fmt::Display for Rule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PublicListChanged(detail) => write!(f, "ds-public-list:{detail}"),
            Self::MissingCatalogEntry(name) => write!(f, "ds-catalog-entry:{name}"),
            Self::MissingBaseline(name) => write!(f, "ds-baseline:{name}"),
            Self::MissingStateStory(name) => write!(f, "ds-state-story:{name}"),
        }
    }
}

pub(crate) fn exported_components(index: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut rest = index;
    while let Some((_, after)) = rest.split_once("export {") {
        let Some((inside, remaining)) = after.split_once('}') else {
            break;
        };
        for part in inside.split(',') {
            let item = part.trim();
            if item.is_empty() || item.starts_with("type ") {
                continue;
            }
            let name = item.rsplit(" as ").next().unwrap_or(item).trim();
            names.push(String::from(name));
        }
        rest = remaining;
    }
    names
}

pub(crate) fn locked_components(lock: &str) -> Result<Vec<String>, String> {
    let value: serde_json::Value =
        serde_json::from_str(lock).map_err(|error| format!("{LOCK_FILE}: {error}"))?;
    value
        .get("components")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| format!("{LOCK_FILE}: components 배열이 없다"))?
        .iter()
        .map(|item| {
            item.as_str()
                .map(String::from)
                .ok_or_else(|| format!("{LOCK_FILE}: 문자열이 아닌 항목"))
        })
        .collect()
}

fn stress_text(root: &Path) -> Result<String, String> {
    let directory = root.join(STRESS_DIR);
    let entries =
        fs::read_dir(&directory).map_err(|error| format!("{}: {error}", directory.display()))?;
    let mut text = String::new();
    for entry in entries {
        let path = entry
            .map_err(|error| format!("{}: {error}", directory.display()))?
            .path();
        if path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.ends_with(".stories.tsx"))
        {
            text.push_str(&read_text(&path)?);
        }
    }
    Ok(text)
}

fn non_empty(path: &Path) -> bool {
    fs::metadata(path).is_ok_and(|meta| meta.is_file() && meta.len() > 0)
}

fn catalog_entry(root: &Path, name: &str) -> Result<bool, String> {
    let path = root.join(CATALOG_DIR).join(format!("{name}.stories.mjs"));
    if !path.is_file() {
        return Ok(false);
    }
    let text = read_text(&path)?;
    Ok(text.contains("export const Light0") && text.contains("export const Dark0"))
}

pub(crate) fn check_repository(root: &Path) -> Result<BTreeSet<Finding<Rule>>, String> {
    let exported = exported_components(&read_text(&root.join(INDEX_FILE))?);
    let locked = locked_components(&read_text(&root.join(LOCK_FILE))?)?;
    let mut findings = BTreeSet::new();
    for name in exported.iter().filter(|name| !locked.contains(name)) {
        findings.insert(Finding::whole_file(
            INDEX_FILE,
            Rule::PublicListChanged(format!("added:{name}")),
        ));
    }
    for name in locked.iter().filter(|name| !exported.contains(name)) {
        findings.insert(Finding::whole_file(
            INDEX_FILE,
            Rule::PublicListChanged(format!("removed:{name}")),
        ));
    }
    let stress = stress_text(root)?;
    for name in &exported {
        if !catalog_entry(root, name)? {
            findings.insert(Finding::whole_file(
                CATALOG_DIR,
                Rule::MissingCatalogEntry(name.clone()),
            ));
        }
        for theme in THEMES {
            let file = format!("{name}-{theme}-chromium-linux-arm64.png");
            if !non_empty(&root.join(BASELINE_DIR).join(&file)) {
                findings.insert(Finding::whole_file(
                    BASELINE_DIR,
                    Rule::MissingBaseline(file),
                ));
            }
        }
        if !stress.contains(&format!("\"{name}\"")) {
            findings.insert(Finding::whole_file(
                STRESS_DIR,
                Rule::MissingStateStory(name.clone()),
            ));
        }
    }
    Ok(findings)
}

pub(crate) fn command() -> ExitCode {
    let outcome = workspace_root().and_then(|root| check_repository(&root));
    report("ds-check", outcome)
}
