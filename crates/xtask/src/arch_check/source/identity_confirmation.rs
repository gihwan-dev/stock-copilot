#[cfg(test)]
mod tests;

use std::collections::{BTreeMap, BTreeSet};

use proc_macro2::{TokenStream, TokenTree};
use quote::ToTokens;
use syn::ext::IdentExt;
use syn::parse::Parser;
use syn::punctuated::Punctuated;
use syn::visit::{self, Visit};
use syn::{
    Attribute, Expr, ExprAsync, ExprCall, ExprClosure, ExprField, ExprForLoop, ExprLoop, ExprMatch,
    ExprPath, ExprStruct, ExprWhile, GenericArgument, GenericParam, Generics, ImplItem, ImplItemFn,
    Item, ItemConst, ItemEnum, ItemExternCrate, ItemFn, ItemImpl, ItemMod, ItemStatic, ItemStruct,
    ItemTrait, ItemType, ItemUse, Macro, Member, Pat, Path, PathArguments, Token, Type,
    TypeParamBound, UseGlob, UseRename, Visibility, WherePredicate,
};

use super::super::rules::IdentityConfirmationRules;
use super::super::{CrateName, Rule};
use super::ast::uses::{UseKind, has_pair, unraw, use_entries};

const ALLOWED_DERIVES: [&str; 3] = ["Debug", "PartialEq", "Eq"];
const PROMPT_METHODS: [&str; 3] = [
    "confirm_identifier_change",
    "confirm_retirement_correction",
    "confirm_link_correction",
];
const DEFINING_MACRO_BANNED: [&str; 8] = [
    "impl", "struct", "mod", "fn", "const", "static", "type", "use",
];
const USE_CASE_STRUCT: &str = "ConfirmInstrumentIdentity";
const DECISION_ENUM: &str = "IdentityConfirmationDecision";
const CONFIRMED_VARIANT: &str = "Confirmed";

#[derive(Clone, Copy, PartialEq, Eq)]
enum Scope {
    Product,
    Test,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DefiningTarget {
    Other,
    Confirmation,
    Seal,
}

fn private(vis: &Visibility) -> bool {
    match vis {
        Visibility::Inherited => true,
        Visibility::Public(_) | Visibility::Restricted(_) => false,
    }
}

fn segment_names(path: &Path) -> Vec<String> {
    path.segments
        .iter()
        .map(|segment| unraw(&segment.ident))
        .collect()
}

fn type_last_name(ty: &Type) -> Option<String> {
    match ty {
        Type::Path(path) => path.path.segments.last().map(|part| unraw(&part.ident)),
        Type::Group(group) => type_last_name(&group.elem),
        Type::Paren(paren) => type_last_name(&paren.elem),
        Type::Array(_)
        | Type::BareFn(_)
        | Type::ImplTrait(_)
        | Type::Infer(_)
        | Type::Macro(_)
        | Type::Never(_)
        | Type::Ptr(_)
        | Type::Reference(_)
        | Type::Slice(_)
        | Type::TraitObject(_)
        | Type::Tuple(_)
        | Type::Verbatim(_)
        | _ => None,
    }
}

fn token_idents(stream: TokenStream, out: &mut Vec<String>) {
    for token in stream {
        match token {
            TokenTree::Group(group) => token_idents(group.stream(), out),
            TokenTree::Ident(ident) => out.push(unraw(&ident)),
            TokenTree::Literal(_) | TokenTree::Punct(_) => {}
        }
    }
}

fn tokens_contain(value: &impl ToTokens, needle: &str) -> bool {
    let mut idents = Vec::new();
    token_idents(value.to_token_stream(), &mut idents);
    idents.iter().any(|ident| ident == needle)
}

fn stream_contains_ident(tokens: TokenStream, needle: &str) -> bool {
    tokens.into_iter().any(|token| match token {
        TokenTree::Group(group) => stream_contains_ident(group.stream(), needle),
        TokenTree::Ident(ident) => unraw(&ident) == needle,
        TokenTree::Literal(_) | TokenTree::Punct(_) => false,
    })
}

fn is_cfg_test_mod(item: &ItemMod) -> bool {
    item.ident.unraw() == "tests"
        && private(&item.vis)
        && item.content.is_some()
        && item.attrs.len() == 1
        && item
            .attrs
            .first()
            .is_some_and(|attr| attr.meta.to_token_stream().to_string() == "cfg (test)")
}

fn is_allowed_derive(attr: &Attribute) -> bool {
    if !attr.path().is_ident("derive") {
        return false;
    }
    attr.parse_args_with(Punctuated::<Path, Token![,]>::parse_terminated)
        .is_ok_and(|paths| {
            !paths.is_empty()
                && paths.iter().all(|path| {
                    path.segments.len() == 1
                        && path.segments.first().is_some_and(|seg| {
                            ALLOWED_DERIVES.contains(&unraw(&seg.ident).as_str())
                        })
                })
        })
}

fn child_directory_prefix(file_path: &str) -> Option<String> {
    file_path.strip_suffix(".rs").map(|stem| format!("{stem}/"))
}

fn is_unauthorized_child_file(rules: &IdentityConfirmationRules, relative: &str) -> bool {
    let in_defining_dir = child_directory_prefix(&rules.defining_file).is_some_and(|prefix| {
        relative.starts_with(&prefix) && relative != rules.defining_test_file
    });
    let in_use_case_dir = child_directory_prefix(&rules.use_case_file)
        .is_some_and(|prefix| relative.starts_with(&prefix));
    let in_allowed_dir = rules.issuance_allowed_files.iter().any(|allowed| {
        child_directory_prefix(allowed).is_some_and(|prefix| relative.starts_with(&prefix))
    });
    in_defining_dir || in_use_case_dir || in_allowed_dir
}

fn has_non_private_impl_item(item: &ImplItem) -> bool {
    match item {
        ImplItem::Const(value) => !private(&value.vis),
        ImplItem::Fn(value) => !private(&value.vis),
        ImplItem::Type(value) => !private(&value.vis),
        ImplItem::Macro(_) | ImplItem::Verbatim(_) | _ => false,
    }
}

fn has_non_private_item(items: &[Item]) -> bool {
    items.iter().any(|item| match item {
        Item::Const(value) => !private(&value.vis),
        Item::Enum(value) => !private(&value.vis),
        Item::ExternCrate(value) => !private(&value.vis),
        Item::Fn(value) => !private(&value.vis),
        Item::Impl(value) => {
            value.trait_.is_some() || value.items.iter().any(has_non_private_impl_item)
        }
        Item::Mod(value) => {
            !private(&value.vis)
                || value
                    .content
                    .as_ref()
                    .is_some_and(|(_, nested)| has_non_private_item(nested))
        }
        Item::Static(value) => !private(&value.vis),
        Item::Struct(value) => !private(&value.vis),
        Item::Trait(value) => !private(&value.vis),
        Item::TraitAlias(value) => !private(&value.vis),
        Item::Type(value) => !private(&value.vis),
        Item::Union(value) => !private(&value.vis),
        Item::Use(value) => !private(&value.vis),
        Item::ForeignMod(_) | Item::Macro(_) | Item::Verbatim(_) | _ => false,
    })
}

fn check_defining_test_file(
    rules: &IdentityConfirmationRules,
    file: &syn::File,
    stream: &TokenStream,
    findings: &mut BTreeSet<Rule>,
) {
    if stream_contains_ident(stream.clone(), &rules.issuer_function) {
        findings.insert(Rule::IdentityConfirmationIssuance);
    }
    let valid_top_level = file.items.iter().all(|item| match item {
        Item::Mod(module) if is_cfg_test_mod(module) => module
            .content
            .as_ref()
            .is_some_and(|(_, nested)| !has_non_private_item(nested)),
        Item::Const(_)
        | Item::Enum(_)
        | Item::ExternCrate(_)
        | Item::Fn(_)
        | Item::ForeignMod(_)
        | Item::Impl(_)
        | Item::Macro(_)
        | Item::Mod(_)
        | Item::Static(_)
        | Item::Struct(_)
        | Item::Trait(_)
        | Item::TraitAlias(_)
        | Item::Type(_)
        | Item::Union(_)
        | Item::Use(_)
        | Item::Verbatim(_)
        | _ => false,
    });
    if !valid_top_level {
        findings.insert(Rule::IdentityConfirmationIssuance);
    }
}

struct PromptAliases {
    prompt_module: String,
    cargo_crate_aliases: BTreeSet<String>,
    module_aliases: BTreeSet<String>,
}

impl PromptAliases {
    fn collect(
        rules: &IdentityConfirmationRules,
        source_aliases: &BTreeMap<String, Vec<String>>,
        file: &syn::File,
    ) -> Self {
        let defining_crate = rules.prompt_defining_crate.replace('-', "_");
        let cargo_crate_aliases: BTreeSet<String> = source_aliases
            .iter()
            .filter(|(_, targets)| targets.iter().any(|target| target == &defining_crate))
            .map(|(alias, _)| alias.clone())
            .collect();
        let mut collector = Self {
            prompt_module: rules.prompt_module.clone(),
            cargo_crate_aliases,
            module_aliases: BTreeSet::from([rules.prompt_module.clone()]),
        };
        collector.visit_file(file);
        collector
    }

