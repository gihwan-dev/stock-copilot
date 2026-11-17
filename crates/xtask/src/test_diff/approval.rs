use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use proc_macro2::{TokenStream, TokenTree};
use quote::ToTokens;
use sha2::{Digest, Sha256};
use syn::visit::{self, Visit};

use crate::check_support::Finding;
use crate::git_base::{self, BaseUse};
use crate::git_hooks::commit_base::{MergeBase, merge_base_kind};

use super::{Files, Rule, Scan, compare_files, scan};

const APPROVAL_FILE: &str = "harness/test-diff/approvals.toml";
const MASTER: &str = "refs/remotes/origin/master";
const FIELDS: [&str; 7] = [
    "task",
    "path",
    "before_sha256",
    "after_sha256",
    "finding",
    "decision",
    "reason",
];

type Findings = BTreeSet<Finding<Rule>>;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Group {
    path: String,
    before: String,
    after: String,
}

fn git(root: &Path, args: &[&str]) -> Option<String> {
    let mut command = git_base::git(root);
    command
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1");
    let result = command.args(args).output().ok()?;
    result
        .status
        .success()
        .then(|| String::from_utf8_lossy(&result.stdout).trim().to_owned())
}

fn is_ancestor(root: &Path, base: &str, head: &str) -> bool {
    git(root, &["merge-base", "--is-ancestor", base, head]).is_some()
}

fn first_parent_ancestor(root: &Path, ancestor: &str, head: &str) -> bool {
    git(root, &["rev-list", "--first-parent", head])
        .is_some_and(|history| history.lines().any(|revision| revision == ancestor))
}

fn commit(root: &Path, reference: &str) -> Option<String> {
    let resolved = git(
        root,
        &[
            "rev-parse",
            "--verify",
            "--quiet",
            &format!("{reference}^{{commit}}"),
        ],
    )?;
    git_base::is_commit_id(&resolved).then_some(resolved)
}

fn pending_master_merge(root: &Path, head: &str, authority: &str) -> bool {
    if head != authority
        || git(root, &["symbolic-ref", "--quiet", "HEAD"]).as_deref() != Some("refs/heads/master")
    {
        return false;
    }
    let Some(path) = git(root, &["rev-parse", "--git-path", "MERGE_HEAD"]) else {
        return false;
    };
    let Ok(contents) = std::fs::read_to_string(root.join(path)) else {
        return false;
    };
    let heads: Vec<&str> = contents.lines().collect();
    let [incoming] = heads.as_slice() else {
        return false;
    };
    git_base::is_commit_id(incoming) && commit(root, incoming).as_deref() == Some(*incoming)
}

fn trusted_authority(root: &Path, head: &str) -> Option<String> {
    let master = commit(root, MASTER)?;
    let branch = git(root, &["symbolic-ref", "--quiet", "HEAD"]);
    if master == head
        && branch
            .as_deref()
            .is_none_or(|name| name == "refs/heads/master")
        && !pending_master_merge(root, head, &master)
    {
        commit(root, &format!("{master}^1"))
    } else {
        Some(master)
    }
}

fn same_entry(root: &Path, revision: &str, parent: Option<&str>, path: &str) -> Option<bool> {
    let current = git(root, &["ls-tree", revision, "--", path])?;
    let previous = match parent {
        Some(ancestor) => git(root, &["ls-tree", ancestor, "--", path])?,
        None => String::new(),
    };
    Some(current == previous)
}

fn trusted_history(
    root: &Path,
    authority: &str,
    head: &str,
    groups: &BTreeMap<Group, BTreeSet<String>>,
) -> bool {
    if git(root, &["rev-parse", "--is-shallow-repository"]).as_deref() != Some("false") {
        return false;
    }
    let Some(history) = git(
        root,
        &[
            "rev-list",
            "--first-parent",
            "--full-history",
            authority,
            "--",
            APPROVAL_FILE,
        ],
    ) else {
        return false;
    };
    let pending_merge = pending_master_merge(root, head, authority);
    history.lines().all(|revision| {
        if !git_base::is_commit_id(revision) {
            return false;
        }
        let Some(line) = git(root, &["rev-list", "--parents", "-n", "1", revision]) else {
            return false;
        };
        let parents: Vec<&str> = line.split_whitespace().skip(1).collect();
        let parent = parents.first().copied();
        match same_entry(root, revision, parent, APPROVAL_FILE) {
            Some(true) => true,
            Some(false) => {
                parents.len() <= 1
                    && (revision != head || pending_merge)
                    && groups
                        .keys()
                        .all(|group| same_entry(root, revision, parent, &group.path) == Some(true))
            }
            None => false,
        }
    })
}

