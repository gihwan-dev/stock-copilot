use std::collections::BTreeMap;

use proc_macro2::{Delimiter, TokenStream, TokenTree};

const TEST_ATTRIBUTE: &str = "test";

fn joined(tokens: &[TokenTree]) -> String {
    tokens
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(" ")
}

fn is_test_attribute(group: &proc_macro2::Group) -> bool {
    let mut last = None;
    for token in group.stream() {
        match token {
            TokenTree::Ident(ident) => last = Some(ident.to_string()),
            TokenTree::Punct(punct) if punct.as_char() == ':' => {}
            TokenTree::Group(_) | TokenTree::Punct(_) | TokenTree::Literal(_) => break,
        }
    }
    last.as_deref() == Some(TEST_ATTRIBUTE)
}

#[derive(Debug)]
struct Walker<'found> {
    path: &'found str,
    found: &'found mut BTreeMap<String, String>,
}

#[derive(Debug, Clone)]
struct Context {
    module: Vec<String>,
    enclosing: String,
}

#[derive(Debug)]
struct Item {
    context: Context,
    attributes: Vec<String>,
    testing: bool,
    start: Option<usize>,
}

fn punct_is(token: Option<&TokenTree>, expected: char) -> bool {
    match token {
        Some(TokenTree::Punct(punct)) => punct.as_char() == expected,
        Some(TokenTree::Group(_) | TokenTree::Ident(_) | TokenTree::Literal(_)) | None => false,
    }
}

fn ident_of(token: Option<&TokenTree>) -> Option<String> {
    match token {
        Some(TokenTree::Ident(ident)) => Some(ident.to_string()),
        Some(TokenTree::Group(_) | TokenTree::Punct(_) | TokenTree::Literal(_)) | None => None,
    }
}

fn group_of(token: Option<&TokenTree>, delimiter: Delimiter) -> Option<&proc_macro2::Group> {
    match token {
        Some(TokenTree::Group(group)) if group.delimiter() == delimiter => Some(group),
        Some(
            TokenTree::Group(_) | TokenTree::Ident(_) | TokenTree::Punct(_) | TokenTree::Literal(_),
        )
        | None => None,
    }
}

fn function_end(tokens: &[TokenTree], from: usize) -> Option<usize> {
    tokens
        .iter()
        .enumerate()
        .skip(from)
        .find_map(|(index, token)| match token {
            TokenTree::Group(group) if group.delimiter() == Delimiter::Brace => Some(Some(index)),
            TokenTree::Punct(punct) if punct.as_char() == ';' => Some(None),
            TokenTree::Group(_)
            | TokenTree::Ident(_)
            | TokenTree::Punct(_)
            | TokenTree::Literal(_) => None,
        })
        .flatten()
}

impl Walker<'_> {
    fn record(&mut self, context: &Context, name: &str, fingerprint: &str) {
        let mut id = vec![String::from(self.path)];
        id.extend(context.module.iter().cloned());
        id.push(String::from(name));
        let slot = self.found.entry(id.join("::")).or_default();
        slot.push_str(fingerprint);
    }

    fn attribute(tokens: &[TokenTree], index: usize, item: &mut Item) -> Option<usize> {
        if !punct_is(tokens.get(index), '#') {
            return None;
        }
        let next = tokens.get(index.saturating_add(1));
        if punct_is(next, '!') {
            let group = group_of(tokens.get(index.saturating_add(2)), Delimiter::Bracket)?;
            let inner = format!("#![{}]", group.stream());
            item.context.enclosing.push_str(&inner);
            return Some(index.saturating_add(3));
        }
        let group = group_of(next, Delimiter::Bracket)?;
        item.attributes.push(format!("#[{}]", group.stream()));
        item.testing = item.testing || is_test_attribute(group);
        Some(index.saturating_add(2))
    }

    fn module(&mut self, tokens: &[TokenTree], index: usize, item: &Item) -> Option<usize> {
        let name = ident_of(tokens.get(index.saturating_add(1)))?;
        let body = group_of(tokens.get(index.saturating_add(2)), Delimiter::Brace)?;
        let mut inner = item.context.clone();
        inner.module.push(name.clone());
        let marker = format!("{} mod {name} ", item.attributes.join(""));
        inner.enclosing.push_str(&marker);
        self.walk(body.stream(), &inner);
        Some(index.saturating_add(2))
    }

    fn function(
        &mut self,
        tokens: &[TokenTree],
        index: usize,
        start: usize,
        item: &Item,
    ) -> Option<usize> {
        let name = ident_of(tokens.get(index.saturating_add(1)))?;
        let end = function_end(tokens, index.saturating_add(2))?;
        if item.testing {
            let body = tokens.get(start..=end).map_or_else(String::new, joined);
            let fingerprint = format!(
                "{}|{}|{body}",
                item.context.enclosing,
                item.attributes.join("")
            );
            self.record(&item.context, &name, &fingerprint);
        }
        Some(end)
    }

    fn macro_call(&mut self, tokens: &[TokenTree], index: usize, item: &Item) -> Option<usize> {
        let token = tokens.get(index)?;
        if !punct_is(tokens.get(index.saturating_add(1)), '!') {
            return None;
        }
        let Some(TokenTree::Group(body)) = tokens.get(index.saturating_add(2)) else {
            return None;
        };
        let mut inner = item.context.clone();
        let marker = format!("{} {token}! ", item.attributes.join(""));
        inner.enclosing.push_str(&marker);
        self.walk(body.stream(), &inner);
        Some(index.saturating_add(2))
    }

    fn item_end(
        &mut self,
        tokens: &[TokenTree],
        index: usize,
        start: usize,
        item: &Item,
    ) -> Option<usize> {
        let token = tokens.get(index)?;
        match ident_of(Some(token)).as_deref() {
            Some("mod") => self.module(tokens, index, item),
            Some("fn") => self.function(tokens, index, start, item),
            Some(_) | None => self.macro_call(tokens, index, item).or_else(|| {
                (punct_is(Some(token), ';') || group_of(Some(token), Delimiter::Brace).is_some())
                    .then_some(index)
            }),
        }
    }

    fn walk(&mut self, stream: TokenStream, context: &Context) {
        let tokens: Vec<TokenTree> = stream.into_iter().collect();
        let mut item = Item {
            context: context.clone(),
            attributes: Vec::new(),
            testing: false,
            start: None,
        };
        let mut index = 0_usize;
        while index < tokens.len() {
            if let Some(next) = Self::attribute(&tokens, index, &mut item) {
                index = next;
                continue;
            }
            let start = *item.start.get_or_insert(index);
            if let Some(end) = self.item_end(&tokens, index, start, &item) {
                item.attributes.clear();
                item.testing = false;
                item.start = None;
                index = end;
            }
            index = index.saturating_add(1);
        }
    }
}

pub(crate) fn test_functions(path: &str, source: &str) -> Option<BTreeMap<String, String>> {
    let stream = source.parse::<TokenStream>().ok()?;
    let mut found = BTreeMap::new();
    let mut walker = Walker {
        path,
        found: &mut found,
    };
    walker.walk(
        stream,
        &Context {
            module: Vec::new(),
            enclosing: String::new(),
        },
    );
    Some(found)
}
