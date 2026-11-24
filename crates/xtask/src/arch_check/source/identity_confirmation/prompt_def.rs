use std::collections::BTreeSet;

use syn::visit::{self, Visit};
use syn::{
    Attribute, Fields, FnArg, ImplItem, Item, ItemEnum, ItemImpl, ItemMod, ItemStruct, ItemTrait,
    ItemUse, Macro, PathArguments, ReturnType, TraitItem, Type, TypeParamBound,
};

use super::super::super::Rule;
use super::super::super::rules::IdentityConfirmationRules;
use super::super::ast::uses::{UseKind, unraw, use_entries};
use super::{
    CONFIRMED_VARIANT, CheckContext, DECISION_ENUM, has_non_private_item, is_cfg_test_mod,
    is_public, is_unit_struct_fields, private, single_segment_type_name, stream_literal_contains,
};
use quote::ToTokens;

const PROMPT_STRUCTS: [&str; 3] = [
    "IdentifierChangePrompt",
    "RetirementCorrectionPrompt",
    "LinkCorrectionPrompt",
];

const PROMPT_METHODS: [(&str, &str); 3] = [
    ("confirm_identifier_change", "IdentifierChangePrompt"),
    (
        "confirm_retirement_correction",
        "RetirementCorrectionPrompt",
    ),
    ("confirm_link_correction", "LinkCorrectionPrompt"),
];

const DECISION_ALLOWED_DERIVES: [&str; 5] = ["Debug", "Clone", "Copy", "PartialEq", "Eq"];

fn is_allowed_decision_derive(attr: &Attribute) -> bool {
    if !attr.path().is_ident("derive") {
        return false;
    }
    attr.parse_args_with(syn::punctuated::Punctuated::<syn::Path, syn::Token![,]>::parse_terminated)
        .is_ok_and(|paths| {
            paths.iter().all(|path| {
                path.segments.last().is_some_and(|seg| {
                    DECISION_ALLOWED_DERIVES.contains(&unraw(&seg.ident).as_str())
                })
            })
        })
}

fn is_canonical_decision_enum(item: &ItemEnum) -> bool {
    if unraw(&item.ident) != DECISION_ENUM
        || !is_public(&item.vis)
        || !item.generics.params.is_empty()
        || item.generics.where_clause.is_some()
        || !item.attrs.iter().all(is_allowed_decision_derive)
        || item.variants.len() != 3
    {
        return false;
    }
    let mut names = BTreeSet::new();
    for variant in &item.variants {
        if variant.discriminant.is_some() || !is_unit_struct_fields(&variant.fields) {
            return false;
        }
        names.insert(unraw(&variant.ident));
    }
    names.contains(CONFIRMED_VARIANT) && names.contains("Denied") && names.contains("Unavailable")
}

fn is_ref_to_type(ty: &Type, expected: &str) -> bool {
    let Type::Reference(reference) = ty else {
        return false;
    };
    reference.mutability.is_none()
        && single_segment_type_name(&reference.elem).as_deref() == Some(expected)
}

fn is_future_decision_output(output: &ReturnType) -> bool {
    let ReturnType::Type(_, ret_ty) = output else {
        return false;
    };
    let Type::ImplTrait(impl_trait) = ret_ty.as_ref() else {
        return false;
    };
    let mut has_future = false;
    let mut has_send = false;
    for bound in &impl_trait.bounds {
        let TypeParamBound::Trait(trait_bound) = bound else {
            continue;
        };
        let Some(last) = trait_bound.path.segments.last() else {
            continue;
        };
        let name = unraw(&last.ident);
        if name == "Send" {
            has_send = true;
        } else if name == "Future" {
            let PathArguments::AngleBracketed(args) = &last.arguments else {
                return false;
            };
            let matches_output = args.args.iter().any(|arg| match arg {
                syn::GenericArgument::AssocType(assoc) => {
                    unraw(&assoc.ident) == "Output"
                        && single_segment_type_name(&assoc.ty).as_deref() == Some(DECISION_ENUM)
                }
                syn::GenericArgument::Lifetime(_)
                | syn::GenericArgument::Type(_)
                | syn::GenericArgument::Const(_)
                | syn::GenericArgument::AssocConst(_)
                | syn::GenericArgument::Constraint(_)
                | _ => false,
            });
            has_future = matches_output;
        }
    }
    has_future && has_send
}