    fn record_entry(&mut self, entry: &super::ast::uses::UseEntry) {
        let UseKind::Rename(alias) = &entry.kind else {
            return;
        };
        if entry.path.len() == 1
            && entry
                .path
                .first()
                .is_some_and(|first| self.cargo_crate_aliases.contains(first))
        {
            self.cargo_crate_aliases.insert(alias.clone());
        }
        if entry.imported() == self.prompt_module || self.module_aliases.contains(entry.imported())
        {
            self.module_aliases.insert(alias.clone());
        }
    }
}

impl<'ast> Visit<'ast> for PromptAliases {
    fn visit_item_use(&mut self, i: &'ast ItemUse) {
        for entry in use_entries(&i.tree) {
            self.record_entry(&entry);
        }
    }

    fn visit_item_extern_crate(&mut self, i: &'ast ItemExternCrate) {
        if self.cargo_crate_aliases.contains(&unraw(&i.ident))
            && let Some((_, rename)) = &i.rename
        {
            self.cargo_crate_aliases.insert(unraw(rename));
        }
        visit::visit_item_extern_crate(self, i);
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Exposure {
    Private,
    Exported,
}

fn exposure(vis: &Visibility) -> Exposure {
    if private(vis) {
        Exposure::Private
    } else {
        Exposure::Exported
    }
}

struct PromptCheck<'a> {
    rules: &'a IdentityConfirmationRules,
    crate_name: &'a CrateName,
    relative: &'a str,
    aliases: PromptAliases,
    findings: &'a mut BTreeSet<Rule>,
}

impl PromptCheck<'_> {
    fn impl_allowed(&self) -> bool {
        self.rules.prompt_impl_crates.contains(self.crate_name)
            || self.rules.prompt_impl_files.contains(self.relative)
    }

    fn check_glob_reexport(&mut self, path: &[String]) {
        let glob_prompt_module = path.iter().any(|segment| {
            segment == &self.rules.prompt_module || self.aliases.module_aliases.contains(segment)
        });
        let glob_cargo_alias = path
            .first()
            .is_some_and(|first| self.aliases.cargo_crate_aliases.contains(first));
        if glob_prompt_module || glob_cargo_alias {
            self.findings
                .insert(Rule::IdentityConfirmationPromptReexport);
        }
    }

    fn check_use_entry(&mut self, entry: &super::ast::uses::UseEntry, exposed: Exposure) {
        let imported = entry.imported();
        let touches_prompt_module =
            imported == self.rules.prompt_module || self.aliases.module_aliases.contains(imported);
        let sensitive = imported == self.rules.prompt_trait || touches_prompt_module;
        match &entry.kind {
            UseKind::Rename(_) => {
                if imported == self.rules.prompt_trait {
                    self.findings.insert(Rule::IdentityConfirmationPromptAlias);
                }
                if exposed == Exposure::Exported && sensitive {
                    self.findings
                        .insert(Rule::IdentityConfirmationPromptReexport);
                }
            }
            UseKind::Name if exposed == Exposure::Exported && sensitive => {
                self.findings
                    .insert(Rule::IdentityConfirmationPromptReexport);
            }
            UseKind::Glob if exposed == Exposure::Exported => {
                self.check_glob_reexport(&entry.path);
            }
            UseKind::Name | UseKind::Glob => {}
        }
    }

    fn check_macro_tokens(&mut self, tokens: TokenStream) {
        let mut idents = Vec::new();
        token_idents(tokens, &mut idents);
        if idents.iter().any(|ident| ident == &self.rules.prompt_trait)
            && idents.iter().any(|ident| ident == "impl")
            && !self.impl_allowed()
        {
            self.findings.insert(Rule::IdentityConfirmationPromptImpl);
        }
        if has_pair(&idents, &self.rules.prompt_trait, "as").is_some() {
            self.findings.insert(Rule::IdentityConfirmationPromptAlias);
        }
        if idents.iter().any(|ident| ident == "pub")
            && idents.iter().any(|ident| ident == "use")
            && idents.iter().any(|ident| {
                ident == &self.rules.prompt_trait
                    || ident == &self.rules.prompt_module
                    || self.aliases.module_aliases.contains(ident)
            })
        {
            self.findings
                .insert(Rule::IdentityConfirmationPromptReexport);
        }
    }
}

impl<'ast> Visit<'ast> for PromptCheck<'_> {
    fn visit_item_use(&mut self, i: &'ast ItemUse) {
        let exposed = exposure(&i.vis);
        for entry in use_entries(&i.tree) {
            self.check_use_entry(&entry, exposed);
        }
    }

