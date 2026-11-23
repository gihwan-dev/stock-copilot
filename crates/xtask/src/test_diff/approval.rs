use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::path::Path;

use proc_macro2::{TokenStream, TokenTree};
use quote::ToTokens;
use sha2::{Digest, Sha256};
use syn::parse::Parser;
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
type ApprovedFindings = BTreeMap<String, AssertionChange>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AssertionChange {
    Forbidden,
    Approved,
}

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
    groups: &BTreeMap<Group, ApprovedFindings>,
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
                parents.len() == 1
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

fn assertion_change(row: &toml::Table) -> Result<AssertionChange, String> {
    match row.get("assertion_change") {
        None | Some(toml::Value::Boolean(false)) => Ok(AssertionChange::Forbidden),
        Some(toml::Value::Boolean(true)) => Ok(AssertionChange::Approved),
        Some(_) => Err(format!(
            "{APPROVAL_FILE}: assertion_change는 불린이어야 한다"
        )),
    }
}

fn parse(contents: &str) -> Result<BTreeMap<Group, ApprovedFindings>, String> {
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
    let mut groups: BTreeMap<Group, ApprovedFindings> = BTreeMap::new();
    for entry in entries {
        let row = entry
            .as_table()
            .ok_or_else(|| format!("{APPROVAL_FILE}: 승인 행이 표가 아니다"))?;
        if FIELDS.iter().any(|field| !row.contains_key(*field))
            || row
                .keys()
                .any(|field| !FIELDS.contains(&field.as_str()) && field != "assertion_change")
        {
            return Err(format!("{APPROVAL_FILE}: 승인 필드가 다르다"));
        }
        let task = text(row, "task")?;
        let path = text(row, "path")?;
        let before = text(row, "before_sha256")?;
        let after = text(row, "after_sha256")?;
        let finding = text(row, "finding")?;
        let decision = text(row, "decision")?;
        let _reason = text(row, "reason")?;
        let assertion_change = assertion_change(row)?;
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
        if groups
            .entry(group)
            .or_default()
            .insert(String::from(finding), assertion_change)
            .is_some()
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
                    || macro_name.starts_with("verify")
                    || macro_name == "expect_err"
                    || macro_name == "unwrap_err";
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

fn macro_contains_control(stream: TokenStream) -> bool {
    stream.into_iter().any(|token| match token {
        TokenTree::Ident(name) => [
            "return", "break", "continue", "if", "match", "loop", "while", "for", "unsafe", "try",
            "exit", "abort",
        ]
        .contains(&name.to_string().as_str()),
        TokenTree::Punct(punct) => punct.as_char() == '?',
        TokenTree::Group(group) => macro_contains_control(group.stream()),
        TokenTree::Literal(_) => false,
    })
}

impl<'ast> Visit<'ast> for SetupControl {
    fn visit_macro(&mut self, i: &'ast syn::Macro) {
        let arguments = syn::punctuated::Punctuated::<syn::Expr, syn::Token![,]>::parse_terminated
            .parse2(i.tokens.clone());
        match arguments {
            Ok(expressions) => {
                let mut nested = Self::default();
                for expression in &expressions {
                    nested.visit_expr(expression);
                }
                self.blocked |= nested.blocked;
            }
            Err(_) => self.blocked |= macro_contains_control(i.tokens.clone()),
        }
    }

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

#[derive(Default)]
struct AssertionHeads {
    names: Vec<String>,
    calls: Vec<String>,
    opaque_macros: Vec<String>,
    lazy_operators: Vec<String>,
    trivial: bool,
    unsafe_code: bool,
}

fn canonical_expr(expression: &syn::Expr) -> &syn::Expr {
    if let syn::Expr::Paren(inner) = expression {
        canonical_expr(&inner.expr)
    } else if let syn::Expr::Group(inner) = expression {
        canonical_expr(&inner.expr)
    } else if let syn::Expr::Cast(inner) = expression {
        canonical_expr(&inner.expr)
    } else {
        expression
    }
}

fn constant_boolean(expression: &syn::Expr) -> Option<bool> {
    let canonical = canonical_expr(expression);
    if let syn::Expr::Lit(literal) = canonical
        && let syn::Lit::Bool(value) = &literal.lit
    {
        Some(value.value)
    } else if let syn::Expr::Unary(unary) = canonical
        && let syn::UnOp::Not(_) = unary.op
    {
        constant_boolean(&unary.expr).map(|value| !value)
    } else {
        None
    }
}

fn same_expression(left: &syn::Expr, right: &syn::Expr) -> bool {
    canonical_expr(left).to_token_stream().to_string()
        == canonical_expr(right).to_token_stream().to_string()
}

fn trivial_predicate(expression: &syn::Expr) -> bool {
    if constant_boolean(expression) == Some(true) {
        return true;
    }
    if let syn::Expr::Binary(binary) = canonical_expr(expression) {
        if let syn::BinOp::Eq(_) | syn::BinOp::Le(_) | syn::BinOp::Ge(_) = binary.op {
            return same_expression(&binary.left, &binary.right);
        }
        if let syn::BinOp::Or(_) = binary.op {
            return constant_boolean(&binary.left) == Some(true)
                || constant_boolean(&binary.right) == Some(true);
        }
    }
    false
}

fn trivial_assertion(item: &syn::Macro) -> bool {
    let Some(name) = item
        .path
        .segments
        .last()
        .map(|segment| segment.ident.to_string())
    else {
        return false;
    };
    let Ok(args) = syn::punctuated::Punctuated::<syn::Expr, syn::Token![,]>::parse_terminated
        .parse2(item.tokens.clone())
    else {
        return false;
    };
    match (name.as_str(), args.first(), args.iter().nth(1)) {
        ("assert" | "debug_assert", Some(expr), _) => trivial_predicate(expr),
        ("assert_eq" | "debug_assert_eq", Some(left), Some(right)) => same_expression(left, right),
        (
            "assert_ne" | "debug_assert_ne",
            Some(syn::Expr::Lit(left)),
            Some(syn::Expr::Lit(right)),
        ) => left.to_token_stream().to_string() != right.to_token_stream().to_string(),
        _ => false,
    }
}

impl AssertionHeads {
    fn inspect_arguments(&mut self, tokens: TokenStream) {
        let Ok(arguments) =
            syn::punctuated::Punctuated::<syn::Expr, syn::Token![,]>::parse_terminated
                .parse2(tokens)
        else {
            self.unsafe_code = true;
            return;
        };
        let mut control = SetupControl::default();
        for argument in &arguments {
            control.visit_expr(argument);
            self.visit_expr(argument);
        }
        self.unsafe_code |= control.blocked;
    }
}

impl<'ast> Visit<'ast> for AssertionHeads {
    fn visit_macro(&mut self, i: &'ast syn::Macro) {
        if contains_assertion(i.path.to_token_stream()) {
            self.trivial |= trivial_assertion(i);
            self.names
                .push(format!("macro:{}", i.path.to_token_stream()));
            self.inspect_arguments(i.tokens.clone());
        } else {
            self.opaque_macros.push(i.to_token_stream().to_string());
        }
        visit::visit_macro(self, i);
    }

    fn visit_expr_call(&mut self, i: &'ast syn::ExprCall) {
        if let syn::Expr::Path(path) = i.func.as_ref() {
            let name = path.path.to_token_stream().to_string();
            self.calls.push(name.clone());
            if contains_assertion(path.path.to_token_stream()) {
                self.names.push(format!("call:{name}"));
            }
        } else {
            self.unsafe_code = true;
        }
        visit::visit_expr_call(self, i);
    }

    fn visit_expr_method_call(&mut self, i: &'ast syn::ExprMethodCall) {
        self.calls.push(format!("method:{}", i.method));
        if ["expect_err", "unwrap_err"].contains(&i.method.to_string().as_str()) {
            self.names.push(format!("method:{}", i.method));
        }
        visit::visit_expr_method_call(self, i);
    }

    fn visit_expr_binary(&mut self, i: &'ast syn::ExprBinary) {
        if let syn::BinOp::And(_) | syn::BinOp::Or(_) = i.op {
            self.lazy_operators.push(i.op.to_token_stream().to_string());
        }
        visit::visit_expr_binary(self, i);
    }

    fn visit_expr_block(&mut self, i: &'ast syn::ExprBlock) {
        self.unsafe_code = true;
        visit::visit_expr_block(self, i);
    }
}

fn assertion_heads(statement: &syn::Stmt) -> Option<AssertionHeads> {
    let mut control = SetupControl::default();
    control.visit_stmt(statement);
    let mut heads = AssertionHeads::default();
    heads.visit_stmt(statement);
    (!control.blocked && !heads.unsafe_code && !heads.names.is_empty()).then_some(heads)
}

fn protected_statement(statement: &syn::Stmt) -> bool {
    if let syn::Stmt::Local(local) = statement
        && let syn::Pat::Ident(binding) = &local.pat
        && binding.ident == "check"
        && binding.subpat.is_none()
    {
        return local
            .attrs
            .iter()
            .chain(&binding.attrs)
            .any(|attribute| contains_assertion(attribute.to_token_stream()))
            || local.init.as_ref().is_some_and(|initial| {
                contains_assertion(initial.expr.to_token_stream())
                    || initial
                        .diverge
                        .as_ref()
                        .is_some_and(|(_, branch)| contains_assertion(branch.to_token_stream()))
            });
    }
    contains_assertion(statement.to_token_stream())
}

fn protected(block: &syn::Block) -> Option<Vec<&syn::Stmt>> {
    let mut control = SetupControl::default();
    let mut assertions = Vec::new();
    for statement in &block.stmts {
        if protected_statement(statement) {
            assertions.push(statement);
        } else {
            control.visit_stmt(statement);
        }
    }
    (!control.blocked).then_some(assertions)
}

fn safe_block(
    before: &syn::Block,
    after: &syn::Block,
    kind: ItemKind,
    approval: AssertionChange,
) -> bool {
    let (Some(left), Some(right)) = (protected(before), protected(after)) else {
        return false;
    };
    if kind == ItemKind::Test && left.is_empty() {
        return false;
    }
    if left.len() == right.len() {
        return left
            .iter()
            .zip(&right)
            .all(|(old, new)| same_assertion(old, new, approval));
    }
    if kind != ItemKind::Support
        || approval != AssertionChange::Approved
        || left.len() > right.len()
    {
        return false;
    }
    let mut remaining = right.iter();
    for old in &left {
        loop {
            let Some(new) = remaining.next() else {
                return false;
            };
            if same_assertion(old, new, approval) {
                break;
            }
            if !safe_added_assertion(new) {
                return false;
            }
        }
    }
    remaining.all(|new| safe_added_assertion(new))
}

fn same_assertion(old: &syn::Stmt, new: &syn::Stmt, approval: AssertionChange) -> bool {
    old.to_token_stream().to_string() == new.to_token_stream().to_string()
        || (approval == AssertionChange::Approved
            && assertion_heads(old).zip(assertion_heads(new)).is_some_and(
                |(before_heads, after_heads)| {
                    before_heads.names == after_heads.names
                        && before_heads.calls == after_heads.calls
                        && before_heads.opaque_macros == after_heads.opaque_macros
                        && before_heads.lazy_operators == after_heads.lazy_operators
                        && !after_heads.trivial
                },
            ))
}

fn safe_added_assertion(statement: &syn::Stmt) -> bool {
    assertion_heads(statement).is_some_and(|heads| {
        !heads.trivial
            && heads.opaque_macros.is_empty()
            && heads.names.iter().all(|name| {
                [
                    "macro:assert",
                    "macro:assert_eq",
                    "macro:assert_ne",
                    "macro:debug_assert",
                    "macro:debug_assert_eq",
                    "macro:debug_assert_ne",
                ]
                .contains(&name.as_str())
            })
    })
}

fn safe_item(before: &str, after: &str, kind: ItemKind, approval: AssertionChange) -> bool {
    let (Some((before_context, before_fn)), Some((after_context, after_fn))) =
        (parsed_item(before), parsed_item(after))
    else {
        return false;
    };
    if before_context != after_context
        || before_fn.sig.ident != after_fn.sig.ident
        || before_fn.vis.to_token_stream().to_string() != after_fn.vis.to_token_stream().to_string()
        || (kind == ItemKind::Test
            && before_fn.sig.to_token_stream().to_string()
                != after_fn.sig.to_token_stream().to_string())
    {
        return false;
    }
    safe_block(&before_fn.block, &after_fn.block, kind, approval)
}

fn parsed_impl(fingerprint: &str) -> Option<(&str, syn::ItemImpl)> {
    fingerprint.match_indices('|').find_map(|(index, _)| {
        let (context, tail) = fingerprint.split_at_checked(index)?;
        syn::parse_str::<syn::ItemImpl>(tail.strip_prefix('|')?)
            .ok()
            .map(|item| (context, item))
    })
}

fn equal_attributes(before: &[syn::Attribute], after: &[syn::Attribute]) -> bool {
    before
        .iter()
        .map(ToTokens::to_token_stream)
        .map(|tokens| tokens.to_string())
        .eq(after
            .iter()
            .map(ToTokens::to_token_stream)
            .map(|tokens| tokens.to_string()))
}

fn safe_method(
    before: &syn::ImplItemFn,
    items: &[syn::ImplItem],
    approval: AssertionChange,
) -> bool {
    let mut matches = items.iter().filter_map(|item| {
        if let syn::ImplItem::Fn(new) = item {
            (new.sig.ident == before.sig.ident).then_some(new)
        } else {
            None
        }
    });
    let Some(after) = matches.next() else {
        return false;
    };
    matches.next().is_none()
        && equal_attributes(&before.attrs, &after.attrs)
        && before.vis.to_token_stream().to_string() == after.vis.to_token_stream().to_string()
        && safe_block(&before.block, &after.block, ItemKind::Support, approval)
}

fn safe_constant(before: &syn::ImplItemConst, items: &[syn::ImplItem]) -> bool {
    items.iter().any(|item| {
        if let syn::ImplItem::Const(after) = item {
            let old_expr = before.expr.to_token_stream();
            let new_expr = after.expr.to_token_stream();
            before.ident == after.ident
                && before.vis.to_token_stream().to_string()
                    == after.vis.to_token_stream().to_string()
                && before.ty.to_token_stream().to_string() == after.ty.to_token_stream().to_string()
                && equal_attributes(&before.attrs, &after.attrs)
                && ((!contains_assertion(old_expr.clone())
                    && !contains_assertion(new_expr.clone()))
                    || old_expr.to_string() == new_expr.to_string())
        } else {
            false
        }
    })
}

fn safe_type(before: &syn::ImplItemType, items: &[syn::ImplItem]) -> bool {
    items.iter().any(|item| {
        if let syn::ImplItem::Type(after) = item {
            let old_type = before.ty.to_token_stream();
            let new_type = after.ty.to_token_stream();
            before.ident == after.ident
                && before.vis.to_token_stream().to_string()
                    == after.vis.to_token_stream().to_string()
                && before.generics.to_token_stream().to_string()
                    == after.generics.to_token_stream().to_string()
                && equal_attributes(&before.attrs, &after.attrs)
                && ((!contains_assertion(old_type.clone())
                    && !contains_assertion(new_type.clone()))
                    || old_type.to_string() == new_type.to_string())
        } else {
            false
        }
    })
}

fn safe_impl(before: &str, after: &str, approval: AssertionChange) -> bool {
    let (Some((before_context, before_impl)), Some((after_context, after_impl))) =
        (parsed_impl(before), parsed_impl(after))
    else {
        return false;
    };
    if before_context != after_context {
        return false;
    }
    before_impl.items.iter().all(|old| match old {
        syn::ImplItem::Fn(before_fn) => safe_method(before_fn, &after_impl.items, approval),
        syn::ImplItem::Const(before_const) => safe_constant(before_const, &after_impl.items),
        syn::ImplItem::Type(before_type) => safe_type(before_type, &after_impl.items),
        syn::ImplItem::Macro(_) | syn::ImplItem::Verbatim(_) => after_impl
            .items
            .iter()
            .any(|new| old.to_token_stream().to_string() == new.to_token_stream().to_string()),
        _ => false,
    })
}

fn support<'a>(scan: &'a Scan, id: &str) -> Option<(&'a str, &'a str)> {
    let mut found = scan.modules.iter().flat_map(|(module, record)| {
        record
            .support
            .iter()
            .filter_map(move |((kind, name), value)| {
                ((kind == "fn" || kind == "impl") && format!("{module}::{kind} {name}") == id)
                    .then_some((kind.as_str(), value.as_str()))
            })
    });
    let value = found.next()?;
    found.next().is_none().then_some(value)
}

fn method_names(fingerprint: &str) -> BTreeSet<String> {
    parsed_impl(fingerprint).map_or_else(BTreeSet::new, |(_, item)| {
        item.items
            .iter()
            .filter_map(|method| {
                if let syn::ImplItem::Fn(function) = method {
                    Some(function.sig.ident.to_string())
                } else {
                    None
                }
            })
            .collect()
    })
}

fn new_helper_names(before: &Scan, after: &Scan) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    for (module, record) in &after.modules {
        let previous = before.modules.get(module);
        for ((kind, name), fingerprint) in &record.support {
            let old = previous.and_then(|entry| entry.support.get(&(kind.clone(), name.clone())));
            match kind.as_str() {
                "fn" if old.is_none() => {
                    names.insert(name.clone());
                }
                "impl" => {
                    let old_methods = old.map_or_else(BTreeSet::new, |item| method_names(item));
                    names.extend(method_names(fingerprint).difference(&old_methods).cloned());
                }
                _ => {}
            }
        }
    }
    names
}

fn count_new_helpers(
    stream: TokenStream,
    names: &BTreeSet<String>,
    counts: &mut BTreeMap<String, usize>,
) {
    for token in stream {
        match token {
            TokenTree::Ident(name) => {
                let normalized = crate::rust_names::ident(&name);
                if names.contains(&normalized) {
                    let count = counts.entry(normalized).or_default();
                    *count = count.saturating_add(1);
                }
            }
            TokenTree::Group(group) => count_new_helpers(group.stream(), names, counts),
            TokenTree::Punct(_) | TokenTree::Literal(_) => {}
        }
    }
}

fn helper_tokens(fingerprint: &str) -> Option<TokenStream> {
    if let Some((_, function)) = parsed_item(fingerprint) {
        Some(function.block.to_token_stream())
    } else if let Some((_, item)) = parsed_impl(fingerprint) {
        let mut stream = TokenStream::new();
        for method in &item.items {
            if let syn::ImplItem::Fn(function) = method {
                stream.extend(function.block.to_token_stream());
            }
        }
        Some(stream)
    } else {
        None
    }
}

fn added_new_helpers(before: &str, after: &str, names: &BTreeSet<String>) -> Option<Vec<String>> {
    let (old, new) = (helper_tokens(before)?, helper_tokens(after)?);
    let mut old_counts = BTreeMap::new();
    let mut new_counts = BTreeMap::new();
    count_new_helpers(old, names, &mut old_counts);
    count_new_helpers(new, names, &mut new_counts);
    Some(
        new_counts
            .into_iter()
            .filter_map(|(name, count)| {
                (count > old_counts.get(&name).copied().unwrap_or(0)).then_some(name)
            })
            .collect(),
    )
}

fn references_new_helper(before: &str, after: &str, names: &BTreeSet<String>) -> bool {
    added_new_helpers(before, after, names).is_none_or(|added| !added.is_empty())
}

fn safe_finding(
    finding: &Finding<Rule>,
    before: &Scan,
    after: &Scan,
    approval: AssertionChange,
    new_helpers: &BTreeSet<String>,
) -> bool {
    match &finding.rule {
        Rule::Modified(id) => {
            before
                .tests
                .get(id)
                .zip(after.tests.get(id))
                .is_some_and(|(old, new)| {
                    !references_new_helper(old, new, new_helpers)
                        && safe_item(old, new, ItemKind::Test, approval)
                })
        }
        Rule::SupportModified(id) => support(before, id).zip(support(after, id)).is_some_and(
            |((old_kind, old), (new_kind, new))| {
                !references_new_helper(old, new, new_helpers)
                    && match (old_kind, new_kind) {
                        ("fn", "fn") => safe_item(old, new, ItemKind::Support, approval),
                        ("impl", "impl") => safe_impl(old, new, approval),
                        _ => false,
                    }
            },
        ),
        Rule::Removed(_)
        | Rule::NotListed(_)
        | Rule::CountDecreased { .. }
        | Rule::Unreachable(_)
        | Rule::SupportRemoved(_)
        | Rule::Shadowing(_) => false,
    }
}

#[derive(Default)]
struct DiagnosticControls {
    kinds: BTreeSet<&'static str>,
}

impl<'ast> Visit<'ast> for DiagnosticControls {
    fn visit_macro(&mut self, i: &'ast syn::Macro) {
        let args = syn::punctuated::Punctuated::<syn::Expr, syn::Token![,]>::parse_terminated
            .parse2(i.tokens.clone());
        match args {
            Ok(expressions) => {
                for expression in &expressions {
                    self.visit_expr(expression);
                }
            }
            Err(_) if macro_contains_control(i.tokens.clone()) => {
                self.kinds.insert("해석되지 않은 매크로의 제어 토큰");
            }
            Err(_) => {}
        }
    }
    fn visit_expr_return(&mut self, i: &'ast syn::ExprReturn) {
        self.kinds.insert("return");
        visit::visit_expr_return(self, i);
    }
    fn visit_expr_break(&mut self, i: &'ast syn::ExprBreak) {
        self.kinds.insert("break");
        visit::visit_expr_break(self, i);
    }
    fn visit_expr_continue(&mut self, i: &'ast syn::ExprContinue) {
        self.kinds.insert("continue");
        visit::visit_expr_continue(self, i);
    }
    fn visit_expr_if(&mut self, i: &'ast syn::ExprIf) {
        self.kinds.insert("if");
        visit::visit_expr_if(self, i);
    }
    fn visit_expr_match(&mut self, i: &'ast syn::ExprMatch) {
        self.kinds.insert("match");
        visit::visit_expr_match(self, i);
    }
    fn visit_expr_loop(&mut self, i: &'ast syn::ExprLoop) {
        self.kinds.insert("loop 반복");
        visit::visit_expr_loop(self, i);
    }
    fn visit_expr_while(&mut self, i: &'ast syn::ExprWhile) {
        self.kinds.insert("while 반복");
        visit::visit_expr_while(self, i);
    }
    fn visit_expr_try_block(&mut self, i: &'ast syn::ExprTryBlock) {
        self.kinds.insert("try 블록");
        visit::visit_expr_try_block(self, i);
    }
    fn visit_expr_unsafe(&mut self, i: &'ast syn::ExprUnsafe) {
        self.kinds.insert("unsafe 블록");
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
            self.kinds.insert("exit·abort 호출");
        }
        visit::visit_expr_call(self, i);
    }
    fn visit_expr_for_loop(&mut self, i: &'ast syn::ExprForLoop) {
        self.kinds.insert("for 반복");
        visit::visit_expr_for_loop(self, i);
    }
    fn visit_expr_closure(&mut self, i: &'ast syn::ExprClosure) {
        self.kinds.insert("클로저");
        visit::visit_expr_closure(self, i);
    }
    fn visit_expr_try(&mut self, i: &'ast syn::ExprTry) {
        self.kinds.insert("오류 전파(?)");
        visit::visit_expr_try(self, i);
    }
}

