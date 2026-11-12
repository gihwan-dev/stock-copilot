use syn::ext::IdentExt;
use syn::visit::{self, Visit};
use syn::{Ident, PatType, PathSegment, UseTree};

pub(super) const SELF_PATH: &str = "self";

pub(super) fn unraw(ident: &Ident) -> String {
    ident.unraw().to_string()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum UseKind {
    Name,
    Rename(String),
    Glob,
}

#[derive(Debug, Clone)]
pub(super) struct UseEntry {
    pub(super) path: Vec<String>,
    pub(super) kind: UseKind,
}

impl UseEntry {
    pub(super) fn imported(&self) -> &str {
        self.path.last().map_or("", String::as_str)
    }
}

fn joined(prefix: &[String], name: String) -> Vec<String> {
    let mut path = prefix.to_vec();
    if name != SELF_PATH {
        path.push(name);
    }
    path
}

fn collect(tree: &UseTree, prefix: &mut Vec<String>, entries: &mut Vec<UseEntry>) {
    match tree {
        UseTree::Path(path) => {
            prefix.push(unraw(&path.ident));
            collect(&path.tree, prefix, entries);
            prefix.pop();
        }
        UseTree::Name(name) => entries.push(UseEntry {
            path: joined(prefix, unraw(&name.ident)),
            kind: UseKind::Name,
        }),
        UseTree::Rename(rename) => entries.push(UseEntry {
            path: joined(prefix, unraw(&rename.ident)),
            kind: UseKind::Rename(unraw(&rename.rename)),
        }),
        UseTree::Glob(_) => entries.push(UseEntry {
            path: prefix.clone(),
            kind: UseKind::Glob,
        }),
        UseTree::Group(group) => {
            for item in &group.items {
                collect(item, prefix, entries);
            }
        }
    }
}

pub(super) fn use_entries(tree: &UseTree) -> Vec<UseEntry> {
    let mut entries = Vec::new();
    collect(tree, &mut Vec::new(), &mut entries);
    entries
}

#[derive(Debug, Default)]
pub(super) struct TypeNames {
    pub(super) names: Vec<String>,
}

impl<'ast> Visit<'ast> for TypeNames {
    fn visit_path_segment(&mut self, i: &'ast PathSegment) {
        self.names.push(unraw(&i.ident));
        visit::visit_path_segment(self, i);
    }

    fn visit_pat_type(&mut self, i: &'ast PatType) {
        self.visit_type(&i.ty);
    }

    fn visit_block(&mut self, _: &'ast syn::Block) {}

    fn visit_expr(&mut self, _: &'ast syn::Expr) {}
}

pub(super) fn starts_with(path: &[String], prefix: &[String]) -> Option<()> {
    (!prefix.is_empty() && path.starts_with(prefix)).then_some(())
}

pub(super) fn has_pair(path: &[String], first: &str, second: &str) -> Option<()> {
    path.iter()
        .zip(path.iter().skip(1))
        .any(|(left, right)| left == first && right == second)
        .then_some(())
}
