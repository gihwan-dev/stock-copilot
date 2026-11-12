use std::collections::{BTreeMap, BTreeSet};

use proc_macro2::TokenStream;
use syn::ext::IdentExt;
use syn::visit::{self, Visit};
use syn::{
    Field, FnArg, ForeignItemFn, ForeignItemMacro, ForeignItemStatic, GenericParam, Generics,
    ImplItemConst, ImplItemFn, ImplItemMacro, ImplItemType, ItemConst, ItemEnum, ItemFn, ItemImpl,
    ItemMacro, ItemMod, ItemStatic, ItemStruct, ItemTrait, ItemType, ItemUnion, Macro, Path,
    ReturnType, Signature, TraitItemConst, TraitItemFn, TraitItemMacro, TraitItemType, Type,
    TypeParamBound,
};

use super::Rule;
use super::resolve::{FileContext, Resolution, TypeCheck};
use super::rules::Kind;
use super::scan::{is_test_module, newtype_field, shape};

const BOOL_TYPE: &str = "bool";

#[derive(Debug, Clone, PartialEq, Eq)]
enum ImplContext {
    Outside,
    Newtype(String, String),
    Trait(Option<String>),
    OtherInherent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Exemption {
    Nothing,
    Inputs,
    Output,
    Bool,
}

#[derive(Debug)]
pub(super) struct RawTypes<'context> {
    context: &'context FileContext<'context>,
    newtypes: BTreeMap<String, String>,
    generics: Vec<BTreeSet<String>>,
    impl_context: ImplContext,
    pub(super) found: Vec<(usize, Rule)>,
}

fn generic_names(generics: &Generics) -> BTreeSet<String> {
    generics
        .params
        .iter()
        .filter_map(|param| match param {
            GenericParam::Type(ty) => Some(ty.ident.unraw().to_string()),
            GenericParam::Const(constant) => Some(constant.ident.unraw().to_string()),
            GenericParam::Lifetime(_) => None,
        })
        .collect()
}

fn path_names(path: &Path) -> Vec<String> {
    path.segments
        .iter()
        .map(|segment| segment.ident.unraw().to_string())
        .collect()
}

fn first_type_argument(ty: &Type) -> Option<&Type> {
    if let Type::Path(path) = ty
        && let Some(last) = path.path.segments.last()
        && let syn::PathArguments::AngleBracketed(angle) = &last.arguments
        && let Some(syn::GenericArgument::Type(inner)) = angle.args.first()
    {
        Some(inner)
    } else {
        None
    }
}

impl<'context> RawTypes<'context> {
    pub(super) fn new(
        context: &'context FileContext<'context>,
        newtypes: BTreeMap<String, String>,
    ) -> Self {
        Self {
            context,
            newtypes,
            generics: Vec::new(),
            impl_context: ImplContext::Outside,
            found: Vec::new(),
        }
    }

    fn scope(&self) -> BTreeSet<String> {
        self.generics.iter().flatten().cloned().collect()
    }

    fn checker<F: FnOnce(&mut TypeCheck<'_>)>(&self, check: F) -> Vec<(usize, Rule)> {
        let scope = self.scope();
        let mut checker = TypeCheck::new(self.context, &scope);
        check(&mut checker);
        checker.found
    }

    fn check_type(&mut self, ty: &Type) {
        let found = self.checker(|checker| checker.visit_type(ty));
        self.found.extend(found);
    }

    fn check_generics(&mut self, generics: &Generics) {
        let found = self.checker(|checker| checker.visit_generics(generics));
        self.found.extend(found);
    }

