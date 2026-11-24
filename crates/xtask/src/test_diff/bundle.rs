pub(crate) mod command;
mod parse;

use std::collections::BTreeSet;
use std::io::Write;
use std::path::Path;

use crate::check_support::Finding;
use crate::git_base;
use crate::test_diff::approval::{
    approvable_rule, commit, digest, first_parent_ancestor, git, is_ancestor, pending_master_merge,
    same_entry, trusted_authority,
};
use crate::test_diff::{Files, RootScans, Rule, base_files, in_scope};

use parse::{BUNDLES_DIR, Bundle, Target, parse as parse_bundle, valid_bundle_relative_path};

type Findings = BTreeSet<Finding<Rule>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TrailerPolicy {
    Required,
    OptionalConsistent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Outcome {
    Approved,
    Rejected,
}

#[derive(Debug)]
struct LoadedBundle {
    path: String,
    bundle: Bundle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IntervalMatch {
    Matched,
    Mismatch,
    Unrelated,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct CheckContext<'a> {
    pub(super) authority: &'a str,
    pub(super) base: &'a str,
    pub(super) head: &'a str,
}

#[derive(Debug, Clone, Copy)]
struct IntervalContext<'a> {
    authority: &'a str,
    base: &'a str,
    head: &'a str,
    pending: MergePending,
    current: &'a Files,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MergePending {
    Active,
    Committed,
}

fn write_line(output: &mut impl Write, line: &str) {
    let _ignored = writeln!(output, "{line}");
}

pub(super) fn commit_parents(root: &Path, revision: &str) -> Option<Vec<String>> {
    let line = git(root, &["rev-list", "--parents", "-n", "1", revision])?;
    let parents: Vec<String> = line.split_whitespace().skip(1).map(String::from).collect();
    parents
        .iter()
        .all(|parent| git_base::is_commit_id(parent))
        .then_some(parents)
}

pub(super) fn tree_of(root: &Path, revision: &str) -> Option<String> {
    let resolved = git(
        root,
        &[
            "rev-parse",
            "--verify",
            "--quiet",
            &format!("{revision}^{{tree}}"),
        ],
    )?;
    git_base::is_commit_id(&resolved).then_some(resolved)
}

pub(super) fn current_worktree_tree(root: &Path) -> Result<String, String> {
    if git(root, &["diff-files", "--quiet", "--ignore-submodules=none"]).is_none() {
        return Err(String::from("작업 사본에 스테이징되지 않은 변경이 있다"));
    }
    let untracked = git_base::untracked_paths(root)?;
    if !untracked.is_empty() {
        let listed: Vec<String> = untracked.into_iter().collect();
        return Err(format!("추적하지 않는 파일이 있다: {}", listed.join(", ")));
    }
    git(root, &["write-tree"])
        .filter(|value| git_base::is_commit_id(value))
        .ok_or_else(|| String::from("현재 인덱스 트리 해시를 계산하지 못했다"))
}

fn pending_merge_head(root: &Path) -> Option<String> {
    let relative = git(root, &["rev-parse", "--git-path", "MERGE_HEAD"])?;
    let contents = std::fs::read_to_string(root.join(relative)).ok()?;
    let heads: Vec<&str> = contents.lines().collect();
    let [incoming] = heads.as_slice() else {
        return None;
    };
    (git_base::is_commit_id(incoming) && commit(root, incoming).as_deref() == Some(*incoming))
        .then(|| String::from(*incoming))
}

fn list_authority_bundles(root: &Path, authority: &str) -> Result<Vec<LoadedBundle>, String> {
    let mut git_cmd = git_base::git(root);
    git_cmd
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .args(["ls-tree", "-r", "-z", authority, "--", BUNDLES_DIR]);
    let output = git_cmd
        .output()
        .map_err(|error| format!("git ls-tree bundles: {error}"))?;
    if !output.status.success() {
        return Err(String::from("승인 묶음 목록을 읽지 못했다"));
    }
    let mut loaded = Vec::new();
    for part in output
        .stdout
        .split(|byte| *byte == 0)
        .filter(|part| !part.is_empty())
    {
        let entry = String::from_utf8_lossy(part);
        let (meta, path) = entry
            .split_once('\t')
            .ok_or_else(|| format!("{BUNDLES_DIR}: ls-tree 형식이 잘못됐다"))?;
        if !meta.starts_with("100644 blob ") || !valid_bundle_relative_path(path) {
            return Err(format!(
                "{path}: 승인 묶음 경로는 {BUNDLES_DIR}/<작업 ID>.toml 일반 파일만 허용한다"
            ));
        }
        let contents = git(root, &["show", &format!("{authority}:{path}")])
            .ok_or_else(|| format!("{path}: 승인 묶음을 읽지 못했다"))?;
        let bundle = parse_bundle(path, &contents)?;
        loaded.push(LoadedBundle {
            path: String::from(path),
            bundle,
        });
    }
    Ok(loaded)
}

fn diff_tree_paths(root: &Path, parent: &str, revision: &str) -> Option<Vec<String>> {
    let mut git_cmd = git_base::git(root);
    git_cmd
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .args([
            "diff-tree",
            "--no-commit-id",
            "--name-only",
            "-r",
            "-z",
            "--no-renames",
            parent,
            revision,
        ]);
    let output = git_cmd.output().ok()?;
    if !output.status.success() {
        return None;
    }
    Some(
        output
            .stdout
            .split(|byte| *byte == 0)
            .filter(|part| !part.is_empty())
            .map(|part| String::from_utf8_lossy(part).into_owned())
            .collect(),
    )
}

fn trusted_bundle_intro(
    root: &Path,
    authority: &str,
    head: &str,
    loaded: &LoadedBundle,
) -> Option<String> {
    if git(root, &["rev-parse", "--is-shallow-repository"]).as_deref() != Some("false") {
        return None;
    }
    let history = git(
        root,
        &[
            "rev-list",
            "--first-parent",
            "--full-history",
            authority,
            "--",
            &loaded.path,
        ],
    )?;
    let pending = pending_master_merge(root, head, authority);
    let mut latest_change: Option<String> = None;
    for revision in history.lines() {
        if !git_base::is_commit_id(revision) {
            return None;
        }
        let parents = commit_parents(root, revision)?;
        let parent = parents.first().map(String::as_str);
        match same_entry(root, revision, parent, &loaded.path) {
            Some(true) => {}
            Some(false) => {
                if parents.len() != 1
                    || (revision == head && !pending)
                    || !loaded
                        .bundle
                        .files
                        .keys()
                        .all(|path| same_entry(root, revision, parent, path) == Some(true))
                {
                    return None;
                }
                if latest_change.is_none() {
                    latest_change = Some(String::from(revision));
                }
            }
            None => return None,
        }
    }
    latest_change
}

fn current_scope_matches_head(root: &Path, head: &str, current: &Files) -> bool {
    let Ok(head_files) = base_files(root, head) else {
        return false;
    };
    let head_scoped: Vec<(&String, &Vec<u8>)> = head_files
        .iter()
        .filter(|(path, _)| in_scope(path) || path.ends_with("Cargo.toml"))
        .collect();
    let current_scoped: Vec<(&String, &Vec<u8>)> = current
        .iter()
        .filter(|(path, _)| in_scope(path) || path.ends_with("Cargo.toml"))
        .collect();
    head_scoped == current_scoped
}

fn check_intro_isolation_path(
    root: &Path,
    base_commit: &str,
    bundle_path: &str,
    intro: &str,
) -> Result<(), (IntervalMatch, String)> {
    let Some(intro_parents) = commit_parents(root, intro) else {
        return Err((
            IntervalMatch::Mismatch,
            String::from("묶음 커밋의 부모를 읽지 못했다"),
        ));
    };
    let intro_parent = intro_parents.first().map(String::as_str);
    if intro_parents.len() != 1 || intro_parent != Some(base_commit) {
        return Err((
            IntervalMatch::Unrelated,
            String::from("구간 불일치: 묶음 커밋의 부모가 base_commit과 다르다"),
        ));
    }
    let intro_paths = intro_parent.and_then(|parent| diff_tree_paths(root, parent, intro));
    let expected = String::from(bundle_path);
    if intro_paths.as_deref() != Some(std::slice::from_ref(&expected)) {
        return Err((
            IntervalMatch::Mismatch,
            String::from("묶음 도입 커밋에 묶음 외 파일 변경이 포함돼 있다"),
        ));
    }
    Ok(())
}

fn check_intro_isolation(
    root: &Path,
    loaded: &LoadedBundle,
    intro: &str,
) -> Result<(), (IntervalMatch, String)> {
    check_intro_isolation_path(root, &loaded.bundle.base_commit, &loaded.path, intro)
}

fn evaluate_committed_merge_head(
    root: &Path,
    context: &IntervalContext<'_>,
    loaded: &LoadedBundle,
    intro: &str,
    merge_head: &str,
) -> (IntervalMatch, Option<String>) {
    let Some(head_parents) = commit_parents(root, context.head) else {
        return (
            IntervalMatch::Mismatch,
            Some(String::from("대상 커밋 부모를 읽지 못했다")),
        );
    };
    if head_parents.len() != 2 {
        return (
            IntervalMatch::Mismatch,
            Some(String::from(
                "병합 커밋이 아닌 커밋에는 묶음을 적용할 수 없다",
            )),
        );
    }
    let first_parent = head_parents.first().map(String::as_str);
    let second_parent = head_parents.get(1).map(String::as_str);
    let base_ok = context.base == intro || context.base == loaded.bundle.base_commit;
    let authority_ok =
        context.authority == intro || first_parent_ancestor(root, context.head, context.authority);
    if first_parent == Some(intro) && authority_ok && second_parent == Some(merge_head) && base_ok {
        if !current_scope_matches_head(root, context.head, context.current) {
            return (
                IntervalMatch::Mismatch,
                Some(String::from(
                    "작업 사본의 소스 파일이 대상 병합 커밋과 다르다",
                )),
            );
        }
        return (IntervalMatch::Matched, None);
    }
    let related = first_parent == Some(intro)
        || second_parent == Some(merge_head)
        || context.base == loaded.bundle.base_commit
        || context.base == intro;
    let status = if related {
        IntervalMatch::Mismatch
    } else {
        IntervalMatch::Unrelated
    };
    (
        status,
        Some(String::from(
            "구간 불일치: 병합 커밋의 첫 부모·둘째 부모·기준 커밋이 묶음과 다르다",
        )),
    )
}

fn evaluate_merge_head_interval(
    root: &Path,
    context: &IntervalContext<'_>,
    loaded: &LoadedBundle,
    intro: &str,
    merge_head: &str,
) -> (IntervalMatch, Option<String>) {
    if let Err((status, reason)) = check_intro_isolation(root, loaded, intro) {
        return (status, Some(reason));
    }
    if context.pending == MergePending::Active {
        return if context.head == context.authority
            && context.authority == intro
            && context.base == intro
            && pending_merge_head(root).as_deref() == Some(merge_head)
        {
            (IntervalMatch::Matched, None)
        } else {
            (
                IntervalMatch::Unrelated,
                Some(String::from(
                    "구간 불일치: 진행 중인 병합의 첫 부모 또는 MERGE_HEAD가 묶음과 다르다",
                )),
            )
        };
    }
    evaluate_committed_merge_head(root, context, loaded, intro, merge_head)
}

fn evaluate_commit_interval(
    root: &Path,
    context: &IntervalContext<'_>,
    loaded: &LoadedBundle,
    target_commit: &str,
) -> (IntervalMatch, Option<String>) {
    if context.pending == MergePending::Active {
        return (
            IntervalMatch::Unrelated,
            Some(String::from(
                "구간 불일치: target_commit 묶음은 이미 생성된 병합 커밋에만 적용된다",
            )),
        );
    }
    let Some(head_parents) = commit_parents(root, context.head) else {
        return (
            IntervalMatch::Mismatch,
            Some(String::from("대상 커밋 부모를 읽지 못했다")),
        );
    };
    let is_merge = head_parents.len() == 2
        && head_parents.first().map(String::as_str) == Some(loaded.bundle.base_commit.as_str());
    if context.head == target_commit && context.base == loaded.bundle.base_commit && is_merge {
        if !current_scope_matches_head(root, context.head, context.current) {
            return (
                IntervalMatch::Mismatch,
                Some(String::from(
                    "작업 사본의 소스 파일이 대상 병합 커밋과 다르다",
                )),
            );
        }
        return (IntervalMatch::Matched, None);
    }
    let related = context.head == target_commit || context.base == loaded.bundle.base_commit;
    let status = if related {
        IntervalMatch::Mismatch
    } else {
        IntervalMatch::Unrelated
    };
    let reason = if !is_merge && context.head == target_commit {
        String::from("병합 커밋이 아닌 커밋에는 묶음을 적용할 수 없다")
    } else {
        format!(
            "구간 불일치: 묶음 구간 {}..{}, 실제 {}..{}",
            loaded.bundle.base_commit, target_commit, context.base, context.head
        )
    };
    (status, Some(reason))
}

fn evaluate_tree_interval(
    root: &Path,
    context: &IntervalContext<'_>,
    loaded: &LoadedBundle,
    target_tree: &str,
) -> (IntervalMatch, Option<String>) {
    if context.pending == MergePending::Active {
        if context.base != loaded.bundle.base_commit || context.head != loaded.bundle.base_commit {
            return (
                IntervalMatch::Unrelated,
                Some(String::from("구간 불일치: 기준 커밋이 묶음과 다르다")),
            );
        }
        return match current_worktree_tree(root) {
            Ok(actual_tree) if actual_tree == target_tree => (IntervalMatch::Matched, None),
            Ok(actual_tree) => (
                IntervalMatch::Mismatch,
                Some(format!(
                    "트리 해시 불일치: 실제 {actual_tree}, 묶음 {target_tree}"
                )),
            ),
            Err(failure) => (IntervalMatch::Mismatch, Some(failure)),
        };
    }
    let Some(head_parents) = commit_parents(root, context.head) else {
        return (
            IntervalMatch::Mismatch,
            Some(String::from("대상 커밋 부모를 읽지 못했다")),
        );
    };
    let is_merge = head_parents.len() == 2
        && head_parents.first().map(String::as_str) == Some(loaded.bundle.base_commit.as_str());
    let actual_tree = tree_of(root, context.head);
    if context.base == loaded.bundle.base_commit
        && is_merge
        && actual_tree.as_deref() == Some(target_tree)
    {
        if !current_scope_matches_head(root, context.head, context.current) {
            return (
                IntervalMatch::Mismatch,
                Some(String::from(
                    "작업 사본의 소스 파일이 대상 병합 커밋과 다르다",
                )),
            );
        }
        return (IntervalMatch::Matched, None);
    }
    let related =
        context.base == loaded.bundle.base_commit || actual_tree.as_deref() == Some(target_tree);
    let status = if related {
        IntervalMatch::Mismatch
    } else {
        IntervalMatch::Unrelated
    };
    let reason = if !is_merge && related {
        String::from("병합 커밋이 아닌 커밋에는 묶음을 적용할 수 없다")
    } else {
        format!(
            "구간 또는 트리 해시 불일치: 묶음 base={} tree={target_tree}",
            loaded.bundle.base_commit
        )
    };
    (status, Some(reason))
}

fn evaluate_interval(
    root: &Path,
    context: &IntervalContext<'_>,
    loaded: &LoadedBundle,
    intro: &str,
) -> (IntervalMatch, Option<String>) {
    match &loaded.bundle.target {
        Target::MergeHead(merge_head) => {
            evaluate_merge_head_interval(root, context, loaded, intro, merge_head)
        }
        Target::Commit(target_commit) => {
            evaluate_commit_interval(root, context, loaded, target_commit)
        }
        Target::Tree(target_tree) => evaluate_tree_interval(root, context, loaded, target_tree),
    }
}

fn check_file_hashes(bundle: &Bundle, before: &Files, current: &Files) -> Vec<String> {
    let mut problems = Vec::new();
    for (path, entry) in &bundle.files {
        match (before.get(path), current.get(path)) {
            (Some(old), Some(new)) => {
                let actual_before = digest(old);
                let actual_after = digest(new);
                if actual_before != entry.before {
                    problems.push(format!(
                        "파일 해시 불일치({path}): before 실제 {actual_before}, 묶음 {}",
                        entry.before
                    ));
                }
                if actual_after != entry.after {
                    problems.push(format!(
                        "파일 해시 불일치({path}): after 실제 {actual_after}, 묶음 {}",
                        entry.after
                    ));
                }
            }
            (None, _) => problems.push(format!("기준 파일 누락: {path}")),
            (_, None) => problems.push(format!("대상 파일 누락: {path}")),
        }
    }
    problems
}

fn check_finding_coverage(
    bundle: &Bundle,
    before: &Files,
    current: &Files,
    actual_names: &BTreeSet<String>,
) -> Vec<String> {
    let scans = RootScans::new(before);
    let mut only_listed = Files::new();
    for path in bundle.files.keys() {
        if let Some(bytes) = current.get(path) {
            only_listed.insert(path.clone(), bytes.clone());
        }
    }
    let listed_names: BTreeSet<String> = scans
        .compare_substituting(&only_listed)
        .into_iter()
        .map(|finding| finding.rule.to_string())
        .collect();
    if listed_names != *actual_names {
        return vec![String::from(
            "묶음에 해시가 없는 파일에서 발생한 지적이 포함돼 있다",
        )];
    }
    let mut problems = Vec::new();
    for path in bundle.files.keys() {
        let mut reduced = only_listed.clone();
        reduced.remove(path);
        let reduced_names: BTreeSet<String> = scans
            .compare_substituting(&reduced)
            .into_iter()
            .map(|finding| finding.rule.to_string())
            .collect();
        if reduced_names == listed_names {
            problems.push(format!("지적에 기여하지 않는 파일 항목이다: {path}"));
        }
    }
    problems
}

fn compare_findings_and_files(
    bundle: &Bundle,
    files: (&Files, &Files),
    actual_findings: &Findings,
) -> Vec<String> {
    let (before, current) = files;
    let mut problems = Vec::new();
    let disallowed: Vec<String> = actual_findings
        .iter()
        .filter(|finding| !approvable_rule(&finding.rule))
        .map(|finding| finding.rule.to_string())
        .collect();
    if !disallowed.is_empty() {
        problems.push(format!("승인 불가 지적 종류: {}", disallowed.join(", ")));
    }
    problems.extend(check_file_hashes(bundle, before, current));
    let actual_names: BTreeSet<String> = actual_findings
        .iter()
        .map(|finding| finding.rule.to_string())
        .collect();
    let unexpected: Vec<&str> = actual_names
        .difference(&bundle.findings)
        .map(String::as_str)
        .collect();
    let missing: Vec<&str> = bundle
        .findings
        .difference(&actual_names)
        .map(String::as_str)
        .collect();
    if !unexpected.is_empty() {
        problems.push(format!(
            "묶음에 없는 추가 지적({}건): {}",
            unexpected.len(),
            unexpected.join(", ")
        ));
    }
    if !missing.is_empty() {
        problems.push(format!(
            "발생하지 않은 묶음 지적({}건): {}",
            missing.len(),
            missing.join(", ")
        ));
    }
    if problems.is_empty() {
        problems.extend(check_finding_coverage(
            bundle,
            before,
            current,
            &actual_names,
        ));
    }
    problems
}

pub(super) fn trailer_problems(
    root: &Path,
    revision: &str,
    expected_bundle_path: &str,
    expected_count: usize,
    policy: TrailerPolicy,
) -> Vec<String> {
    let Some(message) = git(root, &["show", "-s", "--format=%B", revision]) else {
        return vec![format!("병합 커밋 메시지를 읽지 못했다: {revision}")];
    };
    let mut bundle_lines = Vec::new();
    let mut count_lines = Vec::new();
    for line in message.lines().map(str::trim) {
        if let Some(value) = line.strip_prefix("Test-Diff-Bundle:") {
            bundle_lines.push(value.trim().to_owned());
        } else if let Some(value) = line.strip_prefix("Test-Diff-Findings:") {
            count_lines.push(value.trim().to_owned());
        }
    }
    if policy == TrailerPolicy::OptionalConsistent
        && bundle_lines.is_empty()
        && count_lines.is_empty()
    {
        return Vec::new();
    }
    let mut problems = Vec::new();
    if bundle_lines.len() != 1
        || bundle_lines.first().map(String::as_str) != Some(expected_bundle_path)
    {
        let actual = if bundle_lines.is_empty() {
            String::from("없음")
        } else {
            bundle_lines.join(", ")
        };
        problems.push(format!(
            "병합 커밋 메시지 불일치: Test-Diff-Bundle 기대 {expected_bundle_path}, 실제 {actual}"
        ));
    }
    let expected_text = expected_count.to_string();
    if count_lines.len() != 1
        || count_lines.first().map(String::as_str) != Some(expected_text.as_str())
    {
        let actual = if count_lines.is_empty() {
            String::from("없음")
        } else {
            count_lines.join(", ")
        };
        problems.push(format!(
            "병합 커밋 메시지 불일치: Test-Diff-Findings 기대 {expected_text}, 실제 {actual}"
        ));
    }
    problems
}

fn report_untrusted_bundles(
    root: &Path,
    head_files: (&str, &Files),
    actual_names: &BTreeSet<String>,
    trusted_paths: &BTreeSet<String>,
    output: &mut impl Write,
) {
    let (head, current) = head_files;
    let mut seen = BTreeSet::new();
    let head_list = git(
        root,
        &["ls-tree", "-r", "--name-only", head, "--", BUNDLES_DIR],
    );
    let prefix = format!("{BUNDLES_DIR}/");
    for path in head_list
        .iter()
        .flat_map(|text| text.lines().map(String::from))
        .chain(
            current
                .keys()
                .filter(|candidate| candidate.starts_with(&prefix))
                .cloned(),
        )
    {
        if trusted_paths.contains(&path) || !seen.insert(path.clone()) {
            continue;
        }
        let body = git(root, &["show", &format!("{head}:{path}")]).or_else(|| {
            current
                .get(&path)
                .and_then(|bytes| std::str::from_utf8(bytes).ok())
                .map(String::from)
        });
        let Some(text) = body else {
            continue;
        };
        let relevant = match parse_bundle(&path, &text) {
            Ok(bundle) => !bundle.findings.is_disjoint(actual_names),
            Err(_) => true,
        };
        if relevant {
            write_line(
                output,
                &format!("승인 묶음 거부 ({path}): 승인 이력 또는 기준 커밋 신뢰 실패"),
            );
        }
    }
}

#[derive(Default)]
struct EvaluationSummary {
    trusted_paths: BTreeSet<String>,
    matched_paths: Vec<String>,
    approved_paths: Vec<String>,
    diagnostics: Vec<(String, Vec<String>)>,
}

struct BundleInputs<'a> {
    files: (&'a Files, &'a Files),
    raw_findings: &'a Findings,
    actual_names: &'a BTreeSet<String>,
}

