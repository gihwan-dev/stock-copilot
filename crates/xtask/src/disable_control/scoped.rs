use std::collections::BTreeSet;
use std::path::Path;

use syn::ext::IdentExt;
use syn::visit::{self, Visit};
use syn::{Attribute, Expr, File, FnArg, Item, ItemFn, ItemUse, Local, Macro, Type, UseTree};

use crate::workspace_sources::SourceFailure;

const SCOPED_KEYS: [&str; 4] = ["module", "site", "lint", "reason"];
const COLLECT_MACRO: &str = "collect_commands";
const COMMAND_ATTRIBUTE: [&str; 2] = ["tauri", "command"];
const TAURI_CRATE: &str = "tauri";
const INJECTED_TYPES: [&str; 5] = ["AppHandle", "Window", "Webview", "WebviewWindow", "State"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Site {
    CommandCollector,
    Command,
    InjectedCommand,
}

impl Site {
    fn parse(name: &str) -> Option<Self> {
        match name {
            "command-collector" => Some(Self::CommandCollector),
            "command" => Some(Self::Command),
            "injected-command" => Some(Self::InjectedCommand),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct ScopedPermit {
    module: String,
    site: Site,
    lint: String,
}

fn failure(file: &str, message: &str) -> SourceFailure {
    SourceFailure(format!("{file}: {message}"))
}

fn text<'entry>(
    file: &str,
    entry: &'entry toml::Table,
    key: &str,
) -> Result<&'entry str, SourceFailure> {
    entry
        .get(key)
        .and_then(toml::Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            failure(
                file,
                &format!("[[scoped-expect]] 항목마다 {key}가 있어야 한다"),
            )
        })
}

pub(crate) fn scoped_permit(
    root: &Path,
    file: &str,
    entry: &toml::Value,
) -> Result<ScopedPermit, SourceFailure> {
    let table = entry
        .as_table()
        .ok_or_else(|| failure(file, "[[scoped-expect]] 항목은 표여야 한다"))?;
    if let Some(unknown) = table
        .keys()
        .find(|key| !SCOPED_KEYS.contains(&key.as_str()))
    {
        return Err(failure(
            file,
            &format!("[[scoped-expect]] 모르는 키 {unknown}"),
        ));
    }
    text(file, table, "reason")?;
    let module = text(file, table, "module")?;
    if !root.join(format!("{module}.rs")).is_file() || !root.join(module).is_dir() {
        return Err(failure(
            file,
            &format!("{module}.rs와 {module}/가 둘 다 있어야 한다"),
        ));
    }
    let site_name = text(file, table, "site")?;
    let site =
        Site::parse(site_name).ok_or_else(|| failure(file, &format!("모르는 site {site_name}")))?;
    Ok(ScopedPermit {
        module: String::from(module),
        site,
        lint: String::from(text(file, table, "lint")?),
    })
}

pub(crate) fn in_module(module: &str, path: &str) -> bool {
    path == format!("{module}.rs") || path.starts_with(&format!("{module}/"))
}

pub(crate) fn permits(
    permits: &BTreeSet<ScopedPermit>,
    path: &str,
    sites: &BTreeSet<Site>,
    lint: &str,
) -> Option<()> {
    permits
        .iter()
        .any(|permit| {
            permit.lint == lint && sites.contains(&permit.site) && in_module(&permit.module, path)
        })
        .then_some(())
}

fn path_segments(path: &syn::Path) -> Vec<String> {
    path.segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect()
}

fn is_collect_macro(mac: &Macro) -> bool {
    mac.path
        .segments
        .last()
        .is_some_and(|segment| segment.ident == COLLECT_MACRO)
}

#[derive(Debug, Default)]
struct CollectFinder {
    found: Option<()>,
}

impl<'ast> Visit<'ast> for CollectFinder {
    fn visit_macro(&mut self, i: &'ast Macro) {
        if is_collect_macro(i) {
            self.found = Some(());
        }
        visit::visit_macro(self, i);
    }
}

fn is_command(attributes: &[Attribute]) -> bool {
    attributes
        .iter()
        .any(|attribute| path_segments(attribute.path()) == COMMAND_ATTRIBUTE)
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct TauriImports {
    trusted: BTreeSet<String>,
}

#[derive(Debug, Default)]
struct ImportScan {
    imported: BTreeSet<String>,
    shadowed: BTreeSet<String>,
}

fn injected_name(name: &str) -> bool {
    INJECTED_TYPES.contains(&name)
}

impl ImportScan {
    fn tree(&mut self, prefix: &[String], tree: &UseTree) {
        match tree {
            UseTree::Path(path) => {
                let mut next = prefix.to_vec();
                next.push(path.ident.unraw().to_string());
                self.tree(&next, &path.tree);
            }
            UseTree::Name(name) => {
                let leaf = name.ident.unraw().to_string();
                if prefix == [TAURI_CRATE] && injected_name(&leaf) {
                    self.imported.insert(leaf);
                } else if injected_name(&leaf) {
                    self.shadowed.insert(leaf);
                }
            }
            UseTree::Rename(rename) => {
                self.shadowed.extend(
                    [
                        rename.ident.unraw().to_string(),
                        rename.rename.unraw().to_string(),
                    ]
                    .into_iter()
                    .filter(|name| injected_name(name)),
                );
            }
            UseTree::Group(group) => {
                for item in &group.items {
                    self.tree(prefix, item);
                }
            }
            UseTree::Glob(_) => self.shadow_all(),
        }
    }

    fn shadow_all(&mut self) {
        self.shadowed.extend(INJECTED_TYPES.map(String::from));
    }

    fn defined(&mut self, name: &syn::Ident) {
        let text = name.unraw().to_string();
        if injected_name(&text) {
            self.shadowed.insert(text);
        }
    }
}

impl<'ast> Visit<'ast> for ImportScan {
    fn visit_item_use(&mut self, i: &'ast ItemUse) {
        self.tree(&[], &i.tree);
    }

    fn visit_item(&mut self, i: &'ast Item) {
        match i {
            Item::Struct(item) => self.defined(&item.ident),
            Item::Enum(item) => self.defined(&item.ident),
            Item::Union(item) => self.defined(&item.ident),
            Item::Type(item) => self.defined(&item.ident),
            Item::Trait(item) => self.defined(&item.ident),
            Item::TraitAlias(item) => self.defined(&item.ident),
            Item::Fn(item) => self.defined(&item.sig.ident),
            Item::Mod(item) => self.defined(&item.ident),
            Item::Const(_)
            | Item::ExternCrate(_)
            | Item::ForeignMod(_)
            | Item::Impl(_)
            | Item::Macro(_)
            | Item::Static(_)
            | Item::Use(_)
            | Item::Verbatim(_) => {}
            _ => self.shadow_all(),
        }
        visit::visit_item(self, i);
    }
}

pub(crate) fn tauri_imports(file: &File) -> TauriImports {
    let mut scan = ImportScan::default();
    scan.visit_file(file);
    TauriImports {
        trusted: scan.imported.difference(&scan.shadowed).cloned().collect(),
    }
}

fn injected(argument: &FnArg, imports: &TauriImports) -> bool {
    let FnArg::Typed(typed) = argument else {
        return false;
    };
    let Type::Path(path) = typed.ty.as_ref() else {
        return false;
    };
    if path.qself.is_some() {
        return false;
    }
    let names: Vec<String> = path
        .path
        .segments
        .iter()
        .map(|segment| segment.ident.unraw().to_string())
        .collect();
    match names.as_slice() {
        [only] => imports.trusted.contains(only),
        [krate, name] => krate == TAURI_CRATE && injected_name(name),
        _ => false,
    }
}

pub(crate) fn function_sites(function: &ItemFn, imports: &TauriImports) -> BTreeSet<Site> {
    let mut sites = BTreeSet::new();
    let mut finder = CollectFinder::default();
    finder.visit_block(&function.block);
    if finder.found.is_some() {
        sites.insert(Site::CommandCollector);
    }
    if is_command(&function.attrs) {
        sites.insert(Site::Command);
        if function
            .sig
            .inputs
            .iter()
            .all(|argument| injected(argument, imports))
        {
            sites.insert(Site::InjectedCommand);
        }
    }
    sites
}

pub(crate) fn local_sites(local: &Local) -> BTreeSet<Site> {
    let collects = local
        .init
        .as_ref()
        .is_some_and(|init| matches_collect(&init.expr));
    if collects {
        BTreeSet::from([Site::CommandCollector])
    } else {
        BTreeSet::new()
    }
}

fn matches_collect(expression: &Expr) -> bool {
    if let Expr::Macro(mac) = expression {
        is_collect_macro(&mac.mac)
    } else {
        false
    }
}
