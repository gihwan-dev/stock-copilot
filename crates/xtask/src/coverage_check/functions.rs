use std::collections::BTreeSet;

use proc_macro2::{Delimiter, TokenStream, TokenTree};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FunctionSpan {
    pub(crate) name: String,
    pub(crate) lines: BTreeSet<usize>,
}

fn body_end(tokens: &[TokenTree]) -> Option<usize> {
    tokens
        .iter()
        .find_map(|next| match next {
            TokenTree::Group(group) if group.delimiter() == Delimiter::Brace => {
                Some(Some(group.span_close().end().line))
            }
            TokenTree::Punct(punct) if punct.as_char() == ';' => Some(None),
            TokenTree::Group(_)
            | TokenTree::Ident(_)
            | TokenTree::Punct(_)
            | TokenTree::Literal(_) => None,
        })
        .flatten()
}

fn collect(stream: TokenStream, found: &mut Vec<FunctionSpan>) {
    let tokens: Vec<TokenTree> = stream.into_iter().collect();
    for (index, token) in tokens.iter().enumerate() {
        match token {
            TokenTree::Ident(keyword) if keyword == "fn" => {
                let name = match tokens.get(index.saturating_add(1)) {
                    Some(TokenTree::Ident(ident)) => Some(ident.to_string()),
                    Some(TokenTree::Group(_) | TokenTree::Punct(_) | TokenTree::Literal(_))
                    | None => None,
                };
                let end = tokens.get(index.saturating_add(2)..).and_then(body_end);
                if let (Some(named), Some(last)) = (name, end) {
                    found.push(FunctionSpan {
                        name: named,
                        lines: (keyword.span().start().line..=last).collect(),
                    });
                }
            }
            TokenTree::Group(group) => collect(group.stream(), found),
            TokenTree::Ident(_) | TokenTree::Punct(_) | TokenTree::Literal(_) => {}
        }
    }
}

pub(crate) fn functions(source: &str) -> Vec<FunctionSpan> {
    let mut found = Vec::new();
    if let Ok(stream) = source.parse::<TokenStream>() {
        collect(stream, &mut found);
    }
    found
}

pub(crate) fn function_lines(source: &str, name: &str) -> Vec<BTreeSet<usize>> {
    functions(source)
        .into_iter()
        .filter(|span| span.name == name)
        .map(|span| span.lines)
        .collect()
}
