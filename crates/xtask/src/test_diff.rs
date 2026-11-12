mod functions;

#[cfg(test)]
mod tests;

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::io::{Read, Write};
use std::path::Path;
use std::process::{ExitCode, Stdio};
use std::thread;

use crate::check_support::{Finding, report, workspace_root};
use crate::git_base::{git, output, resolve_base};
use crate::nextest_list::list_tests;
pub(crate) use functions::test_functions;

pub(crate) const EXCLUDED_PREFIXES: [&str; 1] = ["harness/"];
const BASE_DIRECTORY: &str = "target/test-diff";
const COPIED_BUILD_OUTPUTS: [&str; 1] = ["apps/desktop/dist"];
const LIST_LABEL: &str = "cargo nextest list";
const SYMLINK_MODE: &str = "120000";

pub(crate) type Sources = BTreeMap<String, String>;

pub(crate) type Files = BTreeMap<String, Vec<u8>>;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Rule {
    Removed(String),
    Modified(String),
    NotListed(String),
    CountDecreased { base: usize, current: usize },
}

impl fmt::Display for Rule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Removed(id) => write!(f, "test-diff:removed:{id}"),
            Self::Modified(id) => write!(f, "test-diff:modified:{id}"),
            Self::NotListed(name) => write!(f, "test-diff:not-listed:{name}"),
            Self::CountDecreased { base, current } => {
                write!(f, "test-diff:count-decreased:{base}->{current}")
            }
        }
    }
}

pub(crate) fn in_scope(path: &str) -> bool {
    Path::new(path)
        .extension()
        .is_some_and(|extension| extension == "rs")
        && !EXCLUDED_PREFIXES
            .iter()
            .any(|prefix| path.starts_with(prefix))
}

fn all_tests(sources: &Sources) -> BTreeMap<String, (String, String)> {
    sources
        .iter()
        .flat_map(|(path, source)| {
            test_functions(path, source)
                .into_iter()
                .flatten()
                .map(|(id, fingerprint)| (id, (path.clone(), fingerprint)))
        })
        .collect()
}

pub(crate) fn compare_sources(base: &Sources, current: &Sources) -> BTreeSet<Finding<Rule>> {
    let now = all_tests(current);
    all_tests(base)
        .into_iter()
        .filter_map(|(id, (path, fingerprint))| match now.get(&id) {
            None => Some(Finding::whole_file(&path, Rule::Removed(id))),
            Some((_, current_fingerprint)) if *current_fingerprint != fingerprint => {
                Some(Finding::whole_file(&path, Rule::Modified(id)))
            }
            Some(_) => None,
        })
        .collect()
}

pub(crate) fn compare_lists(
    base: &BTreeSet<String>,
    current: &BTreeSet<String>,
) -> BTreeSet<Finding<Rule>> {
    let mut findings: BTreeSet<Finding<Rule>> = base
        .difference(current)
        .map(|name| Finding::whole_file(LIST_LABEL, Rule::NotListed(name.clone())))
        .collect();
    if current.len() < base.len() {
        findings.insert(Finding::whole_file(
            LIST_LABEL,
            Rule::CountDecreased {
                base: base.len(),
                current: current.len(),
            },
        ));
    }
    findings
}

fn split_nul(bytes: &[u8]) -> impl Iterator<Item = String> + '_ {
    bytes
        .split(|byte| *byte == 0)
        .filter(|part| !part.is_empty())
        .map(|part| String::from_utf8_lossy(part).into_owned())
}

pub(crate) fn sources(files: &Files) -> Sources {
    files
        .iter()
        .filter(|(path, _)| in_scope(path))
        .map(|(path, bytes)| (path.clone(), String::from_utf8_lossy(bytes).into_owned()))
        .collect()
}

pub(crate) fn current_files(root: &Path) -> Result<Files, String> {
    let mut listing = git(root);
    listing.args([
        "ls-files",
        "-z",
        "--cached",
        "--others",
        "--exclude-standard",
    ]);
    let mut files = Files::new();
    for path in split_nul(&output(listing, "ls-files")?) {
        let full = root.join(&path);
        let kind = fs::symlink_metadata(&full);
        if kind.is_ok_and(|metadata| metadata.is_file()) {
            let bytes = fs::read(&full).map_err(|error| format!("{}: {error}", full.display()))?;
            files.insert(path, bytes);
        }
    }
    Ok(files)
}

