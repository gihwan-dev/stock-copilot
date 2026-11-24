use std::collections::{BTreeMap, BTreeSet};

use syn::visit::{self, Visit};
use syn::{
    Attribute, Expr, Fields, FnArg, ImplItem, ImplItemFn, Item, ItemEnum, ItemImpl, ItemMod,
    ItemStruct, ItemTrait, ItemUse, Macro, Member, Pat, PathArguments, ReturnType, Stmt, TraitItem,
    Type, TypeParamBound, Visibility,
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

const IDENTIFIER_CHANGE_FIELDS: [(&str, &str); 4] = [
    ("instrument_id", "InstrumentId"),
    ("previous", "InstrumentSource"),
    ("next", "InstrumentSource"),
    ("evidence", "IdentityChangeEvidence"),
];

const RETIREMENT_CORRECTION_FIELDS: [(&str, &str); 3] = [
    ("instrument_id", "InstrumentId"),
    ("source", "InstrumentSource"),
    ("evidence", "MappingCorrectionEvidence"),
];

const LINK_CORRECTION_FIELDS: [(&str, &str); 4] = [
    ("instrument_id", "InstrumentId"),
    ("previous", "InstrumentSource"),
    ("mistaken_next", "InstrumentSource"),
    ("evidence", "MappingCorrectionEvidence"),
];

const PROMPT_METHODS: [(&str, &str); 3] = [
    ("confirm_identifier_change", "IdentifierChangePrompt"),
    (
        "confirm_retirement_correction",
        "RetirementCorrectionPrompt",
    ),
    ("confirm_link_correction", "LinkCorrectionPrompt"),
];

const PROMPT_DOMAIN_IMPORTS: [&str; 4] = [
    "IdentityChangeEvidence",
    "InstrumentId",
    "InstrumentSource",
    "MappingCorrectionEvidence",
];

const DECISION_ALLOWED_DERIVES: [&str; 5] = ["Debug", "Clone", "Copy", "PartialEq", "Eq"];

fn canonical_fields_for(struct_name: &str) -> Option<&'static [(&'static str, &'static str)]> {
    match struct_name {
        "IdentifierChangePrompt" => Some(&IDENTIFIER_CHANGE_FIELDS),
        "RetirementCorrectionPrompt" => Some(&RETIREMENT_CORRECTION_FIELDS),
        "LinkCorrectionPrompt" => Some(&LINK_CORRECTION_FIELDS),
        _ => None,
    }
}

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

fn extract_prompt_struct_fields(structure: &ItemStruct) -> Option<Vec<(String, Option<String>)>> {
    let name = unraw(&structure.ident);
    if !PROMPT_STRUCTS.contains(&name.as_str())
        || !is_public(&structure.vis)
        || !structure.attrs.iter().all(is_allowed_decision_derive)
        || !structure.generics.params.is_empty()
        || structure.generics.where_clause.is_some()
    {
        return None;
    }
    let Fields::Named(named) = &structure.fields else {
        return None;
    };
    let mut fields = Vec::new();
    for field in &named.named {
        if !private(&field.vis) || !field.attrs.is_empty() {
            return None;
        }
        let field_name = unraw(field.ident.as_ref()?);
        fields.push((field_name, single_segment_type_name(&field.ty)));
    }
    Some(fields)
}

fn fields_match_canonical(
    declared: &[(String, Option<String>)],
    canonical: &[(&str, &str)],
) -> bool {
    declared.len() == canonical.len()
        && declared
            .iter()
            .zip(canonical.iter())
            .all(|((name, ty), (exp_name, exp_ty))| {
                name == exp_name && ty.as_deref() == Some(*exp_ty)
            })
}

fn single_tail_expr(block: &syn::Block) -> Option<&Expr> {
    if block.stmts.len() != 1 {
        return None;
    }
    let Stmt::Expr(expr, None) = block.stmts.first()? else {
        return None;
    };
    Some(expr)
}

fn unwrap_paren_or_group_expr(mut expr: &Expr) -> &Expr {
    loop {
        if let Expr::Group(group) = expr {
            expr = &group.expr;
        } else if let Expr::Paren(paren) = expr {
            expr = &paren.expr;
        } else {
            return expr;
        }
    }
}

fn is_self_field_expr(expr: &Expr, expected_field: &str) -> bool {
    let Expr::Field(field_expr) = unwrap_paren_or_group_expr(expr) else {
        return false;
    };
    let Expr::Path(base) = unwrap_paren_or_group_expr(&field_expr.base) else {
        return false;
    };
    let Member::Named(member) = &field_expr.member else {
        return false;
    };
    base.qself.is_none()
        && base.path.leading_colon.is_none()
        && base.path.segments.len() == 1
        && base
            .path
            .segments
            .first()
            .is_some_and(|seg| unraw(&seg.ident) == "self")
        && unraw(member) == expected_field
}