    fn visit_item_impl(&mut self, i: &'ast ItemImpl) {
        let implements_prompt = i
            .trait_
            .as_ref()
            .and_then(|(_, path, _)| path.segments.last())
            .is_some_and(|segment| unraw(&segment.ident) == self.rules.prompt_trait);
        if implements_prompt && !self.impl_allowed() {
            self.findings.insert(Rule::IdentityConfirmationPromptImpl);
        }
        visit::visit_item_impl(self, i);
    }

    fn visit_macro(&mut self, i: &'ast Macro) {
        if let Ok(parsed) = syn::parse2::<syn::File>(i.tokens.clone()) {
            self.visit_file(&parsed);
        } else if let Ok(expressions) =
            Punctuated::<Expr, Token![,]>::parse_terminated.parse2(i.tokens.clone())
        {
            for expression in &expressions {
                self.visit_expr(expression);
            }
        } else {
            self.check_macro_tokens(i.tokens.clone());
        }
        visit::visit_macro(self, i);
    }
}

struct UnauthorizedFileCheck<'a> {
    rules: &'a IdentityConfirmationRules,
    in_domain: bool,
    findings: &'a mut BTreeSet<Rule>,
}

impl<'ast> Visit<'ast> for UnauthorizedFileCheck<'_> {
    fn visit_item_impl(&mut self, i: &'ast ItemImpl) {
        if type_last_name(&i.self_ty).is_some_and(|name| {
            name == self.rules.confirmation_type || name == self.rules.seal_type
        }) {
            self.findings.insert(Rule::IdentityConfirmationIssuance);
        }
        visit::visit_item_impl(self, i);
    }

    fn visit_item_type(&mut self, i: &'ast ItemType) {
        if self.in_domain
            && (tokens_contain(&i.ty, &self.rules.confirmation_type)
                || tokens_contain(&i.ty, &self.rules.seal_type))
        {
            self.findings.insert(Rule::IdentityConfirmationIssuance);
        }
        visit::visit_item_type(self, i);
    }

    fn visit_use_rename(&mut self, i: &'ast UseRename) {
        let original = unraw(&i.ident);
        if original == self.rules.issuer_function
            || original == self.rules.seal_type
            || (self.in_domain && original == self.rules.confirmation_type)
        {
            self.findings.insert(Rule::IdentityConfirmationIssuance);
        }
        visit::visit_use_rename(self, i);
    }
}

struct DefiningFileCheck<'a> {
    rules: &'a IdentityConfirmationRules,
    scope: Scope,
    target: DefiningTarget,
    in_canonical_issuer: bool,
    canonical_impl_count: usize,
    findings: &'a mut BTreeSet<Rule>,
}

impl DefiningFileCheck<'_> {
    fn is_canonical_issuer_method(&self, item: &ImplItem) -> bool {
        let ImplItem::Fn(method) = item else {
            return false;
        };
        let is_public = match method.vis {
            Visibility::Public(_) => true,
            Visibility::Inherited | Visibility::Restricted(_) => false,
        };
        if !is_public {
            return false;
        }
        let expected = format!("const fn {} () -> Self", self.rules.issuer_function);
        method.sig.to_token_stream().to_string() == expected
    }
}

impl<'ast> Visit<'ast> for DefiningFileCheck<'_> {
    fn visit_item(&mut self, i: &'ast Item) {
        let outer = self.in_canonical_issuer;
        self.in_canonical_issuer = false;
        visit::visit_item(self, i);
        self.in_canonical_issuer = outer;
    }

    fn visit_expr_closure(&mut self, i: &'ast ExprClosure) {
        let outer = self.in_canonical_issuer;
        self.in_canonical_issuer = false;
        visit::visit_expr_closure(self, i);
        self.in_canonical_issuer = outer;
    }

    fn visit_item_struct(&mut self, i: &'ast ItemStruct) {
        let name = unraw(&i.ident);
        if name == self.rules.confirmation_type {
            if i.fields.iter().any(|field| !private(&field.vis))
                || i.attrs.iter().any(|attr| !is_allowed_derive(attr))
            {
                self.findings.insert(Rule::IdentityConfirmationIssuance);
            }
        } else if name == self.rules.seal_type
            && (!private(&i.vis)
                || i.fields.iter().any(|field| !private(&field.vis))
                || i.attrs.iter().any(|attr| !is_allowed_derive(attr)))
        {
            self.findings.insert(Rule::IdentityConfirmationIssuance);
        }
        visit::visit_item_struct(self, i);
    }

    fn visit_item_impl(&mut self, i: &'ast ItemImpl) {
        let outer = self.target;
        let name = type_last_name(&i.self_ty);
        let is_confirmation = name.as_deref() == Some(self.rules.confirmation_type.as_str());
        let is_seal = name.as_deref() == Some(self.rules.seal_type.as_str());
        self.target = if is_confirmation {
            DefiningTarget::Confirmation
        } else if is_seal {
            DefiningTarget::Seal
        } else {
            DefiningTarget::Other
        };
        if is_seal {
            self.findings.insert(Rule::IdentityConfirmationIssuance);
        }
        if is_confirmation {
            self.canonical_impl_count = self.canonical_impl_count.saturating_add(1);
            let single_canonical = self.scope == Scope::Product
                && i.trait_.is_none()
                && self.canonical_impl_count == 1
                && i.items.len() == 1
                && i.items
                    .first()
                    .is_some_and(|item| self.is_canonical_issuer_method(item));
            if !single_canonical {
                self.findings.insert(Rule::IdentityConfirmationIssuance);
            }
        }
        visit::visit_item_impl(self, i);
        self.target = outer;
    }

    fn visit_impl_item_fn(&mut self, i: &'ast ImplItemFn) {
        let outer = self.in_canonical_issuer;
        self.in_canonical_issuer = self.scope == Scope::Product
            && self.target == DefiningTarget::Confirmation
            && unraw(&i.sig.ident) == self.rules.issuer_function;
        visit::visit_impl_item_fn(self, i);
        self.in_canonical_issuer = outer;
    }

    fn visit_expr_path(&mut self, i: &'ast ExprPath) {
        if self.scope == Scope::Product && !self.in_canonical_issuer {
            let forbidden = segment_names(&i.path).iter().any(|name| {
                name == &self.rules.confirmation_type
                    || name == &self.rules.seal_type
                    || name == &self.rules.issuer_function
                    || (name == "Self" && self.target != DefiningTarget::Other)
            });
            if forbidden {
                self.findings.insert(Rule::IdentityConfirmationIssuance);
            }
        }
        visit::visit_expr_path(self, i);
    }

    fn visit_expr_struct(&mut self, i: &'ast ExprStruct) {
        if self.scope == Scope::Product && !self.in_canonical_issuer {
            let forbidden = segment_names(&i.path).iter().any(|name| {
                name == &self.rules.confirmation_type
                    || name == &self.rules.seal_type
                    || (name == "Self" && self.target != DefiningTarget::Other)
            });
            if forbidden {
                self.findings.insert(Rule::IdentityConfirmationIssuance);
            }
        }
        visit::visit_expr_struct(self, i);
    }

    fn visit_item_type(&mut self, i: &'ast ItemType) {
        if self.scope == Scope::Product
            && (tokens_contain(&i.ty, &self.rules.confirmation_type)
                || tokens_contain(&i.ty, &self.rules.seal_type))
        {
            self.findings.insert(Rule::IdentityConfirmationIssuance);
        }
        visit::visit_item_type(self, i);
    }

    fn visit_use_rename(&mut self, i: &'ast UseRename) {
        let original = unraw(&i.ident);
        if self.scope == Scope::Product
            && (original == self.rules.confirmation_type
                || original == self.rules.seal_type
                || original == self.rules.issuer_function)
        {
            self.findings.insert(Rule::IdentityConfirmationIssuance);
        }
        visit::visit_use_rename(self, i);
    }

    fn visit_item_mod(&mut self, i: &'ast ItemMod) {
        let outer = self.scope;
        let is_history_cases = outer == Scope::Product
            && unraw(&i.ident) == "history_cases"
            && private(&i.vis)
            && i.content.is_none()
            && i.attrs.is_empty();
        if is_history_cases {
            return;
        }
        if outer == Scope::Product && is_cfg_test_mod(i) {
            if let Some((_, nested)) = &i.content
                && has_non_private_item(nested)
            {
                self.findings.insert(Rule::IdentityConfirmationIssuance);
            }
            self.scope = Scope::Test;
            visit::visit_item_mod(self, i);
            self.scope = outer;
            return;
        }
        self.findings.insert(Rule::IdentityConfirmationIssuance);
        visit::visit_item_mod(self, i);
        self.scope = outer;
    }

    fn visit_macro(&mut self, i: &'ast Macro) {
        if self.scope == Scope::Product {
            let mut idents = Vec::new();
            token_idents(i.tokens.clone(), &mut idents);
            let forbidden = idents.iter().any(|ident| {
                ident == &self.rules.confirmation_type
                    || ident == &self.rules.seal_type
                    || ident == &self.rules.issuer_function
                    || DEFINING_MACRO_BANNED.contains(&ident.as_str())
            });
            if forbidden {
                self.findings.insert(Rule::IdentityConfirmationIssuance);
            }
        }
        visit::visit_macro(self, i);
    }
}

