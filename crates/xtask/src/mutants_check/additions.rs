use super::{
    addition_symbols::{self as symbols, Location, Symbols},
    graph::Sources,
    macro_tokens,
};
use std::collections::BTreeSet;
use syn::visit::Visit;

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
    reserved: BTreeSet<String>,
    location_sensitive: bool,
}

impl<'a> Additions<'a> {
    pub(super) fn read(before: &'a Sources, after: &'a Sources) -> Option<Self> {
        let mut prior_ids = BTreeSet::new();
        let mut location_sensitive = false;
        for source in before.values() {
            let names = macro_tokens::identifiers(source.parse().ok()?);
            location_sensitive |= ["file", "line", "module_path"]
                .iter()
                .any(|name| names.contains(*name));
            prior_ids.extend(names);
        }
        Some(Self {
            before,
            after,
            reserved: Symbols::read(before)?.reserved(),
            location_sensitive,
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
        if self.symbols.has_globs() || self.location_sensitive {
            return None;
        }
        if attrs(&before.attrs) != attrs(&after.attrs) {
            return None;
        }
        if path.ends_with("/lib.rs") && self.registration(path, &before, &after)? {
            return Some(Change::Registration);
        }
        let mut added = Vec::new();
        sequence(&before.items, &after.items, &mut added, TestItems::Extend)?;
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
            .flat_map(|item| macro_tokens::identifiers(symbols::stream(item)))
            .collect();
        let validator = Validator {
            prior_ids: &self.prior_ids,
            reserved: &self.reserved,
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
                reserved: &self.reserved,
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
    fn registration(&self, path: &str, before: &syn::File, after: &syn::File) -> Option<bool> {
        let mut added = Vec::new();
        sequence(&before.items, &after.items, &mut added, TestItems::Fixed)?;
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
        let name = crate::rust_names::ident(&module.ident);
        let file = format!("{parent}/{name}.rs");
        name.is_ascii()
            && !self.reserved.contains(&name)
            && module.content.is_none()
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
                    binding.bound.is_ascii()
                        && !self.reserved.contains(&binding.bound)
                        && binding.bound != "_"
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
fn attrs(items: &[syn::Attribute]) -> Vec<String> {
    items
        .iter()
        .map(|item| {
            let tokens: proc_macro2::TokenStream = syn::parse_quote!(#item);
            tokens.to_string()
        })
        .collect()
}

fn module_header(module: &syn::ItemMod) -> String {
    let mut header = module.clone();
    header.content = None;
    header.semi = Some(syn::token::Semi::default());
    symbols::tokens(&syn::Item::Mod(header))
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
    (crate::rust_names::path_is(&cfg.path, "cfg") && cfg.tokens.to_string() == "test")
        .then_some(module)
}

fn nested<'a>(
    before: &syn::Item,
    after: &'a syn::Item,
    added: &mut Vec<&'a syn::Item>,
) -> Option<()> {
    let left = test_module(before)?;
    let right = test_module(after)?;
    let (_, left_items) = left.content.as_ref()?;
    let (_, right_items) = right.content.as_ref()?;
    if module_header(left) != module_header(right) {
        return None;
    }
    sequence(left_items, right_items, added, TestItems::Fixed)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum TestItems {
    Fixed,
    Extend,
}

fn sequence<'a>(
    before: &[syn::Item],
    after: &'a [syn::Item],
    added: &mut Vec<&'a syn::Item>,
    tests: TestItems,
) -> Option<()> {
    let mut old = before.iter().peekable();
    for item in after {
        if let Some(prior) = old.peek() {
            if symbols::tokens(prior) == symbols::tokens(item) {
                old.next();
                continue;
            }
            if tests == TestItems::Extend
                && test_module(prior).is_some()
                && test_module(item).is_some()
            {
                nested(prior, item, added)?;
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
    (!attrs
        .iter()
        .any(|attr| crate::rust_names::path_is(attr.path(), "cfg")))
    .then(|| crate::rust_names::ident(name))
}

struct Validator<'a> {
    prior_ids: &'a BTreeSet<String>,
    reserved: &'a BTreeSet<String>,
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

impl Validator<'_> {
    fn owns_impl(&self, implementation: &syn::ItemImpl) -> bool {
        let syn::Type::Path(target) = implementation.self_ty.as_ref() else {
            return false;
        };
        if target.qself.is_some()
            || target.path.leading_colon.is_some()
            || target.path.segments.len() != 1
        {
            return false;
        }
        let Some(segment) = target.path.segments.first() else {
            return false;
        };
        let name = crate::rust_names::ident(&segment.ident);
        self.nominal.contains(&name)
            && !implementation
                .generics
                .type_params()
                .any(|parameter| crate::rust_names::ident(&parameter.ident) == name)
    }
}

impl<'ast> Visit<'ast> for Validator<'_> {
    fn visit_attribute(&mut self, i: &'ast syn::Attribute) {
        if !macro_tokens::attribute_known(&i.meta)
            || ["macro_export", "macro_use", "proptest_config"]
                .iter()
                .any(|name| crate::rust_names::path_is(i.path(), name))
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
                if symbols::named(i).is_none_or(|identifier| {
                    let name = crate::rust_names::ident(identifier);
                    !name.is_ascii()
                        || self.reserved.contains(&name)
                        || (!self.fresh_file && self.prior_ids.contains(&name))
                }) {
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
        let valid = self.owns_impl(i);
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
                !binding.bound.is_ascii()
                    || self.reserved.contains(&binding.bound)
                    || binding.bound == "_"
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
