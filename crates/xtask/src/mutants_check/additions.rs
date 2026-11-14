use super::{
    addition_symbols::{self as symbols, Location, Symbols},
    graph::Sources,
    macro_tokens,
};
use std::collections::BTreeSet;
use syn::{spanned::Spanned, visit::Visit};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Change {
    OnlyFile,
    Registration,
    Graph,
}

pub(super) struct Additions<'a> {
    before: &'a Sources,
    after: &'a Sources,
    prior_ids: BTreeSet<String>,
    symbols: Symbols,
}

impl<'a> Additions<'a> {
    pub(super) fn read(before: &'a Sources, after: &'a Sources) -> Option<Self> {
        let mut prior_ids = BTreeSet::new();
        for source in before.values() {
            prior_ids.extend(macro_tokens::identifiers(source.parse().ok()?));
        }
        Some(Self {
            before,
            after,
            prior_ids,
            symbols: Symbols::read(after)?,
        })
    }
    pub(super) fn classify(&self, path: &str) -> Change {
        self.decide(path).unwrap_or(Change::Graph)
    }
    fn decide(&self, path: &str) -> Option<Change> {
        let current = self.after.get(path)?;
        let after = syn::parse_file(current).ok()?;
        let Some(previous) = self.before.get(path) else {
            return self.safe_new_file(&after).then_some(Change::OnlyFile);
        };
        let before = syn::parse_file(previous).ok()?;
        if self.symbols.has_globs() {
            return None;
        }
        if attrs(&before.attrs, previous)? != attrs(&after.attrs, current)? {
            return None;
        }
        if path.ends_with("/lib.rs")
            && self.registration(path, (&before, previous), (&after, current))?
        {
            return Some(Change::Registration);
        }
        let mut added = Vec::new();
        sequence(
            (&before.items, previous),
            (&after.items, current),
            &mut added,
            TestItems::Extend,
        )?;
        if added.is_empty() {
            return None;
        }
        let root_added = added
            .iter()
            .copied()
            .filter(|item| is_root(item, &after.items))
            .collect::<Vec<_>>();
        let nominal = nominal_names(&root_added);
        let old_ids = macro_tokens::identifiers(previous.parse().ok()?);
        let references = added
            .iter()
            .filter(|item| !is_use(item))
            .filter_map(|item| symbols::tokens(*item, current))
            .filter_map(|text| text.parse().ok())
            .flat_map(macro_tokens::identifiers)
            .collect();
        let validator = Validator {
            prior_ids: &self.prior_ids,
            nominal: &nominal,
            old_ids: &old_ids,
            references: &references,
            symbols: &self.symbols,
            fresh_file: false,
            safe: true,
            root_impl: false,
        };
        validate(validator, &added, &after.items).then_some(Change::OnlyFile)
    }
    fn safe_new_file(&self, file: &syn::File) -> bool {
        if !file.attrs.is_empty() {
            return false;
        }
        let items: Vec<&syn::Item> = file.items.iter().collect();
        let nominal = nominal_names(&items);
        let empty = BTreeSet::new();
        validate(
            Validator {
                prior_ids: &self.prior_ids,
                nominal: &nominal,
                old_ids: &empty,
                references: &empty,
                symbols: &self.symbols,
                fresh_file: true,
                safe: true,
                root_impl: false,
            },
            &items,
            &file.items,
        )
    }
    fn registration(
        &self,
        path: &str,
        (before, previous): (&syn::File, &str),
        (after, current): (&syn::File, &str),
    ) -> Option<bool> {
        let mut added = Vec::new();
        sequence(
            (&before.items, previous),
            (&after.items, current),
            &mut added,
            TestItems::Fixed,
        )?;
        if added.is_empty() {
            return Some(false);
        }
        let modules = symbols::modules(after);
        let parent = std::path::Path::new(path).parent()?.to_str()?;
        for item in added {
            let valid = if let syn::Item::Mod(module) = item {
                self.registration_module(parent, module)
            } else if let syn::Item::Use(import) = item {
                self.registration_import(&modules, import)?
            } else {
                false
            };
            if !valid {
                return Some(false);
            }
        }
        Some(true)
    }
    fn registration_module(&self, parent: &str, module: &syn::ItemMod) -> bool {
        let file = format!("{parent}/{}.rs", module.ident);
        module.content.is_none()
            && module.attrs.is_empty()
            && !self.before.contains_key(&file)
            && self.after.contains_key(&file)
    }
    fn registration_import(
        &self,
        modules: &BTreeSet<String>,
        import: &syn::ItemUse,
    ) -> Option<bool> {
        if !import.attrs.is_empty() || !public(&import.vis) {
            return Some(false);
        }
        Some(
            symbols::imports(import, modules, Location::Root)?
                .iter()
                .all(|binding| {
                    binding.bound != "_"
                        && !self.prior_ids.contains(&binding.bound)
                        && binding.internal
                        && self.symbols.data(&binding.original)
                }),
        )
    }
}