fn control_flow_detail(statements: &[&syn::Stmt]) -> Option<String> {
    let mut control = SetupControl::default();
    for statement in statements {
        control.visit_stmt(statement);
    }
    if !control.blocked {
        return None;
    }
    let mut details = DiagnosticControls::default();
    for statement in statements {
        details.visit_stmt(statement);
    }
    Some(if details.kinds.is_empty() {
        String::from("보호 문장에 제어 흐름이 있다")
    } else {
        format!(
            "보호 문장에 제어 흐름이 있다: {}",
            details.kinds.into_iter().collect::<Vec<_>>().join(", ")
        )
    })
}

fn added_assertion_problem(statement: &syn::Stmt) -> String {
    if let Some(detail) = control_flow_detail(&[statement]) {
        return detail;
    }
    let Some(heads) = assertion_heads(statement) else {
        return String::from("단언의 호출·블록 또는 매크로 인자를 안전하게 분석할 수 없다");
    };
    if heads.trivial {
        return String::from("자명한 참 단언은 추가할 수 없다");
    }
    if !heads.opaque_macros.is_empty() {
        return format!(
            "추가된 단언에 허용되지 않은 내부 매크로가 있다: {:?}",
            heads.opaque_macros
        );
    }
    format!(
        "추가된 단언은 표준 assert 계열 매크로만 허용한다: {:?}",
        heads.names
    )
}