fn find_prompt_generic_position(file: &syn::File) -> Result<Option<usize>, ()> {
    let mut found = None;
    for item in &file.items {
        let Item::Struct(structure) = item else {
            continue;
        };
        if unraw(&structure.ident) != USE_CASE_STRUCT {
            continue;
        }
        if found.is_some() {
            return Err(());
        }
        let type_params: Vec<String> = structure
            .generics
            .params
            .iter()
            .filter_map(|param| match param {
                GenericParam::Type(ty) => Some(unraw(&ty.ident)),
                GenericParam::Lifetime(_) | GenericParam::Const(_) => None,
            })
            .collect();
        let prompt_field = structure.fields.iter().find(|field| {
            field
                .ident
                .as_ref()
                .is_some_and(|name| unraw(name) == "prompt")
        });
        let Some(field) = prompt_field else {
            return Err(());
        };
        if !private(&field.vis) {
            return Err(());
        }
        let inner_name = match &field.ty {
            Type::Reference(reference) => type_last_name(&reference.elem),
            Type::Path(_) => type_last_name(&field.ty),
            Type::Array(_)
            | Type::BareFn(_)
            | Type::Group(_)
            | Type::ImplTrait(_)
            | Type::Infer(_)
            | Type::Macro(_)
            | Type::Never(_)
            | Type::Paren(_)
            | Type::Ptr(_)
            | Type::Slice(_)
            | Type::TraitObject(_)
            | Type::Tuple(_)
            | Type::Verbatim(_)
            | _ => None,
        };
        let Some(param_name) = inner_name else {
            return Err(());
        };
        let Some(index) = type_params.iter().position(|param| param == &param_name) else {
            return Err(());
        };
        found = Some(index);
    }
    Ok(found)
}

fn type_arg_at_position(self_ty: &Type, position: usize) -> Option<String> {
    let Type::Path(path) = self_ty else {
        return None;
    };
    let last = path.path.segments.last()?;
    if unraw(&last.ident) != USE_CASE_STRUCT {
        return None;
    }
    let PathArguments::AngleBracketed(arguments) = &last.arguments else {
        return None;
    };
    let type_args: Vec<&Type> = arguments
        .args
        .iter()
        .filter_map(|arg| match arg {
            GenericArgument::Type(ty) => Some(ty),
            GenericArgument::Lifetime(_)
            | GenericArgument::Const(_)
            | GenericArgument::AssocType(_)
            | GenericArgument::AssocConst(_)
            | GenericArgument::Constraint(_)
            | _ => None,
        })
        .collect();
    let ty = type_args.get(position)?;
    type_last_name(ty)
}

fn bounds_require_trait(
    bounds: &Punctuated<TypeParamBound, Token![+]>,
    module_name: &str,
    trait_name: &str,
) -> bool {
    bounds.iter().any(|bound| match bound {
        TypeParamBound::Trait(trait_bound) => {
            let names = segment_names(&trait_bound.path);
            match names.as_slice() {
                [single] => single == trait_name,
                [root, module, name] => {
                    root == "crate" && module == module_name && name == trait_name
                }
                _ => false,
            }
        }
        TypeParamBound::Lifetime(_)
        | TypeParamBound::PreciseCapture(_)
        | TypeParamBound::Verbatim(_)
        | _ => false,
    })
}

fn generics_bind_param_to_trait(
    generics: &Generics,
    param_name: &str,
    module_name: &str,
    trait_name: &str,
) -> bool {
    let direct = generics.params.iter().any(|param| match param {
        GenericParam::Type(ty) if unraw(&ty.ident) == param_name => {
            bounds_require_trait(&ty.bounds, module_name, trait_name)
        }
        GenericParam::Type(_) | GenericParam::Lifetime(_) | GenericParam::Const(_) => false,
    });
    if direct {
        return true;
    }
    generics.where_clause.as_ref().is_some_and(|clause| {
        clause.predicates.iter().any(|predicate| match predicate {
            WherePredicate::Type(ty)
                if type_last_name(&ty.bounded_ty).as_deref() == Some(param_name) =>
            {
                bounds_require_trait(&ty.bounds, module_name, trait_name)
            }
            WherePredicate::Type(_) | WherePredicate::Lifetime(_) | _ => false,
        })
    })
}

