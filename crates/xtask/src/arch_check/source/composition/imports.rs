use super::ADAPTERS;
use std::collections::{BTreeMap, BTreeSet};
use syn::ext::IdentExt;
use syn::visit::{self, Visit};
use syn::{File, ItemExternCrate, ItemUse, UseTree, Visibility};

#[derive(Default)]
pub(super) struct Bindings {
    imported: BTreeMap<String, Vec<Vec<String>>>,
    restricted: Vec<Vec<String>>,
    pub(super) prohibited: bool,
}
impl Bindings {
    pub(super) fn of(file: &File, aliases: &BTreeMap<String, Vec<String>>) -> Self {
        let mut bindings = Self {
            imported: aliases
                .iter()
                .map(|(name, path)| (name.clone(), vec![path.clone()]))
                .collect(),
            restricted: Vec::new(),
            prohibited: false,
        };
        bindings.visit_file(file);
        bindings.prohibited = bindings
            .restricted
            .iter()
            .any(|path| bindings.adapter(path));
        bindings
    }
    pub(super) fn adapter(&self, path: &[String]) -> bool {
        let mut pending = vec![(normalized(path), BTreeSet::new())];
        while let Some((resolved, mut visited)) = pending.pop() {
            let Some(first) = resolved.first() else {
                continue;
            };
            let adapter_root = ADAPTERS.contains(&first.as_str());
            let path_value =
                resolved.starts_with(&[String::from("store_sqlite"), String::from("DatabasePath")]);
            if adapter_root && !path_value {
                return true;
            }
            if adapter_root || !visited.insert(first.clone()) {
                continue;
            }
            if resolved.len() > 1 {
                pending.push((resolved.iter().skip(1).cloned().collect(), visited.clone()));
            }
            let Some(alternatives) = self.imported.get(first) else {
                continue;
            };
            for prefix in alternatives {
                let mut candidate = normalized(prefix);
                candidate.extend(resolved.iter().skip(1).cloned());
                pending.push((candidate, visited.clone()));
            }
        }
        false
    }
    pub(super) fn inherit(&mut self, other: &Self) {
        for (name, paths) in &other.imported {
            let entry = self.imported.entry(name.clone()).or_default();
            entry.extend(paths.iter().cloned());
            entry.sort();
            entry.dedup();
        }
        self.prohibited = self.restricted.iter().any(|path| self.adapter(path));
    }
    fn collect(&mut self, tree: &UseTree, prefix: &[String], visibility: &Visibility) {
        match tree {
            UseTree::Path(path) => {
                let mut extended = prefix.to_vec();
                extended.push(path.ident.unraw().to_string());
                self.collect(&path.tree, &extended, visibility);
            }
            UseTree::Name(name) => {
                let mut resolved = prefix.to_vec();
                resolved.push(name.ident.unraw().to_string());
                self.insert(name.ident.unraw().to_string(), resolved, visibility);
            }
            UseTree::Rename(rename) => {
                let mut resolved = prefix.to_vec();
                resolved.push(rename.ident.unraw().to_string());
                self.insert(rename.rename.unraw().to_string(), resolved, visibility);
            }
            UseTree::Group(group) => {
                for child in &group.items {
                    self.collect(child, prefix, visibility);
                }
            }
            UseTree::Glob(_) => {}
        }
    }
    fn insert(&mut self, alias: String, path: Vec<String>, visibility: &Visibility) {
        if normalized(&path).len() == 1 {
            self.restricted.push(path.clone());
        }
        match visibility {
            Visibility::Public(_) | Visibility::Restricted(_) => self.restricted.push(path.clone()),
            Visibility::Inherited => {}
        }
        self.imported.entry(alias).or_default().push(path);
    }
}
fn normalized(path: &[String]) -> Vec<String> {
    path.iter()
        .skip_while(|part| ["crate", "self", "super", "Self"].contains(&part.as_str()))
        .cloned()
        .collect()
}
impl<'ast> Visit<'ast> for Bindings {
    fn visit_impl_item_type(&mut self, i: &'ast syn::ImplItemType) {
        if let syn::Type::Path(value) = &i.ty {
            let path = value
                .path
                .segments
                .iter()
                .map(|segment| segment.ident.unraw().to_string())
                .collect();
            self.imported
                .entry(i.ident.unraw().to_string())
                .or_default()
                .push(path);
        }
        visit::visit_impl_item_type(self, i);
    }
    fn visit_item_use(&mut self, i: &'ast ItemUse) {
        self.collect(&i.tree, &[], &i.vis);
        visit::visit_item_use(self, i);
    }
    fn visit_item_extern_crate(&mut self, i: &'ast ItemExternCrate) {
        let original = i.ident.unraw().to_string();
        let alias = i
            .rename
            .as_ref()
            .map_or_else(|| original.clone(), |(_, name)| name.unraw().to_string());
        self.insert(alias, vec![original], &i.vis);
        visit::visit_item_extern_crate(self, i);
    }
}
