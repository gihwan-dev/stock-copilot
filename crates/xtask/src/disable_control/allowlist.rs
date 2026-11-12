use std::collections::BTreeSet;
use std::path::Path;
use std::str::FromStr;

use proc_macro2::TokenStream;

use super::scoped::{ScopedPermit, Site, permits as scoped_permits, scoped_permit};
use crate::workspace_sources::SourceFailure;

pub(crate) const ALLOWLIST_FILE: &str = "harness/disable-control/allowlist.toml";

const ENTRY_KEYS: [&str; 4] = ["path", "statement", "lint", "reason"];

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Permit {
    pub(crate) path: String,
    pub(crate) statement: String,
    pub(crate) lint: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Allowlist {
    permits: BTreeSet<Permit>,
    scoped: BTreeSet<ScopedPermit>,
}

fn failure(message: &str) -> SourceFailure {
    SourceFailure(format!("{ALLOWLIST_FILE}: {message}"))
}

pub(crate) fn local_text(local: &syn::Local) -> Option<String> {
    let text = local
        .let_token
        .span
        .join(local.semi_token.span)?
        .source_text()?;
    TokenStream::from_str(&text)
        .ok()
        .map(|stream| stream.to_string())
}

fn statement_text(source: &str) -> Result<String, SourceFailure> {
    match syn::parse_str::<syn::Stmt>(source) {
        Ok(syn::Stmt::Local(local)) => {
            local_text(&local).ok_or_else(|| failure(&format!("{source}를 읽지 못했다")))
        }
        Ok(syn::Stmt::Item(_) | syn::Stmt::Expr(..) | syn::Stmt::Macro(_)) | Err(_) => {
            Err(failure(&format!("{source}는 let 문이 아니다")))
        }
    }
}

fn text<'entry>(entry: &'entry toml::Table, key: &str) -> Result<&'entry str, SourceFailure> {
    entry
        .get(key)
        .and_then(toml::Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| failure(&format!("[[expect]] 항목마다 {key}가 있어야 한다")))
}

fn permit(root: &Path, entry: &toml::Value) -> Result<Permit, SourceFailure> {
    let table = entry
        .as_table()
        .ok_or_else(|| failure("[[expect]] 항목은 표여야 한다"))?;
    if let Some(unknown) = table.keys().find(|key| !ENTRY_KEYS.contains(&key.as_str())) {
        return Err(failure(&format!("[[expect]] 모르는 키 {unknown}")));
    }
    let path = text(table, "path")?;
    text(table, "reason")?;
    if !root.join(path).is_file() {
        return Err(failure(&format!("{path}가 없다")));
    }
    Ok(Permit {
        path: String::from(path),
        statement: statement_text(text(table, "statement")?)?,
        lint: String::from(text(table, "lint")?),
    })
}

impl Allowlist {
    pub(crate) fn parse(root: &Path, table: &toml::Table) -> Result<Self, SourceFailure> {
        if let Some(unknown) = table
            .keys()
            .find(|key| *key != "expect" && *key != "scoped-expect")
        {
            return Err(failure(&format!("모르는 키 {unknown}")));
        }
        let entries = table
            .get("expect")
            .and_then(toml::Value::as_array)
            .ok_or_else(|| failure("[[expect]] 표가 없다"))?;
        let permits: Vec<Permit> = entries
            .iter()
            .map(|entry| permit(root, entry))
            .collect::<Result<_, _>>()?;
        let scoped: Vec<ScopedPermit> = match table.get("scoped-expect") {
            None => Vec::new(),
            Some(value) => value
                .as_array()
                .ok_or_else(|| failure("[[scoped-expect]]는 표의 배열이다"))?
                .iter()
                .map(|entry| scoped_permit(root, ALLOWLIST_FILE, entry))
                .collect::<Result<_, _>>()?,
        };
        let unique: BTreeSet<Permit> = permits.iter().cloned().collect();
        let unique_scoped: BTreeSet<ScopedPermit> = scoped.iter().cloned().collect();
        if unique.len() == permits.len() && unique_scoped.len() == scoped.len() {
            Ok(Self {
                permits: unique,
                scoped: unique_scoped,
            })
        } else {
            Err(failure("같은 항목이 두 번 있다"))
        }
    }

    pub(crate) fn permits_scoped(
        &self,
        path: &str,
        sites: &BTreeSet<Site>,
        lint: &str,
    ) -> Option<()> {
        scoped_permits(&self.scoped, path, sites, lint)
    }

    pub(crate) fn permits(&self, path: &str, placed: Option<&str>, lint: &str) -> Option<()> {
        let statement = placed?;
        self.permits
            .iter()
            .any(|permit| {
                permit.path == path && permit.statement == statement && permit.lint == lint
            })
            .then_some(())
    }
}