fn assertion_problem(old: &syn::Stmt, new: &syn::Stmt, approval: AssertionChange) -> String {
    if approval == AssertionChange::Forbidden {
        return String::from("단언 변경에 명시 승인이 없다");
    }
    if let Some(detail) = control_flow_detail(&[old, new]) {
        return detail;
    }
    let (Some(before_heads), Some(after_heads)) = (assertion_heads(old), assertion_heads(new))
    else {
        return String::from("단언의 호출·블록 또는 매크로 인자를 안전하게 분석할 수 없다");
    };
    let mut problems = Vec::new();
    if after_heads.trivial {
        problems.push(String::from("자명한 참 단언은 승인할 수 없다"));
    }
    if before_heads.names != after_heads.names {
        problems.push(format!(
            "단언 호출 종류·대상 변경 ({:?} → {:?})",
            before_heads.names, after_heads.names
        ));
    }
    if before_heads.calls != after_heads.calls {
        problems.push(format!(
            "단언 내부 호출 목록 변경 ({:?} → {:?})",
            before_heads.calls, after_heads.calls
        ));
    }
    if before_heads.opaque_macros != after_heads.opaque_macros {
        problems.push(format!(
            "단언 내부 매크로 변경 ({:?} → {:?})",
            before_heads.opaque_macros, after_heads.opaque_macros
        ));
    }
    if before_heads.lazy_operators != after_heads.lazy_operators {
        problems.push(format!(
            "단언 단락 연산 구조 변경 ({:?} → {:?})",
            before_heads.lazy_operators, after_heads.lazy_operators
        ));
    }
    if problems.is_empty() {
        String::from("단언 구조 보존 규칙 위반")
    } else {
        problems.join(", ")
    }
}

