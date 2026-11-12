use syn::spanned::Spanned;
use syn::visit::{self, Visit};
use syn::{ItemUse, UseTree};

#[derive(Debug, Default)]
struct GlobFinder {
    lines: Vec<usize>,
}

fn glob_lines(tree: &UseTree, lines: &mut Vec<usize>) {
    match tree {
        UseTree::Glob(glob) => lines.push(glob.span().start().line),
        UseTree::Path(path) => glob_lines(&path.tree, lines),
        UseTree::Group(group) => {
            for item in &group.items {
                glob_lines(item, lines);
            }
        }
        UseTree::Name(_) | UseTree::Rename(_) => {}
    }
}

impl<'ast> Visit<'ast> for GlobFinder {
    fn visit_item_use(&mut self, i: &'ast ItemUse) {
        glob_lines(&i.tree, &mut self.lines);
        visit::visit_item_use(self, i);
    }
}

pub(crate) fn glob_import_lines(source: &str) -> Vec<usize> {
    let mut finder = GlobFinder::default();
    if let Ok(file) = syn::parse_file(source) {
        finder.visit_file(&file);
    }
    finder.lines
}
