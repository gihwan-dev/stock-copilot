use std::collections::BTreeSet;

use quote::ToTokens;
use syn::punctuated::Punctuated;
use syn::visit::{self, Visit};
use syn::{
    Expr, ExprField, ExprMatch, ExprPath, Fields, FnArg, GenericArgument, GenericParam, Generics,
    ImplItem, ImplItemFn, Item, ItemEnum, ItemImpl, ItemMod, ItemStruct, ItemUse, Macro, Member,
    Pat, PathArguments, ReturnType, Stmt, Token, TraitBoundModifier, Type, TypeParamBound,
    WherePredicate,
};

use super::super::super::Rule;
use super::super::super::rules::IdentityConfirmationRules;
use super::super::ast::uses::{UseKind, unraw, use_entries};
use super::{
    CONFIRMED_VARIANT, CheckContext, DECISION_ENUM, Scope, USE_CASE_STRUCT, has_non_private_item,
    is_allowed_derive, is_cfg_test_mod, is_direct_canonical_issuer_path, private, segment_names,
    stream_contains_ident, tokens_contain,
};

const OUTCOME_ENUM: &str = "IdentityConfirmationOutcome";
const ORIGIN_ENUM: &str = "IdentityConfirmationOrigin";
const USER_CONFIRMED_VARIANT: &str = "UserConfirmed";
const DENIED_VARIANT: &str = "Denied";
const UNAVAILABLE_VARIANT: &str = "Unavailable";
const CLOCK_TRAIT: &str = "Clock";

const ALLOWED_PROMPT_IMPORTS: [&str; 5] = [
    "IdentityConfirmationPrompt",
    "IdentityConfirmationDecision",
    "IdentifierChangePrompt",
    "RetirementCorrectionPrompt",
    "LinkCorrectionPrompt",
];

const ALLOWED_DOMAIN_IMPORTS: [&str; 7] = [
    "UserIdentityConfirmation",
    "IdentityConfirmationOrigin",
    "IdentityChangeConfirmation",
    "MappingCorrectionConfirmation",
    "ConfirmedIdentifierChange",
    "ConfirmedRetirementCorrection",
    "ConfirmedLinkCorrection",
];

#[derive(Clone, Copy)]
struct CanonicalConfirmOp {
    prompt_method: &'static str,
    prompt_type: &'static str,
    confirmed_type: &'static str,
    confirmation_type: &'static str,
    part_count: usize,
}

const CANONICAL_CONFIRM_OPS: [CanonicalConfirmOp; 3] = [
    CanonicalConfirmOp {
        prompt_method: "confirm_identifier_change",
        prompt_type: "IdentifierChangePrompt",
        confirmed_type: "ConfirmedIdentifierChange",
        confirmation_type: "IdentityChangeConfirmation",
        part_count: 4,
    },
    CanonicalConfirmOp {
        prompt_method: "confirm_retirement_correction",
        prompt_type: "RetirementCorrectionPrompt",
        confirmed_type: "ConfirmedRetirementCorrection",
        confirmation_type: "MappingCorrectionConfirmation",
        part_count: 3,
    },
    CanonicalConfirmOp {
        prompt_method: "confirm_link_correction",
        prompt_type: "LinkCorrectionPrompt",
        confirmed_type: "ConfirmedLinkCorrection",
        confirmation_type: "MappingCorrectionConfirmation",
        part_count: 4,
    },
];

#[derive(Clone, Copy)]
struct UseCaseParams {
    prompt_pos: usize,
    clock_pos: usize,
}

fn no_path_args(arguments: &PathArguments) -> bool {
    match arguments {
        PathArguments::None => true,
        PathArguments::AngleBracketed(_) | PathArguments::Parenthesized(_) => false,
    }
}

fn is_unit_fields(fields: &Fields) -> bool {
    match fields {
        Fields::Unit => true,
        Fields::Named(_) | Fields::Unnamed(_) => false,
    }
}

fn unwrap_type(ty: &Type) -> &Type {
    if let Type::Group(group) = ty {
        unwrap_type(&group.elem)
    } else if let Type::Paren(paren) = ty {
        unwrap_type(&paren.elem)
    } else {
        ty
    }
}

