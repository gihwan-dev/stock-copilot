use std::collections::BTreeSet;
use std::path::Path;

use proc_macro2::TokenStream;

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

fn without_exports(source: &str) -> Option<String> {
    let mut parsed = syn::parse_file(source).ok()?;
    parsed.items.retain(|item| !removable_export(item));
    let tokens: TokenStream = syn::parse_quote!(#parsed);
    Some(tokens.to_string())
}

fn removable_export(item: &syn::Item) -> bool {
    let syn::Item::Use(import) = item else {
        return false;
    };
    match &import.vis {
        syn::Visibility::Public(_) => import.attrs.is_empty(),
        syn::Visibility::Restricted(_) | syn::Visibility::Inherited => false,
    }
}

fn test_support(path: &str, source: &str) -> bool {
    path.split('/').any(|part| part.contains("test"))
        || syn::parse_file(source).is_ok_and(|file| !file.attrs.is_empty())
}

fn narrow_crate(
    root: &Path,
    base: &str,
    globs: &[String],
    changed: &BTreeSet<String>,
    directory: &str,
) -> Result<BTreeSet<String>, String> {
    let before = snapshot(root, directory, Some(base))?;
    let after = snapshot(root, directory, None)?;
    select_sources(&before, &after, globs, changed, directory)
}

pub(super) fn select_sources(
    before: &super::graph::Sources,
    after: &super::graph::Sources,
    globs: &[String],
    changed: &BTreeSet<String>,
    directory: &str,
) -> Result<BTreeSet<String>, String> {
    let prefix = format!("{directory}/");
    let touched: Vec<&String> = changed
        .iter()
        .filter(|path| path.starts_with(&prefix))
        .collect();
    for path in &touched {
        if !path.starts_with(&format!("{directory}/src/"))
            || (!before.contains_key(*path) && !globs.iter().any(|glob| matches_target(glob, path)))
        {
            return Err(String::from("non-target crate input"));
        }
        let source = after
            .get(*path)
            .ok_or("missing, deleted, or non-Rust input")?;
        if test_support(path, source) || path.contains(['*', '?', '[', ']', '{', '}']) {
            return Err(String::from("crate-wide support input"));
        }
    }
    let mut graph = super::graph::Graph::read(after)?;
    graph.extend(super::graph::Graph::read(before)?);
    let additions = super::additions::Additions::read(before, after).ok_or("addition context")?;
    let mut selected = BTreeSet::new();
    let mut seeds = BTreeSet::new();
    for path in touched {
        match additions.classify(path) {
            super::additions::Change::OnlyFile => {
                graph.outward(&BTreeSet::from([path.clone()]))?;
                if globs.iter().any(|glob| matches_target(glob, path)) {
                    selected.insert(path.clone());
                }
            }
            super::additions::Change::Registration => {}
            super::additions::Change::Graph => {
                if path.ends_with("/lib.rs") && !unchanged_lib(before.get(path), after.get(path)) {
                    return Err(String::from("lib structure changed"));
                }
                seeds.insert(path.clone());
            }
        }
    }
    if !seeds.is_empty() {
        selected.extend(
            graph
                .selected(&seeds)?
                .into_iter()
                .filter(|path| globs.iter().any(|glob| matches_target(glob, path))),
        );
    }
    Ok(selected)
}

fn unchanged_lib(before: Option<&String>, after: Option<&String>) -> bool {
    before
        .zip(after)
        .and_then(|(left, right)| without_exports(left).zip(without_exports(right)))
        .is_some_and(|(left, right)| left == right)
}

pub(super) fn snapshot(
    root: &Path,
    directory: &str,
    base: Option<&str>,
) -> Result<super::graph::Sources, String> {
    let paths = match base {
        None => listed_paths(root)?,
        Some(commit) => {
            let mut command = git(root);
            command.args([
                "ls-tree",
                "-r",
                "--name-only",
                "-z",
                commit,
                "--",
                directory,
            ]);
            String::from_utf8(output(command, "ls-tree")?)
                .map_err(|error| error.to_string())?
                .split('\0')
                .filter(|path| !path.is_empty())
                .map(String::from)
                .collect()
        }
    };
    paths
        .into_iter()
        .filter(|path| {
            path.starts_with(&format!("{directory}/"))
                && Path::new(path).extension().is_some_and(|ext| ext == "rs")
        })
        .map(|path| {
            let source = match base {
                None => read_text(&root.join(&path))?,
                Some(commit) => {
                    let mut command = git(root);
                    command.args(["show", &format!("{commit}:{path}")]);
                    String::from_utf8(output(command, "show")?)
                        .map_err(|error| error.to_string())?
                }
            };
            Ok((path, source))
        })
        .collect()
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
            .bytes()
            .all(|byte| byte.is_ascii_digit())
            .then_some(value)
            .and_then(|digits| digits.parse::<usize>().ok())
            .filter(|jobs| (1..=maximum).contains(jobs))
            .ok_or_else(|| {
                format!("STOCK_COPILOT_MUTANTS_JOBS는 ASCII 정수 1..={maximum}이어야 한다: {value}")
            }),
    }
}
