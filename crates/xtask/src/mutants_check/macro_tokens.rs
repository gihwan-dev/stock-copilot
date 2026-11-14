use proc_macro2::{Delimiter, TokenStream, TokenTree};
use std::collections::{BTreeMap, BTreeSet};
use syn::{parse::Parser, visit::Visit};

const STANDARD: &[&str] = &[
    "assert",
    "assert_eq",
    "assert_ne",
    "debug_assert",
    "debug_assert_eq",
    "debug_assert_ne",
    "matches",
    "vec",
    "format",
    "format_args",
    "write",
    "writeln",
    "print",
    "println",
    "eprint",
    "eprintln",
    "panic",
    "unreachable",
    "concat",
    "stringify",
    "file",
    "line",
    "column",
    "module_path",
    "option_env",
    "env",
];
const PROPTEST: &[&str] = &[
    "proptest",
    "prop_assert",
    "prop_assert_eq",
    "prop_assert_ne",
    "prop_oneof",
];
const DERIVES: &[&str] = &[
    "Debug",
    "Clone",
    "Copy",
    "Default",
    "PartialEq",
    "Eq",
    "PartialOrd",
    "Ord",
    "Hash",
];

pub(super) fn attribute_known(meta: &syn::Meta) -> bool {
    if meta.path().is_ident("derive") {
        let syn::Meta::List(list) = meta else {
            return false;
        };
        let parser = syn::punctuated::Punctuated::<syn::Path, syn::Token![,]>::parse_terminated;
        return parser.parse2(list.tokens.clone()).is_ok_and(|paths| {
            paths.iter().all(|path| {
                path.segments.len() == 1 && DERIVES.iter().any(|name| path.is_ident(name))
            })
        });
    }
    [
        "cfg",
        "test",
        "repr",
        "must_use",
        "default",
        "macro_export",
        "macro_use",
        "proptest_config",
    ]
    .iter()
    .any(|name| meta.path().is_ident(name))
}

pub(super) fn identifiers(tokens: TokenStream) -> BTreeSet<String> {
    tokens
        .into_iter()
        .flat_map(|token| match token {
            TokenTree::Ident(name) => BTreeSet::from([name.to_string()]),
            TokenTree::Group(group) => identifiers(group.stream()),
            TokenTree::Literal(literal) => captures(&literal.to_string()),
            TokenTree::Punct(_) => BTreeSet::new(),
        })
        .collect()
}

fn captures(literal: &str) -> BTreeSet<String> {
    let Ok(syn::Lit::Str(text)) = syn::parse_str::<syn::Lit>(literal) else {
        return BTreeSet::new();
    };
    text.value()
        .split('{')
        .skip(1)
        .flat_map(|part| {
            part.split_once('}')
                .map_or(part, |(head, _)| head)
                .split(|next: char| !next.is_alphanumeric() && next != '_')
                .filter(|name| !name.is_empty())
                .map(String::from)
        })
        .collect()
}

#[derive(Clone, Default, PartialEq, Eq)]
struct Definition {
    owners: BTreeSet<String>,
    ids: BTreeSet<String>,
}

#[derive(Default)]
struct Collector {
    owner: String,
    definitions: BTreeMap<String, Definition>,
    imports: Vec<(String, BTreeSet<String>)>,
    prefix: Vec<String>,
    unsupported: bool,
}

impl Collector {
    fn import(&mut self, tree: &syn::UseTree) {
        match tree {
            syn::UseTree::Path(path) => {
                self.prefix.push(path.ident.to_string());
                self.import(&path.tree);
                self.prefix.pop();
            }
            syn::UseTree::Group(group) => {
                for child in &group.items {
                    self.import(child);
                }
            }
            syn::UseTree::Name(name) => {
                self.binding(name.ident.to_string(), name.ident.to_string());
            }
            syn::UseTree::Rename(name) => {
                self.binding(name.rename.to_string(), name.ident.to_string());
            }
            syn::UseTree::Glob(_) => {
                self.imports.push((String::from("*"), self.origins()));
            }
        }
    }
    fn origins(&self) -> BTreeSet<String> {
        let mut origins: BTreeSet<String> = self.prefix.iter().cloned().collect();
        if let Some(root) = self.prefix.first() {
            origins.insert(format!("@{root}"));
        }
        origins
    }
    fn binding(&mut self, alias: String, original: String) {
        let mut names: BTreeSet<String> = self.origins();
        names.insert(original);
        self.imports.push((alias, names));
    }
}

impl<'ast> Visit<'ast> for Collector {
    fn visit_item_use(&mut self, i: &'ast syn::ItemUse) {
        self.import(&i.tree);
        syn::visit::visit_item_use(self, i);
    }
    fn visit_item_macro(&mut self, i: &'ast syn::ItemMacro) {
        if i.mac.path.is_ident("macro_rules")
            && let Some(name) = &i.ident
        {
            let ids = identifiers(i.mac.tokens.clone());
            if [
                "fn", "struct", "enum", "mod", "trait", "impl", "type", "const", "static", "use",
                "extern",
            ]
            .iter()
            .any(|keyword| ids.contains(*keyword))
            {
                self.unsupported = true;
            }
            let definition = self.definitions.entry(name.to_string()).or_default();
            definition.owners.insert(self.owner.clone());
            definition.ids.extend(ids);
        }
        syn::visit::visit_item_macro(self, i);
    }
}

