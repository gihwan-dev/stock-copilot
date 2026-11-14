use std::collections::BTreeSet;
use std::path::Path;

use proc_macro2::TokenStream;
use syn::spanned::Spanned;

use crate::check_support::read_text;
use crate::git_base::{git, listed_paths, output};

use super::{Scope, selected_globs, touched_crates};

pub(super) fn matches_target(glob: &str, file: &str) -> bool {
    glob.strip_suffix("/**/*.rs").map_or_else(
        || glob == file,
        |prefix| {
            file.starts_with(&format!("{prefix}/"))
                && Path::new(file).extension().is_some_and(|ext| ext == "rs")
        },
    )
}

struct References<'a> {
    names: &'a BTreeSet<String>,
    found: bool,
}

impl<'ast> syn::visit::Visit<'ast> for References<'_> {
    fn visit_path(&mut self, i: &'ast syn::Path) {
        if i.segments.len() > 1
            && i.segments
                .iter()
                .any(|segment| self.names.contains(&segment.ident.to_string()))
        {
            self.found = true;
        }
        syn::visit::visit_path(self, i);
    }
    fn visit_use_tree(&mut self, i: &'ast syn::UseTree) {
        let ident = match i {
            syn::UseTree::Path(path) => Some(&path.ident),
            syn::UseTree::Name(name) => Some(&name.ident),
            syn::UseTree::Rename(rename) => Some(&rename.ident),
            syn::UseTree::Group(_) | syn::UseTree::Glob(_) => None,
        };
        if ident.is_some_and(|name| self.names.contains(&name.to_string())) {
            self.found = true;
        }
        syn::visit::visit_use_tree(self, i);
    }
    fn visit_item_mod(&mut self, i: &'ast syn::ItemMod) {
        if self.names.contains(&i.ident.to_string()) {
            self.found = true;
        }
        syn::visit::visit_item_mod(self, i);
    }
}

fn without_exports(source: &str) -> Option<String> {
    let parsed = syn::parse_file(source).ok()?;
    let mut remaining = source.as_bytes().to_vec();
    for declaration in parsed.items {
        if let syn::Item::Use(item) = declaration
            && let syn::Visibility::Public(_) = item.vis
            && item.attrs.is_empty()
        {
            let span = item.span();
            let offset = |position: proc_macro2::LineColumn| {
                source
                    .split_inclusive('\n')
                    .take(position.line.saturating_sub(1))
                    .map(str::len)
                    .sum::<usize>()
                    .saturating_add(position.column)
            };
            remaining
                .get_mut(offset(span.start())..offset(span.end()))?
                .fill(b' ');
        }
    }
    String::from_utf8(remaining)
        .ok()
        .and_then(|text| text.parse::<TokenStream>().ok())
        .map(|tokens| tokens.to_string())
}

fn reexports_only(root: &Path, base: &str, path: &str) -> bool {
    let mut command = git(root);
    command.args(["show", &format!("{base}:{path}")]);
    let previous = output(command, "show")
        .ok()
        .and_then(|bytes| String::from_utf8(bytes).ok());
    previous
        .zip(read_text(&root.join(path)).ok())
        .is_some_and(|(before, after)| {
            without_exports(&before)
                .zip(without_exports(&after))
                .is_some_and(|(left, right)| left == right)
        })
}

fn test_support(path: &str, source: &str) -> bool {
    path.split('/').any(|part| part.contains("test"))
        || syn::parse_file(source).is_ok_and(|file| !file.attrs.is_empty())
}

struct ExternalTestModule<'a> {
    path: &'a str,
    found: bool,
}

impl<'ast> syn::visit::Visit<'ast> for ExternalTestModule<'_> {
    fn visit_item_mod(&mut self, i: &'ast syn::ItemMod) {
        if i.content.is_none()
            && !i.attrs.is_empty()
            && self
                .path
                .split('/')
                .any(|part| i.ident == part.trim_end_matches(".rs"))
        {
            self.found = true;
        }
        syn::visit::visit_item_mod(self, i);
    }
}

fn external_support(parsed: &syn::File, files: &BTreeSet<String>) -> bool {
    files.iter().any(|path| {
        let mut support = ExternalTestModule { path, found: false };
        syn::visit::Visit::visit_file(&mut support, parsed);
        support.found
    })
}

fn narrow_crate(
    root: &Path,
    base: &str,
    globs: &[String],
    changed: &BTreeSet<String>,
    directory: &str,
) -> Result<BTreeSet<String>, String> {
    let prefix = format!("{directory}/");
    let touched: Vec<&String> = changed
        .iter()
        .filter(|path| path.starts_with(&prefix))
        .collect();
    let mut files = BTreeSet::new();
    for path in touched {
        let source = read_text(&root.join(path))?;
        if path.ends_with("/Cargo.toml")
            || test_support(path, &source)
            || (path.ends_with("/lib.rs") && !reexports_only(root, base, path))
        {
            return Err(String::from("crate 전체"));
        }
        if globs.iter().any(|glob| matches_target(glob, path)) {
            if path.contains(['*', '?', '[', ']', '{', '}']) {
                return Err(String::from("glob path"));
            }
            syn::parse_file(&source).map_err(|error| error.to_string())?;
            files.insert(path.clone());
        } else if Path::new(path).extension().is_none_or(|ext| ext != "rs") {
            return Err(String::from("support file"));
        }
    }
    if files.is_empty() {
        return Ok(files);
    }
    let names: BTreeSet<String> = files
        .iter()
        .filter_map(|path| Path::new(path).file_stem()?.to_str().map(String::from))
        .collect();
    for path in listed_paths(root)? {
        if path.starts_with(&prefix) && Path::new(&path).extension().is_some_and(|ext| ext == "rs")
        {
            let source = read_text(&root.join(&path))?;
            let parsed = syn::parse_file(&source).map_err(|error| error.to_string())?;
            if external_support(&parsed, &files) {
                return Err(String::from("external test module"));
            }
            let mut references = References {
                names: &names,
                found: false,
            };
            syn::visit::Visit::visit_file(&mut references, &parsed);
            if references.found && globs.iter().any(|glob| matches_target(glob, &path)) {
                files.insert(path);
            }
        }
    }
    Ok(files)
}

pub(super) fn diff_scope(
    root: &Path,
    base: &str,
    globs: &[String],
    changed: &BTreeSet<String>,
) -> Scope {
    if changed.iter().any(|path| {
        [
            "Cargo.toml",
            "Cargo.lock",
            "rust-toolchain.toml",
            ".cargo/mutants.toml",
            ".config/nextest.toml",
        ]
        .contains(&path.as_str())
    }) {
        return Scope::Everything;
    }
    let mut selected = BTreeSet::new();
    for directory in touched_crates(globs, changed) {
        match narrow_crate(root, base, globs, changed, &directory) {
            Ok(files) => selected.extend(files),
            Err(_) => selected.extend(selected_globs(
                globs,
                &Scope::Crates(BTreeSet::from([directory])),
            )),
        }
    }
    Scope::Files(selected)
}

pub(super) fn jobs_for(cores: usize, override_value: Option<&str>) -> Result<usize, String> {
    let maximum = (cores.checked_div(2).unwrap_or(1)).clamp(1, 6);
    match override_value {
        None => Ok(maximum),
        Some(value) => value
            .parse::<usize>()
            .ok()
            .filter(|jobs| (1..=maximum).contains(jobs))
            .ok_or_else(|| {
                format!("STOCK_COPILOT_MUTANTS_JOBS는 1..={maximum}이어야 한다: {value}")
            }),
    }
}
