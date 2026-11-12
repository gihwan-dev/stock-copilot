use std::collections::BTreeSet;

use syn::ext::IdentExt;
use syn::visit::{self, Visit};
use syn::{
    Attribute, Item, ItemEnum, ItemImpl, ItemMod, ItemStruct, ItemTrait, ItemType, ItemUnion, Path,
    Type,
};

const TEST_MODULE_NAME: &str = "tests";
const TEST_MODULE_SUFFIX: &str = "_tests";
const SELF_SEGMENT: &str = "self";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Origin {
    Local,
    Outside,
}

#[derive(Debug, Default)]
struct LocalNames {
    names: BTreeSet<String>,
}

impl<'ast> Visit<'ast> for LocalNames {
    fn visit_item_struct(&mut self, i: &'ast ItemStruct) {
        self.names.insert(i.ident.unraw().to_string());
        visit::visit_item_struct(self, i);
    }

    fn visit_item_enum(&mut self, i: &'ast ItemEnum) {
        self.names.insert(i.ident.unraw().to_string());
        visit::visit_item_enum(self, i);
    }

    fn visit_item_union(&mut self, i: &'ast ItemUnion) {
        self.names.insert(i.ident.unraw().to_string());
        visit::visit_item_union(self, i);
    }

    fn visit_item_type(&mut self, i: &'ast ItemType) {
        self.names.insert(i.ident.unraw().to_string());
        visit::visit_item_type(self, i);
    }

    fn visit_item_trait(&mut self, i: &'ast ItemTrait) {
        self.names.insert(i.ident.unraw().to_string());
        visit::visit_item_trait(self, i);
    }
}

fn path_origin(path: &Path, local: &BTreeSet<String>) -> Origin {
    let names: Vec<String> = path
        .segments
        .iter()
        .map(|segment| segment.ident.unraw().to_string())
        .collect();
    let relative = match names.split_first() {
        Some((first, rest)) if first == SELF_SEGMENT && path.leading_colon.is_none() => rest,
        Some(_) | None => names.as_slice(),
    };
    match relative {
        [only] if path.leading_colon.is_none() && local.contains(only) => Origin::Local,
        _ => Origin::Outside,
    }
}

fn type_origin(ty: &Type, local: &BTreeSet<String>) -> Origin {
    if let Type::Path(type_path) = ty
        && type_path.qself.is_none()
    {
        path_origin(&type_path.path, local)
    } else if let Type::Reference(reference) = ty {
        type_origin(&reference.elem, local)
    } else if let Type::Paren(inner) = ty {
        type_origin(&inner.elem, local)
    } else if let Type::Group(inner) = ty {
        type_origin(&inner.elem, local)
    } else {
        Origin::Outside
    }
}

#[derive(Debug)]
struct ImplFinder<'local> {
    local: &'local BTreeSet<String>,
    lines: Vec<usize>,
}

impl<'ast> Visit<'ast> for ImplFinder<'_> {
    fn visit_item_impl(&mut self, i: &'ast ItemImpl) {
        let target = type_origin(&i.self_ty, self.local);
        let shadows = match &i.trait_ {
            None => target == Origin::Outside,
            Some((_, path, _)) => {
                target == Origin::Outside && path_origin(path, self.local) == Origin::Outside
            }
        };
        if shadows {
            self.lines.push(i.impl_token.span.start().line);
        }
        visit::visit_item_impl(self, i);
    }
}

fn impl_lines(items: &[Item]) -> Vec<usize> {
    let mut local = LocalNames::default();
    for item in items {
        local.visit_item(item);
    }
    let mut finder = ImplFinder {
        local: &local.names,
        lines: Vec::new(),
    };
    for item in items {
        finder.visit_item(item);
    }
    finder.lines
}

fn is_cfg_test(attribute: &Attribute) -> bool {
    attribute.path().is_ident("cfg")
        && attribute
            .parse_args::<syn::Ident>()
            .is_ok_and(|predicate| predicate == "test")
}

#[derive(Debug, Default)]
struct TestModules {
    lines: Vec<usize>,
}

impl<'ast> Visit<'ast> for TestModules {
    fn visit_item_mod(&mut self, i: &'ast ItemMod) {
        match &i.content {
            Some((_, items)) if i.attrs.iter().any(is_cfg_test) => {
                self.lines.extend(impl_lines(items));
            }
            Some(_) | None => visit::visit_item_mod(self, i),
        }
    }
}

fn is_test_module_file(path: &str) -> bool {
    std::path::Path::new(path)
        .file_stem()
        .and_then(std::ffi::OsStr::to_str)
        .is_some_and(|stem| stem == TEST_MODULE_NAME || stem.ends_with(TEST_MODULE_SUFFIX))
}

pub(crate) fn test_module_impl_lines(path: &str, source: &str) -> Vec<usize> {
    let Ok(file) = syn::parse_file(source) else {
        return Vec::new();
    };
    if is_test_module_file(path) {
        return impl_lines(&file.items);
    }
    let mut modules = TestModules::default();
    modules.visit_file(&file);
    modules.lines
}
