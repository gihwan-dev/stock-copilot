use proc_macro2::{TokenStream, TokenTree};
use quote::ToTokens;
use std::collections::{BTreeMap, BTreeSet};
use syn::ext::IdentExt;
use syn::visit::{self, Visit};

#[derive(Default)]
struct Bindings {
    edges: BTreeMap<String, BTreeSet<String>>,
}

fn names(value: &impl ToTokens) -> BTreeSet<String> {
    token_names(value.to_token_stream())
}
fn token_names(stream: TokenStream) -> BTreeSet<String> {
    let mut output = BTreeSet::new();
    for token in stream {
        match token {
            TokenTree::Ident(ident) => {
                output.insert(ident.unraw().to_string());
            }
            TokenTree::Group(group) => output.extend(token_names(group.stream())),
            TokenTree::Literal(_) | TokenTree::Punct(_) => {}
        }
    }
    output
}

fn imports(tree: &syn::UseTree, prefix: &[String], output: &mut Bindings) {
    match tree {
        syn::UseTree::Path(path) => {
            let mut nested = prefix.to_vec();
            nested.push(path.ident.unraw().to_string());
            imports(&path.tree, &nested, output);
        }
        syn::UseTree::Group(group) => {
            for item in &group.items {
                imports(item, prefix, output);
            }
        }
        syn::UseTree::Name(name) => {
            let mut path = prefix.to_vec();
            path.push(name.ident.unraw().to_string());
            output
                .edges
                .entry(name.ident.unraw().to_string())
                .or_default()
                .extend(path);
        }
        syn::UseTree::Rename(rename) => {
            let mut path = prefix.to_vec();
            path.push(rename.ident.unraw().to_string());
            output
                .edges
                .entry(rename.rename.unraw().to_string())
                .or_default()
                .extend(path);
        }
        syn::UseTree::Glob(_) => {}
    }
}

