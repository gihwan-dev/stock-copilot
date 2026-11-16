use std::collections::BTreeMap;
use syn::visit::{self, Visit};
use syn::{ExprPath, ExprStruct, File, ItemUse, UseTree};

const ROOT: &str = "apps/desktop/src-tauri/src/main.rs";
const ADAPTERS: &[&str] = &[
    "broker_toss",
    "broker_toss_trade",
    "sources",
    "ai_claude_code",
    "ai_codex",
    "store_sqlite",
    "secrets_keychain",
    "platform_macos",
];

fn collect(tree: &UseTree, prefix: &[String], imported: &mut BTreeMap<String, Vec<String>>) {
    match tree {
        UseTree::Path(path) => {
            let mut extended = prefix.to_vec();
            extended.push(path.ident.to_string());
            collect(&path.tree, &extended, imported);
        }
        UseTree::Name(name) => {
            let mut resolved = prefix.to_vec();
            resolved.push(name.ident.to_string());
            imported.insert(name.ident.to_string(), resolved);
        }
        UseTree::Rename(rename) => {
            let mut resolved = prefix.to_vec();
            resolved.push(rename.ident.to_string());
            imported.insert(rename.rename.to_string(), resolved);
        }
        UseTree::Group(group) => {
            for child in &group.items {
                collect(child, prefix, imported);
            }
        }
        UseTree::Glob(_) => {}
    }
}

#[derive(Default)]
struct Construction {
    imported: BTreeMap<String, Vec<String>>,
    found: bool,
}
impl Construction {
    fn path(&mut self, path: &syn::Path) {
        let segments: Vec<String> = path
            .segments
            .iter()
            .map(|segment| segment.ident.to_string())
            .collect();
        let Some(first) = segments.first() else {
            return;
        };
        let mut resolved = self
            .imported
            .get(first)
            .cloned()
            .unwrap_or_else(|| vec![first.clone()]);
        resolved.extend(segments.iter().skip(1).cloned());
        if resolved.starts_with(&[String::from("store_sqlite"), String::from("DatabasePath")]) {
            return;
        }
        if resolved
            .first()
            .is_some_and(|root| ADAPTERS.contains(&root.as_str()))
        {
            self.found = true;
        }
    }
}
impl<'ast> Visit<'ast> for Construction {
    fn visit_item_use(&mut self, i: &'ast ItemUse) {
        collect(&i.tree, &[], &mut self.imported);
        visit::visit_item_use(self, i);
    }
    fn visit_expr_path(&mut self, i: &'ast ExprPath) {
        self.path(&i.path);
        visit::visit_expr_path(self, i);
    }
    fn visit_expr_struct(&mut self, i: &'ast ExprStruct) {
        self.path(&i.path);
        visit::visit_expr_struct(self, i);
    }
    fn visit_item_type(&mut self, i: &'ast syn::ItemType) {
        if let syn::Type::Path(value) = i.ty.as_ref() {
            self.path(&value.path);
        }
        visit::visit_item_type(self, i);
    }
}

pub(super) fn forbidden(relative: &str, file: &File) -> bool {
    if !relative.starts_with("apps/desktop/src-tauri/src/") || relative == ROOT {
        return false;
    }
    let mut visitor = Construction::default();
    for item in &file.items {
        if let syn::Item::Use(import) = item {
            collect(&import.tree, &[], &mut visitor.imported);
        }
    }
    visitor.visit_file(file);
    visitor.found
}