fn is_self_prompt_await(expr: &Expr) -> bool {
    let Expr::Await(awaited) = expr else {
        return false;
    };
    let Expr::MethodCall(call) = awaited.base.as_ref() else {
        return false;
    };
    if !PROMPT_METHODS.contains(&unraw(&call.method).as_str()) {
        return false;
    }
    let Expr::Field(ExprField { base, member, .. }) = call.receiver.as_ref() else {
        return false;
    };
    let Expr::Path(receiver_path) = base.as_ref() else {
        return false;
    };
    let is_self = receiver_path.qself.is_none()
        && receiver_path.path.segments.len() == 1
        && receiver_path
            .path
            .segments
            .first()
            .is_some_and(|seg| unraw(&seg.ident) == "self");
    let is_prompt = match member {
        Member::Named(ident) => unraw(ident) == "prompt",
        Member::Unnamed(_) => false,
    };
    is_self && is_prompt
}

fn is_confirmed_decision_pat(pat: &Pat) -> bool {
    match pat {
        Pat::Path(path) if path.qself.is_none() => {
            let names = segment_names(&path.path);
            names.len() >= 2
                && names.get(names.len().saturating_sub(2)).map(String::as_str)
                    == Some(DECISION_ENUM)
                && names.last().map(String::as_str) == Some(CONFIRMED_VARIANT)
        }
        Pat::Const(_)
        | Pat::Ident(_)
        | Pat::Lit(_)
        | Pat::Macro(_)
        | Pat::Or(_)
        | Pat::Paren(_)
        | Pat::Path(_)
        | Pat::Range(_)
        | Pat::Reference(_)
        | Pat::Rest(_)
        | Pat::Slice(_)
        | Pat::Struct(_)
        | Pat::Tuple(_)
        | Pat::TupleStruct(_)
        | Pat::Type(_)
        | Pat::Verbatim(_)
        | Pat::Wild(_)
        | _ => false,
    }
}

fn is_direct_canonical_issuer_path(rules: &IdentityConfirmationRules, expr: &ExprPath) -> bool {
    if expr.qself.is_some() {
        return false;
    }
    let names = segment_names(&expr.path);
    match names.as_slice() {
        [ty, func] => ty == &rules.confirmation_type && func == &rules.issuer_function,
        [crate_seg, ty, func] => {
            crate_seg == "domain"
                && ty == &rules.confirmation_type
                && func == &rules.issuer_function
        }
        _ => false,
    }
}

struct UseCaseFileCheck<'a> {
    rules: &'a IdentityConfirmationRules,
    prompt_param_pos: Option<usize>,
    seen_use_case_struct: usize,
    scope: Scope,
    in_prompt_gated_impl: bool,
    in_async_method: bool,
    in_confirmed_arm: bool,
    calls_in_arm: usize,
    loop_depth: usize,
    closure_depth: usize,
    approved_call_ptr: Option<*const ExprPath>,
    findings: &'a mut BTreeSet<Rule>,
}

impl UseCaseFileCheck<'_> {
    fn check_signature_leaks(&mut self, value: &impl ToTokens) {
        if self.scope == Scope::Product
            && (tokens_contain(value, &self.rules.confirmation_type)
                || tokens_contain(value, &self.rules.seal_type))
        {
            self.findings.insert(Rule::IdentityConfirmationIssuance);
        }
    }

    fn check_prompt_import_source(&mut self, entry: &super::ast::uses::UseEntry) {
        let imported = entry.imported();
        let is_prompt_symbol = imported == self.rules.prompt_trait || imported == DECISION_ENUM;
        if is_prompt_symbol
            && entry.path.as_slice() != ["crate", self.rules.prompt_module.as_str(), imported]
        {
            self.findings.insert(Rule::IdentityConfirmationIssuance);
        }
    }
}