fn single_segment_ident(ty: &Type) -> Option<String> {
    let Type::Path(path) = unwrap_type(ty) else {
        return None;
    };
    if path.qself.is_some() || path.path.leading_colon.is_some() || path.path.segments.len() != 1 {
        return None;
    }
    let segment = path.path.segments.first()?;
    if !no_path_args(&segment.arguments) {
        return None;
    }
    Some(unraw(&segment.ident))
}

fn field_param_name(ty: &Type) -> Option<String> {
    let inner = unwrap_type(ty);
    if let Type::Reference(reference) = inner {
        if reference.mutability.is_none() {
            single_segment_ident(&reference.elem)
        } else {
            None
        }
    } else {
        single_segment_ident(inner)
    }
}

fn inspect_use_case_struct(file: &syn::File) -> Option<UseCaseParams> {
    let mut found = None;
    for item in &file.items {
        let Item::Struct(structure) = item else {
            continue;
        };
        if unraw(&structure.ident) != USE_CASE_STRUCT {
            continue;
        }
        if found.is_some()
            || !structure.attrs.iter().all(is_allowed_derive)
            || structure.generics.where_clause.is_some()
        {
            return None;
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
        let Fields::Named(named) = &structure.fields else {
            return None;
        };
        if named.named.len() != 2 {
            return None;
        }
        let mut prompt_pos = None;
        let mut clock_pos = None;
        for field in &named.named {
            if !private(&field.vis) {
                return None;
            }
            let field_name = unraw(field.ident.as_ref()?);
            let param_name = field_param_name(&field.ty)?;
            let index = type_params.iter().position(|param| param == &param_name)?;
            if field_name == "prompt" {
                prompt_pos = Some(index);
            } else if field_name == "clock" {
                clock_pos = Some(index);
            } else {
                return None;
            }
        }
        let (Some(prompt_index), Some(clock_index)) = (prompt_pos, clock_pos) else {
            return None;
        };
        if prompt_index == clock_index {
            return None;
        }
        found = Some(UseCaseParams {
            prompt_pos: prompt_index,
            clock_pos: clock_index,
        });
    }
    found
}

fn is_canonical_outcome_enum(item: &ItemEnum) -> bool {
    if unraw(&item.ident) != OUTCOME_ENUM
        || !item.attrs.iter().all(is_allowed_derive)
        || item.generics.where_clause.is_some()
        || item.generics.params.len() != 1
    {
        return false;
    }
    let Some(GenericParam::Type(type_param)) = item.generics.params.first() else {
        return false;
    };
    let param_name = unraw(&type_param.ident);
    if item.variants.len() != 3 {
        return false;
    }
    let mut saw_confirmed = false;
    let mut saw_denied = false;
    let mut saw_unavailable = false;
    for variant in &item.variants {
        if variant.discriminant.is_some() {
            return false;
        }
        match unraw(&variant.ident).as_str() {
            CONFIRMED_VARIANT => {
                let Fields::Unnamed(fields) = &variant.fields else {
                    return false;
                };
                if fields.unnamed.len() != 1 {
                    return false;
                }
                let Some(field) = fields.unnamed.first() else {
                    return false;
                };
                if single_segment_ident(&field.ty).as_deref() != Some(param_name.as_str()) {
                    return false;
                }
                saw_confirmed = true;
            }
            DENIED_VARIANT => {
                if !is_unit_fields(&variant.fields) {
                    return false;
                }
                saw_denied = true;
            }
            UNAVAILABLE_VARIANT => {
                if !is_unit_fields(&variant.fields) {
                    return false;
                }
                saw_unavailable = true;
            }
            _ => return false,
        }
    }
    saw_confirmed && saw_denied && saw_unavailable
}

fn local_use_case_type_args(self_ty: &Type) -> Option<Vec<String>> {
    let Type::Path(path) = unwrap_type(self_ty) else {
        return None;
    };
    if path.qself.is_some() || path.path.leading_colon.is_some() || path.path.segments.len() != 1 {
        return None;
    }
    let segment = path.path.segments.first()?;
    if unraw(&segment.ident) != USE_CASE_STRUCT {
        return None;
    }
    let PathArguments::AngleBracketed(arguments) = &segment.arguments else {
        return None;
    };
    let mut type_args = Vec::new();
    for arg in &arguments.args {
        if let GenericArgument::Type(ty) = arg {
            type_args.push(single_segment_ident(ty)?);
        }
    }
    Some(type_args)
}

fn trait_bound_matches(bound: &TypeParamBound, allowed_paths: &[&[&str]]) -> bool {
    let TypeParamBound::Trait(trait_bound) = bound else {
        return false;
    };
    let no_modifier = match trait_bound.modifier {
        TraitBoundModifier::None => true,
        TraitBoundModifier::Maybe(_) => false,
    };
    if !no_modifier || trait_bound.lifetimes.is_some() {
        return false;
    }
    if trait_bound.path.leading_colon.is_some() {
        return false;
    }
    let names = segment_names(&trait_bound.path);
    let refs: Vec<&str> = names.iter().map(String::as_str).collect();
    allowed_paths.contains(&refs.as_slice())
}

fn bounds_require_canonical_trait(
    bounds: &Punctuated<TypeParamBound, Token![+]>,
    allowed_paths: &[&[&str]],
) -> bool {
    bounds
        .iter()
        .any(|bound| trait_bound_matches(bound, allowed_paths))
}

fn generics_bind_param(generics: &Generics, param_name: &str, allowed_paths: &[&[&str]]) -> bool {
    let is_declared_param = generics.params.iter().any(|param| match param {
        GenericParam::Type(ty) => unraw(&ty.ident) == param_name,
        GenericParam::Lifetime(_) | GenericParam::Const(_) => false,
    });
    if !is_declared_param {
        return false;
    }
    let direct = generics.params.iter().any(|param| match param {
        GenericParam::Type(ty) if unraw(&ty.ident) == param_name => {
            bounds_require_canonical_trait(&ty.bounds, allowed_paths)
        }
        GenericParam::Type(_) | GenericParam::Lifetime(_) | GenericParam::Const(_) => false,
    });
    if direct {
        return true;
    }
    generics.where_clause.as_ref().is_some_and(|clause| {
        clause.predicates.iter().any(|predicate| match predicate {
            WherePredicate::Type(ty)
                if single_segment_ident(&ty.bounded_ty).as_deref() == Some(param_name) =>
            {
                bounds_require_canonical_trait(&ty.bounds, allowed_paths)
            }
            WherePredicate::Type(_) | WherePredicate::Lifetime(_) | _ => false,
        })
    })
}

fn path_matches_one_of(path: &syn::Path, allowed: &[&[&str]]) -> bool {
    if path.leading_colon.is_some() || !path.segments.iter().all(|seg| no_path_args(&seg.arguments))
    {
        return false;
    }
    let names = segment_names(path);
    let refs: Vec<&str> = names.iter().map(String::as_str).collect();
    allowed.contains(&refs.as_slice())
}

fn type_matches_one_of(ty: &Type, allowed: &[&[&str]]) -> bool {
    let Type::Path(path) = unwrap_type(ty) else {
        return false;
    };
    path.qself.is_none() && path_matches_one_of(&path.path, allowed)
}

fn match_op_for_request_type(ty: &Type, prompt_module: &str) -> Option<CanonicalConfirmOp> {
    CANONICAL_CONFIRM_OPS.into_iter().find(|op| {
        let bare = [op.prompt_type];
        let qualified = ["crate", prompt_module, op.prompt_type];
        type_matches_one_of(ty, &[&bare, &qualified])
    })
}

fn return_matches_confirmed_outcome(output: &ReturnType, confirmed_type: &str) -> bool {
    let ReturnType::Type(_, ret_ty) = output else {
        return false;
    };
    let Type::Path(path) = unwrap_type(ret_ty) else {
        return false;
    };
    if path.qself.is_some() || path.path.leading_colon.is_some() || path.path.segments.len() != 1 {
        return false;
    }
    let Some(segment) = path.path.segments.first() else {
        return false;
    };
    if unraw(&segment.ident) != OUTCOME_ENUM {
        return false;
    }
    let PathArguments::AngleBracketed(args) = &segment.arguments else {
        return false;
    };
    if args.args.len() != 1 {
        return false;
    }
    let Some(GenericArgument::Type(inner)) = args.args.first() else {
        return false;
    };
    let bare = [confirmed_type];
    let qualified = ["domain", confirmed_type];
    type_matches_one_of(inner, &[&bare, &qualified])
}

fn unwrap_expr(expr: &Expr) -> &Expr {
    if let Expr::Group(group) = expr {
        unwrap_expr(&group.expr)
    } else if let Expr::Paren(paren) = expr {
        unwrap_expr(&paren.expr)
    } else {
        expr
    }
}

fn expr_single_ident(expr: &Expr) -> Option<String> {
    let Expr::Path(path) = unwrap_expr(expr) else {
        return None;
    };
    if path.qself.is_some() || path.path.leading_colon.is_some() || path.path.segments.len() != 1 {
        return None;
    }
    let segment = path.path.segments.first()?;
    if !no_path_args(&segment.arguments) {
        return None;
    }
    Some(unraw(&segment.ident))
}

fn is_self_field_expr(expr: &Expr, field_name: &str) -> bool {
    let Expr::Field(ExprField { base, member, .. }) = unwrap_expr(expr) else {
        return false;
    };
    let is_self = expr_single_ident(base).as_deref() == Some("self");
    let matches_field = match member {
        Member::Named(ident) => unraw(ident) == field_name,
        Member::Unnamed(_) => false,
    };
    is_self && matches_field
}

fn is_canonical_prompt_await(expr: &Expr, expected_method: &str, req_ident: &str) -> bool {
    let Expr::Await(awaited) = unwrap_expr(expr) else {
        return false;
    };
    let Expr::MethodCall(call) = unwrap_expr(&awaited.base) else {
        return false;
    };
    if call.turbofish.is_some()
        || unraw(&call.method) != expected_method
        || !is_self_field_expr(&call.receiver, "prompt")
        || call.args.len() != 1
    {
        return false;
    }
    let Some(arg) = call.args.first() else {
        return false;
    };
    let Expr::Reference(reference) = unwrap_expr(arg) else {
        return false;
    };
    reference.mutability.is_none()
        && expr_single_ident(&reference.expr).as_deref() == Some(req_ident)
}

fn pat_simple_ident(pat: &Pat) -> Option<String> {
    let Pat::Ident(ident) = pat else {
        return None;
    };
    if ident.by_ref.is_some() || ident.mutability.is_some() || ident.subpat.is_some() {
        return None;
    }
    Some(unraw(&ident.ident))
}

fn is_self_clock_now(expr: &Expr) -> bool {
    let Expr::MethodCall(call) = unwrap_expr(expr) else {
        return false;
    };
    call.turbofish.is_none()
        && call.args.is_empty()
        && unraw(&call.method) == "now"
        && is_self_field_expr(&call.receiver, "clock")
}

fn is_canonical_issuer_call(expr: &Expr, rules: &IdentityConfirmationRules) -> bool {
    let Expr::Call(call) = unwrap_expr(expr) else {
        return false;
    };
    if !call.args.is_empty() {
        return false;
    }
    let Expr::Path(callee) = unwrap_expr(&call.func) else {
        return false;
    };
    is_direct_canonical_issuer_path(rules, callee)
}

fn is_request_into_parts(expr: &Expr, req_ident: &str) -> bool {
    let Expr::MethodCall(call) = unwrap_expr(expr) else {
        return false;
    };
    call.turbofish.is_none()
        && call.args.is_empty()
        && unraw(&call.method) == "into_parts"
        && expr_single_ident(&call.receiver).as_deref() == Some(req_ident)
}

fn call_matches_path<'a>(expr: &'a Expr, allowed: &[&[&str]]) -> Option<&'a syn::ExprCall> {
    let Expr::Call(call) = unwrap_expr(expr) else {
        return None;
    };
    let Expr::Path(ExprPath { qself, path, .. }) = unwrap_expr(&call.func) else {
        return None;
    };
    (qself.is_none() && path_matches_one_of(path, allowed)).then_some(call)
}