fn evaluate_loaded_bundle(
    root: &Path,
    context: &IntervalContext<'_>,
    loaded: &LoadedBundle,
    inputs: &BundleInputs<'_>,
    summary: &mut EvaluationSummary,
) {
    let overlaps = !loaded.bundle.findings.is_disjoint(inputs.actual_names);
    let Some(intro) = trusted_bundle_intro(root, context.authority, context.head, loaded) else {
        if overlaps {
            summary.diagnostics.push((
                loaded.path.clone(),
                vec![String::from("승인 이력 또는 기준 커밋 신뢰 실패")],
            ));
        }
        return;
    };
    summary.trusted_paths.insert(loaded.path.clone());
    let (interval_match, interval_reason) = evaluate_interval(root, context, loaded, &intro);
    match interval_match {
        IntervalMatch::Matched => {
            summary.matched_paths.push(loaded.path.clone());
            let mut problems =
                compare_findings_and_files(&loaded.bundle, inputs.files, inputs.raw_findings);
            if context.pending == MergePending::Committed {
                let policy = match loaded.bundle.target {
                    Target::MergeHead(_) => TrailerPolicy::Required,
                    Target::Commit(_) | Target::Tree(_) => TrailerPolicy::OptionalConsistent,
                };
                problems.extend(trailer_problems(
                    root,
                    context.head,
                    &loaded.path,
                    loaded.bundle.findings.len(),
                    policy,
                ));
            }
            if problems.is_empty() {
                summary.approved_paths.push(loaded.path.clone());
            } else {
                summary.diagnostics.push((loaded.path.clone(), problems));
            }
        }
        IntervalMatch::Mismatch => {
            summary
                .diagnostics
                .push((loaded.path.clone(), interval_reason.into_iter().collect()));
        }
        IntervalMatch::Unrelated => {
            if overlaps {
                summary
                    .diagnostics
                    .push((loaded.path.clone(), interval_reason.into_iter().collect()));
            }
        }
    }
}

