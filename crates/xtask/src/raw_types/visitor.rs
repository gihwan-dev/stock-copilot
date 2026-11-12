use std::collections::BTreeSet;

use proc_macro2::TokenStream;
use syn::ext::IdentExt;
use syn::visit::{self, Visit};
use syn::{
    Field, ForeignItemFn, ForeignItemMacro, ForeignItemStatic, Generics, ImplItemConst, ImplItemFn,
    ImplItemMacro, ImplItemType, ItemConst, ItemEnum, ItemFn, ItemImpl, ItemMacro, ItemMod,
    ItemStatic, ItemStruct, ItemTrait, ItemType, ItemUnion, Macro, Path, Signature, TraitItemConst,
    TraitItemFn, TraitItemMacro, TraitItemType, Type, TypeParamBound,
};

use super::Rule;
use super::catalogue::{
    ACCESSOR_PREFIX, BOOL_TYPE, CONSTRUCTOR_NAMES, STANDARD_ROOTS, TRAIT_BOOL_METHODS,
};
use super::scan::{
    TypeNames, generics_names, input_names, is_test_module, newtype_name, output_names,
    returns_self, type_names,
};

#[derive(Debug, Clone, PartialEq, Eq)]
enum ImplContext {
    Outside,
    Newtype(String),
    OtherInherent,
    BoolTrait(String),
    OtherTrait,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Exemption {
    Nothing,
    Inputs,
    AllButBool,
    Bool,
}

#[derive(Debug)]
pub(super) struct RawTypes<'context> {
    local_traits: &'context BTreeSet<String>,
    newtypes: BTreeSet<String>,
    context: ImplContext,
    pub(super) found: Vec<(usize, Rule)>,
}

fn path_names(path: &Path) -> Vec<(usize, String)> {
    let mut names = TypeNames::new();
    names.visit_path(path);
    names.found
}

fn bounds_names<'bound>(
    bounds: impl IntoIterator<Item = &'bound TypeParamBound>,
) -> Vec<(usize, String)> {
    let mut names = TypeNames::new();
    for bound in bounds {
        names.visit_type_param_bound(bound);
    }
    names.found
}

fn fields_names<'field>(fields: impl IntoIterator<Item = &'field Field>) -> Vec<(usize, String)> {
    fields
        .into_iter()
        .flat_map(|field| type_names(&field.ty))
        .collect()
}

impl<'context> RawTypes<'context> {
    pub(super) fn new(
        local_traits: &'context BTreeSet<String>,
        newtypes: BTreeSet<String>,
    ) -> Self {
        Self {
            local_traits,
            newtypes,
            context: ImplContext::Outside,
            found: Vec::new(),
        }
    }

    fn report(&mut self, names: Vec<(usize, String)>) {
        self.found.extend(
            names
                .into_iter()
                .map(|(line, name)| (line, Rule::RawType(name))),
        );
    }

    fn signature(&mut self, signature: &Signature, exemption: Exemption) {
        let mut names = generics_names(&signature.generics);
        let inputs = input_names(signature);
        let outputs = output_names(signature);
        match exemption {
            Exemption::Nothing => {
                names.extend(inputs);
                names.extend(outputs);
            }
            Exemption::Inputs => names.extend(outputs),
            Exemption::AllButBool => names.extend(
                inputs
                    .into_iter()
                    .chain(outputs)
                    .filter(|(_, name)| name != BOOL_TYPE),
            ),
            Exemption::Bool => {
                names.extend(inputs);
                names.extend(outputs.into_iter().filter(|(_, name)| name == BOOL_TYPE));
            }
        }
        self.report(names);
    }

    fn generics(&mut self, generics: &Generics) {
        self.report(generics_names(generics));
    }

    fn trait_context(&self, path: &Path) -> ImplContext {
        let segments: Vec<String> = path
            .segments
            .iter()
            .map(|segment| segment.ident.unraw().to_string())
            .collect();
        let known = |module: &str, name: &str| {
            TRAIT_BOOL_METHODS
                .iter()
                .any(|(listed_module, listed, _)| *listed_module == module && *listed == name)
        };
        match segments.as_slice() {
            [name] if !self.local_traits.contains(name) && known_trait(name) => {
                ImplContext::BoolTrait(name.clone())
            }
            [root, module, name]
                if STANDARD_ROOTS.contains(&root.as_str()) && known(module, name) =>
            {
                ImplContext::BoolTrait(name.clone())
            }
            _ => ImplContext::OtherTrait,
        }
    }

    fn inherent_context(&self, self_type: &Type) -> ImplContext {
        if let Type::Path(path) = self_type
            && path.qself.is_none()
            && let [segment] = path.path.segments.iter().collect::<Vec<_>>().as_slice()
            && segment.arguments.is_none()
            && self.newtypes.contains(&segment.ident.unraw().to_string())
        {
            ImplContext::Newtype(segment.ident.unraw().to_string())
        } else {
            ImplContext::OtherInherent
        }
    }

