mod crates;
mod modules;

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
use crates::crate_roots;
pub(crate) use modules::STANDARD_MACROS;
use modules::{ModuleRecord, Scan, Walker, has_tests};

pub(crate) const EXCLUDED_PREFIXES: [&str; 1] = ["harness/"];
const LIST_LABEL: &str = "cargo nextest list";
const SYMLINK_MODE: &str = "120000";
const SCOPE_KEYWORDS: [&str; 3] = ["self", "super", "crate"];

pub(crate) type Sources = BTreeMap<String, String>;

pub(crate) type Files = BTreeMap<String, Vec<u8>>;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Rule {
    Removed(String),
    Modified(String),
    NotListed(String),
    CountDecreased { base: usize, current: usize },
    Unreachable(String),
    SupportRemoved(String),
    SupportModified(String),
    Shadowing(String),
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
            Self::Unreachable(path) => write!(f, "test-diff:unreachable:{path}"),
            Self::SupportRemoved(id) => write!(f, "test-diff:support-removed:{id}"),
            Self::SupportModified(id) => write!(f, "test-diff:support-modified:{id}"),
            Self::Shadowing(id) => write!(f, "test-diff:shadowing:{id}"),
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

pub(crate) fn sources(files: &Files) -> Sources {
    files
        .iter()
        .filter(|(path, _)| in_scope(path))
        .map(|(path, bytes)| (path.clone(), String::from_utf8_lossy(bytes).into_owned()))
        .collect()
}

pub(crate) fn scan(files: &Files) -> Scan {
    let texts = sources(files);
    let mut scan = Scan::default();
    for root in crate_roots(files) {
        Walker {
            sources: &texts,
            binary: root.binary.clone(),
            scan: &mut scan,
        }
        .walk_root(&root.file);
    }
    scan
}

fn unreachable_findings(files: &Files, scan: &Scan) -> BTreeSet<Finding<Rule>> {
    sources(files)
        .into_iter()
        .filter(|(path, source)| !scan.reached.contains(path) && has_tests(source))
        .map(|(path, _)| Finding::whole_file(&path, Rule::Unreachable(path.clone())))
        .collect()
}

fn use_leaves(tree: &str) -> Vec<String> {
    let mut leaves = Vec::new();
    let mut prefix: Vec<String> = Vec::new();
    let mut current = String::new();
    for next in tree.chars() {
        match next {
            '{' => {
                prefix.push(current.trim().trim_end_matches("::").trim().to_owned());
                current.clear();
            }
            ',' | '}' => {
                let leaf = current.trim().to_owned();
                if !leaf.is_empty() {
                    let parent = prefix.last().map_or_else(String::new, Clone::clone);
                    leaves.push((parent, leaf));
                }
                current.clear();
                if next == '}' {
                    prefix.pop();
                }
            }
            _ => current.push(next),
        }
    }
    let rest = current.trim().to_owned();
    if !rest.is_empty() {
        leaves.push((String::new(), rest));
    }
    leaves
        .into_iter()
        .filter_map(|(parent, leaf)| {
            let words: Vec<&str> = leaf
                .split(|next: char| !(next.is_alphanumeric() || next == '_'))
                .filter(|word| !word.is_empty())
                .collect();
            match words.as_slice() {
                [.., "as", alias] => Some(String::from(*alias)),
                [.., "self"] | [] => parent
                    .rsplit(|next: char| !(next.is_alphanumeric() || next == '_'))
                    .find(|word| !word.is_empty())
                    .map(String::from),
                [.., last] => Some(String::from(*last)),
            }
        })
        .filter(|name| !SCOPE_KEYWORDS.contains(&name.as_str()))
        .collect()
}

fn introduced_names(kind: &str, name: &str) -> Vec<String> {
    match kind {
        "use" => use_leaves(name),
        "impl" => Vec::new(),
        _ => vec![String::from(name)],
    }
}

fn shadows(kind: &str, name: &str, base: &ModuleRecord) -> bool {
    match kind {
        "use" if name.contains('*') => true,
        "macro_rules" if STANDARD_MACROS.contains(&name) => true,
        _ => introduced_names(kind, name)
            .iter()
            .any(|introduced| base.identifiers.contains(introduced)),
    }
}

fn support_findings(base: &Scan, current: &Scan) -> BTreeSet<Finding<Rule>> {
    let empty = ModuleRecord::default();
    let mut findings = BTreeSet::new();
    for (module, before) in base.modules.iter().filter(|(_, record)| record.testing) {
        let after = current.modules.get(module).unwrap_or(&empty);
        for ((kind, name), fingerprint) in &before.support {
            let id = format!("{module}::{kind} {name}");
            match after.support.get(&(kind.clone(), name.clone())) {
                None => {
                    findings.insert(Finding::whole_file(module, Rule::SupportRemoved(id)));
                }
                Some(now) if now != fingerprint => {
                    findings.insert(Finding::whole_file(module, Rule::SupportModified(id)));
                }
                Some(_) => {}
            }
        }
        for (kind, name) in after.support.keys() {
            let key = (kind.clone(), name.clone());
            if !before.support.contains_key(&key) && shadows(kind, name, before) {
                findings.insert(Finding::whole_file(
                    module,
                    Rule::Shadowing(format!("{module}::{kind} {name}")),
                ));
            }
        }
    }
    findings
}

pub(crate) fn compare_files(base: &Files, current: &Files) -> (BTreeSet<Finding<Rule>>, Scan) {
    let before = scan(base);
    let now = scan(current);
    let mut findings: BTreeSet<Finding<Rule>> = before
        .tests
        .iter()
        .filter_map(|(id, fingerprint)| match now.tests.get(id) {
            None => Some(Finding::whole_file(id, Rule::Removed(id.clone()))),
            Some(current_fingerprint) if current_fingerprint != fingerprint => {
                Some(Finding::whole_file(id, Rule::Modified(id.clone())))
            }
            Some(_) => None,
        })
        .collect();
    findings.extend(unreachable_findings(current, &now));
    findings.extend(support_findings(&before, &now));
    (findings, before)
}

const ROOT_MANIFEST: &str = "Cargo.toml";

const DEFAULT_MANIFESTS: [(&str, &str); 2] = [
    ("Cargo.toml", "[workspace]\nmembers = [\"crates/domain\"]\n"),
    ("crates/domain/Cargo.toml", "[package]\nname = \"domain\"\n"),
];

fn with_default_manifests(sources: &Sources) -> Files {
    let mut files: Files = sources
        .iter()
        .map(|(path, text)| (path.clone(), text.clone().into_bytes()))
        .collect();
    if !files.contains_key(ROOT_MANIFEST) {
        for (path, text) in DEFAULT_MANIFESTS {
            files.insert(String::from(path), text.as_bytes().to_vec());
        }
    }
    files
}

fn texts(files: &Files) -> Sources {
    files
        .iter()
        .map(|(path, bytes)| (path.clone(), String::from_utf8_lossy(bytes).into_owned()))
        .collect()
}

pub(crate) fn compare_sources(base: &Sources, current: &Sources) -> BTreeSet<Finding<Rule>> {
    compare_files(
        &with_default_manifests(base),
        &with_default_manifests(current),
    )
    .0
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

pub(crate) fn check(root: &Path, base: &str) -> Result<BTreeSet<Finding<Rule>>, String> {
    let commit = resolve_base(root, base)?;
    let before = base_files(root, &commit)?;
    let mut findings = compare_sources(&texts(&before), &texts(&current_files(root)?));
    let expected: BTreeSet<String> = scan(&before).tests.keys().cloned().collect();
    findings.extend(compare_lists(&expected, &list_tests(root, &[], None)?));
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
