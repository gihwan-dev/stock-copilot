use super::finding::{Finding, Rule};

pub(crate) const BANNED_TABLES: [&str; 5] = ["target", "profile", "patch", "replace", "features"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ManifestRole {
    WorkspaceRoot,
    Member,
}

fn inherits_workspace_lints(manifest: &toml::Table) -> bool {
    let Some(toml::Value::Table(lints)) = manifest.get("lints") else {
        return false;
    };
    lints.len() == 1 && lints.get("workspace") == Some(&toml::Value::Boolean(true))
}

#[must_use]
pub(crate) fn unsafe_exception_lints(workspace_root: &toml::Table) -> Option<toml::Value> {
    let mut lints = workspace_root
        .get("workspace")
        .and_then(|workspace| workspace.get("lints"))
        .cloned()?;
    let rust = lints.get_mut("rust")?.as_table_mut()?;
    let level = rust.get_mut("unsafe_code")?;
    if level.as_str() != Some("forbid") {
        return None;
    }
    *level = toml::Value::from("deny");
    Some(lints)
}

fn has_exception_lints(manifest: &toml::Table, exception: Option<&toml::Value>) -> bool {
    exception.is_some_and(|expected| manifest.get("lints") == Some(expected))
}

pub(crate) fn manifest_findings(
    path: &str,
    manifest: &toml::Table,
    role: ManifestRole,
    unsafe_exception: Option<&toml::Value>,
) -> Vec<Finding> {
    let mut findings: Vec<Finding> = BANNED_TABLES
        .into_iter()
        .filter(|table| manifest.contains_key(*table))
        .map(|table| Finding::whole_file(path, Rule::ManifestTable(String::from(table))))
        .collect();
    if role == ManifestRole::Member
        && !inherits_workspace_lints(manifest)
        && !has_exception_lints(manifest, unsafe_exception)
    {
        findings.push(Finding::whole_file(path, Rule::LintsNotInherited));
    }
    findings
}