fn is_user_confirmed_origin_call(expr: &Expr, seal_var: &str) -> bool {
    let bare = [ORIGIN_ENUM, USER_CONFIRMED_VARIANT];
    let qualified = ["domain", ORIGIN_ENUM, USER_CONFIRMED_VARIANT];
    let Some(call) = call_matches_path(expr, &[&bare, &qualified]) else {
        return false;
    };
    call.args.len() == 1
        && call.args.first().and_then(expr_single_ident).as_deref() == Some(seal_var)
}

fn is_confirmation_new_call(
    expr: &Expr,
    confirmation_type: &str,
    seal_var: &str,
    evidence_var: &str,
    time_var: &str,
) -> bool {
    let bare = [confirmation_type, "new"];
    let qualified = ["domain", confirmation_type, "new"];
    let Some(call) = call_matches_path(expr, &[&bare, &qualified]) else {
        return false;
    };
    if call.args.len() != 3 {
        return false;
    }
    let Some(origin_arg) = call.args.first() else {
        return false;
    };
    let Some(evidence_arg) = call.args.get(1) else {
        return false;
    };
    let Some(time_arg) = call.args.get(2) else {
        return false;
    };
    is_user_confirmed_origin_call(origin_arg, seal_var)
        && expr_single_ident(evidence_arg).as_deref() == Some(evidence_var)
        && expr_single_ident(time_arg).as_deref() == Some(time_var)
}