fn finish_evaluation(
    root: &Path,
    context: &IntervalContext<'_>,
    actual_names: &BTreeSet<String>,
    summary: EvaluationSummary,
    output: &mut impl Write,
) -> Outcome {
    if summary.matched_paths.len() > 1 {
        write_line(
            output,
            &format!(
                "승인 묶음 거부: 동일 구간에 여러 묶음이 중복 일치한다: {}",
                summary.matched_paths.join(", ")
            ),
        );
        return Outcome::Rejected;
    }
    if summary.approved_paths.len() == 1 && summary.matched_paths.len() == 1 {
        return Outcome::Approved;
    }
    for (path, problems) in summary.diagnostics {
        write_line(
            output,
            &format!("승인 묶음 거부 ({path}): {}", problems.join("; ")),
        );
    }
    report_untrusted_bundles(
        root,
        (context.head, context.current),
        actual_names,
        &summary.trusted_paths,
        output,
    );
    Outcome::Rejected
}

fn report_unrelated_authority_bundles(
    root: &Path,
    check: CheckContext<'_>,
    actual_names: &BTreeSet<String>,
    output: &mut impl Write,
) -> Result<(), String> {
    let bundles = list_authority_bundles(root, check.authority)?;
    let is_merge = commit_parents(root, check.head).is_some_and(|parents| parents.len() == 2);
    let reason = if is_merge {
        "기준 커밋과 현재 HEAD가 승인 묶음 대상 구간과 연결되지 않았다"
    } else {
        "병합 커밋이 아닌 커밋에는 묶음을 적용할 수 없다"
    };
    for loaded in bundles {
        if !loaded.bundle.findings.is_disjoint(actual_names) {
            write_line(
                output,
                &format!("승인 묶음 거부 ({}): {reason}", loaded.path),
            );
        }
    }
    Ok(())
}