impl<'ast> Visit<'ast> for UseCaseFileCheck<'_> {
    fn visit_item(&mut self, i: &'ast Item) {
        let saved = (
            self.in_prompt_gated_impl,
            self.in_async_method,
            self.in_confirmed_arm,
            self.calls_in_arm,
        );
        self.in_prompt_gated_impl = false;
        self.in_async_method = false;
        self.in_confirmed_arm = false;
        self.calls_in_arm = 0;
        visit::visit_item(self, i);
        (
            self.in_prompt_gated_impl,
            self.in_async_method,
            self.in_confirmed_arm,
            self.calls_in_arm,
        ) = saved;
    }

    fn visit_item_use(&mut self, i: &'ast ItemUse) {
        if self.scope == Scope::Product {
            if !private(&i.vis) {
                self.findings.insert(Rule::IdentityConfirmationIssuance);
            }
            for entry in use_entries(&i.tree) {
                self.check_prompt_import_source(&entry);
            }
        }
        visit::visit_item_use(self, i);
    }

    fn visit_use_rename(&mut self, i: &'ast UseRename) {
        if self.scope == Scope::Product {
            self.findings.insert(Rule::IdentityConfirmationIssuance);
        }
        visit::visit_use_rename(self, i);
    }

    fn visit_use_glob(&mut self, i: &'ast UseGlob) {
        if self.scope == Scope::Product {
            self.findings.insert(Rule::IdentityConfirmationIssuance);
        }
        visit::visit_use_glob(self, i);
    }

    fn visit_item_type(&mut self, i: &'ast ItemType) {
        if self.scope == Scope::Product {
            self.findings.insert(Rule::IdentityConfirmationIssuance);
        }
        visit::visit_item_type(self, i);
    }

    fn visit_item_mod(&mut self, i: &'ast ItemMod) {
        let outer = self.scope;
        if outer == Scope::Product && is_cfg_test_mod(i) {
            if let Some((_, nested)) = &i.content
                && has_non_private_item(nested)
            {
                self.findings.insert(Rule::IdentityConfirmationIssuance);
            }
            if stream_contains_ident(i.to_token_stream(), &self.rules.issuer_function)
                || stream_contains_ident(i.to_token_stream(), &self.rules.seal_type)
            {
                self.findings.insert(Rule::IdentityConfirmationIssuance);
            }
            self.scope = Scope::Test;
            visit::visit_item_mod(self, i);
            self.scope = outer;
            return;
        }
        self.findings.insert(Rule::IdentityConfirmationIssuance);
        visit::visit_item_mod(self, i);
        self.scope = outer;
    }

    fn visit_item_trait(&mut self, i: &'ast ItemTrait) {
        if self.scope == Scope::Product {
            self.findings.insert(Rule::IdentityConfirmationIssuance);
        }
        visit::visit_item_trait(self, i);
    }

    fn visit_item_enum(&mut self, i: &'ast ItemEnum) {
        self.check_signature_leaks(i);
        let name = unraw(&i.ident);
        if name == DECISION_ENUM || name == self.rules.confirmation_type {
            self.findings.insert(Rule::IdentityConfirmationIssuance);
        }
        visit::visit_item_enum(self, i);
    }

    fn visit_item_struct(&mut self, i: &'ast ItemStruct) {
        self.check_signature_leaks(i);
        let name = unraw(&i.ident);
        if name == USE_CASE_STRUCT {
            self.seen_use_case_struct = self.seen_use_case_struct.saturating_add(1);
            if self.seen_use_case_struct > 1 {
                self.findings.insert(Rule::IdentityConfirmationIssuance);
            }
        }
        if name == DECISION_ENUM
            || name == self.rules.confirmation_type
            || name == self.rules.seal_type
        {
            self.findings.insert(Rule::IdentityConfirmationIssuance);
        }
        visit::visit_item_struct(self, i);
    }

    fn visit_item_fn(&mut self, i: &'ast ItemFn) {
        self.check_signature_leaks(&i.sig);
        visit::visit_item_fn(self, i);
    }

    fn visit_item_const(&mut self, i: &'ast ItemConst) {
        self.check_signature_leaks(&i.ty);
        if unraw(&i.ident) == CONFIRMED_VARIANT {
            self.findings.insert(Rule::IdentityConfirmationIssuance);
        }
        visit::visit_item_const(self, i);
    }

    fn visit_item_static(&mut self, i: &'ast ItemStatic) {
        self.check_signature_leaks(&i.ty);
        visit::visit_item_static(self, i);
    }

    fn visit_item_impl(&mut self, i: &'ast ItemImpl) {
        let outer = self.in_prompt_gated_impl;
        let is_use_case_inherent = self.scope == Scope::Product
            && i.trait_.is_none()
            && self
                .prompt_param_pos
                .and_then(|pos| type_arg_at_position(&i.self_ty, pos))
                .is_some_and(|param_name| {
                    generics_bind_param_to_trait(
                        &i.generics,
                        &param_name,
                        &self.rules.prompt_module,
                        &self.rules.prompt_trait,
                    )
                });
        self.in_prompt_gated_impl = is_use_case_inherent;
        if !is_use_case_inherent {
            let shadows_prompt_method = i.items.iter().any(|item| match item {
                ImplItem::Fn(method) => PROMPT_METHODS.contains(&unraw(&method.sig.ident).as_str()),
                ImplItem::Const(_)
                | ImplItem::Type(_)
                | ImplItem::Macro(_)
                | ImplItem::Verbatim(_)
                | _ => false,
            });
            if shadows_prompt_method {
                self.findings.insert(Rule::IdentityConfirmationIssuance);
            }
        }
        visit::visit_item_impl(self, i);
        self.in_prompt_gated_impl = outer;
    }

    fn visit_impl_item_fn(&mut self, i: &'ast ImplItemFn) {
        self.check_signature_leaks(&i.sig);
        let outer_async = self.in_async_method;
        let outer_arm = self.in_confirmed_arm;
        let outer_calls = self.calls_in_arm;
        let has_receiver = i.sig.receiver().is_some();
        self.in_async_method =
            self.in_prompt_gated_impl && i.sig.asyncness.is_some() && has_receiver;
        self.in_confirmed_arm = false;
        self.calls_in_arm = 0;
        visit::visit_impl_item_fn(self, i);
        self.in_async_method = outer_async;
        self.in_confirmed_arm = outer_arm;
        self.calls_in_arm = outer_calls;
    }

    fn visit_expr_match(&mut self, i: &'ast ExprMatch) {
        let prompt_match = self.in_async_method
            && self.loop_depth == 0
            && self.closure_depth == 0
            && is_self_prompt_await(&i.expr);
        let outer_arm = self.in_confirmed_arm;
        let outer_calls = self.calls_in_arm;
        self.in_confirmed_arm = false;
        self.calls_in_arm = 0;
        self.visit_expr(&i.expr);
        for arm in &i.arms {
            let confirmed_arm =
                prompt_match && arm.guard.is_none() && is_confirmed_decision_pat(&arm.pat);
            if let Some((_, guard)) = &arm.guard {
                self.in_confirmed_arm = false;
                self.visit_expr(guard);
            }
            self.in_confirmed_arm = confirmed_arm;
            self.calls_in_arm = 0;
            self.visit_expr(&arm.body);
        }
        self.in_confirmed_arm = outer_arm;
        self.calls_in_arm = outer_calls;
    }

    fn visit_expr_for_loop(&mut self, i: &'ast ExprForLoop) {
        self.loop_depth = self.loop_depth.saturating_add(1);
        visit::visit_expr_for_loop(self, i);
        self.loop_depth = self.loop_depth.saturating_sub(1);
    }

    fn visit_expr_while(&mut self, i: &'ast ExprWhile) {
        self.loop_depth = self.loop_depth.saturating_add(1);
        visit::visit_expr_while(self, i);
        self.loop_depth = self.loop_depth.saturating_sub(1);
    }

    fn visit_expr_loop(&mut self, i: &'ast ExprLoop) {
        self.loop_depth = self.loop_depth.saturating_add(1);
        visit::visit_expr_loop(self, i);
        self.loop_depth = self.loop_depth.saturating_sub(1);
    }

    fn visit_expr_closure(&mut self, i: &'ast ExprClosure) {
        self.closure_depth = self.closure_depth.saturating_add(1);
        visit::visit_expr_closure(self, i);
        self.closure_depth = self.closure_depth.saturating_sub(1);
    }

    fn visit_expr_async(&mut self, i: &'ast ExprAsync) {
        self.closure_depth = self.closure_depth.saturating_add(1);
        visit::visit_expr_async(self, i);
        self.closure_depth = self.closure_depth.saturating_sub(1);
    }

    fn visit_expr_call(&mut self, i: &'ast ExprCall) {
        let outer_ptr = self.approved_call_ptr;
        if let Expr::Path(callee) = i.func.as_ref()
            && segment_names(&callee.path)
                .iter()
                .any(|seg| seg == &self.rules.issuer_function)
        {
            let allowed = self.scope == Scope::Product
                && self.in_confirmed_arm
                && self.loop_depth == 0
                && self.closure_depth == 0
                && self.calls_in_arm == 0
                && i.args.is_empty()
                && is_direct_canonical_issuer_path(self.rules, callee);
            if allowed {
                self.calls_in_arm = self.calls_in_arm.saturating_add(1);
                self.approved_call_ptr = Some(std::ptr::from_ref(callee));
            } else {
                self.findings.insert(Rule::IdentityConfirmationIssuance);
            }
        }
        visit::visit_expr_call(self, i);
        self.approved_call_ptr = outer_ptr;
    }

    fn visit_expr_path(&mut self, i: &'ast ExprPath) {
        let is_approved = self
            .approved_call_ptr
            .is_some_and(|ptr| std::ptr::eq(ptr, std::ptr::from_ref(i)));
        if !is_approved {
            let mentions_forbidden = segment_names(&i.path)
                .iter()
                .any(|seg| seg == &self.rules.issuer_function || seg == &self.rules.seal_type);
            if mentions_forbidden {
                self.findings.insert(Rule::IdentityConfirmationIssuance);
            }
        }
        visit::visit_expr_path(self, i);
    }

    fn visit_macro(&mut self, i: &'ast Macro) {
        if stream_contains_ident(i.tokens.clone(), &self.rules.issuer_function)
            || stream_contains_ident(i.tokens.clone(), &self.rules.seal_type)
        {
            self.findings.insert(Rule::IdentityConfirmationIssuance);
        }
        visit::visit_macro(self, i);
    }
}