pub(super) struct Macros {
    definitions: BTreeMap<String, Definition>,
    imports: BTreeMap<String, Vec<(String, BTreeSet<String>)>>,
}

impl Macros {
    pub(super) fn read(
        files: &BTreeMap<String, syn::File>,
        sources: &BTreeMap<String, String>,
    ) -> Result<Self, String> {
        let mut result = Self {
            definitions: BTreeMap::new(),
            imports: BTreeMap::new(),
        };
        for (path, file) in files {
            let mut collector = Collector {
                owner: path.clone(),
                ..Collector::default()
            };
            collector.visit_file(file);
            if collector.imports.iter().any(untrusted_binding) {
                return Err(String::from("untrusted macro binding"));
            }
            if collector.unsupported {
                return Err(String::from("item-generating local macro"));
            }
            for (name, definition) in collector.definitions {
                let known = result.definitions.entry(name).or_default();
                known.owners.extend(definition.owners);
                known.ids.extend(definition.ids);
            }
            result.imports.insert(path.clone(), collector.imports);
        }
        result.aliases();
        for (path, source) in sources {
            result.scan(
                source
                    .parse::<TokenStream>()
                    .map_err(|error| error.to_string())?,
                path,
            )?;
        }
        Ok(result)
    }

    fn aliases(&mut self) {
        loop {
            let before = self.definitions.clone();
            for (alias, ids) in self.imports.values().flatten() {
                merge_alias(&mut self.definitions, alias, ids, &before);
            }
            if self.definitions == before {
                break;
            }
        }
    }

    pub(super) fn owners(&self) -> BTreeMap<String, BTreeSet<String>> {
        self.definitions
            .iter()
            .map(|(name, definition)| (name.clone(), definition.owners.clone()))
            .collect()
    }

    pub(super) fn expanded_ids(&self, tokens: TokenStream) -> BTreeSet<String> {
        let mut ids = identifiers(tokens);
        loop {
            let before = ids.clone();
            ids.extend(
                before
                    .iter()
                    .filter_map(|name| self.definitions.get(name))
                    .flat_map(|definition| definition.ids.iter().cloned()),
            );
            if before == ids {
                return ids;
            }
        }
    }

    fn known(&self, path: &[String], file: &str) -> bool {
        let Some(name) = path.last() else {
            return false;
        };
        let bare = path.len() == 1;
        if !bare
            && self
                .imports
                .get(file)
                .into_iter()
                .flatten()
                .any(|(alias, origins)| path.first() == Some(alias) && !trusted_origin(origins))
        {
            return false;
        }

        if name == "macro_rules" {
            return true;
        }
        let bindings = self
            .imports
            .get(file)
            .into_iter()
            .flatten()
            .filter(|(alias, _)| alias == name)
            .map(|(_, ids)| ids)
            .collect::<Vec<_>>();
        if bare
            && PROPTEST.contains(&name.as_str())
            && self
                .imports
                .get(file)
                .into_iter()
                .flatten()
                .any(|(alias, origins)| alias == "*" && origins.contains("@proptest"))
        {
            return true;
        }
        if bare && !bindings.is_empty() {
            return bindings.iter().all(|ids| self.import_known(ids));
        }
        if self.local_known(path, name) {
            return true;
        }
        (STANDARD.contains(&name.as_str())
            && (bare
                || path
                    .first()
                    .is_some_and(|prefix| ["std", "core"].contains(&prefix.as_str()))))
            || (PROPTEST.contains(&name.as_str())
                && path.first().is_some_and(|prefix| prefix == "proptest"))
    }

    fn local_known(&self, path: &[String], name: &str) -> bool {
        self.definitions.contains_key(name)
            && (path.len() == 1
                || path
                    .iter()
                    .any(|segment| ["crate", "self", "super"].contains(&segment.as_str()))
                || self.definitions.get(name).is_some_and(|definition| {
                    definition.owners.iter().any(|owner| {
                        std::path::Path::new(owner)
                            .file_stem()
                            .and_then(|stem| stem.to_str())
                            .is_some_and(|stem| path.iter().any(|segment| segment == stem))
                    })
                }))
    }

    fn import_known(&self, ids: &BTreeSet<String>) -> bool {
        if !trusted_origin(ids) {
            return false;
        }
        (ids.contains("@proptest") && PROPTEST.iter().any(|name| ids.contains(*name)))
            || ((ids.contains("@std") || ids.contains("@core"))
                && STANDARD.iter().any(|name| ids.contains(*name)))
            || ids.iter().any(|name| self.definitions.contains_key(name))
    }