fn canonical_prompt_method_name(member: &TraitItem) -> Option<String> {
    let TraitItem::Fn(method) = member else {
        return None;
    };
    if !method.attrs.is_empty()
        || method.default.is_some()
        || method.sig.asyncness.is_some()
        || !method.sig.generics.params.is_empty()
        || method.sig.generics.where_clause.is_some()
        || method.sig.inputs.len() != 2
    {
        return None;
    }
    let name = unraw(&method.sig.ident);
    let (_, expected_req) = PROMPT_METHODS
        .iter()
        .find(|(method_name, _)| *method_name == name)?;
    let FnArg::Receiver(receiver) = method.sig.inputs.first()? else {
        return None;
    };
    if receiver.reference.is_none()
        || receiver.mutability.is_some()
        || receiver.colon_token.is_some()
    {
        return None;
    }
    let FnArg::Typed(req_arg) = method.sig.inputs.get(1)? else {
        return None;
    };
    (is_ref_to_type(&req_arg.ty, expected_req) && is_future_decision_output(&method.sig.output))
        .then_some(name)
}

fn is_canonical_prompt_trait(item: &ItemTrait, trait_name: &str) -> bool {
    if unraw(&item.ident) != trait_name
        || !is_public(&item.vis)
        || !item.attrs.is_empty()
        || item.unsafety.is_some()
        || item.auto_token.is_some()
        || !item.generics.params.is_empty()
        || item.generics.where_clause.is_some()
        || item.items.len() != PROMPT_METHODS.len()
    {
        return false;
    }
    let supertraits: BTreeSet<String> = item
        .supertraits
        .iter()
        .filter_map(|bound| match bound {
            TypeParamBound::Trait(tb) => tb.path.segments.last().map(|seg| unraw(&seg.ident)),
            TypeParamBound::Lifetime(_)
            | TypeParamBound::PreciseCapture(_)
            | TypeParamBound::Verbatim(_)
            | _ => None,
        })
        .collect();
    if !supertraits.contains("Send") || !supertraits.contains("Sync") {
        return false;
    }
    let mut seen = BTreeSet::new();
    for member in &item.items {
        let Some(name) = canonical_prompt_method_name(member) else {
            return false;
        };
        seen.insert(name);
    }
    seen.len() == PROMPT_METHODS.len()
}

fn is_prompt_request_struct(structure: &ItemStruct) -> bool {
    let name = unraw(&structure.ident);
    if !PROMPT_STRUCTS.contains(&name.as_str())
        || !is_public(&structure.vis)
        || !structure.attrs.iter().all(is_allowed_decision_derive)
        || !structure.generics.params.is_empty()
        || structure.generics.where_clause.is_some()
    {
        return false;
    }
    let Fields::Named(named) = &structure.fields else {
        return false;
    };
    named
        .named
        .iter()
        .all(|field| private(&field.vis) && field.attrs.is_empty())
}

struct PromptDefNestedBan<'a> {
    findings: &'a mut BTreeSet<Rule>,
}

impl<'ast> Visit<'ast> for PromptDefNestedBan<'_> {
    fn visit_item(&mut self, i: &'ast Item) {
        self.findings.insert(Rule::IdentityConfirmationPromptImpl);
        visit::visit_item(self, i);
    }

    fn visit_macro(&mut self, i: &'ast Macro) {
        self.findings.insert(Rule::IdentityConfirmationPromptImpl);
        visit::visit_macro(self, i);
    }
}

struct PromptDefiningCheck<'a> {
    rules: &'a IdentityConfirmationRules,
    trait_count: usize,
    decision_count: usize,
    findings: &'a mut BTreeSet<Rule>,
}