pub(super) fn apply_with_authority(
    root: &Path,
    check: CheckContext<'_>,
    files: (&Files, &Files),
    raw_findings: &Findings,
    output: &mut impl Write,
) -> Result<Outcome, String> {
    if raw_findings.is_empty()
        || !git_base::is_commit_id(check.authority)
        || !git_base::is_commit_id(check.base)
        || !git_base::is_commit_id(check.head)
    {
        return Ok(Outcome::Rejected);
    }
    let actual_names: BTreeSet<String> = raw_findings
        .iter()
        .map(|finding| finding.rule.to_string())
        .collect();
    let related = is_ancestor(root, check.authority, check.head)
        || first_parent_ancestor(root, check.head, check.authority);
    if !related {
        report_unrelated_authority_bundles(root, check, &actual_names, output)?;
        return Ok(Outcome::Rejected);
    }
    let bundles = list_authority_bundles(root, check.authority)?;
    let pending = if pending_master_merge(root, check.head, check.authority) {
        MergePending::Active
    } else {
        MergePending::Committed
    };
    let context = IntervalContext {
        authority: check.authority,
        base: check.base,
        head: check.head,
        pending,
        current: files.1,
    };
    let inputs = BundleInputs {
        files,
        raw_findings,
        actual_names: &actual_names,
    };
    let mut summary = EvaluationSummary::default();
    for loaded in &bundles {
        evaluate_loaded_bundle(root, &context, loaded, &inputs, &mut summary);
    }
    Ok(finish_evaluation(
        root,
        &context,
        &actual_names,
        summary,
        output,
    ))
}

