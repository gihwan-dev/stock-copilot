use proc_macro2::{Delimiter, Group, Ident, Literal, Punct, Spacing, TokenStream, TokenTree};

use super::finding::{ExpectProblem, Rule};

const BANNED_ATTRIBUTES: [&str; 12] = [
    "allow",
    "warn",
    "deny",
    "forbid",
    "doc",
    "path",
    "ignore",
    "cfg_attr",
    "no_mangle",
    "export_name",
    "link_section",
    "automatically_derived",
];

const TOOL_ATTRIBUTES: [&str; 3] = ["clippy", "rustfmt", "mutants"];

const STANDARD_ROOTS: [&str; 3] = ["core", "std", "alloc"];

const BANNED_MACROS: [&str; 7] = [
    "macro_rules",
    "include",
    "include_str",
    "include_bytes",
    "asm",
    "global_asm",
    "naked_asm",
];

const ENVIRONMENT_MACRO: &str = "env";

const OPTIONAL_ENVIRONMENT_MACRO: &str = "option_env";

const CLOCK_AND_ENVIRONMENT_FUNCTIONS: [&str; 9] = [
    "now",
    "now_utc",
    "now_local",
    "today",
    "elapsed",
    "var",
    "var_os",
    "vars",
    "vars_os",
];

const CLOCK_METHODS: [&str; 1] = ["elapsed"];

