use std::collections::{BTreeMap, BTreeSet};

use crate::workspace_sources::SourceFailure;

pub(crate) const RULES_FILE: &str = "harness/raw-types/rules.toml";

const KEYS: [&str; 12] = [
    "standard-roots",
    "local-roots",
    "constructor-names",
    "constructor-wrappers",
    "accessor-prefix",
    "external-types",
    "external-traits",
    "newtype-fields",
    "borrowed-forms",
    "bool-methods",
    "standard-type",
    "standard-trait",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    Type,
    Trait,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RawRules {
    pub(crate) standard_roots: BTreeSet<String>,
    pub(crate) local_roots: BTreeSet<String>,
    pub(crate) constructor_names: BTreeSet<String>,
    pub(crate) constructor_wrappers: BTreeSet<String>,
    pub(crate) accessor_prefix: String,
    pub(crate) newtype_fields: BTreeSet<String>,
    pub(crate) borrowed_forms: BTreeMap<String, BTreeSet<String>>,
    pub(crate) bool_methods: BTreeSet<(String, String)>,
    standard_types: BTreeMap<String, Prelude>,
    standard_traits: BTreeMap<String, Prelude>,
    external_types: BTreeSet<String>,
    external_traits: BTreeSet<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Prelude {
    Bare,
    Qualified,
}

fn failure(message: &str) -> SourceFailure {
    SourceFailure(format!("{RULES_FILE}: {message}"))
}

fn array<'table>(
    table: &'table toml::Table,
    key: &str,
) -> Result<&'table Vec<toml::Value>, SourceFailure> {
    table
        .get(key)
        .and_then(toml::Value::as_array)
        .ok_or_else(|| failure(&format!("{key} 목록이 없다")))
}

fn text(value: &toml::Value, key: &str) -> Result<String, SourceFailure> {
    value
        .get(key)
        .and_then(toml::Value::as_str)
        .filter(|found| !found.trim().is_empty())
        .map(String::from)
        .ok_or_else(|| failure(&format!("{key}가 없다")))
}

fn exact_keys(value: &toml::Value, keys: &[&str]) -> Result<(), SourceFailure> {
    let table = value
        .as_table()
        .ok_or_else(|| failure("항목은 표여야 한다"))?;
    let mut present: Vec<&str> = table.keys().map(String::as_str).collect();
    present.sort_unstable();
    let mut wanted = keys.to_vec();
    wanted.sort_unstable();
    if present == wanted {
        Ok(())
    } else {
        Err(failure(&format!("항목의 키는 {keys:?}여야 한다")))
    }
}

fn names(table: &toml::Table, key: &str) -> Result<BTreeSet<String>, SourceFailure> {
    array(table, key)?
        .iter()
        .map(|value| {
            value
                .as_str()
                .filter(|found| !found.trim().is_empty())
                .map(String::from)
                .ok_or_else(|| failure(&format!("{key}의 원소는 문자열이다")))
        })
        .collect()
}

fn pairs(
    table: &toml::Table,
    key: &str,
    first: &str,
    second: &str,
) -> Result<Vec<(String, String)>, SourceFailure> {
    array(table, key)?
        .iter()
        .map(|value| {
            exact_keys(value, &[first, second])?;
            Ok((text(value, first)?, text(value, second)?))
        })
        .collect()
}

fn standard(table: &toml::Table, key: &str) -> Result<BTreeMap<String, Prelude>, SourceFailure> {
    array(table, key)?
        .iter()
        .map(|value| {
            exact_keys(value, &["path", "prelude", "reason"])?;
            text(value, "reason")?;
            let prelude = match value.get("prelude").and_then(toml::Value::as_bool) {
                Some(true) => Prelude::Bare,
                Some(false) => Prelude::Qualified,
                None => return Err(failure("prelude는 참·거짓이다")),
            };
            Ok((text(value, "path")?, prelude))
        })
        .collect()
}

fn external(table: &toml::Table, key: &str) -> Result<BTreeSet<String>, SourceFailure> {
    array(table, key)?
        .iter()
        .map(|value| {
            exact_keys(value, &["path", "reason"])?;
            text(value, "reason")?;
            text(value, "path")
        })
        .collect()
}

impl RawRules {
    pub(crate) fn parse(table: &toml::Table) -> Result<Self, SourceFailure> {
        if let Some(unknown) = table.keys().find(|key| !KEYS.contains(&key.as_str())) {
            return Err(failure(&format!("모르는 키 {unknown}")));
        }
        let mut borrowed_forms: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for (owned, borrowed) in pairs(table, "borrowed-forms", "owned", "borrowed")? {
            borrowed_forms.entry(owned).or_default().insert(borrowed);
        }
        Ok(Self {
            standard_roots: names(table, "standard-roots")?,
            local_roots: names(table, "local-roots")?,
            constructor_names: names(table, "constructor-names")?,
            constructor_wrappers: names(table, "constructor-wrappers")?,
            accessor_prefix: table
                .get("accessor-prefix")
                .and_then(toml::Value::as_str)
                .filter(|prefix| !prefix.is_empty())
                .map(String::from)
                .ok_or_else(|| failure("accessor-prefix가 없다"))?,
            newtype_fields: names(table, "newtype-fields")?,
            borrowed_forms,
            bool_methods: pairs(table, "bool-methods", "trait", "method")?
                .into_iter()
                .collect(),
            standard_types: standard(table, "standard-type")?,
            standard_traits: standard(table, "standard-trait")?,
            external_types: external(table, "external-types")?,
            external_traits: external(table, "external-traits")?,
        })
    }

    const fn standard_of(&self, kind: Kind) -> &BTreeMap<String, Prelude> {
        match kind {
            Kind::Type => &self.standard_types,
            Kind::Trait => &self.standard_traits,
        }
    }

    pub(crate) fn standard_allowed(&self, kind: Kind, path: &str) -> Option<()> {
        self.standard_of(kind).contains_key(path).then_some(())
    }

    pub(crate) fn external_allowed(&self, kind: Kind, path: &str) -> Option<()> {
        let listed = match kind {
            Kind::Type => &self.external_types,
            Kind::Trait => &self.external_traits,
        };
        listed.contains(path).then_some(())
    }

    pub(crate) fn prelude(&self, kind: Kind, name: &str) -> Option<String> {
        self.standard_of(kind)
            .iter()
            .find(|(path, prelude)| {
                **prelude == Prelude::Bare
                    && path.rsplit("::").next().is_some_and(|last| last == name)
            })
            .map(|(path, _)| path.clone())
    }
}