fn is_single_ident_expr(expr: &Expr, expected_ident: &str) -> bool {
    let Expr::Path(path_expr) = unwrap_paren_or_group_expr(expr) else {
        return false;
    };
    path_expr.qself.is_none()
        && path_expr.path.leading_colon.is_none()
        && path_expr.path.segments.len() == 1
        && path_expr
            .path
            .segments
            .first()
            .is_some_and(|seg| unraw(&seg.ident) == expected_ident)
}

fn is_pub_crate(vis: &Visibility) -> bool {
    match vis {
        Visibility::Restricted(restricted) => {
            restricted.path.leading_colon.is_none()
                && restricted.path.segments.len() == 1
                && restricted
                    .path
                    .segments
                    .first()
                    .is_some_and(|seg| unraw(&seg.ident) == "crate")
        }
        Visibility::Public(_) | Visibility::Inherited => false,
    }
}

fn is_canonical_prompt_new_sig(method: &ImplItemFn, canonical: &[(&str, &str)]) -> bool {
    if !is_public(&method.vis)
        || method.sig.asyncness.is_some()
        || !method.sig.generics.params.is_empty()
        || method.sig.generics.where_clause.is_some()
        || method.sig.inputs.len() != canonical.len()
    {
        return false;
    }
    let ReturnType::Type(_, ret_ty) = &method.sig.output else {
        return false;
    };
    if single_segment_type_name(ret_ty).as_deref() != Some("Self") {
        return false;
    }
    method
        .sig
        .inputs
        .iter()
        .zip(canonical.iter())
        .all(|(input, (exp_name, exp_ty))| {
            let FnArg::Typed(typed) = input else {
                return false;
            };
            let Pat::Ident(pat_ident) = typed.pat.as_ref() else {
                return false;
            };
            pat_ident.by_ref.is_none()
                && pat_ident.mutability.is_none()
                && pat_ident.subpat.is_none()
                && unraw(&pat_ident.ident) == *exp_name
                && single_segment_type_name(&typed.ty).as_deref() == Some(*exp_ty)
        })
}

fn is_canonical_prompt_new_body(block: &syn::Block, canonical: &[(&str, &str)]) -> bool {
    let Some(Expr::Struct(struct_expr)) = single_tail_expr(block).map(unwrap_paren_or_group_expr)
    else {
        return false;
    };
    if struct_expr.qself.is_some()
        || struct_expr.rest.is_some()
        || struct_expr.path.leading_colon.is_some()
        || struct_expr.path.segments.len() != 1
        || struct_expr
            .path
            .segments
            .first()
            .is_none_or(|seg| unraw(&seg.ident) != "Self")
        || struct_expr.fields.len() != canonical.len()
    {
        return false;
    }
    let mut seen = BTreeSet::new();
    for field_val in &struct_expr.fields {
        let Member::Named(member) = &field_val.member else {
            return false;
        };
        let member_name = unraw(member);
        if !canonical.iter().any(|(exp, _)| *exp == member_name)
            || !is_single_ident_expr(&field_val.expr, &member_name)
            || !seen.insert(member_name)
        {
            return false;
        }
    }
    seen.len() == canonical.len()
}

fn is_canonical_prompt_new(method: &ImplItemFn, canonical: &[(&str, &str)]) -> bool {
    is_canonical_prompt_new_sig(method, canonical)
        && is_canonical_prompt_new_body(&method.block, canonical)
}

fn is_canonical_prompt_accessor(method: &ImplItemFn, field_name: &str, field_type: &str) -> bool {
    if !is_public(&method.vis)
        || method.sig.asyncness.is_some()
        || !method.sig.generics.params.is_empty()
        || method.sig.generics.where_clause.is_some()
        || method.sig.inputs.len() != 1
    {
        return false;
    }
    let Some(FnArg::Receiver(receiver)) = method.sig.inputs.first() else {
        return false;
    };
    if receiver.reference.is_none()
        || receiver.mutability.is_some()
        || receiver.colon_token.is_some()
    {
        return false;
    }
    let ReturnType::Type(_, ret_ty) = &method.sig.output else {
        return false;
    };
    if !is_ref_to_type(ret_ty, field_type) {
        return false;
    }
    let Some(Expr::Reference(ref_expr)) =
        single_tail_expr(&method.block).map(unwrap_paren_or_group_expr)
    else {
        return false;
    };
    ref_expr.mutability.is_none() && is_self_field_expr(&ref_expr.expr, field_name)
}