impl PromptDefiningCheck<'_> {
    fn check_item(&mut self, item: &Item) {
        match item {
            Item::Use(use_item) => self.check_use(use_item),
            Item::Enum(enumeration) => self.check_enum(enumeration),
            Item::Struct(structure) => {
                if !is_prompt_request_struct(structure) {
                    self.findings.insert(Rule::IdentityConfirmationPromptImpl);
                }
            }
            Item::Trait(trait_item) => self.check_trait(trait_item),
            Item::Impl(impl_item) => self.check_impl(impl_item),
            Item::Mod(module) => self.check_mod(module),
            Item::Const(_)
            | Item::ExternCrate(_)
            | Item::Fn(_)
            | Item::ForeignMod(_)
            | Item::Macro(_)
            | Item::Static(_)
            | Item::TraitAlias(_)
            | Item::Type(_)
            | Item::Union(_)
            | Item::Verbatim(_)
            | _ => {
                self.findings.insert(Rule::IdentityConfirmationPromptImpl);
            }
        }
    }

    fn check_use_rename(&mut self, imported: &str, alias: &str) {
        if alias == self.rules.prompt_trait || imported == self.rules.prompt_trait {
            self.findings.insert(Rule::IdentityConfirmationPromptAlias);
        } else {
            self.findings.insert(Rule::IdentityConfirmationPromptImpl);
        }
    }

    fn check_use(&mut self, use_item: &ItemUse) {
        if !private(&use_item.vis) {
            self.findings
                .insert(Rule::IdentityConfirmationPromptReexport);
        }
        for entry in use_entries(&use_item.tree) {
            let imported = entry.imported();
            let shadows_local = imported == self.rules.prompt_trait
                || imported == DECISION_ENUM
                || PROMPT_STRUCTS.contains(&imported);
            match &entry.kind {
                UseKind::Name if !shadows_local => {}
                UseKind::Name | UseKind::Glob => {
                    self.findings.insert(Rule::IdentityConfirmationPromptImpl);
                }
                UseKind::Rename(alias) => self.check_use_rename(imported, alias),
            }
        }
    }

    fn check_enum(&mut self, enumeration: &ItemEnum) {
        self.decision_count = self.decision_count.saturating_add(1);
        if self.decision_count > 1 || !is_canonical_decision_enum(enumeration) {
            self.findings.insert(Rule::IdentityConfirmationPromptImpl);
        }
    }

    fn check_trait(&mut self, trait_item: &ItemTrait) {
        self.trait_count = self.trait_count.saturating_add(1);
        if self.trait_count > 1 || !is_canonical_prompt_trait(trait_item, &self.rules.prompt_trait)
        {
            self.findings.insert(Rule::IdentityConfirmationPromptImpl);
        }
    }

    fn check_impl(&mut self, impl_item: &ItemImpl) {
        let valid_target = impl_item.trait_.is_none()
            && single_segment_type_name(&impl_item.self_ty)
                .is_some_and(|name| PROMPT_STRUCTS.contains(&name.as_str()));
        if !valid_target {
            self.findings.insert(Rule::IdentityConfirmationPromptImpl);
            return;
        }
        for member in &impl_item.items {
            let ImplItem::Fn(method) = member else {
                self.findings.insert(Rule::IdentityConfirmationPromptImpl);
                continue;
            };
            if method.attrs.iter().any(|attr| {
                stream_literal_contains(attr.to_token_stream(), &self.rules.issuer_function)
                    || stream_literal_contains(attr.to_token_stream(), &self.rules.seal_type)
            }) {
                self.findings.insert(Rule::IdentityConfirmationIssuance);
            }
        }
        let mut nested = PromptDefNestedBan {
            findings: self.findings,
        };
        nested.visit_item_impl(impl_item);
    }

    fn check_mod(&mut self, module: &ItemMod) {
        if is_cfg_test_mod(module) {
            if let Some((_, nested)) = &module.content
                && has_non_private_item(nested)
            {
                self.findings.insert(Rule::IdentityConfirmationPromptImpl);
            }
            return;
        }
        self.findings.insert(Rule::IdentityConfirmationPromptImpl);
    }
}

pub(super) fn prompt_defining_file(rules: &IdentityConfirmationRules) -> String {
    format!(
        "crates/{}/src/{}.rs",
        rules.prompt_defining_crate, rules.prompt_module
    )
}

pub(super) fn check_prompt_defining_file(
    rules: &IdentityConfirmationRules,
    ctx: &CheckContext<'_>,
    findings: &mut BTreeSet<Rule>,
) {
    let mut scan = PromptDefiningCheck {
        rules,
        trait_count: 0,
        decision_count: 0,
        findings,
    };
    for item in &ctx.file.items {
        scan.check_item(item);
    }
    if scan.trait_count != 1 || scan.decision_count != 1 {
        findings.insert(Rule::IdentityConfirmationPromptImpl);
    }
}