fn is_canonical_confirmed_tail(
    expr: &Expr,
    op: CanonicalConfirmOp,
    parts: &[String],
    seal_var: &str,
    time_var: &str,
) -> bool {
    let outcome_path = [OUTCOME_ENUM, CONFIRMED_VARIANT];
    let Some(outer_call) = call_matches_path(expr, &[&outcome_path]) else {
        return false;
    };
    if outer_call.args.len() != 1 {
        return false;
    }
    let Some(confirmed_arg) = outer_call.args.first() else {
        return false;
    };
    let bare = [op.confirmed_type, "new"];
    let qualified = ["domain", op.confirmed_type, "new"];
    let Some(confirmed_call) = call_matches_path(confirmed_arg, &[&bare, &qualified]) else {
        return false;
    };
    if confirmed_call.args.len() != op.part_count || parts.len() != op.part_count {
        return false;
    }
    let leading = op.part_count.saturating_sub(1);
    for index in 0..leading {
        let Some(arg) = confirmed_call.args.get(index) else {
            return false;
        };
        if expr_single_ident(arg).as_deref() != parts.get(index).map(String::as_str) {
            return false;
        }
    }
    let Some(conf_arg) = confirmed_call.args.get(leading) else {
        return false;
    };
    let Some(evidence_var) = parts.get(leading) else {
        return false;
    };
    is_confirmation_new_call(
        conf_arg,
        op.confirmation_type,
        seal_var,
        evidence_var,
        time_var,
    )
}

