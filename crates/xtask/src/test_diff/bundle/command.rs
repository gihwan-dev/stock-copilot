use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::path::Path;
use std::process::ExitCode;

use crate::check_support::{Finding, read_text, workspace_root};
use crate::git_base;
use crate::test_diff::approval::{approvable_rule, commit, digest, git, is_ancestor, valid_task};
use crate::test_diff::{
    Files, RootScans, Rule, base_files, compare_files, current_files, in_scope,
};

use super::parse::{BUNDLES_DIR, Bundle, DRAFT_REASON, FileEntry, Target, parse, render};
use super::{
    TrailerPolicy, check_intro_isolation_path, commit_parents, compare_findings_and_files,
    current_worktree_tree, trailer_problems, tree_of,
};

const WORKTREE_TARGET: &str = "WORKTREE";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DraftMode {
    Auto,
    MergeHead,
    TargetCommit,
    TargetTree,
}

fn write_stderr(message: &str) {
    let _ignored = writeln!(std::io::stderr().lock(), "{message}");
}

fn write_stdout(message: &str) -> ExitCode {
    if std::io::stdout()
        .lock()
        .write_all(message.as_bytes())
        .is_ok()
    {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn parse_mode(value: &str) -> Result<DraftMode, String> {
    match value {
        "merge-head" => Ok(DraftMode::MergeHead),
        "target-commit" => Ok(DraftMode::TargetCommit),
        "target-tree" => Ok(DraftMode::TargetTree),
        _ => Err(format!(
            "알 수 없는 모드다(merge-head|target-commit|target-tree): {value}"
        )),
    }
}

fn merge_preview_files(root: &Path, base: &str, target: &str) -> Result<Files, String> {
    if git(root, &["merge-base", base, target]).is_none() {
        return Err(format!(
            "기준 커밋 {base}와 대상 커밋 {target}의 공통 조상이 없다"
        ));
    }
    let tree = git(root, &["merge-tree", "--write-tree", base, target])
        .and_then(|output| output.lines().next().map(String::from))
        .filter(|value| git_base::is_commit_id(value))
        .ok_or_else(|| {
            format!("기준 커밋 {base}와 대상 커밋 {target}의 가상 병합에 충돌이 있거나 실패했다")
        })?;
    base_files(root, &tree)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TargetFilesMode {
    DirectOnly,
    PreviewMerge,
}

fn resolve_target_files(
    root: &Path,
    base: &str,
    target: &str,
    mode: TargetFilesMode,
) -> Result<Files, String> {
    if target == WORKTREE_TARGET {
        return current_files(root);
    }
    if !git_base::is_commit_id(target) || commit(root, target).as_deref() != Some(target) {
        return Err(format!("대상 커밋을 찾지 못했다: {target}"));
    }
    if is_ancestor(root, base, target) {
        return base_files(root, target);
    }
    match mode {
        TargetFilesMode::PreviewMerge => merge_preview_files(root, base, target),
        TargetFilesMode::DirectOnly => Err(format!(
            "기준 커밋 {base}가 대상 커밋 {target}의 조상이 아니다"
        )),
    }
}

fn prune_unneeded_files(
    scans: &RootScans,
    after: &Files,
    changed: &[String],
    expected: &BTreeSet<Finding<Rule>>,
    selected: &mut BTreeSet<String>,
) -> Files {
    let mut subset = Files::new();
    for path in changed {
        selected.insert(path.clone());
        if let Some(bytes) = after.get(path) {
            subset.insert(path.clone(), bytes.clone());
        }
    }
    for path in changed {
        if !selected.contains(path) {
            continue;
        }
        let mut candidate = subset.clone();
        candidate.remove(path);
        if &scans.compare_substituting(&candidate) == expected {
            selected.remove(path);
            subset = candidate;
        }
    }
    subset
}

fn minimal_changed_files(
    before: &Files,
    after: &Files,
    expected: &BTreeSet<Finding<Rule>>,
) -> Result<BTreeMap<String, FileEntry>, String> {
    let scans = RootScans::new(before);
    let changed: Vec<String> = before
        .iter()
        .filter(|(path, old)| in_scope(path) && after.get(*path).is_some_and(|new| new != *old))
        .map(|(path, _)| path.clone())
        .collect();
    let mut selected: BTreeSet<String> = changed
        .iter()
        .filter(|path| {
            let mut single = Files::new();
            if let Some(bytes) = after.get(*path) {
                single.insert((*path).clone(), bytes.clone());
            }
            !scans.compare_substituting(&single).is_empty()
        })
        .cloned()
        .collect();
    let mut subset = Files::new();
    for path in &selected {
        if let Some(bytes) = after.get(path) {
            subset.insert(path.clone(), bytes.clone());
        }
    }
    if &scans.compare_substituting(&subset) != expected {
        subset = prune_unneeded_files(&scans, after, &changed, expected, &mut selected);
    }
    if &scans.compare_substituting(&subset) != expected {
        return Err(String::from(
            "변경된 기존 .rs 파일 해시만으로 지적 집합을 재현하지 못했다",
        ));
    }
    let mut entries = BTreeMap::new();
    for path in selected {
        let old = before
            .get(&path)
            .ok_or_else(|| format!("기준 파일이 없다: {path}"))?;
        let new = after
            .get(&path)
            .ok_or_else(|| format!("대상 파일이 없다: {path}"))?;
        entries.insert(
            path.clone(),
            FileEntry {
                path,
                before: digest(old),
                after: digest(new),
            },
        );
    }
    Ok(entries)
}

fn select_target(root: &Path, base: &str, target: &str, mode: DraftMode) -> Result<Target, String> {
    if target == WORKTREE_TARGET {
        return match mode {
            DraftMode::Auto | DraftMode::TargetTree => {
                Ok(Target::Tree(current_worktree_tree(root)?))
            }
            DraftMode::MergeHead | DraftMode::TargetCommit => Err(String::from(
                "WORKTREE 대상에는 target-tree 모드만 사용할 수 있다",
            )),
        };
    }
    let parents = commit_parents(root, target)
        .ok_or_else(|| format!("대상 커밋 부모를 읽지 못했다: {target}"))?;
    let is_direct_merge = parents.len() == 2 && parents.first().map(String::as_str) == Some(base);
    match mode {
        DraftMode::Auto => {
            if is_direct_merge {
                Ok(Target::Commit(String::from(target)))
            } else {
                Ok(Target::MergeHead(String::from(target)))
            }
        }
        DraftMode::MergeHead => Ok(Target::MergeHead(String::from(target))),
        DraftMode::TargetCommit => {
            if !is_direct_merge {
                return Err(format!(
                    "target-commit 모드는 첫 부모가 기준 커밋인 2부모 병합 커밋만 받는다: {target}"
                ));
            }
            Ok(Target::Commit(String::from(target)))
        }
        DraftMode::TargetTree => {
            let tree = tree_of(root, target)
                .ok_or_else(|| format!("대상 트리 해시를 읽지 못했다: {target}"))?;
            Ok(Target::Tree(tree))
        }
    }
}

pub(crate) fn draft_bundle(
    root: &Path,
    task: &str,
    base: &str,
    target: &str,
    mode: DraftMode,
) -> Result<String, String> {
    if !valid_task(task) {
        return Err(format!("작업 ID 형식이 잘못됐다: {task}"));
    }
    if !git_base::is_commit_id(base) || commit(root, base).as_deref() != Some(base) {
        return Err(format!("기준 커밋은 존재하는 40자리 ID여야 한다: {base}"));
    }
    let files_mode = match mode {
        DraftMode::Auto | DraftMode::MergeHead => TargetFilesMode::PreviewMerge,
        DraftMode::TargetCommit | DraftMode::TargetTree => TargetFilesMode::DirectOnly,
    };
    let before = base_files(root, base)?;
    let after = resolve_target_files(root, base, target, files_mode)?;
    let (findings, _) = compare_files(&before, &after);
    if findings.is_empty() {
        return Err(String::from("해당 구간에 test-diff 지적이 없다"));
    }
    let disallowed: Vec<String> = findings
        .iter()
        .filter(|item| !approvable_rule(&item.rule))
        .map(|item| item.rule.to_string())
        .collect();
    if !disallowed.is_empty() {
        return Err(format!(
            "승인 묶음으로 승인할 수 없는 지적이 있다: {}",
            disallowed.join(", ")
        ));
    }
    let files = minimal_changed_files(&before, &after, &findings)?;
    let selected_target = select_target(root, base, target, mode)?;
    let bundle = Bundle {
        task: String::from(task),
        base_commit: String::from(base),
        target: selected_target,
        reason: String::from(DRAFT_REASON),
        findings: findings.iter().map(|item| item.rule.to_string()).collect(),
        files,
    };
    render(&bundle)
}

fn check_merge_head_binding(
    root: &Path,
    bundle: &Bundle,
    merge_head: &str,
    base: &str,
    target: &str,
) -> Vec<String> {
    let canonical_path = format!("{BUNDLES_DIR}/{}.toml", bundle.task);
    if target == WORKTREE_TARGET {
        let pending = commit(root, "MERGE_HEAD");
        let base_ok = base == bundle.base_commit
            || check_intro_isolation_path(root, &bundle.base_commit, &canonical_path, base).is_ok();
        if pending.as_deref() == Some(merge_head) && base_ok {
            return Vec::new();
        }
        return vec![format!(
            "구간 불일치: WORKTREE가 merge_head={merge_head}, base_commit={} 병합 상태가 아니다(base={base})",
            bundle.base_commit
        )];
    }
    let Some(parents) = commit_parents(root, target) else {
        return vec![format!("대상 커밋 부모를 읽지 못했다: {target}")];
    };
    if target == merge_head && base == bundle.base_commit {
        return Vec::new();
    }
    if parents.len() == 2
        && parents.get(1).map(String::as_str) == Some(merge_head)
        && let Some(intro) = parents.first().map(String::as_str)
        && (base == intro || base == bundle.base_commit)
    {
        return match check_intro_isolation_path(root, &bundle.base_commit, &canonical_path, intro) {
            Ok(()) => Vec::new(),
            Err((_, reason)) => vec![reason],
        };
    }
    vec![format!(
        "구간 불일치: merge_head={merge_head}, base_commit={}와 대상({base}..{target})이 맞지 않는다",
        bundle.base_commit
    )]
}

fn check_tree_binding(
    root: &Path,
    bundle: &Bundle,
    expected_tree: &str,
    base: &str,
    target: &str,
) -> Result<Vec<String>, String> {
    let mut problems = Vec::new();
    if base != bundle.base_commit {
        problems.push(format!(
            "구간 불일치: base {base} != 묶음 base_commit {}",
            bundle.base_commit
        ));
    }
    let actual_tree = if target == WORKTREE_TARGET {
        current_worktree_tree(root)?
    } else {
        let valid_merge = commit_parents(root, target).is_some_and(|parents| {
            parents.len() == 2 && parents.first().map(String::as_str) == Some(base)
        });
        if !valid_merge {
            problems.push(format!(
                "병합 커밋이 아니거나 첫 부모가 기준 커밋과 다르다: {target}"
            ));
        }
        tree_of(root, target).ok_or_else(|| format!("대상 트리 해시를 읽지 못했다: {target}"))?
    };
    if actual_tree != expected_tree {
        problems.push(format!(
            "트리 해시 불일치: 실제 {actual_tree}, 묶음 {expected_tree}"
        ));
    }
    Ok(problems)
}

fn check_target_binding(
    root: &Path,
    bundle: &Bundle,
    base: &str,
    target: &str,
) -> Result<Vec<String>, String> {
    match &bundle.target {
        Target::MergeHead(merge_head) => Ok(check_merge_head_binding(
            root, bundle, merge_head, base, target,
        )),
        Target::Commit(expected) => {
            let mut problems = Vec::new();
            if target != expected || base != bundle.base_commit {
                problems.push(format!(
                    "구간 불일치: 기대 {}..{}, 실제 {base}..{target}",
                    bundle.base_commit, expected
                ));
            }
            let valid_merge = commit_parents(root, target).is_some_and(|parents| {
                parents.len() == 2 && parents.first().map(String::as_str) == Some(base)
            });
            if !valid_merge {
                problems.push(format!(
                    "병합 커밋이 아니거나 첫 부모가 기준 커밋과 다르다: {target}"
                ));
            }
            Ok(problems)
        }
        Target::Tree(expected_tree) => {
            check_tree_binding(root, bundle, expected_tree, base, target)
        }
    }
}

pub(crate) fn check_bundle_file(
    root: &Path,
    bundle_path: &Path,
    base: &str,
    target: &str,
) -> Result<Vec<String>, String> {
    if !git_base::is_commit_id(base) || commit(root, base).as_deref() != Some(base) {
        return Err(format!("기준 커밋은 존재하는 40자리 ID여야 한다: {base}"));
    }
    let resolved_path = if bundle_path.is_absolute() {
        bundle_path.to_path_buf()
    } else {
        root.join(bundle_path)
    };
    let label = bundle_path.to_string_lossy().into_owned();
    let contents = read_text(&resolved_path)?;
    let bundle = parse(&label, &contents)?;
    let files_mode = match &bundle.target {
        Target::MergeHead(merge_head) if target == merge_head => TargetFilesMode::PreviewMerge,
        Target::MergeHead(_) | Target::Commit(_) | Target::Tree(_) => TargetFilesMode::DirectOnly,
    };
    let before = base_files(root, base)?;
    let after = resolve_target_files(root, base, target, files_mode)?;
    let (findings, _) = compare_files(&before, &after);
    let mut problems = check_target_binding(root, &bundle, base, target)?;
    problems.extend(compare_findings_and_files(
        &bundle,
        (&before, &after),
        &findings,
    ));
    if target != WORKTREE_TARGET {
        let trailer_policy = match &bundle.target {
            Target::MergeHead(merge_head) if target == merge_head => None,
            Target::MergeHead(_) => Some(TrailerPolicy::Required),
            Target::Commit(_) | Target::Tree(_) => Some(TrailerPolicy::OptionalConsistent),
        };
        if let Some(policy) = trailer_policy {
            let canonical_bundle_path = format!("{BUNDLES_DIR}/{}.toml", bundle.task);
            problems.extend(trailer_problems(
                root,
                target,
                &canonical_bundle_path,
                bundle.findings.len(),
                policy,
            ));
        }
    }
    Ok(problems)
}

pub(crate) fn message_trailers(root: &Path, bundle_path: &Path) -> Result<String, String> {
    let resolved_path = if bundle_path.is_absolute() {
        bundle_path.to_path_buf()
    } else {
        root.join(bundle_path)
    };
    let label = bundle_path.to_string_lossy().into_owned();
    let bundle = parse(&label, &read_text(&resolved_path)?)?;
    Ok(format!(
        "Test-Diff-Bundle: {BUNDLES_DIR}/{}.toml\nTest-Diff-Findings: {}\n",
        bundle.task,
        bundle.findings.len()
    ))
}

fn run_draft(arguments: &[String]) -> Result<String, String> {
    let (task, base, target, mode) = match arguments {
        [task, base, target] => (
            task.as_str(),
            base.as_str(),
            target.as_str(),
            DraftMode::Auto,
        ),
        [task, base, target, flag, mode_value] if flag == "--mode" => (
            task.as_str(),
            base.as_str(),
            target.as_str(),
            parse_mode(mode_value)?,
        ),
        _ => {
            return Err(String::from(
                "사용법: xtask test-diff-bundle draft <작업 ID> <기준 커밋> <대상 커밋|WORKTREE> [--mode merge-head|target-commit|target-tree]",
            ));
        }
    };
    let root = workspace_root()?;
    draft_bundle(&root, task, base, target, mode)
}

fn run_check(arguments: &[String]) -> Result<Vec<String>, String> {
    let [bundle_path, base, target] = arguments else {
        return Err(String::from(
            "사용법: xtask test-diff-bundle check <묶음 파일> <기준 커밋> <대상 커밋|WORKTREE>",
        ));
    };
    let root = workspace_root()?;
    check_bundle_file(&root, Path::new(bundle_path), base, target)
}

fn run_message(arguments: &[String]) -> Result<String, String> {
    let [bundle_path] = arguments else {
        return Err(String::from(
            "사용법: xtask test-diff-bundle message <묶음 파일>",
        ));
    };
    let root = workspace_root()?;
    message_trailers(&root, Path::new(bundle_path))
}

pub(crate) fn command(arguments: &[String]) -> ExitCode {
    match arguments {
        [subcommand, rest @ ..] if subcommand == "draft" => match run_draft(rest) {
            Ok(rendered) => write_stdout(&rendered),
            Err(failure) => {
                write_stderr(&format!("test-diff-bundle: {failure}"));
                ExitCode::FAILURE
            }
        },
        [subcommand, rest @ ..] if subcommand == "check" => match run_check(rest) {
            Ok(problems) if problems.is_empty() => write_stdout("test-diff-bundle: 통과\n"),
            Ok(problems) => {
                for problem in &problems {
                    write_stderr(&format!("test-diff-bundle: {problem}"));
                }
                write_stderr(&format!("test-diff-bundle: 불일치 {}건", problems.len()));
                ExitCode::FAILURE
            }
            Err(failure) => {
                write_stderr(&format!("test-diff-bundle: {failure}"));
                ExitCode::FAILURE
            }
        },
        [subcommand, rest @ ..] if subcommand == "message" => match run_message(rest) {
            Ok(trailers) => write_stdout(&trailers),
            Err(failure) => {
                write_stderr(&format!("test-diff-bundle: {failure}"));
                ExitCode::FAILURE
            }
        },
        _ => {
            write_stderr("사용법: xtask test-diff-bundle <draft|check|message> ...");
            ExitCode::FAILURE
        }
    }
}
