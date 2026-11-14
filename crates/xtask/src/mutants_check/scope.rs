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
        if Path::new(path).extension().is_some_and(|ext| ext == "rs") {
            if path.contains(['*', '?', '[', ']', '{', '}']) {
                return Err(String::from("glob path"));
            }
            syn::parse_file(&source).map_err(|error| error.to_string())?;
            files.insert(path.clone());
        } else if Path::new(path).extension().is_none_or(|ext| ext != "rs") {
            return Err(String::from("support file"));
        }
    }
    let mut graph = super::graph::Graph::read(&snapshot(root, directory, None)?)?;
    graph.extend(super::graph::Graph::read(&snapshot(
        root,
        directory,
        Some(base),
    )?)?);
    graph.selected(&files).map(|selected| {
        selected
            .into_iter()
            .filter(|path| globs.iter().any(|glob| matches_target(glob, path)))
            .collect()
    })
}

fn snapshot(
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