fn extract_confirmed_bindings(
    stmts: &[Stmt],
    req_ident: &str,
    op: CanonicalConfirmOp,
    rules: &IdentityConfirmationRules,
) -> Option<(String, String, Vec<String>)> {
    let mut time_var = None;
    let mut seal_var = None;
    let mut parts = None;
    for stmt in stmts {
        let Stmt::Local(local) = stmt else {
            return None;
        };
        let Some(init) = &local.init else {
            return None;
        };
        if init.diverge.is_some() {
            return None;
        }
        if is_self_clock_now(&init.expr) {
            if time_var.is_some() {
                return None;
            }
            time_var = Some(pat_simple_ident(&local.pat)?);
        } else if is_canonical_issuer_call(&init.expr, rules) {
            if seal_var.is_some() {
                return None;
            }
            seal_var = Some(pat_simple_ident(&local.pat)?);
        } else if is_request_into_parts(&init.expr, req_ident) {
            if parts.is_some() {
                return None;
            }
            let Pat::Tuple(tuple) = &local.pat else {
                return None;
            };
            if tuple.elems.len() != op.part_count {
                return None;
            }
            let names: Option<Vec<String>> = tuple.elems.iter().map(pat_simple_ident).collect();
            parts = Some(names?);
        } else {
            return None;
        }
    }
    let (Some(time), Some(seal), Some(bound_parts)) = (time_var, seal_var, parts) else {
        return None;
    };
    let mut distinct = BTreeSet::new();
    distinct.insert(time.as_str());
    distinct.insert(seal.as_str());
    for part in &bound_parts {
        distinct.insert(part.as_str());
    }
    if distinct.len() != op.part_count.saturating_add(2) {
        return None;
    }
    Some((time, seal, bound_parts))
}

