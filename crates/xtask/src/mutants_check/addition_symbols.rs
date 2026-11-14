use super::graph::Sources;
use proc_macro2::{Ident, TokenStream};
use std::collections::{BTreeMap, BTreeSet};
use syn::{spanned::Spanned, visit::Visit};

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Kind {
    Data,
    Trait,
    Opaque,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Location {
    Root,
    Nested,
}

#[derive(Clone)]
pub(super) struct Import {
    pub(super) bound: String,
    pub(super) original: String,
    pub(super) internal: bool,
}

fn flatten(
    tree: &syn::UseTree,
    prefix: &mut Vec<String>,
    root_modules: &BTreeSet<String>,
    lib: Location,
    found: &mut Vec<Import>,
) -> Option<()> {
    match tree {
        syn::UseTree::Path(path) => {
            prefix.push(path.ident.to_string());
            flatten(&path.tree, prefix, root_modules, lib, found)?;
            prefix.pop();
        }
        syn::UseTree::Group(group) => {
            for child in &group.items {
                flatten(child, prefix, root_modules, lib, found)?;
            }
        }
        syn::UseTree::Name(name) => binding(
            (name.ident.to_string(), name.ident.to_string()),
            prefix,
            root_modules,
            lib,
            found,
        ),
        syn::UseTree::Rename(name) => binding(
            (name.rename.to_string(), name.ident.to_string()),
            prefix,
            root_modules,
            lib,
            found,
        ),
        syn::UseTree::Glob(_) => return None,
    }
    Some(())
}

fn binding(
    (bound, original): (String, String),
    prefix: &[String],
    modules: &BTreeSet<String>,
    lib: Location,
    found: &mut Vec<Import>,
) {
    let internal = prefix.first().is_some_and(|root| {
        ["crate", "self", "super"].contains(&root.as_str())
            || (lib == Location::Root && modules.contains(root))
    });
    found.push(Import {
        bound,
        original,
        internal,
    });
}

pub(super) fn imports(
    item: &syn::ItemUse,
    modules: &BTreeSet<String>,
    lib: Location,
) -> Option<Vec<Import>> {
    let mut found = Vec::new();
    flatten(&item.tree, &mut Vec::new(), modules, lib, &mut found)?;
    Some(found)
}

pub(super) fn modules(file: &syn::File) -> BTreeSet<String> {
    file.items
        .iter()
        .filter_map(|item| {
            if let syn::Item::Mod(module) = item {
                Some(module.ident.to_string())
            } else {
                None
            }
        })
        .collect()
}

pub(super) fn named(item: &syn::Item) -> Option<&Ident> {
    match item {
        syn::Item::Fn(value) => Some(&value.sig.ident),
        syn::Item::Const(value) => Some(&value.ident),
        syn::Item::Static(value) => Some(&value.ident),
        syn::Item::Struct(value) => Some(&value.ident),
        syn::Item::Enum(value) => Some(&value.ident),
        syn::Item::Type(value) => Some(&value.ident),
        syn::Item::Trait(value) => Some(&value.ident),
        syn::Item::ExternCrate(_)
        | syn::Item::ForeignMod(_)
        | syn::Item::Impl(_)
        | syn::Item::Macro(_)
        | syn::Item::Mod(_)
        | syn::Item::Use(_)
        | syn::Item::Verbatim(_)
        | syn::Item::Union(_)
        | syn::Item::TraitAlias(_)
        | _ => None,
    }
}

pub(super) fn tokens(item: &impl Spanned, source: &str) -> Option<String> {
    let span = item.span();
    slice_tokens(source, span.start(), span.end())
}

pub(super) fn slice_tokens(
    source: &str,
    start: proc_macro2::LineColumn,
    end: proc_macro2::LineColumn,
) -> Option<String> {
    let offset = |position: proc_macro2::LineColumn| {
        source
            .split_inclusive('\n')
            .take(position.line.saturating_sub(1))
            .map(str::len)
            .sum::<usize>()
            .checked_add(position.column)
    };
    source
        .get(offset(start)?..offset(end)?)?
        .parse::<TokenStream>()
        .ok()
        .map(|stream| stream.to_string())
}

#[derive(Default)]
pub(super) struct Symbols {
    kinds: BTreeMap<String, BTreeSet<Kind>>,
    globbed: bool,
}

struct Collector<'a> {
    symbols: &'a mut Symbols,
    pending: Vec<Import>,
    root_modules: BTreeSet<String>,
    lib: bool,
    depth: usize,
}

impl<'ast> Visit<'ast> for Collector<'_> {
    fn visit_item(&mut self, i: &'ast syn::Item) {
        if let Some(name) = named(i) {
            let kind = if let syn::Item::Trait(_) = i {
                Kind::Trait
            } else {
                Kind::Data
            };
            self.symbols
                .kinds
                .entry(name.to_string())
                .or_default()
                .insert(kind);
        }
        syn::visit::visit_item(self, i);
    }
    fn visit_item_mod(&mut self, i: &'ast syn::ItemMod) {
        self.symbols
            .kinds
            .entry(i.ident.to_string())
            .or_default()
            .insert(Kind::Data);
        self.depth = self.depth.saturating_add(1);
        syn::visit::visit_item_mod(self, i);
        self.depth = self.depth.saturating_sub(1);
    }
    fn visit_macro(&mut self, i: &'ast syn::Macro) {
        if super::macro_tokens::identifiers(i.tokens.clone()).contains("use") {
            self.symbols.globbed = true;
        }
    }
    fn visit_item_use(&mut self, i: &'ast syn::ItemUse) {
        if let Some(found) = imports(
            i,
            &self.root_modules,
            if self.lib && self.depth == 0 {
                Location::Root
            } else {
                Location::Nested
            },
        ) {
            self.pending.extend(found);
        } else {
            self.symbols.globbed = true;
        }
    }
}

impl Symbols {
    pub(super) fn read(sources: &Sources) -> Option<Self> {
        let mut symbols = Self::default();
        let mut pending = Vec::new();
        for (path, source) in sources {
            let file = syn::parse_file(source).ok()?;
            let mut collector = Collector {
                symbols: &mut symbols,
                pending: Vec::new(),
                root_modules: modules(&file),
                lib: path.ends_with("/lib.rs"),
                depth: 0,
            };
            collector.visit_file(&file);
            pending.extend(collector.pending);
        }
        for import in &pending {
            if !import.internal {
                symbols
                    .kinds
                    .entry(import.bound.clone())
                    .or_default()
                    .insert(Kind::Opaque);
            }
        }
        loop {
            let before = symbols.kinds.clone();
            symbols.propagate(&pending, &before);
            if before == symbols.kinds {
                break;
            }
        }
        for import in pending {
            if !symbols.kinds.contains_key(&import.bound) {
                symbols
                    .kinds
                    .entry(import.bound)
                    .or_default()
                    .insert(Kind::Opaque);
            }
        }
        Some(symbols)
    }
    fn propagate(&mut self, pending: &[Import], before: &BTreeMap<String, BTreeSet<Kind>>) {
        for import in pending {
            if !import.internal {
                continue;
            }
            if let Some(kinds) = before.get(&import.original) {
                self.kinds
                    .entry(import.bound.clone())
                    .or_default()
                    .extend(kinds);
            }
        }
    }
    pub(super) fn has_globs(&self) -> bool {
        self.globbed
    }
    pub(super) fn data(&self, name: &str) -> bool {
        self.kinds
            .get(name)
            .is_some_and(|kinds| *kinds == BTreeSet::from([Kind::Data]))
    }
}
