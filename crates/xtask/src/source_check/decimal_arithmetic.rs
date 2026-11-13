use std::collections::{BTreeMap, BTreeSet};

use syn::visit::Visit;

use super::decimal_methods::DecimalRules;
use super::finding::{Finding, Rule};

struct Values<'a> {
    policy: &'a DecimalRules,
    path: &'a str,
    aliases: &'a BTreeSet<String>,
    fields: &'a BTreeMap<String, BTreeSet<String>>,
    self_fields: BTreeSet<String>,
    names: BTreeSet<String>,
    found: Vec<Finding>,
}

fn decimal_type(ty: &syn::Type, aliases: &BTreeSet<String>) -> bool {
    if let syn::Type::Path(path) = ty {
        return path
            .path
            .segments
            .last()
            .is_some_and(|segment| aliases.contains(&segment.ident.to_string()));
    }
    if let syn::Type::Reference(reference) = ty {
        return decimal_type(&reference.elem, aliases);
    }
    false
}

impl Values<'_> {
    fn function(&self, signature: &syn::Signature, block: &syn::Block) -> Vec<Finding> {
        let mut scoped = Self {
            policy: self.policy,
            path: self.path,
            aliases: self.aliases,
            fields: self.fields,
            self_fields: self.self_fields.clone(),
            names: BTreeSet::new(),
            found: Vec::new(),
        };
        for argument in &signature.inputs {
            if let syn::FnArg::Typed(typed) = argument
                && let syn::Pat::Ident(name) = typed.pat.as_ref()
                && decimal_type(&typed.ty, self.aliases)
            {
                scoped.names.insert(name.ident.to_string());
            }
        }
        syn::visit::visit_block(&mut scoped, block);
        scoped.found
    }

    fn decimal(&self, expression: &syn::Expr) -> bool {
        if let syn::Expr::Path(path) = expression {
            return path
                .path
                .segments
                .iter()
                .any(|segment| self.aliases.contains(&segment.ident.to_string()))
                || path
                    .path
                    .get_ident()
                    .is_some_and(|name| self.names.contains(&name.to_string()));
        }
        if let syn::Expr::Call(call) = expression {
            return self.decimal(&call.func);
        }
        if let syn::Expr::MethodCall(call) = expression {
            return match call.method.to_string().as_str() {
                "as_decimal" => true,
                "mantissa" | "scale" | "unpack" | "is_zero" | "is_sign_negative"
                | "is_sign_positive" => false,
                _ => self.decimal(&call.receiver),
            };
        }
        if let syn::Expr::Reference(value) = expression {
            return self.decimal(&value.expr);
        }
        if let syn::Expr::Paren(value) = expression {
            return self.decimal(&value.expr);
        }
        if let syn::Expr::Unary(value) = expression {
            return self.decimal(&value.expr);
        }
        if let syn::Expr::Field(value) = expression {
            let member = match &value.member {
                syn::Member::Named(name) => name.to_string(),
                syn::Member::Unnamed(index) => index.index.to_string(),
            };
            return self.self_fields.contains(&member) && matches_self(&value.base);
        }
        false
    }
}

fn matches_self(expression: &syn::Expr) -> bool {
    if let syn::Expr::Path(path) = expression {
        path.path.is_ident("self")
    } else {
        false
    }
}

impl<'ast> Visit<'ast> for Values<'_> {
    fn visit_item_fn(&mut self, i: &'ast syn::ItemFn) {
        self.found.extend(self.function(&i.sig, &i.block));
    }

    fn visit_impl_item_fn(&mut self, i: &'ast syn::ImplItemFn) {
        self.found.extend(self.function(&i.sig, &i.block));
    }

    fn visit_item_impl(&mut self, i: &'ast syn::ItemImpl) {
        let previous = self.self_fields.clone();
        self.self_fields = if let syn::Type::Path(path) = i.self_ty.as_ref() {
            path.path
                .segments
                .last()
                .and_then(|segment| self.fields.get(&segment.ident.to_string()))
                .cloned()
                .unwrap_or_else(BTreeSet::new)
        } else {
            BTreeSet::new()
        };
        syn::visit::visit_item_impl(self, i);
        self.self_fields = previous;
    }

    fn visit_block(&mut self, i: &'ast syn::Block) {
        let previous = self.names.clone();
        syn::visit::visit_block(self, i);
        self.names = previous;
    }

    fn visit_local(&mut self, i: &'ast syn::Local) {
        syn::visit::visit_local(self, i);
        let (pattern, typed) = if let syn::Pat::Type(typed) = &i.pat {
            (typed.pat.as_ref(), decimal_type(&typed.ty, self.aliases))
        } else {
            (&i.pat, false)
        };
        if let syn::Pat::Ident(name) = pattern {
            let decimal = typed || i.init.as_ref().is_some_and(|init| self.decimal(&init.expr));
            self.names.remove(&name.ident.to_string());
            if decimal {
                self.names.insert(name.ident.to_string());
            }
        }
    }

    fn visit_expr_method_call(&mut self, i: &'ast syn::ExprMethodCall) {
        let method = i.method.to_string();
        if self.policy.arithmetic_forbidden(self.path, &method) && self.decimal(&i.receiver) {
            self.found.push(Finding::at_line(
                self.path,
                i.method.span().start().line,
                Rule::DecimalMethod(method),
            ));
        }
        syn::visit::visit_expr_method_call(self, i);
    }
}

pub(super) fn findings(
    policy: &DecimalRules,
    path: &str,
    file: &syn::File,
    aliases: &BTreeSet<String>,
) -> Vec<Finding> {
    let fields: BTreeMap<String, BTreeSet<String>> = file
        .items
        .iter()
        .filter_map(|item| {
            if let syn::Item::Struct(structure) = item {
                Some((
                    structure.ident.to_string(),
                    structure
                        .fields
                        .iter()
                        .enumerate()
                        .filter(|(_, field)| decimal_type(&field.ty, aliases))
                        .map(|(index, field)| {
                            field
                                .ident
                                .as_ref()
                                .map_or_else(|| index.to_string(), ToString::to_string)
                        })
                        .collect(),
                ))
            } else {
                None
            }
        })
        .collect();
    let mut visitor = Values {
        policy,
        path,
        aliases,
        fields: &fields,
        self_fields: BTreeSet::new(),
        names: BTreeSet::new(),
        found: Vec::new(),
    };
    visitor.visit_file(file);
    visitor.found
}