    fn check_bounds<'bound>(&mut self, bounds: impl IntoIterator<Item = &'bound TypeParamBound>) {
        let collected: Vec<&TypeParamBound> = bounds.into_iter().collect();
        let found = self.checker(|checker| {
            for bound in collected {
                checker.visit_type_param_bound(bound);
            }
        });
        self.found.extend(found);
    }

    fn check_fields<'field>(&mut self, fields: impl IntoIterator<Item = &'field Field>) {
        for field in fields {
            self.check_type(&field.ty);
        }
    }

    fn signature(&mut self, signature: &Signature, exemption: Exemption) {
        self.generics.push(generic_names(&signature.generics));
        let mut found = self.checker(|checker| checker.visit_generics(&signature.generics));
        let inputs = self.checker(|checker| {
            for input in &signature.inputs {
                match input {
                    FnArg::Receiver(receiver) => checker.visit_type(&receiver.ty),
                    FnArg::Typed(typed) => checker.visit_type(&typed.ty),
                }
            }
        });
        let outputs = self.checker(|checker| match &signature.output {
            ReturnType::Default => {}
            ReturnType::Type(_, ty) => checker.check_return(ty),
        });
        self.generics.pop();
        let is_bool = |(_, rule): &(usize, Rule)| *rule == Rule::RawType(String::from(BOOL_TYPE));
        match exemption {
            Exemption::Nothing => {
                found.extend(inputs);
                found.extend(outputs);
            }
            Exemption::Inputs => found.extend(outputs),
            Exemption::Output => found.extend(inputs),
            Exemption::Bool => found.extend(
                inputs
                    .into_iter()
                    .chain(outputs)
                    .filter(|finding| !is_bool(finding)),
            ),
        }
        self.found.extend(found);
    }

    fn field_forms(&self, field_shape: &str) -> BTreeSet<String> {
        let mut forms = BTreeSet::from([String::from(field_shape)]);
        if let Some(borrowed) = self.context.rules.borrowed_forms.get(field_shape) {
            forms.extend(borrowed.iter().cloned());
        }
        forms
    }

    fn returns_self(&self, signature: &Signature, newtype: &str) -> Option<()> {
        let ReturnType::Type(_, ty) = &signature.output else {
            return None;
        };
        let names_self = |candidate: &Type| {
            shape(candidate).is_some_and(|name| name == "Self" || name == newtype)
        };
        if names_self(ty) {
            return Some(());
        }
        let wrapper = if let Type::Path(path) = ty.as_ref() {
            path.path
                .segments
                .last()
                .map(|segment| segment.ident.unraw().to_string())
        } else {
            None
        };
        (wrapper.is_some_and(|name| self.context.rules.constructor_wrappers.contains(&name))
            && first_type_argument(ty).is_some_and(names_self))
        .then_some(())
    }

    fn newtype_exemption(&self, signature: &Signature, newtype: &str, field: &str) -> Exemption {
        let rules = self.context.rules;
        let name = signature.ident.unraw().to_string();
        let forms = self.field_forms(field);
        let single_input = match signature.inputs.first() {
            Some(FnArg::Typed(typed)) if signature.inputs.len() == 1 => shape(&typed.ty),
            Some(FnArg::Typed(_) | FnArg::Receiver(_)) | None => None,
        };
        if rules.constructor_names.contains(&name)
            && single_input.is_some_and(|input| forms.contains(&input))
            && self.returns_self(signature, newtype).is_some()
        {
            return Exemption::Inputs;
        }
        let shared_self = signature.inputs.len() == 1
            && signature.receiver().is_some_and(|receiver| {
                receiver.reference.is_some()
                    && receiver.mutability.is_none()
                    && receiver.colon_token.is_none()
            });
        let output = match &signature.output {
            ReturnType::Type(_, ty) => shape(ty),
            ReturnType::Default => None,
        };
        let accessor_forms: BTreeSet<String> = forms
            .iter()
            .cloned()
            .chain(std::iter::once(format!("&{field}")))
            .collect();
        if name.starts_with(rules.accessor_prefix.as_str())
            && shared_self
            && output.is_some_and(|returned| accessor_forms.contains(&returned))
        {
            Exemption::Output
        } else {
            Exemption::Nothing
        }
    }

    fn method_exemption(&self, signature: &Signature) -> Exemption {
        match &self.impl_context {
            ImplContext::Newtype(newtype, field) => {
                self.newtype_exemption(signature, newtype, field)
            }
            ImplContext::Trait(Some(trait_path)) => {
                let name = signature.ident.unraw().to_string();
                if self
                    .context
                    .rules
                    .bool_methods
                    .contains(&(trait_path.clone(), name))
                {
                    Exemption::Bool
                } else {
                    Exemption::Nothing
                }
            }
            ImplContext::Outside | ImplContext::Trait(None) | ImplContext::OtherInherent => {
                Exemption::Nothing
            }
        }
    }

    fn inherent_context(&self, self_type: &Type) -> ImplContext {
        if let Type::Path(path) = self_type
            && path.qself.is_none()
            && let [segment] = path.path.segments.iter().collect::<Vec<_>>().as_slice()
            && segment.arguments.is_none()
            && let Some(field) = self.newtypes.get(&segment.ident.unraw().to_string())
        {
            ImplContext::Newtype(segment.ident.unraw().to_string(), field.clone())
        } else {
            ImplContext::OtherInherent
        }
    }

    fn item_macro(&mut self, tokens: &TokenStream, line: usize) {
        match syn::parse2::<syn::File>(tokens.clone()) {
            Ok(file) => self.visit_file(&file),
            Err(_) => self.found.push((line, Rule::OpaqueItemMacro)),
        }
    }
}

