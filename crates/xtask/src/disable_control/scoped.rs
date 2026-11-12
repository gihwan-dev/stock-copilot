use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use syn::visit::{self, Visit};
use syn::{Attribute, Expr, FnArg, Item, ItemFn, Local, Macro, Meta, Type};

use crate::workspace_sources::SourceFailure;

const SCOPED_KEYS: [&str; 4] = ["module", "site", "lint", "reason"];
const COLLECT_MACRO: &str = "collect_commands";
const COMMAND_ATTRIBUTE: [&str; 2] = ["tauri", "command"];
const INJECTED_TYPES: [&str; 5] = ["AppHandle", "Window", "Webview", "WebviewWindow", "State"];
const TYPE_DERIVE: &str = "Type";
const ROOT_FILES: [&str; 3] = ["lib.rs", "main.rs", "mod.rs"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Site {
    CommandCollector,
    Command,
    InjectedCommand,
    TypeModule,
}

impl Site {
    fn parse(name: &str) -> Option<Self> {
        match name {
            "command-collector" => Some(Self::CommandCollector),
            "command" => Some(Self::Command),
            "injected-command" => Some(Self::InjectedCommand),
            "type-module" => Some(Self::TypeModule),
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

fn injected(argument: &FnArg) -> bool {
    let FnArg::Typed(typed) = argument else {
        return false;
    };
    let Type::Path(path) = typed.ty.as_ref() else {
        return false;
    };
    path.path
        .segments
        .last()
        .is_some_and(|segment| INJECTED_TYPES.contains(&segment.ident.to_string().as_str()))
}

pub(crate) fn function_sites(function: &ItemFn) -> BTreeSet<Site> {
    let mut sites = BTreeSet::new();
    let mut finder = CollectFinder::default();
    finder.visit_block(&function.block);
    if finder.found.is_some() {
        sites.insert(Site::CommandCollector);
    }
    if is_command(&function.attrs) {
        sites.insert(Site::Command);
        if function.sig.inputs.iter().all(injected) {
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

fn derives_type(attributes: &[Attribute]) -> bool {
    attributes.iter().any(|attribute| {
        let Meta::List(list) = &attribute.meta else {
            return false;
        };
        list.path.is_ident("derive")
            && list
                .parse_args_with(
                    syn::punctuated::Punctuated::<syn::Path, syn::Token![,]>::parse_terminated,
                )
                .is_ok_and(|paths| {
                    paths.iter().any(|derived| {
                        derived
                            .segments
                            .last()
                            .is_some_and(|segment| segment.ident == TYPE_DERIVE)
                    })
                })
    })
}

fn type_only(item: &Item) -> bool {
    if let Item::Use(_) = item {
        true
    } else if let Item::Struct(structure) = item {
        derives_type(&structure.attrs)
    } else if let Item::Enum(enumeration) = item {
        derives_type(&enumeration.attrs)
    } else {
        false
    }
}

fn module_file(root: &Path, declaring: &str, name: &str) -> PathBuf {
    let declared = Path::new(declaring);
    let is_root = declared
        .file_name()
        .and_then(|file| file.to_str())
        .is_some_and(|file| ROOT_FILES.contains(&file));
    let directory = if is_root {
        declared
            .parent()
            .map_or_else(PathBuf::new, Path::to_path_buf)
    } else {
        declared.with_extension("")
    };
    root.join(directory).join(format!("{name}.rs"))
}

pub(crate) fn module_sites(root: &Path, declaring: &str, name: &str) -> BTreeSet<Site> {
    let only_types = fs::read_to_string(module_file(root, declaring, name))
        .ok()
        .and_then(|source| syn::parse_file(&source).ok())
        .is_some_and(|file| file.attrs.is_empty() && file.items.iter().all(type_only));
    if only_types {
        BTreeSet::from([Site::TypeModule])
    } else {
        BTreeSet::new()
    }
}
