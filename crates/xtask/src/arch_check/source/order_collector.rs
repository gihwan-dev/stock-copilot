#[cfg(test)]
mod tests;

use std::collections::BTreeSet;

use proc_macro2::{Delimiter, Group, Literal, TokenStream, TokenTree};
use syn::ext::IdentExt;
use syn::{LitByteStr, LitStr};

use super::super::CrateName;
use super::super::rules::OrderCollectorRules;

fn names_in(text: &str, names: &BTreeSet<String>) -> bool {
    names.iter().any(|name| text.contains(name.as_str()))
}

fn literal_text(literal: &Literal) -> String {
    let raw = literal.to_string();
    if let Ok(text) = syn::parse_str::<LitStr>(&raw) {
        return text.value();
    }
    if let Ok(bytes) = syn::parse_str::<LitByteStr>(&raw) {
        return String::from_utf8_lossy(&bytes.value()).into_owned();
    }
    raw
}

fn joined_fragments(stream: TokenStream, joined: &mut String) {
    for token in stream {
        match token {
            TokenTree::Group(group) => joined_fragments(group.stream(), joined),
            TokenTree::Ident(ident) => joined.push_str(&ident.unraw().to_string()),
            TokenTree::Literal(literal) => joined.push_str(&literal_text(&literal)),
            TokenTree::Punct(_) => {}
        }
    }
}

fn synthesized(group: &Group, names: &BTreeSet<String>) -> bool {
    if group.delimiter() != Delimiter::Bracket {
        return false;
    }
    let mut joined = String::new();
    joined_fragments(group.stream(), &mut joined);
    names_in(&joined, names)
}

fn mentions(stream: TokenStream, names: &BTreeSet<String>) -> bool {
    stream.into_iter().any(|token| match token {
        TokenTree::Group(group) => synthesized(&group, names) || mentions(group.stream(), names),
        TokenTree::Ident(ident) => names.contains(&ident.unraw().to_string()),
        TokenTree::Literal(literal) => names_in(&literal_text(&literal), names),
        TokenTree::Punct(_) => false,
    })
}

pub(super) fn mentions_collector(
    rules: &OrderCollectorRules,
    crate_name: &CrateName,
    relative: &str,
    stream: &TokenStream,
) -> bool {
    !rules.allowed_crates.contains(crate_name)
        && !rules.allowed_files.contains(relative)
        && mentions(stream.clone(), &rules.collector_types)
}