fn macro_line(mac: &Macro) -> usize {
    mac.path
        .segments
        .first()
        .map_or(0, |segment| segment.ident.span().start().line)
}

impl<'ast> Visit<'ast> for RawTypes<'_> {
    fn visit_item_mod(&mut self, i: &'ast ItemMod) {
        if is_test_module(&i.attrs).is_none() {
            visit::visit_item_mod(self, i);
        }
    }

    fn visit_item_fn(&mut self, i: &'ast ItemFn) {
        self.signature(&i.sig, Exemption::Nothing);
        visit::visit_item_fn(self, i);
    }

    fn visit_impl_item_fn(&mut self, i: &'ast ImplItemFn) {
        let exemption = self.method_exemption(&i.sig);
        self.signature(&i.sig, exemption);
        visit::visit_impl_item_fn(self, i);
    }

    fn visit_trait_item_fn(&mut self, i: &'ast TraitItemFn) {
        self.signature(&i.sig, Exemption::Nothing);
        visit::visit_trait_item_fn(self, i);
    }

    fn visit_foreign_item_fn(&mut self, i: &'ast ForeignItemFn) {
        self.signature(&i.sig, Exemption::Nothing);
        visit::visit_foreign_item_fn(self, i);
    }

    fn visit_item_struct(&mut self, i: &'ast ItemStruct) {
        self.generics.push(generic_names(&i.generics));
        self.check_generics(&i.generics);
        if newtype_field(i, self.context.rules).is_none() {
            self.check_fields(&i.fields);
        }
        self.generics.pop();
        visit::visit_item_struct(self, i);
    }

    fn visit_item_enum(&mut self, i: &'ast ItemEnum) {
        self.generics.push(generic_names(&i.generics));
        self.check_generics(&i.generics);
        for variant in &i.variants {
            self.check_fields(&variant.fields);
        }
        self.generics.pop();
        visit::visit_item_enum(self, i);
    }

    fn visit_item_union(&mut self, i: &'ast ItemUnion) {
        self.generics.push(generic_names(&i.generics));
        self.check_generics(&i.generics);
        self.check_fields(&i.fields.named);
        self.generics.pop();
        visit::visit_item_union(self, i);
    }

    fn visit_item_type(&mut self, i: &'ast ItemType) {
        self.found
            .push((i.ident.span().start().line, Rule::TypeAlias));
        visit::visit_item_type(self, i);
    }

    fn visit_item_const(&mut self, i: &'ast ItemConst) {
        self.check_type(&i.ty);
        visit::visit_item_const(self, i);
    }

    fn visit_item_static(&mut self, i: &'ast ItemStatic) {
        self.check_type(&i.ty);
        visit::visit_item_static(self, i);
    }

    fn visit_impl_item_const(&mut self, i: &'ast ImplItemConst) {
        self.check_type(&i.ty);
        visit::visit_impl_item_const(self, i);
    }

    fn visit_impl_item_type(&mut self, i: &'ast ImplItemType) {
        self.check_type(&i.ty);
        visit::visit_impl_item_type(self, i);
    }

    fn visit_trait_item_const(&mut self, i: &'ast TraitItemConst) {
        self.check_type(&i.ty);
        visit::visit_trait_item_const(self, i);
    }

    fn visit_trait_item_type(&mut self, i: &'ast TraitItemType) {
        self.generics.push(generic_names(&i.generics));
        self.check_generics(&i.generics);
        self.check_bounds(&i.bounds);
        if let Some((_, default)) = &i.default {
            self.check_type(default);
        }
        self.generics.pop();
        visit::visit_trait_item_type(self, i);
    }

    fn visit_foreign_item_static(&mut self, i: &'ast ForeignItemStatic) {
        self.check_type(&i.ty);
        visit::visit_foreign_item_static(self, i);
    }

    fn visit_item_impl(&mut self, i: &'ast ItemImpl) {
        self.generics.push(generic_names(&i.generics));
        self.check_generics(&i.generics);
        self.check_type(&i.self_ty);
        let context = match &i.trait_ {
            Some((_, path, _)) => {
                let scope = self.scope();
                let resolution = self.context.resolve(&path_names(path), Kind::Trait, &scope);
                let bound = syn::TraitBound {
                    paren_token: None,
                    modifier: syn::TraitBoundModifier::None,
                    lifetimes: None,
                    path: path.clone(),
                };
                let found = self.checker(|checker| checker.visit_trait_bound(&bound));
                self.found.extend(found);
                match resolution {
                    Resolution::Accepted(standard) => ImplContext::Trait(standard),
                    Resolution::Rejected => ImplContext::Trait(None),
                }
            }
            None => self.inherent_context(&i.self_ty),
        };
        let saved = self.impl_context.clone();
        self.impl_context = context;
        for item in &i.items {
            self.visit_impl_item(item);
        }
        self.impl_context = saved;
        self.generics.pop();
    }

    fn visit_item_trait(&mut self, i: &'ast ItemTrait) {
        self.generics.push(generic_names(&i.generics));
        self.check_generics(&i.generics);
        self.check_bounds(&i.supertraits);
        let saved = self.impl_context.clone();
        self.impl_context = ImplContext::Outside;
        visit::visit_item_trait(self, i);
        self.impl_context = saved;
        self.generics.pop();
    }

    fn visit_item_macro(&mut self, i: &'ast ItemMacro) {
        self.item_macro(&i.mac.tokens, macro_line(&i.mac));
    }

    fn visit_impl_item_macro(&mut self, i: &'ast ImplItemMacro) {
        self.item_macro(&i.mac.tokens, macro_line(&i.mac));
    }

    fn visit_trait_item_macro(&mut self, i: &'ast TraitItemMacro) {
        self.item_macro(&i.mac.tokens, macro_line(&i.mac));
    }

    fn visit_foreign_item_macro(&mut self, i: &'ast ForeignItemMacro) {
        self.item_macro(&i.mac.tokens, macro_line(&i.mac));
    }

    fn visit_macro(&mut self, i: &'ast Macro) {
        if let Ok(file) = syn::parse2::<syn::File>(i.tokens.clone()) {
            let saved = self.impl_context.clone();
            self.impl_context = ImplContext::Outside;
            self.visit_file(&file);
            self.impl_context = saved;
        }
    }
}
