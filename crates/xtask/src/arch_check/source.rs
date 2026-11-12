use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use proc_macro2::{TokenStream, TokenTree};
use syn::visit::{self, Visit};
use syn::{ItemExternCrate, ItemImpl, ItemUse, Macro, UseName, UseRename, Visibility};

use super::metadata::MemberPackage;
use super::rules::SourceRules;
use super::tokens::{ConstMarkers, Markers, Words, collect_consts, flatten};
use super::{CheckFailure, CrateName, Finding, Rule};

#[derive(Debug)]
struct SourceFile {
    relative: String,
    markers: Markers,
    consts: Vec<ConstMarkers>,
}

#[derive(Debug, Clone, Copy)]
enum Exposure {
    Private,
    Exported,
}

const fn exposure(visibility: &Visibility) -> Exposure {
    match visibility {
        Visibility::Inherited => Exposure::Private,
        Visibility::Public(_) | Visibility::Restricted(_) => Exposure::Exported,
    }
}

#[derive(Debug, Default)]
struct UseNames {
    names: BTreeSet<String>,
}

impl<'ast> Visit<'ast> for UseNames {
    fn visit_use_name(&mut self, i: &'ast UseName) {
        self.names.insert(i.ident.to_string());
    }

    fn visit_use_rename(&mut self, i: &'ast UseRename) {
        self.names.insert(i.ident.to_string());
    }
}

#[derive(Debug)]
struct AstRules<'context> {
    rules: &'context SourceRules,
    crate_name: &'context CrateName,
    found: BTreeSet<Rule>,
}

impl AstRules<'_> {
    fn presence_impl_allowed(&self) -> Option<()> {
        self.rules
            .presence_impl_crates
            .contains(self.crate_name)
            .then_some(())
    }

    fn exported_item(&mut self) {
        if self.rules.reexport_crates.contains(self.crate_name) {
            self.found.insert(Rule::Reexport);
        }
    }

    fn exported_use(&mut self, item: &ItemUse) {
        self.exported_item();
        let mut names = UseNames::default();
        names.visit_use_tree(&item.tree);
        if names.names.contains(&self.rules.presence_trait) {
            self.found.insert(Rule::UserPresenceReexport);
        }
    }

    fn macro_fallback(&mut self, tokens: &TokenStream) {
        let mut idents = Vec::new();
        collect_idents(&tokens.clone().into_iter().collect::<Vec<_>>(), &mut idents);
        let presence = self.rules.presence_trait.as_str();
        let names_trait = idents.iter().any(|ident| ident == presence);
        if names_trait
            && idents.iter().any(|ident| ident == "impl")
            && self.presence_impl_allowed().is_none()
        {
            self.found.insert(Rule::UserPresenceImpl);
        }
        let renamed = idents
            .iter()
            .zip(idents.iter().skip(1))
            .any(|(first, second)| first == presence && second == "as");
        if renamed {
            self.found.insert(Rule::UserPresenceAlias);
        }
    }
}

fn collect_idents(trees: &[TokenTree], idents: &mut Vec<String>) {
    for tree in trees {
        match tree {
            TokenTree::Group(group) => collect_idents(&flatten(group), idents),
            TokenTree::Ident(ident) => idents.push(ident.to_string()),
            TokenTree::Punct(_) | TokenTree::Literal(_) => {}
        }
    }
}

impl<'ast> Visit<'ast> for AstRules<'_> {
    fn visit_item_use(&mut self, i: &'ast ItemUse) {
        match exposure(&i.vis) {
            Exposure::Exported => self.exported_use(i),
            Exposure::Private => {}
        }
        visit::visit_item_use(self, i);
    }

    fn visit_use_rename(&mut self, i: &'ast UseRename) {
        if i.ident == self.rules.presence_trait {
            self.found.insert(Rule::UserPresenceAlias);
        }
        visit::visit_use_rename(self, i);
    }

    fn visit_item_extern_crate(&mut self, i: &'ast ItemExternCrate) {
        match exposure(&i.vis) {
            Exposure::Exported => self.exported_item(),
            Exposure::Private => {}
        }
        visit::visit_item_extern_crate(self, i);
    }

    fn visit_item_impl(&mut self, i: &'ast ItemImpl) {
        let implements_presence = i
            .trait_
            .as_ref()
            .and_then(|(_, path, _)| path.segments.last())
            .is_some_and(|segment| segment.ident == self.rules.presence_trait);
        if implements_presence && self.presence_impl_allowed().is_none() {
            self.found.insert(Rule::UserPresenceImpl);
        }
        visit::visit_item_impl(self, i);
    }

    fn visit_macro(&mut self, i: &'ast Macro) {
        match syn::parse2::<syn::File>(i.tokens.clone()) {
            Ok(file) => self.visit_file(&file),
            Err(_) => self.macro_fallback(&i.tokens),
        }
        visit::visit_macro(self, i);
    }
}

fn io_failure(path: &Path, error: std::io::Error) -> CheckFailure {
    CheckFailure::Io {
        path: path.to_path_buf(),
        error,
    }
}