fn batch_read(root: &Path, objects: &[(String, String)]) -> Result<Files, String> {
    let mut command = git(root);
    command
        .args(["cat-file", "--batch"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped());
    let mut child = command
        .spawn()
        .map_err(|error| format!("git cat-file: {error}"))?;
    let names: Vec<&str> = objects.iter().map(|(object, _)| object.as_str()).collect();
    let request = format!("{}\n", names.join("\n"));
    let mut input = child
        .stdin
        .take()
        .ok_or_else(|| String::from("git cat-file: 입력을 열지 못했다"))?;
    let writer = thread::spawn(move || input.write_all(request.as_bytes()));
    let mut bytes = Vec::new();
    child
        .stdout
        .take()
        .ok_or_else(|| String::from("git cat-file: 출력을 열지 못했다"))?
        .read_to_end(&mut bytes)
        .map_err(|error| format!("git cat-file: {error}"))?;
    writer
        .join()
        .map_err(|panic| format!("git cat-file: 입력 스레드가 멈췄다: {panic:?}"))?
        .map_err(|error| format!("git cat-file: {error}"))?;
    let status = child
        .wait()
        .map_err(|error| format!("git cat-file: {error}"))?;
    if !status.success() {
        return Err(format!("git cat-file: {status}"));
    }
    let mut files = Files::new();
    let mut rest = bytes.as_slice();
    for (_, path) in objects {
        let header_end = rest
            .iter()
            .position(|byte| *byte == b'\n')
            .ok_or_else(|| String::from("git cat-file: 머리줄이 없다"))?;
        let (header, after) = rest
            .split_at_checked(header_end)
            .ok_or_else(|| String::from("git cat-file: 머리줄이 잘렸다"))?;
        let size: usize = String::from_utf8_lossy(header)
            .rsplit(' ')
            .next()
            .and_then(|text| text.parse().ok())
            .ok_or_else(|| format!("git cat-file: {path}의 크기를 읽지 못했다"))?;
        let (body, remainder) = after
            .get(1..)
            .and_then(|body_start| body_start.split_at_checked(size))
            .ok_or_else(|| format!("git cat-file: {path}의 내용이 잘렸다"))?;
        files.insert(path.clone(), body.to_vec());
        rest = remainder.get(1..).unwrap_or(&[]);
    }
    Ok(files)
}

pub(crate) fn base_files(root: &Path, commit: &str) -> Result<Files, String> {
    let mut listing = git(root);
    listing.args(["ls-tree", "-r", "-z", "--full-tree", commit]);
    let objects: Vec<(String, String)> = split_nul(&output(listing, "ls-tree")?)
        .filter_map(|entry| {
            let (meta, path) = entry.split_once('\t')?;
            let mut fields = meta.split(' ');
            let mode = fields.next()?;
            let kind = fields.next()?;
            let object = fields.next()?;
            (kind == "blob" && mode != SYMLINK_MODE)
                .then(|| (String::from(object), String::from(path)))
        })
        .collect();
    batch_read(root, &objects)
}

fn read_directory(root: &Path, relative: &str, files: &mut Files) -> Result<(), String> {
    let directory = root.join(relative);
    let listing =
        fs::read_dir(&directory).map_err(|error| format!("{}: {error}", directory.display()))?;
    for listed in listing {
        let entry = listed.map_err(|error| format!("{}: {error}", directory.display()))?;
        let child = format!("{relative}/{}", entry.file_name().to_string_lossy());
        let kind = entry
            .file_type()
            .map_err(|error| format!("{}: {error}", entry.path().display()))?;
        if kind.is_dir() {
            read_directory(root, &child, files)?;
        } else if !kind.is_symlink() {
            let bytes = fs::read(entry.path()).map_err(|error| format!("{child}: {error}"))?;
            files.insert(child, bytes);
        }
    }
    Ok(())
}

fn stale_files(
    directory: &Path,
    prefix: &str,
    files: &Files,
    stale: &mut Vec<String>,
) -> Result<(), String> {
    let listing =
        fs::read_dir(directory).map_err(|error| format!("{}: {error}", directory.display()))?;
    for listed in listing {
        let entry = listed.map_err(|error| format!("{}: {error}", directory.display()))?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let relative = format!("{prefix}{name}");
        let kind = entry
            .file_type()
            .map_err(|error| format!("{}: {error}", entry.path().display()))?;
        if kind.is_dir() {
            stale_files(&entry.path(), &format!("{relative}/"), files, stale)?;
        } else if !files.contains_key(&relative) {
            stale.push(relative);
        }
    }
    Ok(())
}

fn write_tree(files: &Files, destination: &Path) -> Result<(), String> {
    fs::create_dir_all(destination)
        .map_err(|error| format!("{}: {error}", destination.display()))?;
    let mut stale = Vec::new();
    stale_files(destination, "", files, &mut stale)?;
    for path in stale {
        let target = destination.join(&path);
        fs::remove_file(&target).map_err(|error| format!("{}: {error}", target.display()))?;
    }
    for (path, bytes) in files {
        let target = destination.join(path);
        if fs::read(&target).is_ok_and(|existing| existing == *bytes) {
            continue;
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|error| format!("{}: {error}", parent.display()))?;
        }
        fs::write(&target, bytes).map_err(|error| format!("{}: {error}", target.display()))?;
    }
    Ok(())
}

pub(crate) fn base_list(root: &Path, files: &Files) -> Result<BTreeSet<String>, String> {
    let directory = root.join(BASE_DIRECTORY);
    let tree = directory.join("tree");
    let mut tree_files = files.clone();
    for output_path in COPIED_BUILD_OUTPUTS {
        let prefix = format!("{output_path}/");
        let missing = !tree_files.keys().any(|path| path.starts_with(&prefix));
        if missing && root.join(output_path).is_dir() {
            read_directory(root, output_path, &mut tree_files)?;
        }
    }
    write_tree(&tree_files, &tree)?;
    list_tests(&tree, &[], Some(&directory.join("target")))
}

pub(crate) fn check(root: &Path, base: &str) -> Result<BTreeSet<Finding<Rule>>, String> {
    let commit = resolve_base(root, base)?;
    let before = base_files(root, &commit)?;
    let now = current_files(root)?;
    let mut findings = compare_sources(&sources(&before), &sources(&now));
    if before != now {
        let current = list_tests(root, &[], None)?;
        findings.extend(compare_lists(&base_list(root, &before)?, &current));
    }
    Ok(findings)
}

pub(crate) fn command(arguments: &[String]) -> ExitCode {
    report(
        "test-diff",
        match arguments {
            [base] => workspace_root().and_then(|root| check(&root, base)),
            _ => Err(String::from("사용법: xtask test-diff <HEAD|커밋 ID>")),
        },
    )
}