fn added_support_assertions_reason(
    left: &[&syn::Stmt],
    right: &[&syn::Stmt],
    approval: AssertionChange,
) -> String {
    let mut remaining = right.iter().enumerate();
    for (old_index, old) in left.iter().enumerate() {
        loop {
            let Some((new_index, new)) = remaining.next() else {
                return format!(
                    "기존 보호된 단언 {}을 보존하지 못함",
                    old_index.saturating_add(1)
                );
            };
            if same_assertion(old, new, approval) {
                break;
            }
            if !safe_added_assertion(new) {
                return format!(
                    "추가된 단언 {}: {}",
                    new_index.saturating_add(1),
                    added_assertion_problem(new)
                );
            }
        }
    }
    for (new_index, new) in remaining {
        if !safe_added_assertion(new) {
            return format!(
                "추가된 단언 {}: {}",
                new_index.saturating_add(1),
                added_assertion_problem(new)
            );
        }
    }
    String::from("함수 구조 보존 규칙 위반")
}

fn unsafe_block_reason(
    before: &syn::Block,
    after: &syn::Block,
    kind: ItemKind,
    approval: AssertionChange,
) -> String {
    let (Some(left), Some(right)) = (protected(before), protected(after)) else {
        return String::from("준비 코드에 제어 흐름 또는 탈출 경로가 있음");
    };
    if kind == ItemKind::Test && left.is_empty() {
        return String::from("기존 테스트에서 보호할 단언을 찾지 못함");
    }
    if left.len() == right.len() {
        if let Some((index, (previous, updated))) = left
            .iter()
            .zip(&right)
            .enumerate()
            .find(|(_, (previous, updated))| !same_assertion(previous, updated, approval))
        {
            return format!(
                "보호된 단언 {}: {}",
                index.saturating_add(1),
                assertion_problem(previous, updated, approval)
            );
        }
        return String::from("함수 구조 보존 규칙 위반");
    }
    if left.len() > right.len() || kind != ItemKind::Support {
        return format!(
            "보호된 단언 수 또는 순서가 달라짐 ({} → {})",
            left.len(),
            right.len()
        );
    }
    if approval != AssertionChange::Approved {
        return format!(
            "단언 변경에 명시 승인이 없다 (보호된 단언 추가: {} → {})",
            left.len(),
            right.len()
        );
    }
    added_support_assertions_reason(&left, &right, approval)
}