impl<'ast> Visit<'ast> for Bindings {
    fn visit_block(&mut self, _: &'ast syn::Block) {}

    fn visit_item_use(&mut self, i: &'ast syn::ItemUse) {
        imports(&i.tree, &[], self);
    }
    fn visit_item_type(&mut self, i: &'ast syn::ItemType) {
        self.edges
            .entry(i.ident.unraw().to_string())
            .or_default()
            .extend(names(&i.ty));
        visit::visit_item_type(self, i);
    }
    fn visit_impl_item_type(&mut self, i: &'ast syn::ImplItemType) {
        self.edges
            .entry(i.ident.unraw().to_string())
            .or_default()
            .extend(names(&i.ty));
    }
    fn visit_item_extern_crate(&mut self, i: &'ast syn::ItemExternCrate) {
        if let Some((_, alias)) = &i.rename {
            self.edges
                .entry(alias.unraw().to_string())
                .or_default()
                .insert(i.ident.unraw().to_string());
        }
    }
}

fn tainted(files: &[syn::File], roots: &BTreeSet<String>) -> BTreeSet<String> {
    let mut bindings = Bindings::default();
    for file in files {
        bindings.visit_file(file);
    }
    let mut marked = roots.clone();
    marked.insert(String::from("broker_toss_schema"));
    loop {
        let added: Vec<String> = bindings
            .edges
            .iter()
            .filter(|(name, dependencies)| {
                !marked.contains(*name) && !dependencies.is_disjoint(&marked)
            })
            .map(|(name, _)| name.clone())
            .collect();
        if added.is_empty() {
            return marked;
        }
        marked.extend(added);
    }
}

fn public(visibility: &syn::Visibility) -> bool {
    if let syn::Visibility::Public(_) = visibility {
        return true;
    }
    false
}

struct Surface<'names> {
    marked: &'names BTreeSet<String>,
    found: bool,
}
impl Surface<'_> {
    fn inspect(&mut self, value: &impl ToTokens) {
        self.found |= !names(value).is_disjoint(self.marked);
    }
    fn inspect_default(&mut self, value: Option<&(syn::Token![=], syn::Type)>) {
        if let Some((_, ty)) = value {
            self.inspect(ty);
        }
    }
    fn structure(&mut self, fields: &syn::Fields) {
        for field in fields {
            if public(&field.vis) {
                self.inspect(&field.ty);
            }
        }
    }
}
impl<'ast> Visit<'ast> for Surface<'_> {
    fn visit_item_const(&mut self, i: &'ast syn::ItemConst) {
        if public(&i.vis) {
            self.inspect(&i.ty);
        }
    }
    fn visit_item_static(&mut self, i: &'ast syn::ItemStatic) {
        if public(&i.vis) {
            self.inspect(&i.ty);
        }
    }
    fn visit_item_extern_crate(&mut self, i: &'ast syn::ItemExternCrate) {
        if public(&i.vis) {
            self.inspect(i);
        }
    }

    fn visit_item_use(&mut self, i: &'ast syn::ItemUse) {
        if public(&i.vis) {
            self.inspect(&i.tree);
        }
    }
    fn visit_item_fn(&mut self, i: &'ast syn::ItemFn) {
        if public(&i.vis) {
            self.inspect(&i.sig);
        }
    }
    fn visit_item_type(&mut self, i: &'ast syn::ItemType) {
        if public(&i.vis) {
            self.inspect(&i.ty);
            self.inspect(&i.generics);
        }
    }
    fn visit_item_struct(&mut self, i: &'ast syn::ItemStruct) {
        if public(&i.vis) {
            self.inspect(&i.generics);
            self.structure(&i.fields);
        }
    }
    fn visit_item_union(&mut self, i: &'ast syn::ItemUnion) {
        if !public(&i.vis) {
            return;
        }
        for field in i.fields.named.iter().filter(|field| public(&field.vis)) {
            self.inspect(&field.ty);
        }
    }
    fn visit_item_enum(&mut self, i: &'ast syn::ItemEnum) {
        if public(&i.vis) {
            self.inspect(&i.generics);
            for variant in &i.variants {
                self.inspect(&variant.fields);
            }
        }
    }
    fn visit_item_trait(&mut self, i: &'ast syn::ItemTrait) {
        if !public(&i.vis) {
            return;
        }
        self.inspect(&i.generics);
        self.inspect(&i.supertraits);
        for item in &i.items {
            if let syn::TraitItem::Fn(function) = item {
                self.inspect(&function.sig);
            }
            if let syn::TraitItem::Type(alias) = item {
                self.inspect(&alias.bounds);
                self.inspect_default(alias.default.as_ref());
            }
            if let syn::TraitItem::Const(constant) = item {
                self.inspect(&constant.ty);
            }
        }
    }
    fn visit_item_impl(&mut self, i: &'ast syn::ItemImpl) {
        for item in &i.items {
            if let syn::ImplItem::Fn(function) = item
                && (public(&function.vis) || i.trait_.is_some())
            {
                self.inspect(&function.sig);
            }
            if let syn::ImplItem::Const(constant) = item
                && (public(&constant.vis) || i.trait_.is_some())
            {
                self.inspect(&constant.ty);
            }
            if let syn::ImplItem::Type(alias) = item
                && (public(&alias.vis) || i.trait_.is_some())
            {
                self.inspect(&alias.ty);
            }
        }
    }
}

pub(super) fn check(files: &[(String, syn::File)], roots: &BTreeSet<String>) -> Vec<String> {
    let parsed: Vec<syn::File> = files.iter().map(|(_, file)| file.clone()).collect();
    let marked = tainted(&parsed, roots);
    files
        .iter()
        .filter_map(|(path, file)| {
            let mut surface = Surface {
                marked: &marked,
                found: false,
            };
            surface.visit_file(file);
            surface.found.then(|| path.clone())
        })
        .collect()
}