    fn method_exemption(&self, signature: &Signature) -> Exemption {
        let name = signature.ident.unraw().to_string();
        match &self.context {
            ImplContext::Newtype(newtype) => {
                if CONSTRUCTOR_NAMES.contains(&name.as_str())
                    && signature.receiver().is_none()
                    && signature.inputs.len() == 1
                    && returns_self(signature, newtype).is_some()
                {
                    Exemption::Inputs
                } else if name.starts_with(ACCESSOR_PREFIX)
                    && signature.inputs.len() == 1
                    && signature.receiver().is_some_and(|receiver| {
                        receiver.reference.is_some()
                            && receiver.mutability.is_none()
                            && receiver.colon_token.is_none()
                    })
                {
                    Exemption::Bool
                } else {
                    Exemption::Nothing
                }
            }
            ImplContext::BoolTrait(trait_name) => {
                if TRAIT_BOOL_METHODS
                    .iter()
                    .any(|(_, listed, method)| listed == trait_name && *method == name)
                {
                    Exemption::AllButBool
                } else {
                    Exemption::Nothing
                }
            }
            ImplContext::Outside | ImplContext::OtherInherent | ImplContext::OtherTrait => {
                Exemption::Nothing
            }
        }
    }

    fn item_macro(&mut self, tokens: &TokenStream, line: usize) {
        match syn::parse2::<syn::File>(tokens.clone()) {
            Ok(file) => self.visit_file(&file),
            Err(_) => self.found.push((line, Rule::OpaqueItemMacro)),
        }
    }
}

fn known_trait(name: &str) -> bool {
    TRAIT_BOOL_METHODS
        .iter()
        .any(|(_, listed, _)| *listed == name)
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
        self.generics(&i.generics);
        if newtype_name(i).is_none() {
            self.report(fields_names(&i.fields));
        }
        visit::visit_item_struct(self, i);
    }

    fn visit_item_enum(&mut self, i: &'ast ItemEnum) {
        self.generics(&i.generics);
        for variant in &i.variants {
            self.report(fields_names(&variant.fields));
        }
        visit::visit_item_enum(self, i);
    }

    fn visit_item_union(&mut self, i: &'ast ItemUnion) {
        self.generics(&i.generics);
        self.report(fields_names(&i.fields.named));
        visit::visit_item_union(self, i);
    }

    fn visit_item_type(&mut self, i: &'ast ItemType) {
        self.found
            .push((i.ident.span().start().line, Rule::TypeAlias));
        visit::visit_item_type(self, i);
    }

    fn visit_item_const(&mut self, i: &'ast ItemConst) {
        self.report(type_names(&i.ty));
        visit::visit_item_const(self, i);
    }

    fn visit_item_static(&mut self, i: &'ast ItemStatic) {
        self.report(type_names(&i.ty));
        visit::visit_item_static(self, i);
    }

    fn visit_impl_item_const(&mut self, i: &'ast ImplItemConst) {
        self.report(type_names(&i.ty));
        visit::visit_impl_item_const(self, i);
    }

    fn visit_impl_item_type(&mut self, i: &'ast ImplItemType) {
        self.report(type_names(&i.ty));
        visit::visit_impl_item_type(self, i);
    }

    fn visit_trait_item_const(&mut self, i: &'ast TraitItemConst) {
        self.report(type_names(&i.ty));
        visit::visit_trait_item_const(self, i);
    }

    fn visit_trait_item_type(&mut self, i: &'ast TraitItemType) {
        self.generics(&i.generics);
        self.report(bounds_names(&i.bounds));
        if let Some((_, default)) = &i.default {
            self.report(type_names(default));
        }
        visit::visit_trait_item_type(self, i);
    }

    fn visit_foreign_item_static(&mut self, i: &'ast ForeignItemStatic) {
        self.report(type_names(&i.ty));
        visit::visit_foreign_item_static(self, i);
    }

    fn visit_item_impl(&mut self, i: &'ast ItemImpl) {
        self.generics(&i.generics);
        self.report(type_names(&i.self_ty));
        let context = match &i.trait_ {
            Some((_, path, _)) => {
                self.report(path_names(path));
                self.trait_context(path)
            }
            None => self.inherent_context(&i.self_ty),
        };
        let saved = self.context.clone();
        self.context = context;
        for item in &i.items {
            self.visit_impl_item(item);
        }
        self.context = saved;
    }

    fn visit_item_trait(&mut self, i: &'ast ItemTrait) {
        self.generics(&i.generics);
        self.report(bounds_names(&i.supertraits));
        visit::visit_item_trait(self, i);
    }

    fn visit_item_macro(&mut self, i: &'ast ItemMacro) {
        self.item_macro(&i.mac.tokens, i.mac.path_span_line());
    }

    fn visit_impl_item_macro(&mut self, i: &'ast ImplItemMacro) {
        self.item_macro(&i.mac.tokens, i.mac.path_span_line());
    }

    fn visit_trait_item_macro(&mut self, i: &'ast TraitItemMacro) {
        self.item_macro(&i.mac.tokens, i.mac.path_span_line());
    }

    fn visit_foreign_item_macro(&mut self, i: &'ast ForeignItemMacro) {
        self.item_macro(&i.mac.tokens, i.mac.path_span_line());
    }

    fn visit_macro(&mut self, i: &'ast Macro) {
        if let Ok(file) = syn::parse2::<syn::File>(i.tokens.clone()) {
            let saved = self.context.clone();
            self.context = ImplContext::Outside;
            self.visit_file(&file);
            self.context = saved;
        }
    }
}

trait MacroLine {
    fn path_span_line(&self) -> usize;
}

impl MacroLine for Macro {
    fn path_span_line(&self) -> usize {
        self.path
            .segments
            .first()
            .map_or(0, |segment| segment.ident.span().start().line)
    }
}