const DOMAIN_FLOATS: [&str; 2] = ["f32", "f64"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FloatScope {
    Domain,
    Elsewhere,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ClockScope {
    Allowed,
    Forbidden,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Scope {
    pub(crate) float: FloatScope,
    pub(crate) clock: ClockScope,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Style {
    Outer,
    Inner,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CfgKind {
    Test,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Follower {
    Module,
    Other,
}

pub(crate) fn token_rules(stream: TokenStream, scope: Scope) -> Vec<(usize, Rule)> {
    let tokens: Vec<TokenTree> = stream.into_iter().collect();
    let mut found = Vec::new();
    walk(&tokens, scope, &mut found);
    found
}

fn walk(tokens: &[TokenTree], scope: Scope, found: &mut Vec<(usize, Rule)>) {
    let mut rest = tokens;
    while let Some((first, remaining)) = rest.split_first() {
        let line = first.span().start().line;
        let rules = match first {
            TokenTree::Group(group) => {
                walk(&children(group), scope, found);
                Vec::new()
            }
            TokenTree::Punct(punct) => punct_rules(punct, remaining, scope),
            TokenTree::Ident(ident) => ident_rules(ident, remaining, scope),
            TokenTree::Literal(literal) => literal_rules(literal, scope),
        };
        found.extend(rules.into_iter().map(|rule| (line, rule)));
        rest = remaining;
    }
}

fn children(group: &Group) -> Vec<TokenTree> {
    group.stream().into_iter().collect()
}

fn ident_name(ident: &Ident) -> String {
    let text = ident.to_string();
    match text.strip_prefix("r#") {
        Some(raw) => String::from(raw),
        None => text,
    }
}

fn is_punct(token: &TokenTree, expected: char) -> bool {
    if let TokenTree::Punct(punct) = token {
        punct.as_char() == expected
    } else {
        false
    }
}

fn is_delimited(group: &Group, expected: Delimiter) -> bool {
    group.delimiter() == expected
}

fn punct_rules(punct: &Punct, remaining: &[TokenTree], scope: Scope) -> Vec<Rule> {
    match punct.as_char() {
        '#' => attribute_at(remaining),
        ':' if punct.spacing() == Spacing::Joint => path_call(remaining, scope),
        '.' => method_call(remaining, scope),
        _ => Vec::new(),
    }
}

fn attribute_at(remaining: &[TokenTree]) -> Vec<Rule> {
    match remaining {
        [TokenTree::Punct(bang), TokenTree::Group(group), ..]
            if bang.as_char() == '!' && is_delimited(group, Delimiter::Bracket) =>
        {
            attribute_rules(Style::Inner, &children(group), &[])
        }
        [TokenTree::Group(group), following @ ..] if is_delimited(group, Delimiter::Bracket) => {
            attribute_rules(Style::Outer, &children(group), following)
        }
        _ => Vec::new(),
    }
}

fn clock_rule(name: &str, scope: Scope) -> Vec<Rule> {
    match scope.clock {
        ClockScope::Forbidden => vec![Rule::ClockOrEnvironment(String::from(name))],
        ClockScope::Allowed => Vec::new(),
    }
}

fn path_call(remaining: &[TokenTree], scope: Scope) -> Vec<Rule> {
    match remaining {
        [TokenTree::Punct(second), TokenTree::Ident(ident), ..] if second.as_char() == ':' => {
            let name = ident_name(ident);
            if CLOCK_AND_ENVIRONMENT_FUNCTIONS.contains(&name.as_str()) {
                clock_rule(&name, scope)
            } else {
                Vec::new()
            }
        }
        _ => Vec::new(),
    }
}

fn method_call(remaining: &[TokenTree], scope: Scope) -> Vec<Rule> {
    match remaining {
        [TokenTree::Ident(ident), ..] if CLOCK_METHODS.contains(&ident_name(ident).as_str()) => {
            clock_rule(&ident_name(ident), scope)
        }
        _ => Vec::new(),
    }
}

fn skip_path_separator(tokens: &[TokenTree]) -> &[TokenTree] {
    match tokens {
        [first, second, tail @ ..] if is_punct(first, ':') && is_punct(second, ':') => tail,
        _ => tokens,
    }
}

fn attribute_path(content: &[TokenTree]) -> (Vec<String>, &[TokenTree]) {
    let mut segments = Vec::new();
    let mut rest = skip_path_separator(content);
    while let [TokenTree::Ident(ident), tail @ ..] = rest {
        segments.push(ident_name(ident));
        let after = skip_path_separator(tail);
        if after.len() == tail.len() {
            return (segments, tail);
        }
        rest = after;
    }
    (segments, rest)
}

fn attribute_rules(style: Style, content: &[TokenTree], following: &[TokenTree]) -> Vec<Rule> {
    let (segments, arguments) = attribute_path(content);
    if let ([only], [TokenTree::Group(inner)]) = (segments.as_slice(), arguments)
        && only == "unsafe"
    {
        return attribute_rules(style, &children(inner), following);
    }
    let builtin = match segments.as_slice() {
        [tool, _, ..] if TOOL_ATTRIBUTES.contains(&tool.as_str()) => {
            return vec![Rule::Attribute(tool.clone())];
        }
        [single] => Some(single.as_str()),
        [root, .., last] if STANDARD_ROOTS.contains(&root.as_str()) => Some(last.as_str()),
        _ => None,
    };
    match builtin {
        Some("expect") => expect_rules(style, arguments),
        Some("cfg") => cfg_rules(style, arguments, following),
        Some(name) if BANNED_ATTRIBUTES.contains(&name) => {
            vec![Rule::Attribute(String::from(name))]
        }
        Some(_) | None => Vec::new(),
    }
}

fn is_reason(item: &[TokenTree]) -> bool {
    if let [TokenTree::Ident(key), ..] = item {
        key == "reason"
    } else {
        false
    }
}

fn is_string_reason(item: &[TokenTree]) -> bool {
    if let [TokenTree::Ident(_), equals, TokenTree::Literal(text)] = item {
        let literal = text.to_string();
        is_punct(equals, '=')
            && (literal.starts_with('"') || literal.starts_with("r#") || literal.starts_with("r\""))
    } else {
        false
    }
}

fn is_lint_path(item: &[TokenTree]) -> bool {
    let (segments, rest) = attribute_path(item);
    !segments.is_empty() && rest.is_empty()
}

fn expect_rules(style: Style, arguments: &[TokenTree]) -> Vec<Rule> {
    if style == Style::Inner {
        return vec![Rule::Expect(ExpectProblem::Inner)];
    }
    let [TokenTree::Group(group)] = arguments else {
        return vec![Rule::Expect(ExpectProblem::Malformed)];
    };
    if !is_delimited(group, Delimiter::Parenthesis) {
        return vec![Rule::Expect(ExpectProblem::Malformed)];
    }
    let content = children(group);
    let (reasons, lints): (Vec<&[TokenTree]>, Vec<&[TokenTree]>) = content
        .split(|token| is_punct(token, ','))
        .filter(|item| !item.is_empty())
        .partition(|item| is_reason(item));
    let mut problems = Vec::new();
    if lints.len() > 1 {
        problems.push(Rule::Expect(ExpectProblem::MultipleLints));
    }
    if reasons.is_empty() {
        problems.push(Rule::Expect(ExpectProblem::NoReason));
    }
    if lints.is_empty()
        || reasons.len() > 1
        || !reasons.iter().all(|item| is_string_reason(item))
        || !lints.iter().all(|item| is_lint_path(item))
    {
        problems.push(Rule::Expect(ExpectProblem::Malformed));
    }
    problems
}

fn cfg_kind(arguments: &[TokenTree]) -> CfgKind {
    if let [TokenTree::Group(group)] = arguments
        && is_delimited(group, Delimiter::Parenthesis)
        && let [TokenTree::Ident(predicate)] = children(group).as_slice()
        && predicate == "test"
    {
        CfgKind::Test
    } else {
        CfgKind::Other
    }
}

fn follower(tokens: &[TokenTree]) -> Follower {
    match tokens {
        [hash, TokenTree::Group(group), tail @ ..]
            if is_punct(hash, '#') && is_delimited(group, Delimiter::Bracket) =>
        {
            follower(tail)
        }
        [
            TokenTree::Ident(visibility),
            TokenTree::Group(group),
            tail @ ..,
        ] if visibility == "pub" && is_delimited(group, Delimiter::Parenthesis) => follower(tail),
        [TokenTree::Ident(visibility), tail @ ..] if visibility == "pub" => follower(tail),
        [TokenTree::Ident(keyword), ..] if keyword == "mod" => Follower::Module,
        _ => Follower::Other,
    }
}

fn cfg_rules(style: Style, arguments: &[TokenTree], following: &[TokenTree]) -> Vec<Rule> {
    match (cfg_kind(arguments), style) {
        (CfgKind::Other, Style::Outer | Style::Inner) => vec![Rule::CfgNotTest],
        (CfgKind::Test, Style::Inner) => vec![Rule::CfgTestOutsideModule],
        (CfgKind::Test, Style::Outer) => match follower(following) {
            Follower::Module => Vec::new(),
            Follower::Other => vec![Rule::CfgTestOutsideModule],
        },
    }
}

fn macro_rule(name: &str, scope: Scope) -> Vec<Rule> {
    if BANNED_MACROS.contains(&name) {
        vec![Rule::Macro(String::from(name))]
    } else if name == "cfg" {
        vec![Rule::CfgMacro]
    } else if name == ENVIRONMENT_MACRO || name == OPTIONAL_ENVIRONMENT_MACRO {
        clock_rule(name, scope)
    } else {
        Vec::new()
    }
}

fn ident_rules(ident: &Ident, remaining: &[TokenTree], scope: Scope) -> Vec<Rule> {
    let name = ident_name(ident);
    let mut rules = Vec::new();
    if let [bang, TokenTree::Group(_) | TokenTree::Ident(_), ..] = remaining
        && is_punct(bang, '!')
    {
        rules.extend(macro_rule(&name, scope));
    }
    if name == "use" {
        rules.extend(use_rules(remaining, scope));
    }
    if scope.float == FloatScope::Domain && DOMAIN_FLOATS.contains(&name.as_str()) {
        rules.push(Rule::DomainFloat(name));
    }
    rules
}

fn flatten(tokens: &[TokenTree], flat: &mut Vec<TokenTree>) {
    for token in tokens {
        if let TokenTree::Group(group) = token {
            flatten(&children(group), flat);
        } else {
            flat.push(token.clone());
        }
    }
}

fn use_item_rules(rest: &[TokenTree], scope: Scope) -> Vec<Rule> {
    match rest {
        [TokenTree::Ident(module), TokenTree::Ident(keyword), ..]
            if module == ENVIRONMENT_MACRO && keyword == "as" =>
        {
            clock_rule(ENVIRONMENT_MACRO, scope)
        }
        [TokenTree::Ident(module), first, second, star, ..]
            if module == ENVIRONMENT_MACRO
                && is_punct(first, ':')
                && is_punct(second, ':')
                && is_punct(star, '*') =>
        {
            clock_rule("env::*", scope)
        }
        [TokenTree::Ident(ident), ..] => {
            let name = ident_name(ident);
            if CLOCK_AND_ENVIRONMENT_FUNCTIONS.contains(&name.as_str()) {
                clock_rule(&name, scope)
            } else if name == ENVIRONMENT_MACRO {
                Vec::new()
            } else {
                macro_rule(&name, scope)
            }
        }
        _ => Vec::new(),
    }
}

fn use_rules(remaining: &[TokenTree], scope: Scope) -> Vec<Rule> {
    let end = remaining
        .iter()
        .position(|token| is_punct(token, ';'))
        .unwrap_or(remaining.len());
    let mut flat = Vec::new();
    flatten(remaining.get(..end).unwrap_or(remaining), &mut flat);
    let mut rules = Vec::new();
    let mut rest = flat.as_slice();
    while let Some((_, tail)) = rest.split_first() {
        rules.extend(use_item_rules(rest, scope));
        rest = tail;
    }
    rules
}

fn literal_rules(literal: &Literal, scope: Scope) -> Vec<Rule> {
    if scope.float == FloatScope::Elsewhere {
        return Vec::new();
    }
    let text = literal.to_string();
    if !text.starts_with(|character: char| character.is_ascii_digit()) || text.starts_with("0x") {
        return Vec::new();
    }
    DOMAIN_FLOATS
        .into_iter()
        .filter(|float| text.ends_with(float))
        .map(|float| Rule::DomainFloat(String::from(float)))
        .collect()
}