fn text<'a>(table: &'a toml::Table, key: &str) -> Result<&'a str, String> {
    table
        .get(key)
        .and_then(toml::Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| format!("{APPROVAL_FILE}: {key}가 없다"))
}

fn valid_hash(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn valid_task(task: &str) -> bool {
    task.split_once('-').is_some_and(|(prefix, number)| {
        !prefix.is_empty()
            && prefix.bytes().all(|byte| byte.is_ascii_uppercase())
            && !number.is_empty()
            && number.bytes().all(|byte| byte.is_ascii_digit())
    })
}

fn valid_path(path: &str) -> bool {
    Path::new(path)
        .extension()
        .is_some_and(|extension| extension == "rs")
        && path
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"/._-".contains(&byte))
        && !path.starts_with('/')
        && !path
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
}

fn parse(contents: &str) -> Result<BTreeMap<Group, BTreeSet<String>>, String> {
    let table: toml::Table = contents
        .parse()
        .map_err(|error: toml::de::Error| format!("{APPROVAL_FILE}: {error}"))?;
    if table.len() != 1 {
        return Err(format!("{APPROVAL_FILE}: 승인 목록만 허용한다"));
    }
    let entries = table
        .get("approval")
        .and_then(toml::Value::as_array)
        .ok_or_else(|| format!("{APPROVAL_FILE}: approval 목록이 없다"))?;
    let mut groups: BTreeMap<Group, BTreeSet<String>> = BTreeMap::new();
    for entry in entries {
        let row = entry
            .as_table()
            .ok_or_else(|| format!("{APPROVAL_FILE}: 승인 행이 표가 아니다"))?;
        if row.len() != FIELDS.len() || FIELDS.iter().any(|field| !row.contains_key(*field)) {
            return Err(format!("{APPROVAL_FILE}: 승인 필드가 다르다"));
        }
        let task = text(row, "task")?;
        let path = text(row, "path")?;
        let before = text(row, "before_sha256")?;
        let after = text(row, "after_sha256")?;
        let finding = text(row, "finding")?;
        let decision = text(row, "decision")?;
        let _reason = text(row, "reason")?;
        if !valid_task(task)
            || !valid_path(path)
            || !valid_hash(before)
            || !valid_hash(after)
            || before == after
            || decision != "approved"
        {
            return Err(format!("{APPROVAL_FILE}: 승인 값이 잘못됐다"));
        }
        let group = Group {
            path: String::from(path),
            before: String::from(before),
            after: String::from(after),
        };
        if !groups
            .entry(group)
            .or_default()
            .insert(String::from(finding))
        {
            return Err(format!("{APPROVAL_FILE}: 중복 승인이다"));
        }
    }
    Ok(groups)
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn contains_assertion(stream: TokenStream) -> bool {
    let mut tokens = stream.into_iter().peekable();
    while let Some(token) = tokens.next() {
        match token {
            TokenTree::Ident(name) => {
                let macro_name = name.to_string();
                let is_bang = match tokens.peek() {
                    Some(TokenTree::Punct(punct)) => punct.as_char() == '!',
                    Some(TokenTree::Group(_) | TokenTree::Ident(_) | TokenTree::Literal(_))
                    | None => false,
                };
                let named_check = macro_name.starts_with("assert")
                    || macro_name == "check"
                    || macro_name.starts_with("check_")
                    || macro_name.starts_with("verify");
                if named_check
                    || ((macro_name.contains("assert")
                        || macro_name == "panic"
                        || macro_name == "unreachable")
                        && is_bang)
                {
                    return true;
                }
            }
            TokenTree::Group(group) if contains_assertion(group.stream()) => return true,
            TokenTree::Group(_) | TokenTree::Punct(_) | TokenTree::Literal(_) => {}
        }
    }
    false
}

#[derive(Default)]
struct SetupControl {
    blocked: bool,
}

impl<'ast> Visit<'ast> for SetupControl {
    fn visit_expr_return(&mut self, i: &'ast syn::ExprReturn) {
        self.blocked = true;
        visit::visit_expr_return(self, i);
    }
    fn visit_expr_break(&mut self, i: &'ast syn::ExprBreak) {
        self.blocked = true;
        visit::visit_expr_break(self, i);
    }
    fn visit_expr_continue(&mut self, i: &'ast syn::ExprContinue) {
        self.blocked = true;
        visit::visit_expr_continue(self, i);
    }
    fn visit_expr_if(&mut self, i: &'ast syn::ExprIf) {
        self.blocked = true;
        visit::visit_expr_if(self, i);
    }
    fn visit_expr_match(&mut self, i: &'ast syn::ExprMatch) {
        self.blocked = true;
        visit::visit_expr_match(self, i);
    }
    fn visit_expr_loop(&mut self, i: &'ast syn::ExprLoop) {
        self.blocked = true;
        visit::visit_expr_loop(self, i);
    }
    fn visit_expr_while(&mut self, i: &'ast syn::ExprWhile) {
        self.blocked = true;
        visit::visit_expr_while(self, i);
    }
    fn visit_expr_for_loop(&mut self, i: &'ast syn::ExprForLoop) {
        self.blocked = true;
        visit::visit_expr_for_loop(self, i);
    }
    fn visit_expr_closure(&mut self, i: &'ast syn::ExprClosure) {
        self.blocked = true;
        visit::visit_expr_closure(self, i);
    }
    fn visit_expr_try(&mut self, i: &'ast syn::ExprTry) {
        self.blocked = true;
        visit::visit_expr_try(self, i);
    }
    fn visit_expr_try_block(&mut self, i: &'ast syn::ExprTryBlock) {
        self.blocked = true;
        visit::visit_expr_try_block(self, i);
    }
    fn visit_expr_unsafe(&mut self, i: &'ast syn::ExprUnsafe) {
        self.blocked = true;
        visit::visit_expr_unsafe(self, i);
    }
    fn visit_expr_call(&mut self, i: &'ast syn::ExprCall) {
        if let syn::Expr::Path(path) = i.func.as_ref()
            && path
                .path
                .segments
                .last()
                .is_some_and(|segment| segment.ident == "exit" || segment.ident == "abort")
        {
            self.blocked = true;
        }
        visit::visit_expr_call(self, i);
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ItemKind {
    Test,
    Support,
}

fn parsed_item(fingerprint: &str) -> Option<(&str, syn::ItemFn)> {
    fingerprint.match_indices('|').find_map(|(index, _)| {
        let (context, tail) = fingerprint.split_at_checked(index)?;
        syn::parse_str::<syn::ItemFn>(tail.strip_prefix('|')?)
            .ok()
            .map(|item| (context, item))
    })
}

fn safe_item(before: &str, after: &str, kind: ItemKind) -> bool {
    let (Some((before_context, before_fn)), Some((after_context, after_fn))) =
        (parsed_item(before), parsed_item(after))
    else {
        return false;
    };
    if before_context != after_context {
        return false;
    }
    if before_fn.sig.to_token_stream().to_string() != after_fn.sig.to_token_stream().to_string()
        || before_fn.vis.to_token_stream().to_string() != after_fn.vis.to_token_stream().to_string()
    {
        return false;
    }
    let protected = |item: &syn::ItemFn| {
        let mut control = SetupControl::default();
        let mut assertions = Vec::new();
        for statement in &item.block.stmts {
            let tokens = statement.to_token_stream();
            if contains_assertion(tokens.clone()) {
                assertions.push(tokens.to_string());
            } else {
                control.visit_stmt(statement);
            }
        }
        (!control.blocked).then_some(assertions)
    };
    match (protected(&before_fn), protected(&after_fn)) {
        (Some(left), Some(right)) => {
            left == right && (kind == ItemKind::Support || !left.is_empty())
        }
        _ => false,
    }
}

fn support<'a>(scan: &'a Scan, id: &str) -> Option<&'a str> {
    let mut found = scan.modules.iter().flat_map(|(module, record)| {
        record
            .support
            .iter()
            .filter_map(move |((kind, name), value)| {
                (kind == "fn" && format!("{module}::{kind} {name}") == id).then_some(value.as_str())
            })
    });
    let value = found.next()?;
    found.next().is_none().then_some(value)
}

fn safe_finding(finding: &Finding<Rule>, before: &Scan, after: &Scan) -> bool {
    match &finding.rule {
        Rule::Modified(id) => before
            .tests
            .get(id)
            .zip(after.tests.get(id))
            .is_some_and(|(old, new)| safe_item(old, new, ItemKind::Test)),
        Rule::SupportModified(id) => support(before, id)
            .zip(support(after, id))
            .is_some_and(|(old, new)| safe_item(old, new, ItemKind::Support)),
        Rule::Removed(_)
        | Rule::NotListed(_)
        | Rule::CountDecreased { .. }
        | Rule::Unreachable(_)
        | Rule::SupportRemoved(_)
        | Rule::Shadowing(_) => false,
    }
}

pub(super) fn apply_with_authority(
    root: &Path,
    authority: &str,
    head: &str,
    files: (&Files, &Files),
    mut findings: Findings,
) -> Result<Findings, String> {
    let (before, current) = files;
    if !git_base::is_commit_id(authority) || !git_base::is_commit_id(head) {
        return Ok(findings);
    }
    let related =
        is_ancestor(root, authority, head) || first_parent_ancestor(root, head, authority);
    let incoming = !related
        && match merge_base_kind(root) {
            Ok(MergeBase::IncomingDefault(base)) => base == authority,
            Ok(MergeBase::ForkPoint) | Err(_) => false,
        };
    if !related && !incoming {
        return Ok(findings);
    }
    let Some(entry) = git(root, &["ls-tree", authority, "--", APPROVAL_FILE]) else {
        return Ok(findings);
    };
    if !entry.starts_with("100644 blob ") || !entry.ends_with(&format!("\t{APPROVAL_FILE}")) {
        return Ok(findings);
    }
    let Some(contents) = git(root, &["show", &format!("{authority}:{APPROVAL_FILE}")]) else {
        return Ok(findings);
    };
    let groups = parse(&contents)?;
    if !trusted_history(root, authority, head, &groups) {
        return Ok(findings);
    }
    let base_scan = scan(before);
    for (group, expected) in groups {
        let Some((old, new)) = before.get(&group.path).zip(current.get(&group.path)) else {
            continue;
        };
        if digest(old) != group.before || digest(new) != group.after {
            continue;
        }
        let mut hybrid = before.clone();
        hybrid.insert(group.path, new.clone());
        let (isolated, _) = compare_files(before, &hybrid);
        let actual: BTreeSet<String> = isolated.iter().map(|item| item.rule.to_string()).collect();
        let target_scan = scan(&hybrid);
        if actual == expected
            && !isolated.is_empty()
            && isolated
                .iter()
                .all(|item| findings.contains(item) && safe_finding(item, &base_scan, &target_scan))
        {
            findings = findings.difference(&isolated).cloned().collect();
        }
    }
    Ok(findings)
}

pub(super) fn apply(
    root: &Path,
    before: &Files,
    current: &Files,
    findings: Findings,
    _base_use: BaseUse,
) -> Result<Findings, String> {
    if !findings.iter().any(|finding| match &finding.rule {
        Rule::Modified(_) | Rule::SupportModified(_) => true,
        Rule::Removed(_)
        | Rule::NotListed(_)
        | Rule::CountDecreased { .. }
        | Rule::Unreachable(_)
        | Rule::SupportRemoved(_)
        | Rule::Shadowing(_) => false,
    }) {
        return Ok(findings);
    }
    let Some(head) = commit(root, "HEAD") else {
        return Ok(findings);
    };
    let Some(authority) = trusted_authority(root, &head) else {
        return Ok(findings);
    };
    apply_with_authority(root, &authority, &head, (before, current), findings)
}
