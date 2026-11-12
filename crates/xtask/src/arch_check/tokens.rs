use std::collections::BTreeSet;

use proc_macro2::{Group, Literal, TokenStream, TokenTree};
use syn::{LitByteStr, LitCStr, LitStr};

#[derive(Debug, Clone, Copy)]
pub(crate) struct Words<'rules> {
    pub(crate) write_methods: &'rules [String],
    pub(crate) order_path_segments: &'rules [String],
}

#[derive(Debug, Clone, Default)]
pub(crate) struct Markers {
    pub(crate) order_paths: Vec<String>,
    pub(crate) write_methods: Vec<String>,
    pub(crate) idents: BTreeSet<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct ConstMarkers {
    pub(crate) name: String,
    pub(crate) markers: Markers,
}

fn literal_text(literal: &Literal) -> Option<String> {
    let stream = TokenStream::from(TokenTree::Literal(literal.clone()));
    syn::parse2::<LitStr>(stream.clone())
        .map(|text| text.value())
        .or_else(|_| {
            syn::parse2::<LitByteStr>(stream.clone())
                .map(|bytes| String::from_utf8_lossy(&bytes.value()).into_owned())
        })
        .or_else(|_| {
            syn::parse2::<LitCStr>(stream).map(|text| text.value().to_string_lossy().into_owned())
        })
        .ok()
}

fn ident_text(ident: &proc_macro2::Ident) -> String {
    let text = ident.to_string();
    match text.strip_prefix("r#") {
        Some(raw) => String::from(raw),
        None => text,
    }
}

impl Words<'_> {
    fn is_write_method(&self, text: &str) -> Option<()> {
        self.write_methods
            .iter()
            .any(|method| method.eq_ignore_ascii_case(text))
            .then_some(())
    }

    fn is_order_path(&self, text: &str) -> Option<()> {
        text.split('/')
            .any(|segment| {
                self.order_path_segments
                    .iter()
                    .any(|order| order.eq_ignore_ascii_case(segment))
            })
            .then_some(())
    }
}

fn inline_arguments(text: &str) -> Vec<String> {
    text.split('{')
        .skip(1)
        .filter_map(|tail| tail.split(['}', ':']).next())
        .filter(|name| {
            name.chars()
                .next()
                .is_some_and(|first| first.is_ascii_alphabetic() || first == '_')
                && name
                    .chars()
                    .all(|character| character.is_ascii_alphanumeric() || character == '_')
        })
        .map(String::from)
        .collect()
}

impl Markers {
    fn absorb_literal(&mut self, words: Words<'_>, literal: &Literal) {
        if let Some(text) = literal_text(literal) {
            if words.is_write_method(&text).is_some() {
                self.write_methods.push(text.clone());
            }
            self.idents.extend(inline_arguments(&text));
            if words.is_order_path(&text).is_some() {
                self.order_paths.push(text);
            }
        }
    }

    fn absorb_ident(&mut self, words: Words<'_>, ident: &proc_macro2::Ident) {
        let text = ident_text(ident);
        if words.is_write_method(&text).is_some() {
            self.write_methods.push(text.clone());
        }
        self.idents.insert(text);
    }

    pub(crate) fn absorb(&mut self, words: Words<'_>, trees: &[TokenTree]) {
        for tree in trees {
            match tree {
                TokenTree::Group(group) => self.absorb(words, &flatten(group)),
                TokenTree::Ident(ident) => self.absorb_ident(words, ident),
                TokenTree::Literal(literal) => self.absorb_literal(words, literal),
                TokenTree::Punct(_) => {}
            }
        }
    }
}

pub(crate) fn flatten(group: &Group) -> Vec<TokenTree> {
    group.stream().into_iter().collect()
}

fn is_keyword(tree: &TokenTree, keyword: &str) -> Option<()> {
    match tree {
        TokenTree::Ident(ident) => (*ident == keyword).then_some(()),
        TokenTree::Group(_) | TokenTree::Punct(_) | TokenTree::Literal(_) => None,
    }
}

fn is_punct(tree: &TokenTree, character: char) -> Option<()> {
    match tree {
        TokenTree::Punct(punct) => (punct.as_char() == character).then_some(()),
        TokenTree::Group(_) | TokenTree::Ident(_) | TokenTree::Literal(_) => None,
    }
}

fn const_header(trees: &[TokenTree]) -> Option<(String, &[TokenTree])> {
    let after_mut = match trees.split_first() {
        Some((first, rest)) if is_keyword(first, "mut").is_some() => rest,
        Some(_) | None => trees,
    };
    let (name, rest) = after_mut.split_first()?;
    let (colon, body) = rest.split_first()?;
    is_punct(colon, ':')?;
    match name {
        TokenTree::Ident(ident) => Some((ident_text(ident), body)),
        TokenTree::Group(_) | TokenTree::Punct(_) | TokenTree::Literal(_) => None,
    }
}

fn split_at_semicolon(trees: &[TokenTree]) -> (&[TokenTree], &[TokenTree]) {
    match trees
        .iter()
        .position(|tree| is_punct(tree, ';').is_some())
        .and_then(|index| trees.split_at_checked(index))
    {
        Some((body, rest)) => (body, rest),
        None => (trees, &[]),
    }
}

pub(crate) fn collect_consts(words: Words<'_>, trees: &[TokenTree], found: &mut Vec<ConstMarkers>) {
    let mut rest = trees;
    while let Some((first, tail)) = rest.split_first() {
        rest = tail;
        let opens_item = is_keyword(first, "const")
            .or_else(|| is_keyword(first, "static"))
            .and_then(|()| const_header(tail));
        if let Some((name, body_and_rest)) = opens_item {
            let (body, remaining) = split_at_semicolon(body_and_rest);
            let mut markers = Markers::default();
            markers.absorb(words, body);
            found.push(ConstMarkers { name, markers });
            rest = remaining;
        }
        if let TokenTree::Group(group) = first {
            collect_consts(words, &flatten(group), found);
        }
    }
}
