use std::collections::{BTreeMap, BTreeSet};

use super::macro_tokens;
use proc_macro2::TokenStream;
use syn::visit::Visit;

pub(super) type Sources = BTreeMap<String, String>;
type Names = BTreeMap<String, BTreeSet<String>>;

#[derive(Default)]
pub(super) struct Graph {
    edges: BTreeMap<String, BTreeSet<String>>,
    uncertain: BTreeSet<String>,
}

fn definition(item: &syn::Item) -> Option<String> {
    match item {
        syn::Item::Const(value) => Some(crate::rust_names::ident(&value.ident)),
        syn::Item::Enum(value) => Some(crate::rust_names::ident(&value.ident)),
        syn::Item::Fn(value) => Some(crate::rust_names::ident(&value.sig.ident)),
        syn::Item::Static(value) => Some(crate::rust_names::ident(&value.ident)),
        syn::Item::Struct(value) => Some(crate::rust_names::ident(&value.ident)),
        syn::Item::Trait(value) => Some(crate::rust_names::ident(&value.ident)),
        syn::Item::TraitAlias(value) => Some(crate::rust_names::ident(&value.ident)),
        syn::Item::Type(value) => Some(crate::rust_names::ident(&value.ident)),
        syn::Item::Union(value) => Some(crate::rust_names::ident(&value.ident)),
        syn::Item::ExternCrate(_)
        | syn::Item::ForeignMod(_)
        | syn::Item::Impl(_)
        | syn::Item::Macro(_)
        | syn::Item::Mod(_)
        | syn::Item::Use(_)
        | syn::Item::Verbatim(_)
        | _ => None,
    }
}

#[derive(Default)]
struct Imports {
    aliases: Vec<(String, BTreeSet<String>)>,
    prefix: Vec<String>,
    uncertain: bool,
}

impl Imports {
    fn tree(&mut self, tree: &syn::UseTree) {
        match tree {
            syn::UseTree::Path(path) => {
                self.prefix.push(crate::rust_names::ident(&path.ident));
                self.tree(&path.tree);
                self.prefix.pop();
            }
            syn::UseTree::Group(group) => {
                for child in &group.items {
                    self.tree(child);
                }
            }
            syn::UseTree::Name(name) => {
                let mut names: BTreeSet<String> = self.prefix.iter().cloned().collect();
                names.insert(crate::rust_names::ident(&name.ident));
                self.aliases
                    .push((crate::rust_names::ident(&name.ident), names));
            }
            syn::UseTree::Rename(rename) => {
                let mut names: BTreeSet<String> = self.prefix.iter().cloned().collect();
                names.insert(crate::rust_names::ident(&rename.ident));
                self.aliases
                    .push((crate::rust_names::ident(&rename.rename), names));
            }
            syn::UseTree::Glob(_) => {
                self.uncertain = true;
            }
        }
    }
}

#[derive(Default)]
struct Uncertainty {
    found: bool,
    structural: bool,
}

impl<'ast> Visit<'ast> for Uncertainty {
    fn visit_item_mod(&mut self, i: &'ast syn::ItemMod) {
        if i.content.is_none() && !i.attrs.is_empty() {
            self.structural = true;
        }
        syn::visit::visit_item_mod(self, i);
    }
    fn visit_item_use(&mut self, i: &'ast syn::ItemUse) {
        let mut imports = Imports::default();
        imports.tree(&i.tree);
        let exported = match i.vis {
            syn::Visibility::Public(_) | syn::Visibility::Restricted(_) => true,
            syn::Visibility::Inherited => false,
        };
        if imports.uncertain && exported {
            self.structural = true;
        }
        syn::visit::visit_item_use(self, i);
    }

    fn visit_type_trait_object(&mut self, i: &'ast syn::TypeTraitObject) {
        self.structural = true;
        syn::visit::visit_type_trait_object(self, i);
    }
    fn visit_item_impl(&mut self, i: &'ast syn::ItemImpl) {
        self.structural |= blanket_impl(i);
        syn::visit::visit_item_impl(self, i);
    }
    fn visit_attribute(&mut self, i: &'ast syn::Attribute) {
        if crate::rust_names::path_is(i.path(), "path") {
            self.structural = true;
        } else if crate::rust_names::path_is(i.path(), "proptest_config")
            || !macro_tokens::attribute_known(&i.meta)
        {
            self.found = true;
        }
        syn::visit::visit_attribute(self, i);
    }
}