fn unsafe_item_reason(
    before: &str,
    after: &str,
    kind: ItemKind,
    approval: AssertionChange,
) -> String {
    let (Some((old_context, old)), Some((new_context, new))) =
        (parsed_item(before), parsed_item(after))
    else {
        return String::from("함수 구조를 파싱할 수 없음");
    };
    if old_context != new_context
        || old.sig.ident != new.sig.ident
        || old.vis.to_token_stream().to_string() != new.vis.to_token_stream().to_string()
        || (kind == ItemKind::Test
            && old.sig.to_token_stream().to_string() != new.sig.to_token_stream().to_string())
    {
        return String::from("함수의 문맥·이름·가시성·테스트 서명 변경");
    }
    unsafe_block_reason(&old.block, &new.block, kind, approval)
}

fn unsafe_method_reason(
    before: &syn::ImplItemFn,
    items: &[syn::ImplItem],
    approval: AssertionChange,
) -> String {
    let matches: Vec<&syn::ImplItemFn> = items
        .iter()
        .filter_map(|item| {
            if let syn::ImplItem::Fn(new) = item {
                (new.sig.ident == before.sig.ident).then_some(new)
            } else {
                None
            }
        })
        .collect();
    match matches.as_slice() {
        [] => format!(
            "impl 메서드 {}의 구조 보존 실패: 메서드가 삭제됨",
            before.sig.ident
        ),
        [after] => {
            if !equal_attributes(&before.attrs, &after.attrs)
                || before.vis.to_token_stream().to_string()
                    != after.vis.to_token_stream().to_string()
            {
                format!(
                    "impl 메서드 {}의 구조 보존 실패: 속성 또는 가시성 변경",
                    before.sig.ident
                )
            } else {
                format!(
                    "impl 메서드 {}의 구조 보존 실패: {}",
                    before.sig.ident,
                    unsafe_block_reason(&before.block, &after.block, ItemKind::Support, approval)
                )
            }
        }
        _ => format!(
            "impl 메서드 {}의 구조 보존 실패: 메서드가 중복됨",
            before.sig.ident
        ),
    }
}