fn is_confirmed_decision_expr(expr: &Expr) -> bool {
    match expr {
        Expr::Path(path) if path.qself.is_none() => {
            let names = segment_names(&path.path);
            names.last().map(String::as_str) == Some(CONFIRMED_VARIANT)
        }
        Expr::Group(group) => is_confirmed_decision_expr(&group.expr),
        Expr::Paren(paren) => is_confirmed_decision_expr(&paren.expr),
        Expr::Array(_)
        | Expr::Assign(_)
        | Expr::Async(_)
        | Expr::Await(_)
        | Expr::Binary(_)
        | Expr::Block(_)
        | Expr::Break(_)
        | Expr::Call(_)
        | Expr::Cast(_)
        | Expr::Closure(_)
        | Expr::Const(_)
        | Expr::Continue(_)
        | Expr::Field(_)
        | Expr::ForLoop(_)
        | Expr::If(_)
        | Expr::Index(_)
        | Expr::Infer(_)
        | Expr::Let(_)
        | Expr::Lit(_)
        | Expr::Loop(_)
        | Expr::Macro(_)
        | Expr::Match(_)
        | Expr::MethodCall(_)
        | Expr::Path(_)
        | Expr::Range(_)
        | Expr::RawAddr(_)
        | Expr::Reference(_)
        | Expr::Repeat(_)
        | Expr::Return(_)
        | Expr::Struct(_)
        | Expr::Try(_)
        | Expr::TryBlock(_)
        | Expr::Tuple(_)
        | Expr::Unary(_)
        | Expr::Unsafe(_)
        | Expr::Verbatim(_)
        | Expr::While(_)
        | Expr::Yield(_)
        | _ => false,
    }
}

struct ProtectedFakeCheck<'a> {
    rules: &'a IdentityConfirmationRules,
    scope: Scope,
    in_confirmed_arm: bool,
    calls_in_arm: usize,
    loop_depth: usize,
    closure_depth: usize,
    approved_call_ptr: Option<*const ExprPath>,
    findings: &'a mut BTreeSet<Rule>,
}

impl<'ast> Visit<'ast> for ProtectedFakeCheck<'_> {
    fn visit_item_use(&mut self, i: &'ast ItemUse) {
        let exposed = !private(&i.vis);
        for entry in use_entries(&i.tree) {
            let imported = entry.imported();
            let sensitive = imported == self.rules.issuer_function
                || imported == self.rules.confirmation_type
                || imported == self.rules.seal_type
                || imported == self.rules.prompt_trait
                || imported == self.rules.prompt_module;
            match entry.kind {
                UseKind::Rename(_) if sensitive => {
                    self.findings.insert(Rule::IdentityConfirmationIssuance);
                }
                UseKind::Glob if exposed => {
                    self.findings.insert(Rule::IdentityConfirmationIssuance);
                }
                UseKind::Name | UseKind::Rename(_) if exposed && sensitive => {
                    self.findings.insert(Rule::IdentityConfirmationIssuance);
                }
                UseKind::Name | UseKind::Rename(_) | UseKind::Glob => {}
            }
        }
        visit::visit_item_use(self, i);
    }

    fn visit_item_mod(&mut self, i: &'ast ItemMod) {
        let outer = self.scope;
        if outer == Scope::Product && is_cfg_test_mod(i) {
            if let Some((_, nested)) = &i.content
                && has_non_private_item(nested)
            {
                self.findings.insert(Rule::IdentityConfirmationIssuance);
            }
            self.scope = Scope::Test;
            visit::visit_item_mod(self, i);
            self.scope = outer;
            return;
        }
        self.findings.insert(Rule::IdentityConfirmationIssuance);
        visit::visit_item_mod(self, i);
        self.scope = outer;
    }

    fn visit_expr_match(&mut self, i: &'ast ExprMatch) {
        let outer_arm = self.in_confirmed_arm;
        let outer_calls = self.calls_in_arm;
        let non_constant_scrutinee = !is_confirmed_decision_expr(&i.expr);
        self.in_confirmed_arm = false;
        self.calls_in_arm = 0;
        self.visit_expr(&i.expr);
        for arm in &i.arms {
            let confirmed_arm = non_constant_scrutinee
                && arm.guard.is_none()
                && is_confirmed_decision_pat(&arm.pat);
            if let Some((_, guard)) = &arm.guard {
                self.in_confirmed_arm = false;
                self.visit_expr(guard);
            }
            self.in_confirmed_arm = confirmed_arm;
            self.calls_in_arm = 0;
            self.visit_expr(&arm.body);
        }
        self.in_confirmed_arm = outer_arm;
        self.calls_in_arm = outer_calls;
    }

    fn visit_expr_for_loop(&mut self, i: &'ast ExprForLoop) {
        self.loop_depth = self.loop_depth.saturating_add(1);
        visit::visit_expr_for_loop(self, i);
        self.loop_depth = self.loop_depth.saturating_sub(1);
    }

    fn visit_expr_while(&mut self, i: &'ast ExprWhile) {
        self.loop_depth = self.loop_depth.saturating_add(1);
        visit::visit_expr_while(self, i);
        self.loop_depth = self.loop_depth.saturating_sub(1);
    }

    fn visit_expr_loop(&mut self, i: &'ast ExprLoop) {
        self.loop_depth = self.loop_depth.saturating_add(1);
        visit::visit_expr_loop(self, i);
        self.loop_depth = self.loop_depth.saturating_sub(1);
    }

    fn visit_expr_closure(&mut self, i: &'ast ExprClosure) {
        self.closure_depth = self.closure_depth.saturating_add(1);
        visit::visit_expr_closure(self, i);
        self.closure_depth = self.closure_depth.saturating_sub(1);
    }

    fn visit_expr_async(&mut self, i: &'ast ExprAsync) {
        self.closure_depth = self.closure_depth.saturating_add(1);
        visit::visit_expr_async(self, i);
        self.closure_depth = self.closure_depth.saturating_sub(1);
    }

    fn visit_expr_call(&mut self, i: &'ast ExprCall) {
        let outer_ptr = self.approved_call_ptr;
        if let Expr::Path(callee) = i.func.as_ref()
            && segment_names(&callee.path)
                .iter()
                .any(|seg| seg == &self.rules.issuer_function)
        {
            let allowed = self.in_confirmed_arm
                && self.loop_depth == 0
                && self.closure_depth == 0
                && self.calls_in_arm == 0
                && i.args.is_empty()
                && is_direct_canonical_issuer_path(self.rules, callee);
            if allowed {
                self.calls_in_arm = self.calls_in_arm.saturating_add(1);
                self.approved_call_ptr = Some(std::ptr::from_ref(callee));
            } else {
                self.findings.insert(Rule::IdentityConfirmationIssuance);
            }
        }
        visit::visit_expr_call(self, i);
        self.approved_call_ptr = outer_ptr;
    }

    fn visit_expr_path(&mut self, i: &'ast ExprPath) {
        let is_approved = self
            .approved_call_ptr
            .is_some_and(|ptr| std::ptr::eq(ptr, std::ptr::from_ref(i)));
        if !is_approved {
            let forbidden = segment_names(&i.path)
                .iter()
                .any(|seg| seg == &self.rules.issuer_function || seg == &self.rules.seal_type);
            if forbidden {
                self.findings.insert(Rule::IdentityConfirmationIssuance);
            }
        }
        visit::visit_expr_path(self, i);
    }

    fn visit_macro(&mut self, i: &'ast Macro) {
        if stream_contains_ident(i.tokens.clone(), &self.rules.issuer_function)
            || stream_contains_ident(i.tokens.clone(), &self.rules.seal_type)
        {
            self.findings.insert(Rule::IdentityConfirmationIssuance);
        }
        visit::visit_macro(self, i);
    }
}

