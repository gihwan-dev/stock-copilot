use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;

mod ast;

use proc_macro2::{TokenStream, TokenTree};
use syn::visit::Visit;

use super::metadata::Workspace;
use super::rules::SourceRules;
use super::tokens::{ConstMarkers, Markers, Words, collect_consts};
use super::{CheckFailure, CrateName, Finding, Rule};
use ast::AstRules;

#[derive(Debug)]
struct SourceFile {
    relative: String,
    markers: Markers,
    consts: Vec<ConstMarkers>,
}

fn io_failure(path: &Path, error: std::io::Error) -> CheckFailure {
    CheckFailure::Io {
        path: path.to_path_buf(),
        error,
    }
}

fn package_files(
    directory: &Path,
    skipped: &BTreeSet<PathBuf>,
) -> Result<Vec<PathBuf>, CheckFailure> {
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
            } else if !skipped.contains(&path) {
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
    let mut ast = AstRules::new(rules, crate_name, &file);
    ast.visit_file(&file);
    findings.extend(
        ast.found
            .into_iter()
            .map(|rule| Finding::new(rule, String::from(relative))),
    );
    let words = Words {
        write_means: &rules.write_means,
        order_words: &rules.order_words,
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

fn write_means(markers: &Markers) -> &Vec<String> {
    &markers.write_means
}

fn order_writes(files: &[SourceFile], findings: &mut BTreeSet<Finding>) {
    let order_consts = marked_consts(files, order_paths);
    let write_consts = marked_consts(files, write_means);
    for file in files {
        let has_order =
            !file.markers.order_paths.is_empty() || !file.markers.idents.is_disjoint(&order_consts);
        let has_write =
            !file.markers.write_means.is_empty() || !file.markers.idents.is_disjoint(&write_consts);
        if has_order && has_write {
            findings.insert(Finding::new(Rule::TossOrderWrite, file.relative.clone()));
        }
    }
}

fn http_write_means(
    rules: &SourceRules,
    by_crate: &BTreeMap<&CrateName, Vec<SourceFile>>,
    findings: &mut BTreeSet<Finding>,
) {
    for row in &rules.http_writes {
        findings.extend(
            by_crate
                .get(&row.dependent)
                .into_iter()
                .flatten()
                .filter(|file| !row.allowed_files.contains(&file.relative))
                .filter(|file| !file.markers.write_means.is_empty())
                .map(|file| Finding::new(Rule::HttpWriteMeans, file.relative.clone())),
        );
    }
}

pub(crate) fn check_sources(
    rules: &SourceRules,
    workspace: &Workspace,
) -> Result<BTreeSet<Finding>, CheckFailure> {
    let mut findings = BTreeSet::new();
    let mut by_crate: BTreeMap<&CrateName, Vec<SourceFile>> = BTreeMap::new();
    let mut skipped: BTreeSet<PathBuf> = workspace
        .packages
        .iter()
        .map(|package| package.directory.clone())
        .collect();
    skipped.insert(workspace.target_directory.clone());
    for package in &workspace.packages {
        for path in package_files(&package.directory, &skipped)? {
            if let Some(file) =
                scan_file(rules, &workspace.root, &package.name, &path, &mut findings)?
            {
                by_crate.entry(&package.name).or_default().push(file);
            }
        }
    }
    for (crate_name, files) in &by_crate {
        if !rules.order_write_crates.contains(crate_name) {
            order_writes(files, &mut findings);
        }
    }
    http_write_means(rules, &by_crate, &mut findings);
    Ok(findings)
}
