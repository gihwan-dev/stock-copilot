use super::graph::Sources;
use proc_macro2::{Ident, TokenStream};
use std::collections::{BTreeMap, BTreeSet};
use syn::visit::Visit;

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
    origin: String,
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
            prefix.push(crate::rust_names::ident(&path.ident));
            flatten(&path.tree, prefix, root_modules, lib, found)?;
            prefix.pop();
        }
        syn::UseTree::Group(group) => {
            for child in &group.items {
                flatten(child, prefix, root_modules, lib, found)?;
            }
        }
        syn::UseTree::Name(name) => binding(
            (
                crate::rust_names::ident(&name.ident),
                crate::rust_names::ident(&name.ident),
            ),
            prefix,
            root_modules,
            lib,
            found,
        ),
        syn::UseTree::Rename(name) => binding(
            (
                crate::rust_names::ident(&name.rename),
                crate::rust_names::ident(&name.ident),
            ),
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
    let origin = prefix.first().cloned().unwrap_or_else(|| original.clone());
    found.push(Import {
        bound,
        original,
        internal,
        origin,
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
                Some(crate::rust_names::ident(&module.ident))
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

pub(super) fn stream(item: &syn::Item) -> TokenStream {
    syn::parse_quote!(#item)
}

pub(super) fn tokens(item: &syn::Item) -> String {
    stream(item).to_string()
}

#[derive(Default)]
pub(super) struct Symbols {
    kinds: BTreeMap<String, BTreeSet<Kind>>,
    globbed: bool,
    reserved: BTreeSet<String>,
}

struct Collector<'a> {
    symbols: &'a mut Symbols,
    pending: Vec<Import>,
    root_modules: BTreeSet<String>,
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
                .entry(crate::rust_names::ident(name))
                .or_default()
                .insert(kind);
        }
        syn::visit::visit_item(self, i);
    }
    fn visit_item_mod(&mut self, i: &'ast syn::ItemMod) {
        self.symbols
            .kinds
            .entry(crate::rust_names::ident(&i.ident))
            .or_default()
            .insert(Kind::Data);
        self.depth = self.depth.saturating_add(1);
        syn::visit::visit_item_mod(self, i);
        self.depth = self.depth.saturating_sub(1);
    }
    fn visit_path(&mut self, i: &'ast syn::Path) {
        if i.segments.len() > 1
            && let Some(first) = i.segments.first()
        {
            let name = crate::rust_names::ident(&first.ident);
            if !["crate", "self", "super"].contains(&name.as_str()) {
                self.symbols.reserved.insert(name);
            }
        }
        syn::visit::visit_path(self, i);
    }
    fn visit_item_extern_crate(&mut self, i: &'ast syn::ItemExternCrate) {
        self.symbols
            .reserved
            .insert(crate::rust_names::ident(&i.ident));
        if let Some((_, alias)) = &i.rename {
            self.symbols
                .reserved
                .insert(crate::rust_names::ident(alias));
        }
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
            if self.depth == 0 {
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
        for source in sources.values() {
            let file = syn::parse_file(source).ok()?;
            let mut collector = Collector {
                symbols: &mut symbols,
                pending: Vec::new(),
                root_modules: modules(&file),
                depth: 0,
            };
            collector.visit_file(&file);
            pending.extend(collector.pending);
        }
        for import in &pending {
            if !import.internal {
                symbols.reserved.extend([
                    import.origin.clone(),
                    import.original.clone(),
                    import.bound.clone(),
                ]);
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
    pub(super) fn reserved(&self) -> BTreeSet<String> {
        let mut names = crate::rust_names::prelude();
        names.extend(self.reserved.iter().cloned());
        names
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
