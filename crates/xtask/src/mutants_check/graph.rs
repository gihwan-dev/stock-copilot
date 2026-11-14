use std::collections::{BTreeMap, BTreeSet};

use proc_macro2::{TokenStream, TokenTree};
use syn::visit::Visit;

pub(super) type Sources = BTreeMap<String, String>;
type Names = BTreeMap<String, BTreeSet<String>>;

#[derive(Default)]
pub(super) struct Graph {
    edges: BTreeMap<String, BTreeSet<String>>,
    uncertain: BTreeSet<String>,
}

fn identifiers(tokens: TokenStream) -> BTreeSet<String> {
    tokens
        .into_iter()
        .flat_map(|token| match token {
            TokenTree::Ident(name) => BTreeSet::from([name.to_string()]),
            TokenTree::Group(group) => identifiers(group.stream()),
            TokenTree::Literal(_) | TokenTree::Punct(_) => BTreeSet::new(),
        })
        .collect()
}

fn definition(item: &syn::Item) -> Option<String> {
    match item {
        syn::Item::Const(value) => Some(value.ident.to_string()),
        syn::Item::Enum(value) => Some(value.ident.to_string()),
        syn::Item::Fn(value) => Some(value.sig.ident.to_string()),
        syn::Item::Static(value) => Some(value.ident.to_string()),
        syn::Item::Struct(value) => Some(value.ident.to_string()),
        syn::Item::Trait(value) => Some(value.ident.to_string()),
        syn::Item::TraitAlias(value) => Some(value.ident.to_string()),
        syn::Item::Type(value) => Some(value.ident.to_string()),
        syn::Item::Union(value) => Some(value.ident.to_string()),
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
                self.prefix.push(path.ident.to_string());
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
                names.insert(name.ident.to_string());
                self.aliases.push((name.ident.to_string(), names));
            }
            syn::UseTree::Rename(rename) => {
                let mut names: BTreeSet<String> = self.prefix.iter().cloned().collect();
                names.insert(rename.ident.to_string());
                self.aliases.push((rename.rename.to_string(), names));
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
        if imports.uncertain {
            self.structural = true;
        }
        syn::visit::visit_item_use(self, i);
    }

    fn visit_macro(&mut self, _i: &'ast syn::Macro) {
        self.found = true;
    }
    fn visit_attribute(&mut self, i: &'ast syn::Attribute) {
        if i.path().is_ident("path") {
            self.structural = true;
        } else if !["cfg", "test", "repr"]
            .iter()
            .any(|name| i.path().is_ident(name))
        {
            self.found = true;
        }
        syn::visit::visit_attribute(self, i);
    }
}

fn impl_owners(item: &syn::Item, path: &str, names: &mut Names) -> Result<(), String> {
    if let syn::Item::Impl(implementation) = item {
        let syn::Type::Path(target) = implementation.self_ty.as_ref() else {
            return Err(String::from("complex impl target"));
        };
        for segment in &target.path.segments {
            names
                .entry(segment.ident.to_string())
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
        let stem = std::path::Path::new(path)
            .file_stem()
            .and_then(|name| name.to_str())
            .ok_or("module name")?;
        names
            .entry(String::from(stem))
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
                .any(|prefix| segment.ident == *prefix)
        });
        if rooted
            && i.segments.len() > 1
            && !i.segments.iter().any(|segment| {
                self.names
                    .get(&segment.ident.to_string())
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
        if let syn::Item::Use(import) = item {
            imports.tree(&import.tree);
        }
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
            let ids = identifiers(
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
            if ids.contains("super") {
                let parent = std::path::Path::new(path)
                    .parent()
                    .and_then(|folder| folder.to_str())
                    .ok_or("parent module")?;
                edges.extend(
                    sources
                        .keys()
                        .filter(|candidate| candidate.starts_with(parent))
                        .cloned(),
                );
            }
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

    pub(super) fn selected(&self, seeds: &BTreeSet<String>) -> Result<BTreeSet<String>, String> {
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
        let incoming = self
            .edges
            .iter()
            .filter(|(_, edges)| !edges.is_disjoint(seeds))
            .map(|(path, _)| path.clone());
        closure.extend(incoming);
        Ok(closure)
    }
}
