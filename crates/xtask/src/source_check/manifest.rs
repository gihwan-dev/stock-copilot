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

pub(crate) fn manifest_findings(
    path: &str,
    manifest: &toml::Table,
    role: ManifestRole,
) -> Vec<Finding> {
    let mut findings: Vec<Finding> = BANNED_TABLES
        .into_iter()
        .filter(|table| manifest.contains_key(*table))
        .map(|table| Finding::whole_file(path, Rule::ManifestTable(String::from(table))))
        .collect();
    if role == ManifestRole::Member && !inherits_workspace_lints(manifest) {
        findings.push(Finding::whole_file(path, Rule::LintsNotInherited));
    }
    findings
}