fn blanket_impl(implementation: &syn::ItemImpl) -> bool {
    if implementation.trait_.is_none() {
        return false;
    }
    let syn::Type::Path(target) = implementation.self_ty.as_ref() else {
        return false;
    };
    let Some(first) = target.path.segments.first() else {
        return true;
    };
    let name = crate::rust_names::ident(&first.ident);
    implementation
        .generics
        .type_params()
        .any(|parameter| crate::rust_names::ident(&parameter.ident) == name)
}

fn impl_owners(item: &syn::Item, path: &str, names: &mut Names) -> Result<(), String> {
    if let syn::Item::Impl(implementation) = item {
        let syn::Type::Path(target) = implementation.self_ty.as_ref() else {
            return Err(String::from("complex impl target"));
        };
        if let Some((_, trait_path, _)) = &implementation.trait_ {
            for segment in &trait_path.segments {
                names
                    .entry(crate::rust_names::ident(&segment.ident))
                    .or_default()
                    .insert(String::from(path));
            }
        }
        for segment in &target.path.segments {
            names
                .entry(crate::rust_names::ident(&segment.ident))
                .or_default()
                .insert(String::from(path));
        }
    }
    Ok(())
}

fn item_names(items: &[syn::Item], path: &str, names: &mut Names) -> Result<(), String> {
    for item in items {
        impl_owners(item, path, names)?;
        if let Some(name) = definition(item) {
            names.entry(name).or_default().insert(String::from(path));
        }
        if let syn::Item::Mod(module) = item
            && let Some((_, children)) = &module.content
        {
            item_names(children, path, names)?;
        }
        if let syn::Item::Verbatim(_) = item {
            return Err(String::from("unsupported item"));
        }
    }
    Ok(())
}

fn names_for(sources: &Sources) -> Result<(Names, BTreeMap<String, syn::File>), String> {
    let mut names = Names::new();
    let mut parsed = BTreeMap::new();
    for (path, source) in sources {
        if path.ends_with("/mod.rs") {
            return Err(String::from("mod.rs module"));
        }
        let file = syn::parse_file(source).map_err(|error| error.to_string())?;
        if macro_tokens::identifiers(
            source
                .parse()
                .map_err(|error: proc_macro2::LexError| error.to_string())?,
        )
        .iter()
        .any(|name| !name.is_ascii())
        {
            return Err(String::from("non-ASCII identifier normalization"));
        }
        let stem = std::path::Path::new(path)
            .file_stem()
            .and_then(|name| name.to_str())
            .ok_or("module name")?;
        names
            .entry(crate::rust_names::normalize(stem))
            .or_default()
            .insert(path.clone());
        item_names(&file.items, path, &mut names)?;
        parsed.insert(path.clone(), file);
    }
    Ok((names, parsed))
}

fn resolve_aliases(names: &mut Names, aliases: &[(String, BTreeSet<String>)]) {
    loop {
        let before = names.clone();
        for (alias, identifiers) in aliases {
            let owners: BTreeSet<String> = identifiers
                .iter()
                .filter_map(|name| before.get(name))
                .flatten()
                .cloned()
                .collect();
            names.entry(alias.clone()).or_default().extend(owners);
        }
        if *names == before {
            break;
        }
    }
}

struct RootedPaths<'a> {
    names: &'a Names,
    unresolved: bool,
}

impl<'ast> Visit<'ast> for RootedPaths<'_> {
    fn visit_path(&mut self, i: &'ast syn::Path) {
        let rooted = i.segments.first().is_some_and(|segment| {
            ["crate", "self", "super"]
                .iter()
                .any(|prefix| crate::rust_names::ident(&segment.ident) == *prefix)
        });
        if rooted
            && i.segments.len() > 1
            && !i.segments.iter().any(|segment| {
                self.names
                    .get(&crate::rust_names::ident(&segment.ident))
                    .is_some_and(|owners| !owners.is_empty())
            })
        {
            self.unresolved = true;
        }
        syn::visit::visit_path(self, i);
    }
}

