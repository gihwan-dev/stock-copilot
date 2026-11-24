mod prompt_def;
#[cfg(test)]
mod tests;
mod use_case;

use std::collections::{BTreeMap, BTreeSet};

use proc_macro2::{Delimiter, TokenStream, TokenTree};
use quote::ToTokens;
use syn::ext::IdentExt;
use syn::parse::Parser;
use syn::punctuated::Punctuated;
use syn::visit::{self, Visit};
use syn::{
    Attribute, Expr, ExprAsync, ExprCall, ExprClosure, ExprForLoop, ExprLoop, ExprMatch, ExprPath,
    ExprStruct, ExprWhile, Fields, FnArg, ImplItem, ImplItemFn, ImplItemType, Item, ItemConst,
    ItemEnum, ItemExternCrate, ItemFn, ItemImpl, ItemMod, ItemStatic, ItemStruct, ItemTrait,
    ItemType, ItemUse, LitByteStr, LitCStr, LitStr, Macro, Pat, Path, PathArguments, ReturnType,
    Signature, Stmt, Token, TraitItemType, Type, TypeBareFn, TypeImplTrait, TypePath,
    TypeReference, TypeTraitObject, UseRename, Visibility,
};

use super::super::rules::IdentityConfirmationRules;
use super::super::{CrateName, Rule};
use super::ast::uses::{UseKind, has_pair, unraw, use_entries};

const ALLOWED_DERIVES: [&str; 3] = ["Debug", "PartialEq", "Eq"];
const DEFINING_MACRO_BANNED: [&str; 8] = [
    "impl", "struct", "mod", "fn", "const", "static", "type", "use",
];
const CONFIRMED_WRAPPER_TYPES: [&str; 5] = [
    "IdentityChangeConfirmation",
    "ConfirmedIdentifierChange",
    "MappingCorrectionConfirmation",
    "ConfirmedRetirementCorrection",
    "ConfirmedLinkCorrection",
];
const OUTER_CONFIRMED_TYPES: [&str; 3] = [
    "ConfirmedIdentifierChange",
    "ConfirmedRetirementCorrection",
    "ConfirmedLinkCorrection",
];
const INNER_CONFIRMATION_WRAPPERS: [&str; 2] = [
    "IdentityChangeConfirmation",
    "MappingCorrectionConfirmation",
];
const ORIGIN_ENUM_TYPE: &str = "IdentityConfirmationOrigin";
const ALL_CONFIRMATION_TYPES: [&str; 8] = [
    "UserIdentityConfirmation",
    "UserIdentityConfirmationSeal",
    "IdentityConfirmationOrigin",
    "IdentityChangeConfirmation",
    "MappingCorrectionConfirmation",
    "ConfirmedIdentifierChange",
    "ConfirmedRetirementCorrection",
    "ConfirmedLinkCorrection",
];
const BANNED_PASTE_MACROS: [&str; 6] = [
    "paste",
    "_tauri_specta_paste",
    "fn_datatype",
    "_fn_datatype",
    "_fn_datatype_internal",
    "_collect_functions",
];
const PASTE_REEXPORT_CRATES: [&str; 2] = ["tauri_specta", "specta"];
const PASTE_REEXPORT_MODULES: [&str; 2] = ["internal", "function"];
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
    Wrapper,
    Origin,
}