fn is_canonical_prompt_into_parts(method: &ImplItemFn, canonical: &[(&str, &str)]) -> bool {
    if !is_pub_crate(&method.vis)
        || method.sig.asyncness.is_some()
        || !method.sig.generics.params.is_empty()
        || method.sig.generics.where_clause.is_some()
        || method.sig.inputs.len() != 1
    {
        return false;
    }
    let Some(FnArg::Receiver(receiver)) = method.sig.inputs.first() else {
        return false;
    };
    if receiver.reference.is_some()
        || receiver.mutability.is_some()
        || receiver.colon_token.is_some()
    {
        return false;
    }
    let ReturnType::Type(_, ret_ty) = &method.sig.output else {
        return false;
    };
    let Type::Tuple(ret_tuple) = ret_ty.as_ref() else {
        return false;
    };
    if ret_tuple.elems.len() != canonical.len()
        || !ret_tuple
            .elems
            .iter()
            .zip(canonical.iter())
            .all(|(elem, (_, exp_ty))| single_segment_type_name(elem).as_deref() == Some(*exp_ty))
    {
        return false;
    }
    let Some(Expr::Tuple(body_tuple)) =
        single_tail_expr(&method.block).map(unwrap_paren_or_group_expr)
    else {
        return false;
    };
    body_tuple.elems.len() == canonical.len()
        && body_tuple
            .elems
            .iter()
            .zip(canonical.iter())
            .all(|(elem, (exp_field, _))| is_self_field_expr(elem, exp_field))
}

fn is_canonical_prompt_method(method: &ImplItemFn, canonical: &[(&str, &str)]) -> bool {
    let name = unraw(&method.sig.ident);
    if name == "new" {
        return is_canonical_prompt_new(method, canonical);
    }
    if name == "into_parts" {
        return is_canonical_prompt_into_parts(method, canonical);
    }
    if let Some((_, field_type)) = canonical.iter().find(|(field, _)| *field == name) {
        return is_canonical_prompt_accessor(method, &name, field_type);
    }
    false
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
    struct_fields: BTreeMap<String, Vec<(String, Option<String>)>>,
    impl_methods: BTreeMap<String, BTreeSet<String>>,
    findings: &'a mut BTreeSet<Rule>,
}

impl PromptDefiningCheck<'_> {
    fn check_item(&mut self, item: &Item) {
        match item {
            Item::Use(use_item) => self.check_use(use_item),
            Item::Enum(enumeration) => self.check_enum(enumeration),
            Item::Struct(structure) => self.check_struct(structure),
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

    fn check_struct(&mut self, structure: &ItemStruct) {
        let Some(fields) = extract_prompt_struct_fields(structure) else {
            self.findings.insert(Rule::IdentityConfirmationPromptImpl);
            return;
        };
        let name = unraw(&structure.ident);
        if self.struct_fields.insert(name, fields).is_some() {
            self.findings.insert(Rule::IdentityConfirmationPromptImpl);
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
            let allowed_origin = match entry.path.as_slice() {
                [root, mid, leaf]
                    if (root == "std" || root == "core") && mid == "future" && leaf == "Future" =>
                {
                    true
                }
                [root, leaf]
                    if root == "domain" && PROMPT_DOMAIN_IMPORTS.contains(&leaf.as_str()) =>
                {
                    true
                }
                _ => false,
            };
            match &entry.kind {
                UseKind::Name if allowed_origin => {}
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
        let Some(struct_name) = single_segment_type_name(&impl_item.self_ty) else {
            self.findings.insert(Rule::IdentityConfirmationPromptImpl);
            return;
        };
        let Some(canonical) = canonical_fields_for(&struct_name) else {
            self.findings.insert(Rule::IdentityConfirmationPromptImpl);
            return;
        };
        if impl_item.trait_.is_some()
            || !impl_item.generics.params.is_empty()
            || impl_item.generics.where_clause.is_some()
            || !impl_item.attrs.is_empty()
        {
            self.findings.insert(Rule::IdentityConfirmationPromptImpl);
            return;
        }
        let methods = self.impl_methods.entry(struct_name).or_default();
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
            let method_name = unraw(&method.sig.ident);
            if !methods.insert(method_name) || !is_canonical_prompt_method(method, canonical) {
                self.findings.insert(Rule::IdentityConfirmationPromptImpl);
            }
        }
        let mut nested = PromptDefNestedBan {
            findings: self.findings,
        };
        nested.visit_item_impl(impl_item);
    }

    fn verify_implemented_structs(&mut self) {
        for struct_name in self.impl_methods.keys() {
            let Some(canonical) = canonical_fields_for(struct_name) else {
                self.findings.insert(Rule::IdentityConfirmationPromptImpl);
                continue;
            };
            let valid_fields = self
                .struct_fields
                .get(struct_name)
                .is_some_and(|declared| fields_match_canonical(declared, canonical));
            if !valid_fields {
                self.findings.insert(Rule::IdentityConfirmationPromptImpl);
            }
        }
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
        struct_fields: BTreeMap::new(),
        impl_methods: BTreeMap::new(),
        findings,
    };
    for item in &ctx.file.items {
        scan.check_item(item);
    }
    scan.verify_implemented_structs();
    if scan.trait_count != 1 || scan.decision_count != 1 {
        findings.insert(Rule::IdentityConfirmationPromptImpl);
    }
}
