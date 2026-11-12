use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use proc_macro2::{TokenStream, TokenTree};
use syn::ext::IdentExt;
use syn::visit::{self, Visit};
use syn::{ItemUse, UseTree};

use super::{Findings, Rule};

pub(crate) const WEBVIEW_API_FILE: &str = "harness/capability-check/webview-apis.toml";
const SOURCE_DIRECTORY: &str = "src";
const BUILD_SCRIPT: &str = "build.rs";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WebviewApis {
    allowed: BTreeMap<String, BTreeSet<String>>,
}

fn strings(table: &toml::Table, key: &str) -> Result<Vec<String>, String> {
    table
        .get(key)
        .and_then(toml::Value::as_array)
        .ok_or_else(|| format!("{WEBVIEW_API_FILE}: [[api]]마다 {key} 목록이 있어야 한다"))?
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(String::from)
                .ok_or_else(|| format!("{WEBVIEW_API_FILE}: {key}의 원소는 문자열이다"))
        })
        .collect()
}

impl WebviewApis {
    pub(crate) fn parse(text: &str) -> Result<Self, String> {
        let table: toml::Table = text
            .parse()
            .map_err(|error| format!("{WEBVIEW_API_FILE}: {error}"))?;
        if table.keys().any(|key| key != "reason" && key != "api") {
            return Err(format!("{WEBVIEW_API_FILE}: reason과 api만 둔다"));
        }
        let mut allowed = BTreeMap::new();
        for entry in table
            .get("api")
            .and_then(toml::Value::as_array)
            .ok_or_else(|| format!("{WEBVIEW_API_FILE}: [[api]]가 없다"))?
        {
            let group = entry
                .as_table()
                .ok_or_else(|| format!("{WEBVIEW_API_FILE}: [[api]]는 표다"))?;
            let reason = group.get("reason").and_then(toml::Value::as_str);
            if group.len() != 3 || reason.is_none_or(|written| written.trim().is_empty()) {
                return Err(format!(
                    "{WEBVIEW_API_FILE}: [[api]]는 names, allowed-files, reason만 갖는다"
                ));
            }
            let files: BTreeSet<String> = strings(group, "allowed-files")?.into_iter().collect();
            let names = strings(group, "names")?;
            if let Some(repeated) = names.iter().find(|name| allowed.contains_key(*name)) {
                return Err(format!("{WEBVIEW_API_FILE}: {repeated}이 두 번 있다"));
            }
            allowed.extend(names.into_iter().map(|name| (name, files.clone())));
        }
        Ok(Self { allowed })
    }

    pub(crate) fn load(root: &Path) -> Result<Self, String> {
        Self::parse(
            &fs::read_to_string(root.join(WEBVIEW_API_FILE))
                .map_err(|error| format!("{WEBVIEW_API_FILE}: {error}"))?,
        )
    }
}

#[derive(Debug, Default)]
struct Renames {
    found: Vec<(String, String)>,
}

fn renames(tree: &UseTree, found: &mut Vec<(String, String)>) {
    match tree {
        UseTree::Path(path) => renames(&path.tree, found),
        UseTree::Rename(rename) => found.push((
            rename.ident.unraw().to_string(),
            rename.rename.unraw().to_string(),
        )),
        UseTree::Group(group) => {
            for item in &group.items {
                renames(item, found);
            }
        }
        UseTree::Name(_) | UseTree::Glob(_) => {}
    }
}

impl<'ast> Visit<'ast> for Renames {
    fn visit_item_use(&mut self, i: &'ast ItemUse) {
        renames(&i.tree, &mut self.found);
        visit::visit_item_use(self, i);
    }
}

fn idents(stream: TokenStream, found: &mut BTreeSet<String>) {
    for tree in stream {
        match tree {
            TokenTree::Group(group) => idents(group.stream(), found),
            TokenTree::Ident(ident) => {
                found.insert(ident.unraw().to_string());
            }
            TokenTree::Punct(_) | TokenTree::Literal(_) => {}
        }
    }
}

fn rust_files(directory: &Path, found: &mut Vec<PathBuf>) {
    let Ok(listing) = fs::read_dir(directory) else {
        return;
    };
    let mut entries: Vec<PathBuf> = listing
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .collect();
    entries.sort();
    for entry in entries {
        if entry.is_dir() {
            rust_files(&entry, found);
        } else if entry.extension().is_some_and(|extension| extension == "rs") {
            found.push(entry);
        }
    }
}

pub(crate) fn sources(directory: &Path) -> Vec<(String, String)> {
    let mut files = Vec::new();
    rust_files(&directory.join(SOURCE_DIRECTORY), &mut files);
    files.push(directory.join(BUILD_SCRIPT));
    files
        .into_iter()
        .filter_map(|file| {
            let text = fs::read_to_string(&file).ok()?;
            let relative = file
                .strip_prefix(directory)
                .ok()?
                .to_str()?
                .replace('\\', "/");
            Some((relative, text))
        })
        .collect()
}

fn with_aliases(
    apis: &WebviewApis,
    files: &[(String, String)],
) -> BTreeMap<String, (String, BTreeSet<String>)> {
    let mut names: BTreeMap<String, (String, BTreeSet<String>)> = apis
        .allowed
        .iter()
        .map(|(name, allowed)| (name.clone(), (name.clone(), allowed.clone())))
        .collect();
    let pairs: Vec<(String, String)> = files
        .iter()
        .filter_map(|(_, text)| syn::parse_file(text).ok())
        .flat_map(|file| {
            let mut found = Renames::default();
            found.visit_file(&file);
            found.found
        })
        .collect();
    let mut changed = true;
    while changed {
        changed = false;
        for (original, alias) in &pairs {
            if let Some(entry) = names.get(original).cloned()
                && !names.contains_key(alias)
            {
                names.insert(alias.clone(), entry);
                changed = true;
            }
        }
    }
    names
}

pub(crate) fn webview_api_findings(
    apis: &WebviewApis,
    files: &[(String, String)],
    findings: &mut Findings,
) {
    let names = with_aliases(apis, files);
    for (path, text) in files {
        let Ok(stream) = text.parse::<TokenStream>() else {
            findings.add(path, Rule::Unparsable);
            continue;
        };
        let mut found = BTreeSet::new();
        idents(stream, &mut found);
        for ident in &found {
            if let Some((original, allowed)) = names.get(ident)
                && !allowed.contains(path)
            {
                findings.add(path, Rule::WebviewApi(original.clone()));
            }
        }
    }
}