fn public(vis: &syn::Visibility) -> bool {
    match vis {
        syn::Visibility::Public(_) => true,
        syn::Visibility::Restricted(_) | syn::Visibility::Inherited => false,
    }
}
fn is_use(item: &syn::Item) -> bool {
    match item {
        syn::Item::Use(_) => true,
        syn::Item::Const(_)
        | syn::Item::Enum(_)
        | syn::Item::ExternCrate(_)
        | syn::Item::Fn(_)
        | syn::Item::ForeignMod(_)
        | syn::Item::Impl(_)
        | syn::Item::Macro(_)
        | syn::Item::Mod(_)
        | syn::Item::Static(_)
        | syn::Item::Struct(_)
        | syn::Item::Trait(_)
        | syn::Item::TraitAlias(_)
        | syn::Item::Type(_)
        | syn::Item::Union(_)
        | syn::Item::Verbatim(_)
        | _ => false,
    }
}
fn attrs(items: &[syn::Attribute], source: &str) -> Option<Vec<String>> {
    items
        .iter()
        .map(|item| symbols::tokens(item, source))
        .collect()
}

fn test_module(item: &syn::Item) -> Option<&syn::ItemMod> {
    let syn::Item::Mod(module) = item else {
        return None;
    };
    if module.attrs.len() != 1 {
        return None;
    }
    let syn::Meta::List(cfg) = &module.attrs.first()?.meta else {
        return None;
    };
    (cfg.path.is_ident("cfg") && cfg.tokens.to_string() == "test").then_some(module)
}

fn nested<'a>(
    before: &syn::Item,
    previous: &str,
    after: &'a syn::Item,
    current: &str,
    added: &mut Vec<&'a syn::Item>,
) -> Option<()> {
    let left = test_module(before)?;
    let right = test_module(after)?;
    let (left_brace, left_items) = left.content.as_ref()?;
    let (right_brace, right_items) = right.content.as_ref()?;
    let left_header = symbols::slice_tokens(
        previous,
        left.span().start(),
        left_brace.span.open().start(),
    )?;
    let right_header = symbols::slice_tokens(
        current,
        right.span().start(),
        right_brace.span.open().start(),
    )?;
    if left_header != right_header {
        return None;
    }
    sequence(
        (left_items, previous),
        (right_items, current),
        added,
        TestItems::Fixed,
    )
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum TestItems {
    Fixed,
    Extend,
}

fn sequence<'a>(
    (before, previous): (&[syn::Item], &str),
    (after, current): (&'a [syn::Item], &str),
    added: &mut Vec<&'a syn::Item>,
    tests: TestItems,
) -> Option<()> {
    let mut old = before.iter().peekable();
    for item in after {
        if let Some(prior) = old.peek() {
            if symbols::tokens(*prior, previous)? == symbols::tokens(item, current)? {
                old.next();
                continue;
            }
            if tests == TestItems::Extend
                && test_module(prior).is_some()
                && test_module(item).is_some()
            {
                nested(prior, previous, item, current, added)?;
                old.next();
                continue;
            }
        }
        added.push(item);
    }
    old.next().is_none().then_some(())
}

fn nominal_names(items: &[&syn::Item]) -> BTreeSet<String> {
    items
        .iter()
        .filter_map(|item| match item {
            syn::Item::Struct(value) => unconditional_name(&value.ident, &value.attrs),
            syn::Item::Enum(value) => unconditional_name(&value.ident, &value.attrs),
            syn::Item::Const(_)
            | syn::Item::ExternCrate(_)
            | syn::Item::Fn(_)
            | syn::Item::ForeignMod(_)
            | syn::Item::Impl(_)
            | syn::Item::Macro(_)
            | syn::Item::Mod(_)
            | syn::Item::Static(_)
            | syn::Item::Trait(_)
            | syn::Item::TraitAlias(_)
            | syn::Item::Type(_)
            | syn::Item::Union(_)
            | syn::Item::Use(_)
            | syn::Item::Verbatim(_)
            | _ => None,
        })
        .collect()
}

fn unconditional_name(name: &proc_macro2::Ident, attrs: &[syn::Attribute]) -> Option<String> {
    (!attrs.iter().any(|attr| attr.path().is_ident("cfg"))).then(|| name.to_string())
}

struct Validator<'a> {
    prior_ids: &'a BTreeSet<String>,
    nominal: &'a BTreeSet<String>,
    old_ids: &'a BTreeSet<String>,
    references: &'a BTreeSet<String>,
    symbols: &'a Symbols,
    fresh_file: bool,
    safe: bool,
    root_impl: bool,
}