fn package_files(directory: &Path) -> Result<Vec<PathBuf>, CheckFailure> {
    let mut files = Vec::new();
    let mut pending = vec![directory.to_path_buf()];
    while let Some(current) = pending.pop() {
        for listed in fs::read_dir(&current).map_err(|error| io_failure(&current, error))? {
            let entry = listed.map_err(|error| io_failure(&current, error))?;
            let path = entry.path();
            let kind = entry
                .file_type()
                .map_err(|error| io_failure(&path, error))?;
            if !kind.is_dir() {
                files.push(path);
            } else if entry.file_name() != "target" && !path.join("Cargo.toml").exists() {
                pending.push(path);
            }
        }
    }
    files.sort();
    Ok(files)
}

fn relative_text(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .ok()
        .unwrap_or(path)
        .iter()
        .map(|part| part.to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

fn host_allowed(rules: &SourceRules, crate_name: &CrateName, relative: &str) -> Option<()> {
    (rules.host_crates.contains(crate_name) || rules.host_files.contains(relative)).then_some(())
}

fn scan_rust(
    rules: &SourceRules,
    crate_name: &CrateName,
    relative: &str,
    text: &str,
    findings: &mut BTreeSet<Finding>,
) -> Option<SourceFile> {
    let (Ok(file), Ok(stream)) = (syn::parse_file(text), TokenStream::from_str(text)) else {
        findings.insert(Finding::new(Rule::SourceParse, String::from(relative)));
        return None;
    };
    let mut ast = AstRules {
        rules,
        crate_name,
        found: BTreeSet::new(),
    };
    ast.visit_file(&file);
    findings.extend(
        ast.found
            .into_iter()
            .map(|rule| Finding::new(rule, String::from(relative))),
    );
    let words = Words {
        write_methods: &rules.write_methods,
        order_path_segments: &rules.order_path_segments,
    };
    let trees: Vec<TokenTree> = stream.into_iter().collect();
    let mut markers = Markers::default();
    markers.absorb(words, &trees);
    let mut consts = Vec::new();
    collect_consts(words, &trees, &mut consts);
    Some(SourceFile {
        relative: String::from(relative),
        markers,
        consts,
    })
}

fn scan_file(
    rules: &SourceRules,
    root: &Path,
    crate_name: &CrateName,
    path: &Path,
    findings: &mut BTreeSet<Finding>,
) -> Result<Option<SourceFile>, CheckFailure> {
    let relative = relative_text(root, path);
    let bytes = fs::read(path).map_err(|error| io_failure(path, error))?;
    let lowered = String::from_utf8_lossy(&bytes).to_ascii_lowercase();
    if lowered.contains(&rules.host_needle.to_ascii_lowercase())
        && host_allowed(rules, crate_name, &relative).is_none()
    {
        findings.insert(Finding::new(Rule::TossHost, relative.clone()));
    }
    if path.extension().is_none_or(|extension| extension != "rs") {
        return Ok(None);
    }
    if let Ok(text) = String::from_utf8(bytes) {
        return Ok(scan_rust(rules, crate_name, &relative, &text, findings));
    }
    findings.insert(Finding::new(Rule::SourceParse, relative));
    Ok(None)
}

fn marked_consts(files: &[SourceFile], pick: fn(&Markers) -> &Vec<String>) -> BTreeSet<String> {
    let consts: Vec<&ConstMarkers> = files.iter().flat_map(|file| &file.consts).collect();
    let mut marked: BTreeSet<String> = consts
        .iter()
        .filter(|item| !pick(&item.markers).is_empty())
        .map(|item| item.name.clone())
        .collect();
    loop {
        let grown: BTreeSet<String> = consts
            .iter()
            .filter(|item| !item.markers.idents.is_disjoint(&marked))
            .map(|item| item.name.clone())
            .filter(|name| !marked.contains(name))
            .collect();
        if grown.is_empty() {
            return marked;
        }
        marked.extend(grown);
    }
}

fn order_paths(markers: &Markers) -> &Vec<String> {
    &markers.order_paths
}

fn write_methods(markers: &Markers) -> &Vec<String> {
    &markers.write_methods
}

fn order_writes(files: &[SourceFile], findings: &mut BTreeSet<Finding>) {
    let order_consts = marked_consts(files, order_paths);
    let write_consts = marked_consts(files, write_methods);
    for file in files {
        let has_order =
            !file.markers.order_paths.is_empty() || !file.markers.idents.is_disjoint(&order_consts);
        let has_write = !file.markers.write_methods.is_empty()
            || !file.markers.idents.is_disjoint(&write_consts);
        if has_order && has_write {
            findings.insert(Finding::new(Rule::TossOrderWrite, file.relative.clone()));
        }
    }
}

pub(crate) fn check_sources(
    rules: &SourceRules,
    root: &Path,
    packages: &[MemberPackage],
) -> Result<BTreeSet<Finding>, CheckFailure> {
    let mut findings = BTreeSet::new();
    let mut by_crate: BTreeMap<&CrateName, Vec<SourceFile>> = BTreeMap::new();
    for package in packages {
        for path in package_files(&package.directory)? {
            if let Some(file) = scan_file(rules, root, &package.name, &path, &mut findings)? {
                by_crate.entry(&package.name).or_default().push(file);
            }
        }
    }
    for (crate_name, files) in &by_crate {
        if !rules.order_write_crates.contains(crate_name) {
            order_writes(files, &mut findings);
        }
    }
    Ok(findings)
}