pub(super) fn has_candidate_bundle(root: &Path, current: &Files) -> bool {
    let prefix = format!("{BUNDLES_DIR}/");
    if current.keys().any(|path| path.starts_with(&prefix)) {
        return true;
    }
    let Some(head) = commit(root, "HEAD") else {
        return false;
    };
    let head_has = git(
        root,
        &["ls-tree", "-r", "--name-only", &head, "--", BUNDLES_DIR],
    )
    .is_some_and(|list| !list.trim().is_empty());
    if head_has {
        return true;
    }
    trusted_authority(root, &head).is_some_and(|authority| {
        git(
            root,
            &[
                "ls-tree",
                "-r",
                "--name-only",
                &authority,
                "--",
                BUNDLES_DIR,
            ],
        )
        .is_some_and(|list| !list.trim().is_empty())
    })
}

pub(super) fn apply_to(
    root: &Path,
    base: &str,
    files: (&Files, &Files),
    raw_findings: &Findings,
    output: &mut impl Write,
) -> Result<Outcome, String> {
    if raw_findings.is_empty() {
        return Ok(Outcome::Rejected);
    }
    let Some(head) = commit(root, "HEAD") else {
        return Ok(Outcome::Rejected);
    };
    let actual_names: BTreeSet<String> = raw_findings
        .iter()
        .map(|finding| finding.rule.to_string())
        .collect();
    let Some(authority) = trusted_authority(root, &head) else {
        report_untrusted_bundles(
            root,
            (&head, files.1),
            &actual_names,
            &BTreeSet::new(),
            output,
        );
        return Ok(Outcome::Rejected);
    };
    let check = CheckContext {
        authority: &authority,
        base,
        head: &head,
    };
    let outcome = apply_with_authority(root, check, files, raw_findings, output)?;
    if outcome == Outcome::Rejected
        && !is_ancestor(root, &authority, &head)
        && !first_parent_ancestor(root, &head, &authority)
    {
        report_untrusted_bundles(
            root,
            (&head, files.1),
            &actual_names,
            &BTreeSet::new(),
            output,
        );
    }
    Ok(outcome)
}