fn new_helper_reason(before: &str, after: &str, helpers: &BTreeSet<String>) -> Option<String> {
    let added = added_new_helpers(before, after, helpers)?;
    (!added.is_empty()).then(|| format!("새 보조 함수 참조가 추가됨: {}", added.join(", ")))
}

fn unsafe_finding_reason(
    finding: &Finding<Rule>,
    before: &Scan,
    after: &Scan,
    approval: AssertionChange,
    helpers: &BTreeSet<String>,
) -> String {
    match &finding.rule {
        Rule::Modified(id) => match before.tests.get(id).zip(after.tests.get(id)) {
            Some((old, new)) if parsed_item(old).is_none() || parsed_item(new).is_none() => {
                String::from("함수 구조를 파싱할 수 없음")
            }
            Some((old, new)) => new_helper_reason(old, new, helpers)
                .unwrap_or_else(|| unsafe_item_reason(old, new, ItemKind::Test, approval)),
            None => String::from("기존 또는 변경된 테스트를 찾을 수 없음"),
        },
        Rule::SupportModified(id) => match support(before, id).zip(support(after, id)) {
            Some((("fn", old), ("fn", new)))
                if parsed_item(old).is_none() || parsed_item(new).is_none() =>
            {
                String::from("함수 구조를 파싱할 수 없음")
            }
            Some((("fn", old), ("fn", new))) => new_helper_reason(old, new, helpers)
                .unwrap_or_else(|| unsafe_item_reason(old, new, ItemKind::Support, approval)),
            Some((("impl", old), ("impl", new))) => match (parsed_impl(old), parsed_impl(new)) {
                (Some((old_context, old_impl)), Some((new_context, new_impl)))
                    if old_context == new_context =>
                {
                    if let Some(reason) = new_helper_reason(old, new, helpers) {
                        return reason;
                    }
                    for item in &old_impl.items {
                        if let syn::ImplItem::Fn(method) = item
                            && !safe_method(method, &new_impl.items, approval)
                        {
                            return unsafe_method_reason(method, &new_impl.items, approval);
                        }
                    }
                    String::from("impl의 기존 상수·타입·항목 구조 보존 실패")
                }
                _ => String::from("impl 문맥 변경 또는 파싱 실패"),
            },
            Some(_) => String::from("보조 항목 종류 변경"),
            None => format!("승인 불가 보조 항목 종류 또는 항목 누락: {id}"),
        },
        Rule::Removed(_)
        | Rule::NotListed(_)
        | Rule::CountDecreased { .. }
        | Rule::Unreachable(_)
        | Rule::SupportRemoved(_)
        | Rule::Shadowing(_) => String::from("승인 불가 지적 종류"),
    }
}

enum GroupDiagnosis {
    FileLevel(String),
    Blockers(Vec<(String, String)>),
}

struct Diagnosis<'a> {
    before: &'a Files,
    current: &'a Files,
    findings: &'a Findings,
    base_scan: Scan,
    helpers: BTreeSet<String>,
}

