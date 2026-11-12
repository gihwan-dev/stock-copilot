use std::collections::{BTreeMap, BTreeSet};

use proc_macro2::{Delimiter, Group, TokenStream, TokenTree};

use super::Sources;

const TEST_ATTRIBUTE: &str = "test";

pub(crate) const STANDARD_MACROS: [&str; 18] = [
    "assert",
    "assert_eq",
    "assert_ne",
    "debug_assert",
    "debug_assert_eq",
    "debug_assert_ne",
    "matches",
    "panic",
    "unreachable",
    "todo",
    "unimplemented",
    "vec",
    "format",
    "write",
    "writeln",
    "println",
    "eprintln",
    "dbg",
];
const TEST_CONFIGURATION: &str = "cfg(test)";
const FUNCTION_QUALIFIERS: [&str; 4] = ["fn", "unsafe", "async", "extern"];

pub(crate) type SupportKey = (String, String);

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct ModuleRecord {
    pub(crate) testing: bool,
    pub(crate) support: BTreeMap<SupportKey, String>,
    pub(crate) identifiers: BTreeSet<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Scan {
    pub(crate) tests: BTreeMap<String, String>,
    pub(crate) modules: BTreeMap<String, ModuleRecord>,
    pub(crate) reached: BTreeSet<String>,
}

#[derive(Debug, Clone)]
struct Location {
    module: Vec<String>,
    enclosing: String,
    directory: String,
    testing: bool,
}

#[derive(Debug, Default)]
struct Item {
    attributes: Vec<String>,
    test: bool,
    start: Option<usize>,
}

fn joined(tokens: &[TokenTree]) -> String {
    tokens.iter().cloned().collect::<TokenStream>().to_string()
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

fn group_of(token: Option<&TokenTree>, delimiter: Delimiter) -> Option<&Group> {
    match token {
        Some(TokenTree::Group(group)) if group.delimiter() == delimiter => Some(group),
        Some(
            TokenTree::Group(_) | TokenTree::Ident(_) | TokenTree::Punct(_) | TokenTree::Literal(_),
        )
        | None => None,
    }
}

fn is_test_attribute(group: &Group) -> bool {
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

fn semicolon_after(tokens: &[TokenTree], from: usize) -> Option<usize> {
    tokens
        .iter()
        .enumerate()
        .skip(from)
        .find(|(_, token)| punct_is(Some(token), ';'))
        .map(|(index, _)| index)
}

fn body_or_semicolon(tokens: &[TokenTree], from: usize) -> Option<usize> {
    tokens
        .iter()
        .enumerate()
        .skip(from)
        .find(|(_, token)| {
            punct_is(Some(token), ';') || group_of(Some(token), Delimiter::Brace).is_some()
        })
        .map(|(index, _)| index)
}

fn identifiers(stream: TokenStream, found: &mut BTreeSet<String>) {
    for token in stream {
        match token {
            TokenTree::Ident(ident) => {
                found.insert(ident.to_string());
            }
            TokenTree::Group(group) => identifiers(group.stream(), found),
            TokenTree::Punct(_) | TokenTree::Literal(_) => {}
        }
    }
}

pub(crate) fn parent_directory(path: &str) -> String {
    path.rsplit_once('/')
        .map_or_else(String::new, |(parent, _)| String::from(parent))
}

#[derive(Debug)]
struct Span {
    key: SupportKey,
    end: usize,
    body: usize,
}

fn item_text(tokens: &[TokenTree], item: &Item, index: usize, end: usize) -> String {
    let start = item.start.unwrap_or(index);
    format!(
        "{}|{}",
        item.attributes.join(""),
        tokens.get(start..=end).map_or_else(String::new, joined)
    )
}

fn span(kind: &str, name: String, end: usize, body: usize) -> Span {
    Span {
        key: (String::from(kind), name),
        end,
        body,
    }
}

fn constant(tokens: &[TokenTree], index: usize, keyword: &str) -> Option<Span> {
    let following = ident_of(tokens.get(index.saturating_add(1)));
    if following
        .as_deref()
        .is_some_and(|word| FUNCTION_QUALIFIERS.contains(&word))
    {
        return None;
    }
    let name = following.unwrap_or_else(|| String::from("_"));
    Some(span(
        keyword,
        name,
        semicolon_after(tokens, index)?,
        index.saturating_add(2),
    ))
}

fn simple_item(tokens: &[TokenTree], index: usize, keyword: &str) -> Option<Span> {
    let next = tokens.get(index.saturating_add(1));
    let after_name = index.saturating_add(2);
    match keyword {
        "const" | "static" => constant(tokens, index, keyword),
        "use" => {
            let end = semicolon_after(tokens, index)?;
            let tree = tokens
                .get(index.saturating_add(1)..end)
                .map_or_else(String::new, joined);
            Some(span(keyword, tree, end, end.saturating_add(1)))
        }
        "type" => Some(span(
            keyword,
            ident_of(next)?,
            semicolon_after(tokens, index)?,
            after_name,
        )),
        "struct" | "enum" | "union" | "trait" => Some(span(
            keyword,
            ident_of(next)?,
            body_or_semicolon(tokens, after_name)?,
            after_name,
        )),
        "impl" => {
            let end = body_or_semicolon(tokens, index.saturating_add(1))?;
            let header = tokens.get(index..end).map_or_else(String::new, joined);
            Some(span(keyword, header, end, index.saturating_add(1)))
        }
        "extern" if ident_of(next).as_deref() == Some("crate") => {
            let end = semicolon_after(tokens, index)?;
            Some(span(
                "extern crate",
                ident_of(tokens.get(after_name))?,
                end,
                end.saturating_add(1),
            ))
        }
        "macro_rules" if punct_is(next, '!') => {
            let body = index.saturating_add(3);
            let end = if group_of(tokens.get(body), Delimiter::Brace).is_some() {
                body
            } else {
                semicolon_after(tokens, body)?
            };
            Some(span(keyword, ident_of(tokens.get(after_name))?, end, body))
        }
        _ => None,
    }
}

#[derive(Debug)]
pub(crate) struct Walker<'scan> {
    pub(crate) sources: &'scan Sources,
    pub(crate) binary: String,
    pub(crate) scan: &'scan mut Scan,
}

impl Walker<'_> {
    fn module_id(&self, location: &Location) -> String {
        format!("{} {}", self.binary, location.module.join("::"))
    }

    fn record(&mut self, location: &Location) -> &mut ModuleRecord {
        let id = self.module_id(location);
        let record = self.scan.modules.entry(id).or_default();
        record.testing = record.testing || location.testing;
        record
    }

    fn support(
        &mut self,
        location: &Location,
        key: SupportKey,
        fingerprint: &str,
        body: &[TokenTree],
    ) {
        let record = self.record(location);
        for token in body {
            identifiers(TokenStream::from(token.clone()), &mut record.identifiers);
        }
        let slot = record.support.entry(key).or_default();
        slot.push_str(fingerprint);
    }

    fn test(&mut self, location: &Location, name: &str, fingerprint: &str, body: &[TokenTree]) {
        let mut path = location.module.clone();
        path.push(String::from(name));
        let id = format!("{} {}", self.binary, path.join("::"));
        self.scan.tests.entry(id).or_default().push_str(fingerprint);
        let record = self.record(location);
        record.testing = true;
        for token in body {
            identifiers(TokenStream::from(token.clone()), &mut record.identifiers);
        }
    }

    pub(crate) fn walk_root(&mut self, file: &str) {
        let location = Location {
            module: Vec::new(),
            enclosing: String::new(),
            directory: parent_directory(file),
            testing: false,
        };
        self.walk_file(file, &location);
    }

    fn walk_file(&mut self, file: &str, location: &Location) {
        self.scan.reached.insert(String::from(file));
        self.record(location);
        let parsed = self
            .sources
            .get(file)
            .and_then(|source| source.parse::<TokenStream>().ok());
        if let Some(stream) = parsed {
            self.walk(stream, location);
        }
    }

    fn file_module(&mut self, name: &str, attributes: &str, location: &Location) {
        let candidates = [
            format!("{}/{name}.rs", location.directory),
            format!("{}/{name}/mod.rs", location.directory),
        ];
        let Some(file) = candidates
            .into_iter()
            .find(|candidate| self.sources.contains_key(candidate))
        else {
            return;
        };
        let inner = Location {
            module: [location.module.clone(), vec![String::from(name)]].concat(),
            enclosing: format!("{}{attributes} mod {name} ", location.enclosing),
            directory: format!("{}/{name}", location.directory),
            testing: location.testing || attributes.contains(TEST_CONFIGURATION),
        };
        self.walk_file(&file, &inner);
    }

    fn inline_module(&mut self, name: &str, attributes: &str, body: &Group, location: &Location) {
        let inner = Location {
            module: [location.module.clone(), vec![String::from(name)]].concat(),
            enclosing: format!("{}{attributes} mod {name} ", location.enclosing),
            directory: format!("{}/{name}", location.directory),
            testing: location.testing || attributes.contains(TEST_CONFIGURATION),
        };
        self.record(&inner);
        self.walk(body.stream(), &inner);
    }

    fn attribute(
        tokens: &[TokenTree],
        index: usize,
        item: &mut Item,
        location: &mut Location,
    ) -> Option<usize> {
        if !punct_is(tokens.get(index), '#') {
            return None;
        }
        let next = tokens.get(index.saturating_add(1));
        if punct_is(next, '!') {
            let group = group_of(tokens.get(index.saturating_add(2)), Delimiter::Bracket)?;
            let inner = format!("#![{}]", group.stream());
            location.enclosing.push_str(&inner);
            return Some(index.saturating_add(3));
        }
        let group = group_of(next, Delimiter::Bracket)?;
        item.attributes.push(format!("#[{}]", group.stream()));
        item.test = item.test || is_test_attribute(group);
        Some(index.saturating_add(2))
    }

    fn module(
        &mut self,
        tokens: &[TokenTree],
        index: usize,
        attributes: &str,
        location: &Location,
    ) -> Option<usize> {
        let name = ident_of(tokens.get(index.saturating_add(1)))?;
        self.support(
            location,
            (String::from("mod"), name.clone()),
            attributes,
            &[],
        );
        if let Some(body) = group_of(tokens.get(index.saturating_add(2)), Delimiter::Brace) {
            self.inline_module(&name, attributes, body, location);
        } else {
            self.file_module(&name, attributes, location);
        }
        Some(index.saturating_add(2))
    }

    fn function(
        &mut self,
        tokens: &[TokenTree],
        index: usize,
        item: &Item,
        location: &Location,
    ) -> Option<usize> {
        let name = ident_of(tokens.get(index.saturating_add(1)))?;
        let end = body_or_semicolon(tokens, index.saturating_add(2))?;
        let text = item_text(tokens, item, index, end);
        let body = tokens.get(index.saturating_add(2)..=end).unwrap_or(&[]);
        if item.test {
            let fingerprint = format!("{}|{text}", location.enclosing);
            self.test(location, &name, &fingerprint, body);
        } else {
            self.support(location, (String::from("fn"), name), &text, body);
        }
        Some(end)
    }

    fn macro_call(
        &mut self,
        tokens: &[TokenTree],
        index: usize,
        item: &Item,
        location: &Location,
    ) -> Option<usize> {
        let token = tokens.get(index)?;
        if !punct_is(tokens.get(index.saturating_add(1)), '!') {
            return None;
        }
        let Some(TokenTree::Group(body)) = tokens.get(index.saturating_add(2)) else {
            return None;
        };
        let marker = format!("{} {token}! ", item.attributes.join(""));
        let inner = Location {
            enclosing: format!("{}{marker}", location.enclosing),
            ..location.clone()
        };
        self.walk(body.stream(), &inner);
        let end = index.saturating_add(2);
        Some(if punct_is(tokens.get(end.saturating_add(1)), ';') {
            end.saturating_add(1)
        } else {
            end
        })
    }

    fn item(
        &mut self,
        tokens: &[TokenTree],
        index: usize,
        item: &Item,
        location: &Location,
    ) -> Option<usize> {
        let token = tokens.get(index)?;
        let keyword = ident_of(Some(token));
        match keyword.as_deref() {
            Some("mod") => self.module(tokens, index, &item.attributes.join(""), location),
            Some("fn") => self.function(tokens, index, item, location),
            Some(word) => {
                if let Some(span) = simple_item(tokens, index, word) {
                    let text = item_text(tokens, item, index, span.end);
                    let body = tokens.get(span.body..=span.end).unwrap_or(&[]);
                    self.support(location, span.key, &text, body);
                    Some(span.end)
                } else {
                    self.macro_call(tokens, index, item, location)
                }
            }
            None => punct_is(Some(token), ';').then_some(index),
        }
    }

    fn walk(&mut self, stream: TokenStream, location: &Location) {
        let tokens: Vec<TokenTree> = stream.into_iter().collect();
        let mut current = location.clone();
        let mut item = Item::default();
        let mut index = 0_usize;
        while index < tokens.len() {
            if let Some(next) = Self::attribute(&tokens, index, &mut item, &mut current) {
                index = next;
                continue;
            }
            item.start.get_or_insert(index);
            if let Some(end) = self.item(&tokens, index, &item, &current) {
                item = Item::default();
                index = end;
            }
            index = index.saturating_add(1);
        }
    }
}

pub(crate) fn has_tests(source: &str) -> bool {
    let sources = Sources::from([(String::from("orphan.rs"), String::from(source))]);
    let mut scan = Scan::default();
    Walker {
        sources: &sources,
        binary: String::new(),
        scan: &mut scan,
    }
    .walk_root("orphan.rs");
    !scan.tests.is_empty()
}
