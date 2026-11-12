use std::collections::{BTreeMap, BTreeSet};

use syn::ext::IdentExt;
use syn::visit::{self, Visit};
use syn::{
    Attribute, Fields, GenericArgument, ItemEnum, ItemMod, ItemStruct, ItemTrait, ItemUnion,
    ItemUse, PathArguments, Type, UseTree,
};

use super::Rule;
use super::rules::{Kind, RawRules};

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

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Definitions {
    pub(super) types: BTreeSet<String>,
    pub(super) traits: BTreeSet<String>,
    pub(super) modules: BTreeSet<String>,
}

impl Definitions {
    pub(super) fn has(&self, kind: Kind, name: &str) -> Option<()> {
        let names = match kind {
            Kind::Type => &self.types,
            Kind::Trait => &self.traits,
        };
        names.contains(name).then_some(())
    }

    pub(super) fn extend(&mut self, other: &Self) {
        self.types.extend(other.types.iter().cloned());
        self.traits.extend(other.traits.iter().cloned());
        self.modules.extend(other.modules.iter().cloned());
    }
}

#[derive(Debug, Default)]
pub(super) struct ModuleScan {
    inline: Vec<String>,
    pub(super) declared: Vec<(Vec<String>, String)>,
    pub(super) definitions: Definitions,
}

impl<'ast> Visit<'ast> for ModuleScan {
    fn visit_item_mod(&mut self, i: &'ast ItemMod) {
        if is_test_module(&i.attrs).is_some() {
            return;
        }
        let name = i.ident.unraw().to_string();
        self.definitions.modules.insert(name.clone());
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
        self.definitions.traits.insert(i.ident.unraw().to_string());
        visit::visit_item_trait(self, i);
    }

    fn visit_item_struct(&mut self, i: &'ast ItemStruct) {
        self.definitions.types.insert(i.ident.unraw().to_string());
        visit::visit_item_struct(self, i);
    }

    fn visit_item_enum(&mut self, i: &'ast ItemEnum) {
        self.definitions.types.insert(i.ident.unraw().to_string());
        visit::visit_item_enum(self, i);
    }

    fn visit_item_union(&mut self, i: &'ast ItemUnion) {
        self.definitions.types.insert(i.ident.unraw().to_string());
        visit::visit_item_union(self, i);
    }
}

#[derive(Debug, Default)]
pub(super) struct UseScan {
    prefix: Vec<String>,
    line: usize,
    pub(super) imports: BTreeMap<String, Vec<String>>,
    pub(super) found: Vec<(usize, Rule)>,
}

impl UseScan {
    fn tree(&mut self, tree: &UseTree) {
        match tree {
            UseTree::Path(path) => {
                self.prefix.push(path.ident.unraw().to_string());
                self.tree(&path.tree);
                self.prefix.pop();
            }
            UseTree::Name(name) => {
                let ident = name.ident.unraw().to_string();
                let (imported, full) = if ident == "self" {
                    (self.prefix.last().cloned(), self.prefix.clone())
                } else {
                    let mut full = self.prefix.clone();
                    full.push(ident.clone());
                    (Some(ident), full)
                };
                if let Some(alias) = imported {
                    self.imports.insert(alias, full);
                }
            }
            UseTree::Rename(rename) => {
                let alias = rename.rename.unraw().to_string();
                if alias != "_" {
                    self.found.push((self.line, Rule::UseRename));
                    let mut full = self.prefix.clone();
                    full.push(rename.ident.unraw().to_string());
                    self.imports.insert(alias, full);
                }
            }
            UseTree::Glob(_) => self.found.push((self.line, Rule::GlobImport)),
            UseTree::Group(group) => {
                for item in &group.items {
                    self.tree(item);
                }
            }
        }
    }
}

impl<'ast> Visit<'ast> for UseScan {
    fn visit_item_mod(&mut self, i: &'ast ItemMod) {
        if is_test_module(&i.attrs).is_none() {
            visit::visit_item_mod(self, i);
        }
    }

    fn visit_item_use(&mut self, i: &'ast ItemUse) {
        self.line = i.use_token.span.start().line;
        self.prefix.clear();
        self.tree(&i.tree);
    }
}

pub(super) fn shape(ty: &Type) -> Option<String> {
    if let Type::Path(path) = ty
        && path.qself.is_none()
        && let Some(last) = path.path.segments.last()
    {
        let name = last.ident.unraw().to_string();
        return match &last.arguments {
            PathArguments::None => Some(name),
            PathArguments::AngleBracketed(angle) => angle
                .args
                .iter()
                .map(|argument| {
                    if let GenericArgument::Type(inner) = argument {
                        shape(inner)
                    } else {
                        None
                    }
                })
                .collect::<Option<Vec<String>>>()
                .map(|parts| format!("{name}<{}>", parts.join(", "))),
            PathArguments::Parenthesized(_) => None,
        };
    }
    if let Type::Reference(reference) = ty
        && reference.mutability.is_none()
    {
        return shape(&reference.elem).map(|inner| format!("&{inner}"));
    }
    if let Type::Slice(slice) = ty {
        return shape(&slice.elem).map(|inner| format!("[{inner}]"));
    }
    None
}

const fn inherited(visibility: &syn::Visibility) -> Option<()> {
    match visibility {
        syn::Visibility::Inherited => Some(()),
        syn::Visibility::Public(_) | syn::Visibility::Restricted(_) => None,
    }
}

pub(super) fn newtype_field(item: &ItemStruct, rules: &RawRules) -> Option<(String, String)> {
    if let Fields::Unnamed(fields) = &item.fields
        && fields.unnamed.len() == 1
        && let Some(field) = fields.unnamed.first()
        && inherited(&field.vis).is_some()
        && let Some(field_shape) = shape(&field.ty)
        && rules.newtype_fields.contains(&field_shape)
    {
        Some((item.ident.unraw().to_string(), field_shape))
    } else {
        None
    }
}

#[derive(Debug)]
pub(super) struct Newtypes<'rules> {
    rules: &'rules RawRules,
    pub(super) fields: BTreeMap<String, String>,
}

impl<'rules> Newtypes<'rules> {
    pub(super) const fn new(rules: &'rules RawRules) -> Self {
        Self {
            rules,
            fields: BTreeMap::new(),
        }
    }
}

impl<'ast> Visit<'ast> for Newtypes<'_> {
    fn visit_item_mod(&mut self, i: &'ast ItemMod) {
        if is_test_module(&i.attrs).is_none() {
            visit::visit_item_mod(self, i);
        }
    }

    fn visit_item_struct(&mut self, i: &'ast ItemStruct) {
        if let Some((name, field_shape)) = newtype_field(i, self.rules) {
            self.fields.insert(name, field_shape);
        }
        visit::visit_item_struct(self, i);
    }
}
