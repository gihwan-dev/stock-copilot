use std::collections::{BTreeMap, BTreeSet};

use syn::ext::IdentExt;
use syn::visit::{self, Visit};
use syn::{
    AngleBracketedGenericArguments, Block, Expr, GenericArgument, Path, PathArguments, TraitBound,
    Type, TypeMacro, TypePath,
};

use super::Rule;
use super::rules::{Kind, RawRules};
use super::scan::Definitions;

const RESULT_PATH: &str = "result::Result";

const SELF_TYPE: &str = "Self";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Resolution {
    Accepted(Option<String>),
    Rejected,
}

#[derive(Debug)]
pub(super) struct FileContext<'context> {
    pub(super) rules: &'context RawRules,
    pub(super) local: &'context Definitions,
    pub(super) dependencies: &'context BTreeMap<String, Definitions>,
    pub(super) imports: &'context BTreeMap<String, Vec<String>>,
}

fn accepted(found: Option<()>, path: Option<String>) -> Resolution {
    match found {
        Some(()) => Resolution::Accepted(path),
        None => Resolution::Rejected,
    }
}

impl FileContext<'_> {
    pub(super) fn resolve(
        &self,
        segments: &[String],
        kind: Kind,
        generics: &BTreeSet<String>,
    ) -> Resolution {
        let Some(first) = segments.first() else {
            return Resolution::Rejected;
        };
        if first == SELF_TYPE || generics.contains(first) {
            return Resolution::Accepted(None);
        }
        let expanded: Vec<String> = match self.imports.get(first) {
            Some(full) => full
                .iter()
                .chain(segments.iter().skip(1))
                .cloned()
                .collect(),
            None => segments.to_vec(),
        };
        self.classify(&expanded, kind)
    }

    fn single(&self, name: &str, kind: Kind) -> Resolution {
        if self.local.has(kind, name).is_some() {
            return Resolution::Accepted(None);
        }
        match self.rules.prelude(kind, name) {
            Some(full) => Resolution::Accepted(Some(full)),
            None => Resolution::Rejected,
        }
    }

    fn classify(&self, path: &[String], kind: Kind) -> Resolution {
        match path {
            [] => Resolution::Rejected,
            [single] => self.single(single, kind),
            [first, rest @ ..] => {
                let last = rest.last().map_or("", String::as_str);
                if self.rules.standard_roots.contains(first) {
                    let standard = rest.join("::");
                    let found = self.rules.standard_allowed(kind, &standard);
                    accepted(found, Some(standard))
                } else if self.rules.local_roots.contains(first)
                    || self.local.modules.contains(first)
                {
                    accepted(self.local.has(kind, last), None)
                } else if let Some(definitions) = self.dependencies.get(first) {
                    accepted(definitions.has(kind, last), None)
                } else {
                    accepted(self.rules.external_allowed(kind, &path.join("::")), None)
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum UnitPlace {
    Allowed,
    Forbidden,
}

#[derive(Debug)]
pub(super) struct TypeCheck<'check> {
    context: &'check FileContext<'check>,
    generics: &'check BTreeSet<String>,
    unit: UnitPlace,
    pub(super) found: Vec<(usize, Rule)>,
}

fn segment_names(path: &Path) -> Vec<String> {
    path.segments
        .iter()
        .map(|segment| segment.ident.unraw().to_string())
        .collect()
}

fn path_line(path: &Path) -> usize {
    path.segments
        .last()
        .map_or(0, |segment| segment.ident.span().start().line)
}

pub(super) fn is_unit(ty: &Type) -> Option<()> {
    if let Type::Tuple(tuple) = ty
        && tuple.elems.is_empty()
    {
        Some(())
    } else {
        None
    }
}

impl<'check> TypeCheck<'check> {
    pub(super) const fn new(
        context: &'check FileContext<'check>,
        generics: &'check BTreeSet<String>,
    ) -> Self {
        Self {
            context,
            generics,
            unit: UnitPlace::Forbidden,
            found: Vec::new(),
        }
    }

    pub(super) fn check_return(&mut self, ty: &Type) {
        self.unit = UnitPlace::Allowed;
        self.visit_type(ty);
    }

    fn named(&mut self, path: &Path, kind: Kind) -> Resolution {
        let resolution = self
            .context
            .resolve(&segment_names(path), kind, self.generics);
        if resolution == Resolution::Rejected {
            let last = path
                .segments
                .last()
                .map_or_else(String::new, |segment| segment.ident.unraw().to_string());
            self.found.push((path_line(path), Rule::RawType(last)));
        }
        resolution
    }

    fn arguments(&mut self, path: &Path, result: Option<()>) {
        for segment in &path.segments {
            match &segment.arguments {
                PathArguments::None => {}
                PathArguments::AngleBracketed(angle) => self.angle_arguments(angle, result),
                PathArguments::Parenthesized(parenthesized) => {
                    visit::visit_parenthesized_generic_arguments(self, parenthesized);
                }
            }
        }
    }

    fn angle_arguments(&mut self, angle: &AngleBracketedGenericArguments, result: Option<()>) {
        let error_is_unit = angle
            .args
            .iter()
            .filter_map(type_argument)
            .nth(1)
            .and_then(is_unit);
        let ok_unit = result.is_some() && error_is_unit.is_none();
        for (position, argument) in angle.args.iter().enumerate() {
            let Some(ty) = type_argument(argument) else {
                visit::visit_generic_argument(self, argument);
                continue;
            };
            if ok_unit && position == 0 {
                self.unit = UnitPlace::Allowed;
            }
            self.visit_type(ty);
        }
    }
}

fn type_argument(argument: &GenericArgument) -> Option<&Type> {
    if let GenericArgument::Type(ty) = argument {
        Some(ty)
    } else {
        None
    }
}

impl<'ast> Visit<'ast> for TypeCheck<'_> {
    fn visit_type(&mut self, i: &'ast Type) {
        let place = self.unit;
        self.unit = UnitPlace::Forbidden;
        if is_unit(i).is_some() && place == UnitPlace::Forbidden {
            let line = if let Type::Tuple(tuple) = i {
                tuple.paren_token.span.open().start().line
            } else {
                0
            };
            self.found.push((line, Rule::Unit));
        }
        visit::visit_type(self, i);
    }

    fn visit_type_path(&mut self, i: &'ast TypePath) {
        if let Some(qself) = &i.qself {
            self.visit_type(&qself.ty);
            self.arguments(&i.path, None);
            return;
        }
        let result = match self.named(&i.path, Kind::Type) {
            Resolution::Accepted(Some(full)) if full == RESULT_PATH => Some(()),
            Resolution::Accepted(_) | Resolution::Rejected => None,
        };
        self.arguments(&i.path, result);
    }

    fn visit_trait_bound(&mut self, i: &'ast TraitBound) {
        self.named(&i.path, Kind::Trait);
        self.arguments(&i.path, None);
    }

    fn visit_type_macro(&mut self, i: &'ast TypeMacro) {
        self.found
            .push((path_line(&i.mac.path), Rule::OpaqueItemMacro));
    }

    fn visit_expr(&mut self, _i: &'ast Expr) {}

    fn visit_block(&mut self, _i: &'ast Block) {}
}