fn is_canonical_confirmed_arm_body(
    body: &Expr,
    req_ident: &str,
    op: CanonicalConfirmOp,
    rules: &IdentityConfirmationRules,
) -> bool {
    let Expr::Block(block) = unwrap_expr(body) else {
        return false;
    };
    if block.label.is_some() || block.block.stmts.len() != 4 {
        return false;
    }
    let Some((Stmt::Expr(tail, None), leading)) = block.block.stmts.split_last() else {
        return false;
    };
    let Some((time_var, seal_var, parts)) =
        extract_confirmed_bindings(leading, req_ident, op, rules)
    else {
        return false;
    };
    is_canonical_confirmed_tail(tail, op, &parts, &seal_var, &time_var)
}

fn is_non_confirmed_outcome_expr(expr: &Expr) -> bool {
    let Expr::Path(ExprPath { qself, path, .. }) = unwrap_expr(expr) else {
        return false;
    };
    if qself.is_some() {
        return false;
    }
    let denied = [OUTCOME_ENUM, DENIED_VARIANT];
    let unavailable = [OUTCOME_ENUM, UNAVAILABLE_VARIANT];
    path_matches_one_of(path, &[&denied, &unavailable])
}

fn is_canonical_confirmed_pat(pat: &Pat, prompt_module: &str) -> bool {
    let Pat::Path(path) = pat else {
        return false;
    };
    if path.qself.is_some() {
        return false;
    }
    let bare = [DECISION_ENUM, CONFIRMED_VARIANT];
    let qualified = ["crate", prompt_module, DECISION_ENUM, CONFIRMED_VARIANT];
    path_matches_one_of(&path.path, &[&bare, &qualified])
}

fn is_canonical_match(
    match_expr: &ExprMatch,
    req_ident: &str,
    op: CanonicalConfirmOp,
    rules: &IdentityConfirmationRules,
) -> bool {
    if !is_canonical_prompt_await(&match_expr.expr, op.prompt_method, req_ident) {
        return false;
    }
    let mut confirmed_arms = 0_usize;
    for arm in &match_expr.arms {
        if arm.guard.is_some() {
            return false;
        }
        if is_canonical_confirmed_pat(&arm.pat, &rules.prompt_module) {
            confirmed_arms = confirmed_arms.saturating_add(1);
            if confirmed_arms > 1
                || !is_canonical_confirmed_arm_body(&arm.body, req_ident, op, rules)
            {
                return false;
            }
        } else if !is_non_confirmed_outcome_expr(&arm.body) {
            return false;
        }
    }
    confirmed_arms == 1
}

fn is_canonical_confirm_method(method: &ImplItemFn, rules: &IdentityConfirmationRules) -> bool {
    if method.sig.asyncness.is_none()
        || !method.sig.generics.params.is_empty()
        || method.sig.generics.where_clause.is_some()
        || method.sig.inputs.len() != 2
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
    let Some(FnArg::Typed(req_arg)) = method.sig.inputs.get(1) else {
        return false;
    };
    let Some(req_ident) = pat_simple_ident(&req_arg.pat) else {
        return false;
    };
    let Some(op) = match_op_for_request_type(&req_arg.ty, &rules.prompt_module) else {
        return false;
    };
    if !return_matches_confirmed_outcome(&method.sig.output, op.confirmed_type) {
        return false;
    }
    let [Stmt::Expr(Expr::Match(match_expr), None)] = method.block.stmts.as_slice() else {
        return false;
    };
    is_canonical_match(match_expr, &req_ident, op, rules)
}

fn is_canonical_new_method(method: &ImplItemFn, rules: &IdentityConfirmationRules) -> bool {
    if unraw(&method.sig.ident) != "new"
        || method.sig.asyncness.is_some()
        || !method.sig.generics.params.is_empty()
        || method.sig.generics.where_clause.is_some()
        || method.sig.inputs.len() != 2
    {
        return false;
    }
    let ReturnType::Type(_, ret_ty) = &method.sig.output else {
        return false;
    };
    if single_segment_ident(ret_ty).as_deref() != Some("Self") {
        return false;
    }
    !tokens_contain(method, &rules.issuer_function)
        && !tokens_contain(method, &rules.seal_type)
        && !tokens_contain(method, &rules.confirmation_type)
}

