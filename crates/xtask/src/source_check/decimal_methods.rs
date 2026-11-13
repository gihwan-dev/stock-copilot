use std::collections::BTreeSet;

use proc_macro2::{Delimiter, TokenStream, TokenTree};

use super::failure::CheckFailure;
use super::finding::{Finding, Rule};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct DecimalRules {
    rounding: BTreeSet<String>,
    parsing: BTreeSet<String>,
    exact: BTreeSet<String>,
    generic: BTreeSet<String>,
    arithmetic: BTreeSet<String>,
    floats: BTreeSet<String>,
    arithmetic_files: BTreeSet<String>,
    rounding_files: BTreeSet<String>,
    parsing_files: BTreeSet<String>,
}

fn names(table: &toml::Table, key: &str) -> Result<BTreeSet<String>, CheckFailure> {
    match table.get(key) {
        None => Ok(BTreeSet::new()),
        Some(value) => value
            .as_array()
            .and_then(|values| {
                values
                    .iter()
                    .map(|item| item.as_str().map(String::from))
                    .collect()
            })
            .ok_or_else(|| {
                CheckFailure::Allowlist(format!("decimal-methods.{key}: 문자열 목록이 필요하다"))
            }),
    }
}

fn paths(table: &toml::Table, key: &str) -> Result<BTreeSet<String>, CheckFailure> {
    match table.get(key) {
        None => Ok(BTreeSet::new()),
        Some(value) => value
            .as_array()
            .and_then(|values| {
                values
                    .iter()
                    .map(|entry| {
                        let path = entry.get("path")?.as_str()?;
                        let reason = entry.get("reason")?.as_str()?;
                        (!path.is_empty() && !reason.trim().is_empty()).then(|| String::from(path))
                    })
                    .collect()
            })
            .ok_or_else(|| {
                CheckFailure::Allowlist(format!("{key}: path와 reason 목록이 필요하다"))
            }),
    }
}

impl DecimalRules {
    pub(crate) fn parse(table: &toml::Table) -> Result<Self, CheckFailure> {
        let empty = toml::Table::new();
        let methods = match table.get("decimal-methods") {
            Some(value) => value.as_table().ok_or_else(|| {
                CheckFailure::Allowlist(String::from("decimal-methods: 표가 필요하다"))
            })?,
            None => &empty,
        };
        Ok(Self {
            rounding: names(methods, "rounding")?,
            parsing: names(methods, "parsing")?,
            exact: names(methods, "exact-parsing")?,
            generic: names(methods, "generic-parsing")?,
            arithmetic: names(methods, "arithmetic")?,
            floats: names(methods, "float-conversion")?,
            arithmetic_files: paths(table, "decimal-arithmetic")?,
            rounding_files: paths(table, "decimal-rounding")?,
            parsing_files: paths(table, "decimal-parsing")?,
        })
    }

    pub(crate) fn findings(&self, path: &str, stream: TokenStream) -> Vec<Finding> {
        let syntax = syn::parse2::<syn::File>(stream.clone());
        let tokens: Vec<TokenTree> = stream.into_iter().collect();
        let mut aliases = BTreeSet::from([String::from("Decimal")]);
        aliases_in(&tokens, &mut aliases);
        let mut findings = Vec::new();
        self.walk(path, &tokens, &aliases, &mut findings);
        if let Ok(file) = syntax {
            findings.extend(super::decimal_arithmetic::findings(
                self, path, &file, &aliases,
            ));
        }
        findings
    }

    pub(super) fn arithmetic_forbidden(&self, path: &str, method: &str) -> bool {
        self.arithmetic.contains(method) && !self.arithmetic_files.contains(path)
    }

    fn walk(
        &self,
        path: &str,
        tokens: &[TokenTree],
        aliases: &BTreeSet<String>,
        findings: &mut Vec<Finding>,
    ) {
        for (index, token) in tokens.iter().enumerate() {
            if let TokenTree::Group(group) = token {
                self.walk(
                    path,
                    &group.stream().into_iter().collect::<Vec<_>>(),
                    aliases,
                    findings,
                );
            }
            let TokenTree::Ident(ident) = token else {
                continue;
            };
            let method = ident.to_string().trim_start_matches("r#").to_owned();
            let before = tokens.get(..index).unwrap_or(&[]);
            let after = tokens.get(index.saturating_add(1)..).unwrap_or(&[]);
            let receiver = decimal_receiver(before, aliases);
            let dot = before.last().is_some_and(|previous| punct(previous, '.'));
            let call = after.first().is_some_and(|next| match next {
                TokenTree::Group(group) => group.delimiter() == Delimiter::Parenthesis,
                TokenTree::Ident(_) | TokenTree::Punct(_) | TokenTree::Literal(_) => false,
            });
            let rounding = self.rounding.contains(&method)
                && ((dot && call) || receiver)
                && !self.rounding_files.contains(path);
            let lossy = self.parsing.contains(&method) && receiver;
            let exact =
                self.exact.contains(&method) && receiver && !self.parsing_files.contains(path);
            let generic = self.generic.contains(&method) && decimal_turbofish(after, aliases);
            let arithmetic = receiver && self.arithmetic_forbidden(path, &method);
            let float_conversion = self.floats.contains(&method) && (receiver || dot);
            if rounding || lossy || exact || generic || arithmetic || float_conversion {
                findings.push(Finding::at_line(
                    path,
                    ident.span().start().line,
                    Rule::DecimalMethod(method),
                ));
            }
        }
    }
}

fn punct(token: &TokenTree, expected: char) -> bool {
    if let TokenTree::Punct(value) = token {
        value.as_char() == expected
    } else {
        false
    }
}

fn is_decimal(token: &TokenTree, aliases: &BTreeSet<String>) -> bool {
    if let TokenTree::Ident(value) = token {
        aliases.contains(&value.to_string())
    } else {
        false
    }
}

fn decimal_receiver(before: &[TokenTree], aliases: &BTreeSet<String>) -> bool {
    match before {
        [.., name, first, second] if punct(first, ':') && punct(second, ':') => {
            if is_decimal(name, aliases) {
                return true;
            }
            if punct(name, '>') {
                return before
                    .iter()
                    .rev()
                    .take_while(|token| !punct(token, '<'))
                    .any(|token| is_decimal(token, aliases));
            }
            false
        }
        _ => false,
    }
}

fn decimal_turbofish(after: &[TokenTree], aliases: &BTreeSet<String>) -> bool {
    match after {
        [first, second, open, rest @ ..]
            if punct(first, ':') && punct(second, ':') && punct(open, '<') =>
        {
            rest.iter()
                .take_while(|token| !punct(token, '>'))
                .any(|token| is_decimal(token, aliases))
        }
        _ => false,
    }
}

fn aliases_in(tokens: &[TokenTree], aliases: &mut BTreeSet<String>) {
    let mut remaining = tokens;
    while let Some((_, tail)) = remaining.split_first() {
        if let [name, TokenTree::Ident(keyword), TokenTree::Ident(alias), ..] = remaining
            && is_decimal(name, aliases)
            && keyword == "as"
        {
            aliases.insert(alias.to_string());
        }
        remaining = tail;
    }
    for token in tokens {
        if let TokenTree::Group(group) = token {
            aliases_in(&group.stream().into_iter().collect::<Vec<_>>(), aliases);
        }
    }
}