fn collect_imports(file: &syn::File, imports: &mut Imports) {
    for item in &file.items {
        if let syn::Item::Use(import) = item
            && !contains_glob(&import.tree)
        {
            imports.tree(&import.tree);
        }
    }
}

fn contains_glob(tree: &syn::UseTree) -> bool {
    match tree {
        syn::UseTree::Glob(_) => true,
        syn::UseTree::Path(path) => contains_glob(&path.tree),
        syn::UseTree::Group(group) => group.items.iter().any(contains_glob),
        syn::UseTree::Name(_) | syn::UseTree::Rename(_) => false,
    }
}

fn validate_aliases(names: &Names, imports: &Imports) -> Result<(), String> {
    if imports.aliases.iter().any(|(alias, identifiers)| {
        identifiers
            .iter()
            .any(|name| ["crate", "self", "super"].contains(&name.as_str()))
            && names.get(alias).is_none_or(BTreeSet::is_empty)
    }) {
        return Err(String::from("unresolved rooted import"));
    }
    Ok(())
}

impl Graph {
    pub(super) fn read(sources: &Sources) -> Result<Self, String> {
        let (mut names, parsed) = names_for(sources)?;
        let mut imports = Imports::default();
        let mut graph = Self::default();
        for (path, file) in &parsed {
            collect_imports(file, &mut imports);
            let mut uncertainty = Uncertainty::default();
            uncertainty.visit_file(file);
            if uncertainty.structural {
                return Err(String::from("unresolved module/import"));
            }
            if uncertainty.found {
                graph.uncertain.insert(path.clone());
            }
        }
        if imports.uncertain || !graph.uncertain.is_empty() {
            return Err(String::from("glob or macro may hide an incoming reference"));
        }
        let macros = macro_tokens::Macros::read(&parsed, sources)?;
        for (name, owners) in macros.owners() {
            names.entry(name).or_default().extend(owners);
        }
        resolve_aliases(&mut names, &imports.aliases);
        validate_aliases(&names, &imports)?;

        for (path, source) in sources {
            let mut paths = RootedPaths {
                names: &names,
                unresolved: false,
            };
            paths.visit_file(parsed.get(path).ok_or("parsed file")?);
            if paths.unresolved {
                graph.uncertain.insert(path.clone());
            }
            let ids = macros.expanded_ids(
                source
                    .parse::<TokenStream>()
                    .map_err(|error| error.to_string())?,
            );
            let mut edges: BTreeSet<String> = ids
                .iter()
                .filter_map(|name| names.get(name))
                .flatten()
                .cloned()
                .collect();
            edges.remove(path);
            graph.edges.insert(path.clone(), edges);
        }
        Ok(graph)
    }

    pub(super) fn extend(&mut self, other: Self) {
        self.uncertain.extend(other.uncertain);
        for (path, edges) in other.edges {
            self.edges.entry(path).or_default().extend(edges);
        }
    }

    pub(super) fn outward(&self, seeds: &BTreeSet<String>) -> Result<BTreeSet<String>, String> {
        let mut closure = seeds.clone();
        loop {
            if !closure.is_disjoint(&self.uncertain) {
                return Err(String::from("unresolved macro"));
            }
            let before = closure.clone();
            for path in &before {
                closure.extend(
                    self.edges
                        .get(path)
                        .ok_or("missing graph node")?
                        .iter()
                        .cloned(),
                );
            }
            if before == closure {
                break;
            }
        }
        Ok(closure)
    }

    pub(super) fn selected(&self, seeds: &BTreeSet<String>) -> Result<BTreeSet<String>, String> {
        let mut closure = self.outward(seeds)?;
        let incoming = self
            .edges
            .iter()
            .filter(|(_, edges)| !edges.is_disjoint(seeds))
            .map(|(path, _)| path.clone());
        closure.extend(incoming);
        Ok(closure)
    }
}