struct UseCaseFileCheck<'a> {
    rules: &'a IdentityConfirmationRules,
    params: Option<UseCaseParams>,
    scope: Scope,
    seen_struct: usize,
    seen_enum: usize,
    findings: &'a mut BTreeSet<Rule>,
}

impl UseCaseFileCheck<'_> {
    fn check_use_entry_allowed(&mut self, entry: &super::super::ast::uses::UseEntry) {
        match entry.kind {
            UseKind::Name => {}
            UseKind::Rename(_) | UseKind::Glob => {
                self.findings.insert(Rule::IdentityConfirmationIssuance);
                return;
            }
        }
        let path: Vec<&str> = entry.path.iter().map(String::as_str).collect();
        let allowed = match path.as_slice() {
            ["crate", CLOCK_TRAIT] => true,
            ["crate", module, symbol] if *module == self.rules.prompt_module => {
                ALLOWED_PROMPT_IMPORTS.contains(symbol)
            }
            ["domain", symbol] => ALLOWED_DOMAIN_IMPORTS.contains(symbol),
            _ => false,
        };
        if !allowed {
            self.findings.insert(Rule::IdentityConfirmationIssuance);
        }
    }

    fn impl_binds_canonical_traits(&self, item: &ItemImpl) -> bool {
        let Some(params) = self.params else {
            return false;
        };
        let Some(type_args) = local_use_case_type_args(&item.self_ty) else {
            return false;
        };
        let Some(prompt_arg) = type_args.get(params.prompt_pos) else {
            return false;
        };
        let Some(clock_arg) = type_args.get(params.clock_pos) else {
            return false;
        };
        let prompt_bare = [self.rules.prompt_trait.as_str()];
        let prompt_qualified = [
            "crate",
            self.rules.prompt_module.as_str(),
            self.rules.prompt_trait.as_str(),
        ];
        let clock_bare = [CLOCK_TRAIT];
        let clock_qualified = ["crate", CLOCK_TRAIT];
        generics_bind_param(
            &item.generics,
            prompt_arg,
            &[&prompt_bare, &prompt_qualified],
        ) && generics_bind_param(&item.generics, clock_arg, &[&clock_bare, &clock_qualified])
    }

    fn check_top_level_item(&mut self, item: &Item) {
        match item {
            Item::Use(use_item) => self.check_use_item(use_item),
            Item::Struct(structure) => self.check_struct_item(structure),
            Item::Enum(enumeration) => self.check_enum_item(enumeration),
            Item::Impl(impl_item) => self.check_impl_item(impl_item),
            Item::Mod(module) => self.check_mod_item(module),
            Item::Const(_)
            | Item::ExternCrate(_)
            | Item::Fn(_)
            | Item::ForeignMod(_)
            | Item::Macro(_)
            | Item::Static(_)
            | Item::Trait(_)
            | Item::TraitAlias(_)
            | Item::Type(_)
            | Item::Union(_)
            | Item::Verbatim(_)
            | _ => {
                self.findings.insert(Rule::IdentityConfirmationIssuance);
            }
        }
    }

    fn check_use_item(&mut self, use_item: &ItemUse) {
        if !private(&use_item.vis) {
            self.findings.insert(Rule::IdentityConfirmationIssuance);
        }
        for entry in use_entries(&use_item.tree) {
            self.check_use_entry_allowed(&entry);
        }
    }

    fn check_struct_item(&mut self, structure: &ItemStruct) {
        self.seen_struct = self.seen_struct.saturating_add(1);
        let clean_attrs = structure.attrs.iter().all(is_allowed_derive)
            && structure.fields.iter().all(|field| field.attrs.is_empty());
        if unraw(&structure.ident) != USE_CASE_STRUCT
            || self.seen_struct > 1
            || self.params.is_none()
            || !clean_attrs
        {
            self.findings.insert(Rule::IdentityConfirmationIssuance);
        }
    }

    fn check_enum_item(&mut self, enumeration: &ItemEnum) {
        self.seen_enum = self.seen_enum.saturating_add(1);
        if self.seen_enum > 1 || !is_canonical_outcome_enum(enumeration) {
            self.findings.insert(Rule::IdentityConfirmationIssuance);
        }
    }

    fn check_impl_item(&mut self, impl_item: &ItemImpl) {
        if impl_item.trait_.is_some() || local_use_case_type_args(&impl_item.self_ty).is_none() {
            self.findings.insert(Rule::IdentityConfirmationIssuance);
            return;
        }
        let gated = self.impl_binds_canonical_traits(impl_item);
        for member in &impl_item.items {
            let ImplItem::Fn(method) = member else {
                self.findings.insert(Rule::IdentityConfirmationIssuance);
                continue;
            };
            let allowed = is_canonical_new_method(method, self.rules)
                || (gated && is_canonical_confirm_method(method, self.rules));
            if !allowed {
                self.findings.insert(Rule::IdentityConfirmationIssuance);
            }
        }
        let mut nested = NestedItemBan {
            rules: self.rules,
            findings: self.findings,
        };
        nested.visit_item_impl(impl_item);
    }

    fn check_mod_item(&mut self, module: &ItemMod) {
        if self.scope == Scope::Product && is_cfg_test_mod(module) {
            if let Some((_, nested)) = &module.content
                && has_non_private_item(nested)
            {
                self.findings.insert(Rule::IdentityConfirmationIssuance);
            }
            if stream_contains_ident(module.to_token_stream(), &self.rules.issuer_function)
                || stream_contains_ident(module.to_token_stream(), &self.rules.seal_type)
                || super::stream_literal_contains(
                    module.to_token_stream(),
                    &self.rules.issuer_function,
                )
                || super::stream_literal_contains(module.to_token_stream(), &self.rules.seal_type)
            {
                self.findings.insert(Rule::IdentityConfirmationIssuance);
            }
            return;
        }
        self.findings.insert(Rule::IdentityConfirmationIssuance);
    }
}