impl Diagnosis<'_> {
    fn blockers(
        &self,
        isolated: &Findings,
        target: &Scan,
        expected: &ApprovedFindings,
    ) -> GroupDiagnosis {
        let blockers: Vec<(String, String)> = isolated
            .iter()
            .filter_map(|item| {
                let name = item.rule.to_string();
                let approved = expected.get(&name)?;
                (!safe_finding(item, &self.base_scan, target, *approved, &self.helpers)).then(
                    || {
                        (
                            name,
                            unsafe_finding_reason(
                                item,
                                &self.base_scan,
                                target,
                                *approved,
                                &self.helpers,
                            ),
                        )
                    },
                )
            })
            .collect();
        if blockers.is_empty() {
            GroupDiagnosis::FileLevel(String::from("동일 파일의 승인 조건 미충족"))
        } else {
            GroupDiagnosis::Blockers(blockers)
        }
    }

    fn reason(&self, group: &Group, expected: &ApprovedFindings) -> GroupDiagnosis {
        let (Some(old), Some(new)) = (self.before.get(&group.path), self.current.get(&group.path))
        else {
            let missing = match (
                self.before.contains_key(&group.path),
                self.current.contains_key(&group.path),
            ) {
                (false, false) => "before·current",
                (false, true) => "before",
                (true, false) => "current",
                (true, true) => "unknown",
            };
            return GroupDiagnosis::FileLevel(format!(
                "승인 대상 전·후 파일 누락 (path={}, 누락={missing})",
                group.path
            ));
        };
        let actual_before = digest(old);
        let actual_after = digest(new);
        let mut mismatches = Vec::new();
        if actual_before != group.before {
            mismatches.push(format!(
                "before_sha256: 실제 {actual_before}, 승인 {}",
                group.before
            ));
        }
        if actual_after != group.after {
            mismatches.push(format!(
                "after_sha256: 실제 {actual_after}, 승인 {}",
                group.after
            ));
        }
        if !mismatches.is_empty() {
            return GroupDiagnosis::FileLevel(format!(
                "파일 전·후 SHA-256 불일치 (path={}, {})",
                group.path,
                mismatches.join("; ")
            ));
        }
        let mut hybrid = self.before.clone();
        hybrid.insert(group.path.clone(), new.clone());
        let (isolated, _) = compare_files(self.before, &hybrid);
        let actual: BTreeSet<String> = isolated.iter().map(|item| item.rule.to_string()).collect();
        if actual != expected.keys().cloned().collect() || isolated.is_empty() {
            return GroupDiagnosis::FileLevel(format!(
                "지적 집합 불일치 (path={}, 실제 {:?}, 승인 {:?})",
                group.path,
                actual,
                expected.keys().collect::<Vec<_>>()
            ));
        }
        if let Some(missing) = isolated.iter().find(|item| !self.findings.contains(*item)) {
            return GroupDiagnosis::FileLevel(format!("현재 지적 집합에 없음: {}", missing.rule));
        }
        self.blockers(&isolated, &scan(&hybrid), expected)
    }
}

fn format_group_diagnosis(diagnosis: &GroupDiagnosis, target_finding: &str) -> String {
    match diagnosis {
        GroupDiagnosis::FileLevel(message) => message.clone(),
        GroupDiagnosis::Blockers(blockers) => blockers
            .iter()
            .map(|(item_name, detail)| {
                if item_name == target_finding {
                    detail.clone()
                } else {
                    format!("{item_name}: {detail}")
                }
            })
            .collect::<Vec<_>>()
            .join("; "),
    }
}

fn diagnostic_trusted(
    root: &Path,
    authority: &str,
    head: &str,
    groups: &BTreeMap<Group, ApprovedFindings>,
) -> bool {
    let incoming = match merge_base_kind(root) {
        Ok(MergeBase::IncomingDefault(base)) => base == authority,
        Ok(MergeBase::ForkPoint) | Err(_) => false,
    };
    git_base::is_commit_id(authority)
        && git_base::is_commit_id(head)
        && (is_ancestor(root, authority, head)
            || first_parent_ancestor(root, head, authority)
            || incoming)
        && git(root, &["ls-tree", authority, "--", APPROVAL_FILE]).is_some_and(|entry| {
            entry.starts_with("100644 blob ") && entry.ends_with(&format!("\t{APPROVAL_FILE}"))
        })
        && trusted_history(root, authority, head, groups)
}

fn approvable_rule(rule: &Rule) -> bool {
    match rule {
        Rule::Modified(_) | Rule::SupportModified(_) => true,
        Rule::Removed(_)
        | Rule::NotListed(_)
        | Rule::CountDecreased { .. }
        | Rule::Unreachable(_)
        | Rule::SupportRemoved(_)
        | Rule::Shadowing(_) => false,
    }
}

fn append_reason_entry(result: &mut BTreeMap<String, Vec<String>>, name: String, reason: String) {
    let entries = result.entry(name).or_default();
    if !entries.contains(&reason) {
        entries.push(reason);
    }
}

fn join_reasons(entries: BTreeMap<String, Vec<String>>) -> BTreeMap<String, String> {
    entries
        .into_iter()
        .map(|(name, parts)| (name, parts.join("; ")))
        .collect()
}

fn diagnose_group(
    context: &Diagnosis<'_>,
    group: &Group,
    expected: &ApprovedFindings,
    trusted: Option<()>,
    result: &mut BTreeMap<String, Vec<String>>,
) {
    let applicable: Vec<(&Finding<Rule>, String)> = context
        .findings
        .iter()
        .filter_map(|item| {
            let name = item.rule.to_string();
            expected.contains_key(&name).then_some((item, name))
        })
        .collect();
    if applicable.is_empty() {
        return;
    }
    let group_reason = trusted.and_then(|()| {
        applicable
            .iter()
            .any(|(item, _)| approvable_rule(&item.rule))
            .then(|| context.reason(group, expected))
    });
    for (item, name) in applicable {
        let reason = if !approvable_rule(&item.rule) {
            String::from("승인 불가 지적 종류")
        } else if let Some(detail) = &group_reason {
            format_group_diagnosis(detail, &name)
        } else {
            String::from("승인 이력 또는 기준 커밋 신뢰 실패")
        };
        append_reason_entry(result, name, reason);
    }
}