fn is_root(item: &syn::Item, roots: &[syn::Item]) -> bool {
    roots.iter().any(|root| std::ptr::eq(item, root))
}

fn validate(mut validator: Validator<'_>, items: &[&syn::Item], roots: &[syn::Item]) -> bool {
    if !validator.fresh_file && !items.iter().any(|item| !is_use(item)) {
        return false;
    }
    for item in items {
        validator.root_impl = is_root(item, roots)
            && match item {
                syn::Item::Impl(_) => true,
                syn::Item::Const(_)
                | syn::Item::Enum(_)
                | syn::Item::ExternCrate(_)
                | syn::Item::Fn(_)
                | syn::Item::ForeignMod(_)
                | syn::Item::Macro(_)
                | syn::Item::Mod(_)
                | syn::Item::Static(_)
                | syn::Item::Struct(_)
                | syn::Item::Trait(_)
                | syn::Item::TraitAlias(_)
                | syn::Item::Type(_)
                | syn::Item::Union(_)
                | syn::Item::Use(_)
                | syn::Item::Verbatim(_)
                | _ => false,
            };
        validator.visit_item(item);
    }
    validator.safe
}

impl<'ast> Visit<'ast> for Validator<'_> {
    fn visit_attribute(&mut self, i: &'ast syn::Attribute) {
        if !macro_tokens::attribute_known(&i.meta)
            || ["macro_export", "macro_use", "proptest_config"]
                .iter()
                .any(|name| i.path().is_ident(name))
        {
            self.safe = false;
        }
    }
    fn visit_item(&mut self, i: &'ast syn::Item) {
        match i {
            syn::Item::Fn(_)
            | syn::Item::Const(_)
            | syn::Item::Static(_)
            | syn::Item::Struct(_)
            | syn::Item::Enum(_)
            | syn::Item::Type(_)
            | syn::Item::Trait(_) => {
                if !self.fresh_file
                    && symbols::named(i)
                        .is_none_or(|name| self.prior_ids.contains(&name.to_string()))
                {
                    self.safe = false;
                }
            }
            syn::Item::Impl(_) | syn::Item::Use(_) => {}
            syn::Item::Mod(module) => {
                if !self.fresh_file || module.content.is_none() {
                    self.safe = false;
                }
            }
            syn::Item::ExternCrate(_)
            | syn::Item::ForeignMod(_)
            | syn::Item::Macro(_)
            | syn::Item::TraitAlias(_)
            | syn::Item::Union(_)
            | syn::Item::Verbatim(_)
            | _ => self.safe = false,
        }
        syn::visit::visit_item(self, i);
    }
    fn visit_item_impl(&mut self, i: &'ast syn::ItemImpl) {
        let valid = if let syn::Type::Path(target) = i.self_ty.as_ref() {
            target.qself.is_none()
                && target.path.leading_colon.is_none()
                && target.path.segments.len() == 1
                && target.path.segments.first().is_some_and(|segment| {
                    self.nominal.contains(&segment.ident.to_string())
                        && !i
                            .generics
                            .type_params()
                            .any(|parameter| parameter.ident == segment.ident)
                })
        } else {
            false
        };
        if !valid || !self.root_impl {
            self.safe = false;
        }
        let was_root = self.root_impl;
        self.root_impl = false;
        syn::visit::visit_item_impl(self, i);
        self.root_impl = was_root;
    }
    fn visit_macro(&mut self, i: &'ast syn::Macro) {
        let names = macro_tokens::identifiers(i.tokens.clone());
        if ["impl", "macro_rules", "mod", "use", "extern"]
            .iter()
            .any(|name| names.contains(*name))
        {
            self.safe = false;
        }
    }
    fn visit_item_use(&mut self, i: &'ast syn::ItemUse) {
        let Some(bindings) = symbols::imports(i, &BTreeSet::new(), Location::Nested) else {
            self.safe = false;
            return;
        };
        if !self.fresh_file
            && bindings.iter().any(|binding| {
                binding.bound == "_"
                    || self.old_ids.contains(&binding.bound)
                    || !self.references.contains(&binding.bound)
                    || !binding.internal
                    || !self.symbols.data(&binding.original)
            })
        {
            self.safe = false;
        }
    }
}
