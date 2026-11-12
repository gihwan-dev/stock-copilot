use std::collections::BTreeSet;

use syn::ext::IdentExt;
use syn::visit::{self, Visit};
use syn::{
    Attribute, Block, Expr, Fields, FnArg, GenericArgument, Generics, ItemMod, ItemStruct,
    ItemTrait, PathArguments, ReturnType, Signature, Type, TypePath,
};

use super::catalogue::{BOOL_TYPE, BYTE_VECTOR, raw_names};

pub(super) fn is_test_module(attributes: &[Attribute]) -> Option<()> {
    attributes
        .iter()
        .any(|attribute| {
            attribute.path().is_ident("cfg")
                && attribute
                    .meta
                    .require_list()
                    .is_ok_and(|list| list.tokens.to_string() == "test")
        })
        .then_some(())
}

#[derive(Debug, Default)]
pub(super) struct ModuleScan {
    inline: Vec<String>,
    pub(super) declared: Vec<(Vec<String>, String)>,
    pub(super) traits: BTreeSet<String>,
}

impl<'ast> Visit<'ast> for ModuleScan {
    fn visit_item_mod(&mut self, i: &'ast ItemMod) {
        if is_test_module(&i.attrs).is_some() {
            return;
        }
        let name = i.ident.unraw().to_string();
        match &i.content {
            Some((_, items)) => {
                self.inline.push(name);
                for item in items {
                    self.visit_item(item);
                }
                self.inline.pop();
            }
            None => self.declared.push((self.inline.clone(), name)),
        }
    }

    fn visit_item_trait(&mut self, i: &'ast ItemTrait) {
        self.traits.insert(i.ident.unraw().to_string());
        visit::visit_item_trait(self, i);
    }
}

#[derive(Debug)]
pub(super) struct TypeNames {
    raw: Vec<&'static str>,
    pub(super) found: Vec<(usize, String)>,
}

impl TypeNames {
    pub(super) fn new() -> Self {
        Self {
            raw: raw_names(),
            found: Vec::new(),
        }
    }
}

impl<'ast> Visit<'ast> for TypeNames {
    fn visit_type_path(&mut self, i: &'ast TypePath) {
        if let Some(last) = i.path.segments.last() {
            let name = last.ident.unraw().to_string();
            if self.raw.contains(&name.as_str()) {
                self.found.push((last.ident.span().start().line, name));
            }
        }
        visit::visit_type_path(self, i);
    }

    fn visit_expr(&mut self, _i: &'ast Expr) {}

    fn visit_block(&mut self, _i: &'ast Block) {}
}

fn plain_last_ident(ty: &Type) -> Option<String> {
    if let Type::Path(path) = ty
        && path.qself.is_none()
        && let Some(last) = path.path.segments.last()
        && last.arguments.is_none()
    {
        Some(last.ident.unraw().to_string())
    } else {
        None
    }
}

fn first_type_argument(arguments: &PathArguments) -> Option<&Type> {
    if let PathArguments::AngleBracketed(angle) = arguments
        && let Some(GenericArgument::Type(ty)) = angle.args.first()
    {
        Some(ty)
    } else {
        None
    }
}

fn newtype_field(ty: &Type) -> Option<()> {
    if let Some(name) = plain_last_ident(ty) {
        return (name != BOOL_TYPE && raw_names().contains(&name.as_str())).then_some(());
    }
    let (container, element) = BYTE_VECTOR;
    if let Type::Path(path) = ty
        && path.qself.is_none()
        && let Some(last) = path.path.segments.last()
        && last.ident == container
        && let PathArguments::AngleBracketed(angle) = &last.arguments
        && angle.args.len() == 1
        && first_type_argument(&last.arguments)
            .and_then(plain_last_ident)
            .is_some_and(|inner| inner == element)
    {
        Some(())
    } else {
        None
    }
}

pub(super) fn newtype_name(item: &ItemStruct) -> Option<String> {
    if let Fields::Unnamed(fields) = &item.fields
        && fields.unnamed.len() == 1
        && let Some(field) = fields.unnamed.first()
        && matches_inherited(&field.vis).is_some()
        && newtype_field(&field.ty).is_some()
    {
        Some(item.ident.unraw().to_string())
    } else {
        None
    }
}

const fn matches_inherited(visibility: &syn::Visibility) -> Option<()> {
    match visibility {
        syn::Visibility::Inherited => Some(()),
        syn::Visibility::Public(_) | syn::Visibility::Restricted(_) => None,
    }
}

#[derive(Debug, Default)]
pub(super) struct Newtypes {
    pub(super) names: BTreeSet<String>,
}

impl<'ast> Visit<'ast> for Newtypes {
    fn visit_item_mod(&mut self, i: &'ast ItemMod) {
        if is_test_module(&i.attrs).is_none() {
            visit::visit_item_mod(self, i);
        }
    }

    fn visit_item_struct(&mut self, i: &'ast ItemStruct) {
        if let Some(name) = newtype_name(i) {
            self.names.insert(name);
        }
        visit::visit_item_struct(self, i);
    }
}

pub(super) fn type_names(ty: &Type) -> Vec<(usize, String)> {
    let mut names = TypeNames::new();
    names.visit_type(ty);
    names.found
}

pub(super) fn generics_names(generics: &Generics) -> Vec<(usize, String)> {
    let mut names = TypeNames::new();
    names.visit_generics(generics);
    names.found
}

pub(super) fn input_names(signature: &Signature) -> Vec<(usize, String)> {
    signature
        .inputs
        .iter()
        .flat_map(|input| match input {
            FnArg::Receiver(receiver) => type_names(&receiver.ty),
            FnArg::Typed(typed) => type_names(&typed.ty),
        })
        .collect()
}

pub(super) fn output_names(signature: &Signature) -> Vec<(usize, String)> {
    match &signature.output {
        ReturnType::Default => Vec::new(),
        ReturnType::Type(_, ty) => type_names(ty),
    }
}

pub(super) fn returns_self(signature: &Signature, newtype: &str) -> Option<()> {
    let ReturnType::Type(_, ty) = &signature.output else {
        return None;
    };
    let names_self = |candidate: &Type| {
        plain_last_ident(candidate).is_some_and(|name| name == "Self" || name == newtype)
    };
    if names_self(ty) {
        return Some(());
    }
    if let Type::Path(path) = ty.as_ref()
        && path.qself.is_none()
        && let Some(last) = path.path.segments.last()
        && super::catalogue::CONSTRUCTOR_WRAPPERS.contains(&last.ident.to_string().as_str())
        && first_type_argument(&last.arguments).is_some_and(names_self)
    {
        Some(())
    } else {
        None
    }
}