type ReasonMap = BTreeMap<String, Vec<String>>;
type TrustedGroups = BTreeMap<Group, ApprovedFindings>;

pub(super) struct DiagnosisOutcome {
    reasons: ReasonMap,
    trusted_groups: Option<TrustedGroups>,
}

impl DiagnosisOutcome {
    pub(super) fn into_reasons(self) -> BTreeMap<String, String> {
        join_reasons(self.reasons)
    }
}

pub(super) fn diagnose(
    root: &Path,
    authority: Option<&str>,
    head: &str,
    files: (&Files, &Files),
    findings: &Findings,
) -> DiagnosisOutcome {
    let mut reasons = BTreeMap::new();
    if findings.is_empty() {
        return DiagnosisOutcome {
            reasons,
            trusted_groups: None,
        };
    }
    let source = authority
        .map(String::from)
        .or_else(|| commit(root, MASTER))
        .unwrap_or_else(|| String::from(head));
    let Some(contents) = git(root, &["show", &format!("{source}:{APPROVAL_FILE}")]) else {
        return DiagnosisOutcome {
            reasons,
            trusted_groups: None,
        };
    };
    let Ok(groups) = parse(&contents) else {
        return DiagnosisOutcome {
            reasons,
            trusted_groups: None,
        };
    };
    let trusted =
        (authority.is_some() && diagnostic_trusted(root, &source, head, &groups)).then_some(());
    let (before, current) = files;
    let base_scan = scan(before);
    let helpers = new_helper_names(&base_scan, &scan(current));
    let context = Diagnosis {
        before,
        current,
        findings,
        base_scan,
        helpers,
    };
    for (group, expected) in &groups {
        diagnose_group(&context, group, expected, trusted, &mut reasons);
    }
    DiagnosisOutcome {
        reasons,
        trusted_groups: trusted.map(|()| groups),
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
    let new_helpers = new_helper_names(&base_scan, &scan(current));
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
        if actual == expected.keys().cloned().collect()
            && !isolated.is_empty()
            && isolated.iter().all(|item| {
                findings.contains(item)
                    && expected
                        .get(&item.rule.to_string())
                        .is_some_and(|approval| {
                            safe_finding(item, &base_scan, &target_scan, *approval, &new_helpers)
                        })
            })
        {
            findings = findings.difference(&isolated).cloned().collect();
        }
    }
    Ok(findings)
}

fn untrusted_approval_rows(
    contents: &str,
    findings: &Findings,
    trusted_groups: Option<&BTreeMap<Group, ApprovedFindings>>,
) -> BTreeMap<String, String> {
    let Ok(groups) = parse(contents) else {
        return BTreeMap::new();
    };
    findings
        .iter()
        .filter_map(|finding| {
            let name = finding.rule.to_string();
            let has_untrusted = groups.iter().any(|(group, expected)| {
                expected.contains_key(&name)
                    && trusted_groups.and_then(|trusted| trusted.get(group)) != Some(expected)
            });
            has_untrusted.then(|| {
                let reason = if approvable_rule(&finding.rule) {
                    String::from("승인 이력 또는 기준 커밋 신뢰 실패")
                } else {
                    String::from("승인 불가 지적 종류")
                };
                (name, reason)
            })
        })
        .collect()
}

fn untrusted_observed_rows(
    root: &Path,
    head: &str,
    current: &Files,
    findings: &Findings,
    trusted_groups: Option<&BTreeMap<Group, ApprovedFindings>>,
) -> BTreeMap<String, String> {
    if findings.is_empty() {
        return BTreeMap::new();
    }
    let committed = git(root, &["show", &format!("{head}:{APPROVAL_FILE}")]);
    let working = current
        .get(APPROVAL_FILE)
        .and_then(|bytes| std::str::from_utf8(bytes).ok());
    let mut result = BTreeMap::new();
    for contents in [committed.as_deref(), working].into_iter().flatten() {
        for (name, reason) in untrusted_approval_rows(contents, findings, trusted_groups) {
            result.entry(name).or_insert(reason);
        }
    }
    result
}

pub(super) fn apply_to(
    root: &Path,
    files: (&Files, &Files),
    findings: Findings,
    _base_use: BaseUse,
    output: &mut impl Write,
) -> Result<Findings, String> {
    let (before, current) = files;
    let eligible = findings
        .iter()
        .any(|finding| approvable_rule(&finding.rule));
    let Some(head) = commit(root, "HEAD") else {
        return Ok(findings);
    };
    let authority = trusted_authority(root, &head);
    let remaining = match (eligible, authority.as_deref()) {
        (true, Some(selected)) => {
            apply_with_authority(root, selected, &head, (before, current), findings)?
        }
        _ => findings,
    };
    let mut outcome = diagnose(
        root,
        authority.as_deref(),
        &head,
        (before, current),
        &remaining,
    );
    for (finding, reason) in untrusted_observed_rows(
        root,
        &head,
        current,
        &remaining,
        outcome.trusted_groups.as_ref(),
    ) {
        append_reason_entry(&mut outcome.reasons, finding, reason);
    }
    for (finding, reason) in outcome.into_reasons() {
        if writeln!(output, "승인 거부: {finding}: {reason}").is_err() {
            break;
        }
    }
    Ok(remaining)
}

pub(super) fn apply(
    root: &Path,
    before: &Files,
    current: &Files,
    findings: Findings,
    base_use: BaseUse,
) -> Result<Findings, String> {
    apply_to(
        root,
        (before, current),
        findings,
        base_use,
        &mut std::io::stderr().lock(),
    )
}