    fn scan(&self, tokens: TokenStream, file: &str) -> Result<(), String> {
        let values: Vec<TokenTree> = tokens.into_iter().collect();
        for (index, token) in values.iter().enumerate() {
            if let TokenTree::Group(group) = token {
                self.scan(group.stream(), file)?;
            }
            self.scan_punct(&values, index, file)?;
        }
        Ok(())
    }
    fn scan_punct(&self, values: &[TokenTree], index: usize, file: &str) -> Result<(), String> {
        let Some(TokenTree::Punct(punct)) = values.get(index) else {
            return Ok(());
        };
        if punct.as_char() == '#' {
            check_attribute(values, index)?;
        }
        if punct.as_char() != '!'
            || index == 0
            || values
                .get(index.saturating_add(1))
                .is_none_or(|next| !matches_group(next))
        {
            return Ok(());
        }
        let path = preceding_path(values, index);
        if path.last().is_some_and(|name| {
            [
                "if", "else", "while", "return", "let", "match", "in", "loop",
            ]
            .contains(&name.as_str())
        }) {
            return Ok(());
        }
        self.check_format(values, index, &path)?;
        if !path.is_empty() && !self.known(&path, file) {
            return Err(format!("unknown macro: {}", path.join("::")));
        }
        Ok(())
    }
    fn check_format(
        &self,
        values: &[TokenTree],
        index: usize,
        path: &[String],
    ) -> Result<(), String> {
        if !path.last().is_some_and(|name| {
            [
                "format",
                "format_args",
                "write",
                "writeln",
                "print",
                "println",
                "eprint",
                "eprintln",
                "panic",
                "assert",
                "assert_eq",
                "assert_ne",
                "debug_assert",
                "debug_assert_eq",
                "debug_assert_ne",
            ]
            .contains(&name.as_str())
        }) {
            return Ok(());
        }
        if let Some(TokenTree::Group(group)) = values.get(index.saturating_add(1)) {
            let ids = self.expanded_ids(group.stream());
            if ["concat", "stringify", "env", "option_env"]
                .iter()
                .any(|name| ids.contains(*name))
            {
                return Err(String::from("constructed format captures"));
            }
        }
        Ok(())
    }
}

fn trusted_origin(origins: &BTreeSet<String>) -> bool {
    origins.iter().any(|origin| {
        ["@std", "@core", "@proptest", "@crate", "@self", "@super"].contains(&origin.as_str())
    })
}

fn untrusted_binding(binding: &(String, BTreeSet<String>)) -> bool {
    let (name, origins) = binding;
    if name == "proptest_config" {
        return true;
    }
    (STANDARD.contains(&name.as_str())
        || PROPTEST.contains(&name.as_str())
        || DERIVES.contains(&name.as_str())
        || name == "*"
        || name == "proptest_config")
        && !trusted_origin(origins)
}

fn merge_alias(
    definitions: &mut BTreeMap<String, Definition>,
    alias: &str,
    ids: &BTreeSet<String>,
    before: &BTreeMap<String, Definition>,
) {
    for found in ids.iter().filter_map(|name| before.get(name)) {
        let known = definitions.entry(String::from(alias)).or_default();
        known.owners.extend(found.owners.iter().cloned());
        known.ids.extend(found.ids.iter().cloned());
    }
}

fn matches_group(token: &TokenTree) -> bool {
    match token {
        TokenTree::Group(_) => true,
        TokenTree::Ident(_) | TokenTree::Literal(_) | TokenTree::Punct(_) => false,
    }
}

fn preceding_path(tokens: &[TokenTree], end: usize) -> Vec<String> {
    let mut path = Vec::new();
    let mut cursor = end;
    while let Some(TokenTree::Ident(name)) =
        cursor.checked_sub(1).and_then(|index| tokens.get(index))
    {
        path.push(name.to_string());
        cursor = cursor.saturating_sub(1);
        if cursor < 2
            || tokens
                .get(cursor.saturating_sub(1))
                .is_none_or(|token| token.to_string() != ":")
            || tokens
                .get(cursor.saturating_sub(2))
                .is_none_or(|token| token.to_string() != ":")
        {
            break;
        }
        cursor = cursor.saturating_sub(2);
    }
    path.reverse();
    path
}

fn check_attribute(tokens: &[TokenTree], index: usize) -> Result<(), String> {
    let next = tokens.get(index.saturating_add(1));
    let offset = if next.is_some_and(|token| token.to_string() == "!") {
        2
    } else {
        1
    };
    if let Some(TokenTree::Group(group)) = tokens.get(index.saturating_add(offset))
        && group.delimiter() == Delimiter::Bracket
    {
        let meta = syn::parse2::<syn::Meta>(group.stream()).map_err(|error| error.to_string())?;
        if !attribute_known(&meta) {
            return Err(String::from("unknown macro attribute"));
        }
    }
    Ok(())
}