struct NestedItemBan<'a> {
    rules: &'a IdentityConfirmationRules,
    findings: &'a mut BTreeSet<Rule>,
}

impl<'ast> Visit<'ast> for NestedItemBan<'_> {
    fn visit_item(&mut self, i: &'ast Item) {
        self.findings.insert(Rule::IdentityConfirmationIssuance);
        visit::visit_item(self, i);
    }

    fn visit_macro(&mut self, i: &'ast Macro) {
        self.findings.insert(Rule::IdentityConfirmationIssuance);
        visit::visit_macro(self, i);
    }

    fn visit_attribute(&mut self, i: &'ast syn::Attribute) {
        if super::stream_literal_contains(i.to_token_stream(), &self.rules.issuer_function)
            || super::stream_literal_contains(i.to_token_stream(), &self.rules.seal_type)
        {
            self.findings.insert(Rule::IdentityConfirmationIssuance);
        }
        visit::visit_attribute(self, i);
    }
}

pub(super) fn check_use_case_file(
    rules: &IdentityConfirmationRules,
    ctx: &CheckContext<'_>,
    findings: &mut BTreeSet<Rule>,
) {
    if !rules.issuance_allowed_files.contains(ctx.relative) {
        if stream_contains_ident(ctx.tokens.clone(), &rules.issuer_function)
            || stream_contains_ident(ctx.tokens.clone(), &rules.seal_type)
        {
            findings.insert(Rule::IdentityConfirmationIssuance);
        }
        return;
    }
    if ctx.file.attrs.iter().any(|attr| {
        super::stream_literal_contains(attr.to_token_stream(), &rules.issuer_function)
            || super::stream_literal_contains(attr.to_token_stream(), &rules.seal_type)
    }) {
        findings.insert(Rule::IdentityConfirmationIssuance);
    }
    let params = inspect_use_case_struct(ctx.file);
    let mut scan = UseCaseFileCheck {
        rules,
        params,
        scope: Scope::Product,
        seen_struct: 0,
        seen_enum: 0,
        findings,
    };
    for item in &ctx.file.items {
        scan.check_top_level_item(item);
    }
}