const fn matches_defining_seal_target(target: DefiningTarget) -> bool {
    match target {
        DefiningTarget::Confirmation | DefiningTarget::Seal => true,
        DefiningTarget::Other | DefiningTarget::Wrapper | DefiningTarget::Origin => false,
    }
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

fn literal_text(raw: &str) -> Option<String> {
    if let Ok(text) = syn::parse_str::<LitStr>(raw) {
        Some(text.value())
    } else if let Ok(text) = syn::parse_str::<LitByteStr>(raw) {
        Some(String::from_utf8_lossy(&text.value()).into_owned())
    } else if let Ok(text) = syn::parse_str::<LitCStr>(raw) {
        Some(text.value().to_string_lossy().into_owned())
    } else {
        None
    }
}

fn stream_literal_contains(tokens: TokenStream, needle: &str) -> bool {
    tokens.into_iter().any(|token| match token {
        TokenTree::Group(group) => stream_literal_contains(group.stream(), needle),
        TokenTree::Literal(literal) => {
            literal_text(&literal.to_string()).is_some_and(|value| value.contains(needle))
        }
        TokenTree::Ident(_) | TokenTree::Punct(_) => false,
    })
}

pub(crate) fn is_cfg_test_mod(item: &ItemMod) -> bool {
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
    let in_prompt_dir = child_directory_prefix(&prompt_def::prompt_defining_file(rules))
        .is_some_and(|prefix| relative.starts_with(&prefix));
    let in_allowed_dir = rules.issuance_allowed_files.iter().any(|allowed| {
        child_directory_prefix(allowed).is_some_and(|prefix| relative.starts_with(&prefix))
    });
    in_defining_dir || in_use_case_dir || in_prompt_dir || in_allowed_dir
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
            UseKind::Rename(alias) => {
                if imported == self.rules.prompt_trait || alias == &self.rules.prompt_trait {
                    self.findings.insert(Rule::IdentityConfirmationPromptAlias);
                }
                if exposed == Exposure::Exported && (sensitive || alias == &self.rules.prompt_trait)
                {
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
        let has_prompt = idents.iter().any(|ident| ident == &self.rules.prompt_trait);
        if has_prompt && !self.impl_allowed() {
            self.findings.insert(Rule::IdentityConfirmationPromptImpl);
        }
        if has_pair(&idents, &self.rules.prompt_trait, "as").is_some()
            || has_pair(&idents, "as", &self.rules.prompt_trait).is_some()
        {
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
    fn visit_attribute(&mut self, i: &'ast Attribute) {
        let tokens = i.to_token_stream();
        if !self.impl_allowed()
            && (stream_literal_contains(tokens.clone(), &self.rules.prompt_trait)
                || stream_contains_ident(tokens, &self.rules.prompt_trait))
        {
            self.findings.insert(Rule::IdentityConfirmationPromptImpl);
        }
        visit::visit_attribute(self, i);
    }

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
            if !self.impl_allowed()
                && stream_contains_ident(i.tokens.clone(), &self.rules.prompt_trait)
            {
                self.findings.insert(Rule::IdentityConfirmationPromptImpl);
            }
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
    in_xtask: bool,
    findings: &'a mut BTreeSet<Rule>,
}

impl<'ast> Visit<'ast> for UnauthorizedFileCheck<'_> {
    fn visit_attribute(&mut self, i: &'ast Attribute) {
        if stream_literal_contains(i.to_token_stream(), &self.rules.issuer_function)
            || stream_literal_contains(i.to_token_stream(), &self.rules.seal_type)
        {
            self.findings.insert(Rule::IdentityConfirmationIssuance);
        }
        visit::visit_attribute(self, i);
    }

    fn visit_macro(&mut self, i: &'ast Macro) {
        if !self.in_xtask
            && (stream_literal_contains(i.tokens.clone(), &self.rules.issuer_function)
                || stream_literal_contains(i.tokens.clone(), &self.rules.seal_type))
        {
            self.findings.insert(Rule::IdentityConfirmationIssuance);
        }
        visit::visit_macro(self, i);
    }

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

fn is_public(vis: &Visibility) -> bool {
    match vis {
        Visibility::Public(_) => true,
        Visibility::Inherited | Visibility::Restricted(_) => false,
    }
}

fn single_segment_type_name(ty: &Type) -> Option<String> {
    let inner = if let Type::Group(group) = ty {
        &group.elem
    } else if let Type::Paren(paren) = ty {
        &paren.elem
    } else {
        ty
    };
    let Type::Path(path) = inner else {
        return None;
    };
    if path.qself.is_some() || path.path.leading_colon.is_some() || path.path.segments.len() != 1 {
        return None;
    }
    let segment = path.path.segments.first()?;
    let no_args = match segment.arguments {
        PathArguments::None => true,
        PathArguments::AngleBracketed(_) | PathArguments::Parenthesized(_) => false,
    };
    no_args.then(|| unraw(&segment.ident))
}

fn has_single_private_seal_field(fields: &Fields, seal_type: &str) -> bool {
    let Fields::Unnamed(unnamed) = fields else {
        return false;
    };
    if unnamed.unnamed.len() != 1 {
        return false;
    }
    let Some(field) = unnamed.unnamed.first() else {
        return false;
    };
    private(&field.vis) && single_segment_type_name(&field.ty).as_deref() == Some(seal_type)
}

fn is_unit_struct_fields(fields: &Fields) -> bool {
    match fields {
        Fields::Unit => true,
        Fields::Named(_) | Fields::Unnamed(_) => false,
    }
}

fn all_fields_private(fields: &Fields) -> bool {
    match fields {
        Fields::Unit => true,
        Fields::Named(named) => named.named.iter().all(|field| private(&field.vis)),
        Fields::Unnamed(unnamed) => unnamed.unnamed.iter().all(|field| private(&field.vis)),
    }
}

fn tokens_contain_any_confirmation(value: &impl ToTokens) -> bool {
    let mut idents = Vec::new();
    token_idents(value.to_token_stream(), &mut idents);
    idents
        .iter()
        .any(|ident| ALL_CONFIRMATION_TYPES.contains(&ident.as_str()))
}

fn unwrap_group_or_paren_type(ty: &Type) -> &Type {
    if let Type::Group(group) = ty {
        unwrap_group_or_paren_type(&group.elem)
    } else if let Type::Paren(paren) = ty {
        unwrap_group_or_paren_type(&paren.elem)
    } else {
        ty
    }
}

fn type_is_shared_ref(ty: &Type) -> bool {
    match unwrap_group_or_paren_type(ty) {
        Type::Reference(reference) => reference.mutability.is_none(),
        Type::Array(_)
        | Type::BareFn(_)
        | Type::Group(_)
        | Type::ImplTrait(_)
        | Type::Infer(_)
        | Type::Macro(_)
        | Type::Never(_)
        | Type::Paren(_)
        | Type::Path(_)
        | Type::Ptr(_)
        | Type::Slice(_)
        | Type::TraitObject(_)
        | Type::Tuple(_)
        | Type::Verbatim(_)
        | _ => false,
    }
}

struct MutRefFinder {
    found: bool,
}

impl<'ast> Visit<'ast> for MutRefFinder {
    fn visit_type_reference(&mut self, i: &'ast TypeReference) {
        if i.mutability.is_some() {
            self.found = true;
        }
        visit::visit_type_reference(self, i);
    }
}

fn type_has_mut_ref(ty: &Type) -> bool {
    let mut finder = MutRefFinder { found: false };
    finder.visit_type(ty);
    finder.found
}

struct CallbackOrDynamicFinder {
    found: bool,
}

impl<'ast> Visit<'ast> for CallbackOrDynamicFinder {
    fn visit_type_bare_fn(&mut self, i: &'ast TypeBareFn) {
        self.found = true;
        visit::visit_type_bare_fn(self, i);
    }

    fn visit_type_impl_trait(&mut self, i: &'ast TypeImplTrait) {
        self.found = true;
        visit::visit_type_impl_trait(self, i);
    }

    fn visit_type_trait_object(&mut self, i: &'ast TypeTraitObject) {
        self.found = true;
        visit::visit_type_trait_object(self, i);
    }
}

fn type_has_callback_or_dynamic(ty: &Type) -> bool {
    let mut finder = CallbackOrDynamicFinder { found: false };
    finder.visit_type(ty);
    finder.found
}

fn is_shared_self_only_signature(sig: &Signature) -> bool {
    if !sig.generics.params.is_empty() || sig.generics.where_clause.is_some() {
        return false;
    }
    if sig.inputs.len() != 1 {
        return false;
    }
    matches_shared_receiver(sig.inputs.first())
}

fn matches_shared_receiver(arg: Option<&FnArg>) -> bool {
    let Some(FnArg::Receiver(receiver)) = arg else {
        return false;
    };
    receiver.reference.is_some() && receiver.mutability.is_none() && receiver.colon_token.is_none()
}

fn signature_has_mut_input(sig: &Signature) -> bool {
    sig.inputs.iter().any(|input| match input {
        FnArg::Receiver(receiver) => receiver.mutability.is_some(),
        FnArg::Typed(typed) => type_has_mut_ref(&typed.ty),
    })
}

fn signature_has_generics_or_callbacks(sig: &Signature) -> bool {
    if !sig.generics.params.is_empty() || sig.generics.where_clause.is_some() {
        return true;
    }
    sig.inputs.iter().any(|input| match input {
        FnArg::Receiver(_) => false,
        FnArg::Typed(typed) => type_has_callback_or_dynamic(&typed.ty),
    })
}

fn signature_returns_owned_confirmation(sig: &Signature) -> bool {
    match &sig.output {
        ReturnType::Default => false,
        ReturnType::Type(_, ret_ty) => {
            !type_is_shared_ref(ret_ty) && tokens_contain_any_confirmation(ret_ty.as_ref())
        }
    }
}

fn signature_exposes_owned_confirmation(sig: &Signature) -> bool {
    signature_has_mut_input(sig)
        || signature_has_generics_or_callbacks(sig)
        || signature_returns_owned_confirmation(sig)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ImplKind {
    Inherent,
    Trait,
}

fn non_wrapper_fn_violates_confirmation_boundary(
    vis: &Visibility,
    impl_kind: ImplKind,
    sig: &Signature,
) -> bool {
    if impl_kind == ImplKind::Inherent && private(vis) {
        return false;
    }
    if !tokens_contain_any_confirmation(sig) {
        return false;
    }
    if is_public(vis) || impl_kind == ImplKind::Trait {
        return true;
    }
    sig.inputs.len() != 1 || signature_exposes_owned_confirmation(sig)
}

fn input_contains_any_ident(sig: &Signature, banned: &[&str]) -> bool {
    sig.inputs.iter().any(|input| {
        let mut idents = Vec::new();
        token_idents(input.to_token_stream(), &mut idents);
        idents
            .iter()
            .any(|ident| ident == "Self" || banned.contains(&ident.as_str()))
    })
}

fn is_valid_wrapper_new(
    self_name: &str,
    method: &ImplItemFn,
    rules: &IdentityConfirmationRules,
) -> bool {
    let returns_self = match &method.sig.output {
        ReturnType::Type(_, ret_ty) => single_segment_type_name(ret_ty).as_deref() == Some("Self"),
        ReturnType::Default => false,
    };
    let forbidden_input = input_contains_any_ident(&method.sig, &OUTER_CONFIRMED_TYPES)
        || (INNER_CONFIRMATION_WRAPPERS.contains(&self_name)
            && input_contains_any_ident(&method.sig, &INNER_CONFIRMATION_WRAPPERS));
    returns_self
        && !forbidden_input
        && !signature_has_mut_input(&method.sig)
        && !signature_has_generics_or_callbacks(&method.sig)
        && !tokens_contain(method, &rules.issuer_function)
        && !tokens_contain(method, &rules.seal_type)
}

fn is_valid_wrapper_impl(
    self_name: &str,
    impl_item: &ItemImpl,
    rules: &IdentityConfirmationRules,
) -> bool {
    if impl_item.trait_.is_some()
        || !impl_item.generics.params.is_empty()
        || impl_item.generics.where_clause.is_some()
    {
        return false;
    }
    impl_item.items.iter().all(|member| {
        let ImplItem::Fn(method) = member else {
            return false;
        };
        if unraw(&method.sig.ident) == "new" {
            is_valid_wrapper_new(self_name, method, rules)
        } else {
            is_shared_self_only_signature(&method.sig)
                && !signature_exposes_owned_confirmation(&method.sig)
        }
    })
}

struct DefiningFileCheck<'a> {
    rules: &'a IdentityConfirmationRules,
    scope: Scope,
    target: DefiningTarget,
    impl_kind: ImplKind,
    in_canonical_issuer: bool,
    in_canonical_seal_field: bool,
    confirmation_struct_count: usize,
    seal_struct_count: usize,
    canonical_impl_count: usize,
    findings: &'a mut BTreeSet<Rule>,
}

impl DefiningFileCheck<'_> {
    fn is_canonical_issuer_method(&self, item: &ImplItem) -> bool {
        let ImplItem::Fn(method) = item else {
            return false;
        };
        if !is_public(&method.vis) {
            return false;
        }
        let expected = format!("const fn {} () -> Self", self.rules.issuer_function);
        method.sig.to_token_stream().to_string() == expected
    }

    fn classify_impl_target(&self, impl_item: &ItemImpl) -> (Option<String>, DefiningTarget) {
        let single = single_segment_type_name(&impl_item.self_ty);
        let name = type_last_name(&impl_item.self_ty);
        let target = if name.as_deref() == Some(self.rules.confirmation_type.as_str()) {
            DefiningTarget::Confirmation
        } else if name.as_deref() == Some(self.rules.seal_type.as_str()) {
            DefiningTarget::Seal
        } else if single
            .as_deref()
            .is_some_and(|item_name| CONFIRMED_WRAPPER_TYPES.contains(&item_name))
        {
            DefiningTarget::Wrapper
        } else if name.as_deref() == Some(ORIGIN_ENUM_TYPE) {
            DefiningTarget::Origin
        } else {
            DefiningTarget::Other
        };
        (single, target)
    }

    fn check_impl_constraints(&mut self, impl_item: &ItemImpl, single: Option<&str>) {
        if self.target == DefiningTarget::Seal
            || (self.scope == Scope::Product && self.target == DefiningTarget::Origin)
        {
            self.findings.insert(Rule::IdentityConfirmationIssuance);
        }
        if self.scope == Scope::Product
            && self.impl_kind == ImplKind::Trait
            && (self.target != DefiningTarget::Other || tokens_contain_any_confirmation(impl_item))
        {
            self.findings.insert(Rule::IdentityConfirmationIssuance);
        }
        if self.scope == Scope::Product
            && self.target != DefiningTarget::Confirmation
            && self.target != DefiningTarget::Wrapper
            && tokens_contain_any_confirmation(&impl_item.self_ty)
        {
            self.findings.insert(Rule::IdentityConfirmationIssuance);
        }
        if self.scope == Scope::Product
            && self.target == DefiningTarget::Wrapper
            && !is_valid_wrapper_impl(single.unwrap_or(""), impl_item, self.rules)
        {
            self.findings.insert(Rule::IdentityConfirmationIssuance);
        }
        if self.target == DefiningTarget::Confirmation {
            self.canonical_impl_count = self.canonical_impl_count.saturating_add(1);
            let single_canonical = self.scope == Scope::Product
                && impl_item.trait_.is_none()
                && single == Some(self.rules.confirmation_type.as_str())
                && self.canonical_impl_count == 1
                && impl_item.items.len() == 1
                && impl_item
                    .items
                    .first()
                    .is_some_and(|item| self.is_canonical_issuer_method(item));
            if !single_canonical {
                self.findings.insert(Rule::IdentityConfirmationIssuance);
            }
        }
    }
}

impl<'ast> Visit<'ast> for DefiningFileCheck<'_> {
    fn visit_attribute(&mut self, i: &'ast Attribute) {
        if self.scope == Scope::Product
            && (stream_literal_contains(i.to_token_stream(), &self.rules.issuer_function)
                || stream_literal_contains(i.to_token_stream(), &self.rules.seal_type))
        {
            self.findings.insert(Rule::IdentityConfirmationIssuance);
        }
        visit::visit_attribute(self, i);
    }

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
        let outer_seal_field = self.in_canonical_seal_field;
        if name == self.rules.confirmation_type {
            self.confirmation_struct_count = self.confirmation_struct_count.saturating_add(1);
            let valid = self.scope == Scope::Product
                && self.confirmation_struct_count == 1
                && is_public(&i.vis)
                && i.generics.params.is_empty()
                && i.generics.where_clause.is_none()
                && i.attrs.iter().all(is_allowed_derive)
                && has_single_private_seal_field(&i.fields, &self.rules.seal_type);
            if valid {
                self.in_canonical_seal_field = true;
            } else {
                self.findings.insert(Rule::IdentityConfirmationIssuance);
            }
        } else if name == self.rules.seal_type {
            self.seal_struct_count = self.seal_struct_count.saturating_add(1);
            let valid = self.scope == Scope::Product
                && self.seal_struct_count == 1
                && private(&i.vis)
                && i.generics.params.is_empty()
                && i.generics.where_clause.is_none()
                && i.attrs.iter().all(is_allowed_derive)
                && is_unit_struct_fields(&i.fields);
            if !valid {
                self.findings.insert(Rule::IdentityConfirmationIssuance);
            }
        } else if self.scope == Scope::Product && CONFIRMED_WRAPPER_TYPES.contains(&name.as_str()) {
            let valid = all_fields_private(&i.fields)
                && i.generics.params.is_empty()
                && i.generics.where_clause.is_none()
                && i.attrs.iter().all(is_allowed_derive);
            if !valid {
                self.findings.insert(Rule::IdentityConfirmationIssuance);
            }
        } else if self.scope == Scope::Product
            && !private(&i.vis)
            && tokens_contain_any_confirmation(&i.fields)
        {
            self.findings.insert(Rule::IdentityConfirmationIssuance);
        }
        visit::visit_item_struct(self, i);
        self.in_canonical_seal_field = outer_seal_field;
    }

    fn visit_item_enum(&mut self, i: &'ast ItemEnum) {
        if self.scope != Scope::Product {
            visit::visit_item_enum(self, i);
            return;
        }
        let name = unraw(&i.ident);
        let invalid_origin = name == ORIGIN_ENUM_TYPE
            && (!i.generics.params.is_empty()
                || i.generics.where_clause.is_some()
                || !i.attrs.iter().all(is_allowed_derive));
        let leaking_other = name != ORIGIN_ENUM_TYPE
            && !private(&i.vis)
            && i.variants
                .iter()
                .any(|variant| tokens_contain_any_confirmation(&variant.fields));
        if invalid_origin || leaking_other {
            self.findings.insert(Rule::IdentityConfirmationIssuance);
        }
        visit::visit_item_enum(self, i);
    }

    fn visit_item_trait(&mut self, i: &'ast ItemTrait) {
        if self.scope == Scope::Product
            && (tokens_contain(i, &self.rules.confirmation_type)
                || tokens_contain(i, &self.rules.seal_type)
                || tokens_contain_any_confirmation(i))
        {
            self.findings.insert(Rule::IdentityConfirmationIssuance);
        }
        visit::visit_item_trait(self, i);
    }

    fn visit_item_static(&mut self, i: &'ast ItemStatic) {
        if self.scope == Scope::Product
            && !private(&i.vis)
            && tokens_contain_any_confirmation(&i.ty)
        {
            self.findings.insert(Rule::IdentityConfirmationIssuance);
        }
        visit::visit_item_static(self, i);
    }

    fn visit_item_const(&mut self, i: &'ast ItemConst) {
        if self.scope == Scope::Product
            && !private(&i.vis)
            && tokens_contain_any_confirmation(&i.ty)
        {
            self.findings.insert(Rule::IdentityConfirmationIssuance);
        }
        visit::visit_item_const(self, i);
    }

    fn visit_item_fn(&mut self, i: &'ast ItemFn) {
        if self.scope == Scope::Product
            && non_wrapper_fn_violates_confirmation_boundary(&i.vis, ImplKind::Inherent, &i.sig)
        {
            self.findings.insert(Rule::IdentityConfirmationIssuance);
        }
        visit::visit_item_fn(self, i);
    }

    fn visit_type_path(&mut self, i: &'ast TypePath) {
        if self.scope == Scope::Product
            && !self.in_canonical_seal_field
            && segment_names(&i.path)
                .iter()
                .any(|name| name == &self.rules.seal_type)
        {
            self.findings.insert(Rule::IdentityConfirmationIssuance);
        }
        visit::visit_type_path(self, i);
    }

    fn visit_impl_item_type(&mut self, i: &'ast ImplItemType) {
        if self.scope == Scope::Product
            && (tokens_contain(i, &self.rules.confirmation_type)
                || tokens_contain(i, &self.rules.seal_type)
                || tokens_contain_any_confirmation(i))
        {
            self.findings.insert(Rule::IdentityConfirmationIssuance);
        }
        visit::visit_impl_item_type(self, i);
    }

    fn visit_trait_item_type(&mut self, i: &'ast TraitItemType) {
        if self.scope == Scope::Product
            && (tokens_contain(i, &self.rules.confirmation_type)
                || tokens_contain(i, &self.rules.seal_type)
                || tokens_contain_any_confirmation(i))
        {
            self.findings.insert(Rule::IdentityConfirmationIssuance);
        }
        visit::visit_trait_item_type(self, i);
    }

    fn visit_item_impl(&mut self, i: &'ast ItemImpl) {
        let outer = self.target;
        let outer_kind = self.impl_kind;
        self.impl_kind = if i.trait_.is_some() {
            ImplKind::Trait
        } else {
            ImplKind::Inherent
        };
        let (single, target) = self.classify_impl_target(i);
        self.target = target;
        self.check_impl_constraints(i, single.as_deref());
        visit::visit_item_impl(self, i);
        self.target = outer;
        self.impl_kind = outer_kind;
    }

    fn visit_impl_item_fn(&mut self, i: &'ast ImplItemFn) {
        if self.scope == Scope::Product
            && self.target == DefiningTarget::Other
            && non_wrapper_fn_violates_confirmation_boundary(&i.vis, self.impl_kind, &i.sig)
        {
            self.findings.insert(Rule::IdentityConfirmationIssuance);
        }
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
                    || (name == "Self" && matches_defining_seal_target(self.target))
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
                    || (name == "Self" && matches_defining_seal_target(self.target))
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
                || tokens_contain(&i.ty, &self.rules.seal_type)
                || tokens_contain_any_confirmation(&i.ty))
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

pub(crate) fn is_direct_canonical_issuer_path(
    rules: &IdentityConfirmationRules,
    expr: &ExprPath,
) -> bool {
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

fn is_confirmed_decision_expr(expr: &Expr) -> bool {
    match expr {
        Expr::Path(path) if path.qself.is_none() => {
            let names = segment_names(&path.path);
            names.last().map(String::as_str) == Some(CONFIRMED_VARIANT)
        }
        Expr::Group(group) => is_confirmed_decision_expr(&group.expr),
        Expr::Paren(paren) => is_confirmed_decision_expr(&paren.expr),
        Expr::Block(block) => {
            if let [Stmt::Expr(inner, None)] = block.block.stmts.as_slice() {
                is_confirmed_decision_expr(inner)
            } else {
                false
            }
        }
        Expr::Array(_)
        | Expr::Assign(_)
        | Expr::Async(_)
        | Expr::Await(_)
        | Expr::Binary(_)
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

    fn visit_attribute(&mut self, i: &'ast Attribute) {
        if stream_literal_contains(i.to_token_stream(), &self.rules.issuer_function)
            || stream_literal_contains(i.to_token_stream(), &self.rules.seal_type)
        {
            self.findings.insert(Rule::IdentityConfirmationIssuance);
        }
        visit::visit_attribute(self, i);
    }

    fn visit_macro(&mut self, i: &'ast Macro) {
        if stream_contains_ident(i.tokens.clone(), &self.rules.issuer_function)
            || stream_contains_ident(i.tokens.clone(), &self.rules.seal_type)
            || stream_literal_contains(i.tokens.clone(), &self.rules.issuer_function)
            || stream_literal_contains(i.tokens.clone(), &self.rules.seal_type)
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

fn is_paste_bracket_group(group: &proc_macro2::Group) -> bool {
    if group.delimiter() != Delimiter::Bracket {
        return false;
    }
    let items: Vec<TokenTree> = group.stream().into_iter().collect();
    matches_punct(items.first(), '<') && matches_punct(items.last(), '>')
}

fn stream_has_paste_brackets(tokens: TokenStream) -> bool {
    for tree in tokens {
        if let TokenTree::Group(group) = tree
            && (is_paste_bracket_group(&group) || stream_has_paste_brackets(group.stream()))
        {
            return true;
        }
    }
    false
}

fn matches_punct(tree: Option<&TokenTree>, expected: char) -> bool {
    let Some(TokenTree::Punct(punct)) = tree else {
        return false;
    };
    punct.as_char() == expected
}

fn stream_has_paste_macro_call(tokens: TokenStream) -> bool {
    if stream_has_paste_brackets(tokens.clone()) {
        return true;
    }
    let flat: Vec<TokenTree> = tokens.into_iter().collect();
    for (index, tree) in flat.iter().enumerate() {
        match tree {
            TokenTree::Group(group) => {
                if stream_has_paste_macro_call(group.stream()) {
                    return true;
                }
            }
            TokenTree::Ident(ident) => {
                let name = unraw(ident);
                if name == "_tauri_specta_paste"
                    || name == "_fn_datatype"
                    || name == "_fn_datatype_internal"
                    || name == "_collect_functions"
                {
                    return true;
                }
                if (name == "paste" || name == "fn_datatype")
                    && matches_punct(flat.get(index.saturating_add(1)), '!')
                {
                    return true;
                }
            }
            TokenTree::Literal(_) | TokenTree::Punct(_) => {}
        }
    }
    false
}

struct PasteAliasCollector {
    crate_aliases: BTreeSet<String>,
}

impl<'ast> Visit<'ast> for PasteAliasCollector {
    fn visit_item_extern_crate(&mut self, i: &'ast ItemExternCrate) {
        if let Some((_, alias)) = &i.rename
            && self.crate_aliases.contains(&unraw(&i.ident))
        {
            self.crate_aliases.insert(unraw(alias));
        }
    }

    fn visit_item_use(&mut self, i: &'ast ItemUse) {
        for entry in use_entries(&i.tree) {
            if let UseKind::Rename(alias) = &entry.kind
                && entry.path.len() == 1
                && self.crate_aliases.contains(entry.imported())
            {
                self.crate_aliases.insert(alias.clone());
            }
        }
    }
}

fn collect_paste_crate_aliases(
    source_aliases: &BTreeMap<String, Vec<String>>,
    file: &syn::File,
) -> BTreeSet<String> {
    let mut crate_aliases: BTreeSet<String> = PASTE_REEXPORT_CRATES
        .into_iter()
        .map(String::from)
        .collect();
    for (alias, targets) in source_aliases {
        if targets
            .iter()
            .any(|target| PASTE_REEXPORT_CRATES.contains(&target.as_str()))
        {
            crate_aliases.insert(alias.clone());
        }
    }
    loop {
        let before = crate_aliases.len();
        let mut collector = PasteAliasCollector { crate_aliases };
        collector.visit_file(file);
        crate_aliases = collector.crate_aliases;
        if crate_aliases.len() == before {
            break;
        }
    }
    crate_aliases
}

struct ReexportedPasteCheck<'a> {
    crate_aliases: BTreeSet<String>,
    findings: &'a mut BTreeSet<Rule>,
}

impl ReexportedPasteCheck<'_> {
    fn touches_paste_reexport_module(&self, segments: &[String]) -> bool {
        segments
            .iter()
            .zip(segments.iter().skip(1))
            .any(|(root, child)| {
                self.crate_aliases.contains(root)
                    && PASTE_REEXPORT_MODULES.contains(&child.as_str())
            })
    }
}

impl<'ast> Visit<'ast> for ReexportedPasteCheck<'_> {
    fn visit_item_use(&mut self, i: &'ast ItemUse) {
        for entry in use_entries(&i.tree) {
            if BANNED_PASTE_MACROS.contains(&entry.imported())
                || self.touches_paste_reexport_module(&entry.path)
            {
                self.findings.insert(Rule::IdentityConfirmationIssuance);
            }
        }
        visit::visit_item_use(self, i);
    }

    fn visit_macro(&mut self, i: &'ast Macro) {
        let names = segment_names(&i.path);
        let banned_callee = names
            .last()
            .is_some_and(|name| BANNED_PASTE_MACROS.contains(&name.as_str()));
        if banned_callee
            || self.touches_paste_reexport_module(&names)
            || stream_has_paste_macro_call(i.tokens.clone())
        {
            self.findings.insert(Rule::IdentityConfirmationIssuance);
        }
        visit::visit_macro(self, i);
    }

    fn visit_expr_path(&mut self, i: &'ast ExprPath) {
        if self.touches_paste_reexport_module(&segment_names(&i.path)) {
            self.findings.insert(Rule::IdentityConfirmationIssuance);
        }
        visit::visit_expr_path(self, i);
    }
}

fn contains_paste_bracket_text(text: &str) -> bool {
    let mut after_open = false;
    for ch in text.chars() {
        if after_open {
            if ch == '<' {
                return true;
            }
            if !ch.is_whitespace() {
                after_open = ch == '[';
            }
        } else if ch == '[' {
            after_open = true;
        }
    }
    false
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
        || ctx.text.contains("paste")
        || ctx.text.contains("internal")
        || ctx.text.contains("fn_datatype")
        || ctx.text.contains("_collect_functions")
        || contains_paste_bracket_text(ctx.text)
        || ctx.text.contains("specta")
        || ctx.text.contains('\\')
    {
        return true;
    }
    let defining_crate = rules.prompt_defining_crate.replace('-', "_");
    ctx.source_aliases.iter().any(|(alias, targets)| {
        targets.iter().any(|target| {
            target == &defining_crate || PASTE_REEXPORT_CRATES.contains(&target.as_str())
        }) && ctx.text.contains(alias)
    })
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
        in_xtask: ctx.crate_name.as_str() == "xtask",
        findings,
    };
    scan.visit_file(ctx.file);
}

fn check_defining_file(
    rules: &IdentityConfirmationRules,
    ctx: &CheckContext<'_>,
    findings: &mut BTreeSet<Rule>,
) {
    let mut scan = DefiningFileCheck {
        rules,
        scope: Scope::Product,
        target: DefiningTarget::Other,
        impl_kind: ImplKind::Inherent,
        in_canonical_issuer: false,
        in_canonical_seal_field: false,
        confirmation_struct_count: 0,
        seal_struct_count: 0,
        canonical_impl_count: 0,
        findings,
    };
    scan.visit_file(ctx.file);
    if scan.confirmation_struct_count != 1
        || scan.seal_struct_count != 1
        || scan.canonical_impl_count != 1
    {
        findings.insert(Rule::IdentityConfirmationIssuance);
    }
}

pub(super) fn check(rules: &IdentityConfirmationRules, ctx: &CheckContext<'_>) -> BTreeSet<Rule> {
    let mut findings = BTreeSet::new();

    let unauthorized_child = is_unauthorized_child_file(rules, ctx.relative);
    if unauthorized_child {
        findings.insert(Rule::IdentityConfirmationIssuance);
    }

    let prompt_file = prompt_def::prompt_defining_file(rules);
    let is_special = unauthorized_child
        || ctx.relative == rules.defining_file
        || ctx.relative == rules.defining_test_file
        || ctx.relative == rules.use_case_file
        || ctx.relative == prompt_file
        || rules.issuance_allowed_files.contains(ctx.relative);
    if !is_special && !may_touch_identity_confirmation(rules, ctx) {
        return findings;
    }

    let mut paste_check = ReexportedPasteCheck {
        crate_aliases: collect_paste_crate_aliases(ctx.source_aliases, ctx.file),
        findings: &mut findings,
    };
    paste_check.visit_file(ctx.file);

    let aliases = PromptAliases::collect(rules, ctx.source_aliases, ctx.file);
    let mut prompt_check = PromptCheck {
        rules,
        crate_name: ctx.crate_name,
        relative: ctx.relative,
        aliases,
        findings: &mut findings,
    };
    prompt_check.visit_file(ctx.file);

    if ctx.relative == prompt_file {
        prompt_def::check_prompt_defining_file(rules, ctx, &mut findings);
    }

    if ctx.relative == rules.defining_file {
        check_defining_file(rules, ctx, &mut findings);
    } else if ctx.relative == rules.defining_test_file {
        check_defining_test_file(rules, ctx.file, ctx.tokens, &mut findings);
    } else if ctx.relative == rules.use_case_file {
        use_case::check_use_case_file(rules, ctx, &mut findings);
    } else if rules.issuance_allowed_files.contains(ctx.relative) {
        check_protected_fake_file(rules, ctx, &mut findings);
    } else {
        check_unauthorized_file(rules, ctx, &mut findings);
    }

    findings
}