pub(super) struct CheckContext<'a> {
    pub(super) crate_name: &'a CrateName,
    pub(super) source_aliases: &'a BTreeMap<String, Vec<String>>,
    pub(super) relative: &'a str,
    pub(super) text: &'a str,
    pub(super) file: &'a syn::File,
    pub(super) tokens: &'a TokenStream,
}

fn may_touch_identity_confirmation(
    rules: &IdentityConfirmationRules,
    ctx: &CheckContext<'_>,
) -> bool {
    if ctx.text.contains(&rules.issuer_function)
        || ctx.text.contains(&rules.seal_type)
        || ctx.text.contains(&rules.confirmation_type)
        || ctx.text.contains(&rules.prompt_trait)
        || ctx.text.contains(&rules.prompt_module)
    {
        return true;
    }
    let defining_crate = rules.prompt_defining_crate.replace('-', "_");
    ctx.source_aliases.iter().any(|(alias, targets)| {
        targets.iter().any(|target| target == &defining_crate) && ctx.text.contains(alias)
    })
}

fn check_use_case_file(
    rules: &IdentityConfirmationRules,
    ctx: &CheckContext<'_>,
    findings: &mut BTreeSet<Rule>,
) {
    if rules.issuance_allowed_files.contains(ctx.relative) {
        let prompt_param_pos = if let Ok(position) = find_prompt_generic_position(ctx.file) {
            position
        } else {
            findings.insert(Rule::IdentityConfirmationIssuance);
            None
        };
        let mut scan = UseCaseFileCheck {
            rules,
            prompt_param_pos,
            seen_use_case_struct: 0,
            scope: Scope::Product,
            in_prompt_gated_impl: false,
            in_async_method: false,
            in_confirmed_arm: false,
            calls_in_arm: 0,
            loop_depth: 0,
            closure_depth: 0,
            approved_call_ptr: None,
            findings,
        };
        scan.visit_file(ctx.file);
    } else if stream_contains_ident(ctx.tokens.clone(), &rules.issuer_function)
        || stream_contains_ident(ctx.tokens.clone(), &rules.seal_type)
    {
        findings.insert(Rule::IdentityConfirmationIssuance);
    }
}

fn check_protected_fake_file(
    rules: &IdentityConfirmationRules,
    ctx: &CheckContext<'_>,
    findings: &mut BTreeSet<Rule>,
) {
    if stream_contains_ident(ctx.tokens.clone(), &rules.seal_type) {
        findings.insert(Rule::IdentityConfirmationIssuance);
    }
    let mut scan = ProtectedFakeCheck {
        rules,
        scope: Scope::Product,
        in_confirmed_arm: false,
        calls_in_arm: 0,
        loop_depth: 0,
        closure_depth: 0,
        approved_call_ptr: None,
        findings,
    };
    scan.visit_file(ctx.file);
}

fn check_unauthorized_file(
    rules: &IdentityConfirmationRules,
    ctx: &CheckContext<'_>,
    findings: &mut BTreeSet<Rule>,
) {
    if stream_contains_ident(ctx.tokens.clone(), &rules.issuer_function)
        || stream_contains_ident(ctx.tokens.clone(), &rules.seal_type)
    {
        findings.insert(Rule::IdentityConfirmationIssuance);
    }
    let mut scan = UnauthorizedFileCheck {
        rules,
        in_domain: ctx.crate_name.as_str() == "domain",
        findings,
    };
    scan.visit_file(ctx.file);
}

pub(super) fn check(rules: &IdentityConfirmationRules, ctx: &CheckContext<'_>) -> BTreeSet<Rule> {
    let mut findings = BTreeSet::new();

    let unauthorized_child = is_unauthorized_child_file(rules, ctx.relative);
    if unauthorized_child {
        findings.insert(Rule::IdentityConfirmationIssuance);
    }

    let is_special = unauthorized_child
        || ctx.relative == rules.defining_file
        || ctx.relative == rules.defining_test_file
        || ctx.relative == rules.use_case_file
        || rules.issuance_allowed_files.contains(ctx.relative);
    if !is_special && !may_touch_identity_confirmation(rules, ctx) {
        return findings;
    }

    let aliases = PromptAliases::collect(rules, ctx.source_aliases, ctx.file);
    let mut prompt_check = PromptCheck {
        rules,
        crate_name: ctx.crate_name,
        relative: ctx.relative,
        aliases,
        findings: &mut findings,
    };
    prompt_check.visit_file(ctx.file);

    if ctx.relative == rules.defining_file {
        let mut scan = DefiningFileCheck {
            rules,
            scope: Scope::Product,
            target: DefiningTarget::Other,
            in_canonical_issuer: false,
            canonical_impl_count: 0,
            findings: &mut findings,
        };
        scan.visit_file(ctx.file);
    } else if ctx.relative == rules.defining_test_file {
        check_defining_test_file(rules, ctx.file, ctx.tokens, &mut findings);
    } else if ctx.relative == rules.use_case_file {
        check_use_case_file(rules, ctx, &mut findings);
    } else if rules.issuance_allowed_files.contains(ctx.relative) {
        check_protected_fake_file(rules, ctx, &mut findings);
    } else {
        check_unauthorized_file(rules, ctx, &mut findings);
    }

    findings
}
