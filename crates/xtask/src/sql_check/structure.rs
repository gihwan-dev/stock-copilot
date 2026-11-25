use std::collections::BTreeSet;
use std::fmt;
use std::fs;
use std::path::Path;

use syn::spanned::Spanned;
use syn::visit::Visit;
use syn::{Expr, ExprMethodCall, Item, ItemFn, Lit, Type};

use super::rules::{Recognition, recognize};
use super::statement::{self, Problem, Source};
use crate::check_support::{Finding, read_text};

pub(crate) const STORE_SOURCE: &str = "crates/adapters/store-sqlite/src/";
pub(crate) const SQL_MODULE: &str = "crates/adapters/store-sqlite/src/sql/";
pub(crate) const MIGRATIONS: &str = "crates/adapters/store-sqlite/migrations";
pub(crate) const PROTECTED_FILE: &str = "harness/sql-check/money-protected.toml";
pub(crate) const FORWARDERS_FILE: &str = "harness/sql-check/forwarders.toml";

pub(crate) const SQL_METHODS: [(&str, usize, usize); 13] = [
    ("execute", 2, 0),
    ("execute_batch", 1, 0),
    ("prepare", 1, 0),
    ("prepare_cached", 1, 0),
    ("query_row", 3, 0),
    ("query_row_and_then", 3, 0),
    ("query_one", 3, 0),
    ("pragma_query", 3, 1),
    ("pragma_query_value", 3, 1),
    ("pragma_update", 3, 1),
    ("pragma_update_and_check", 4, 1),
    ("pragma", 4, 1),
    ("query_and_then", 3, 0),
];

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Structure {
    OutsideModule,
    ModuleItem,
    ModuleMacro,
    Statement(Problem),
    CallArgument(String),
    Unparsable,
}

impl fmt::Display for Structure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OutsideModule => f.write_str("sql-structure:outside-module"),
            Self::ModuleItem => f.write_str("sql-structure:module-item"),
            Self::ModuleMacro => f.write_str("sql-structure:module-macro"),
            Self::Statement(Problem::Unparsable) => f.write_str("sql-structure:unparsable"),
            Self::Statement(Problem::StatementCount(count)) => {
                write!(f, "sql-structure:statements:{count}")
            }
            Self::Statement(Problem::DdlOutsideMigration) => {
                f.write_str("sql-structure:ddl-outside-migration")
            }
            Self::Statement(Problem::MoneyPlacement(column)) => {
                write!(f, "sql-structure:money-placement:{column}")
            }
            Self::CallArgument(method) => write!(f, "sql-structure:call-argument:{method}"),
            Self::Unparsable => f.write_str("sql-structure:rust-unparsable"),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Context {
    pub(crate) migrations: BTreeSet<String>,
    pub(crate) money: BTreeSet<String>,
    pub(crate) forwarders: BTreeSet<(String, String)>,
}

fn string_list(root: &Path, file: &str, key: &str) -> Result<Vec<toml::Value>, String> {
    let table: toml::Table = read_text(&root.join(file))?
        .parse()
        .map_err(|error| format!("{file}: {error}"))?;
    table
        .get(key)
        .and_then(toml::Value::as_array)
        .cloned()
        .ok_or_else(|| format!("{file}: {key} 목록이 없다"))
}

fn text_of(value: &toml::Value, key: &str, file: &str) -> Result<String, String> {
    value
        .get(key)
        .and_then(toml::Value::as_str)
        .filter(|text| !text.trim().is_empty())
        .map(String::from)
        .ok_or_else(|| format!("{file}: {key} 값이 없다"))
}

pub(crate) fn migration_texts(root: &Path) -> Result<Vec<(String, String)>, String> {
    let directory = root.join(MIGRATIONS);
    let mut names = fs::read_dir(&directory)
        .map_err(|error| format!("{MIGRATIONS}: {error}"))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "sql"))
        .collect::<Vec<_>>();
    names.sort();
    names
        .iter()
        .map(|path| {
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .map_or_else(String::new, String::from);
            read_text(path).map(|text| (name, text))
        })
        .collect()
}

impl Context {
    pub(crate) fn load(root: &Path) -> Result<Self, String> {
        let texts = migration_texts(root)?;
        let mut money = statement::money_columns(&texts)?;
        for entry in string_list(root, PROTECTED_FILE, "column")? {
            text_of(&entry, "reason", PROTECTED_FILE)?;
            money.insert(text_of(&entry, "name", PROTECTED_FILE)?.to_lowercase());
        }
        let mut forwarders = BTreeSet::new();
        for entry in string_list(root, FORWARDERS_FILE, "forwarder")? {
            text_of(&entry, "reason", FORWARDERS_FILE)?;
            forwarders.insert((
                text_of(&entry, "file", FORWARDERS_FILE)?,
                text_of(&entry, "function", FORWARDERS_FILE)?,
            ));
        }
        Ok(Self {
            migrations: texts.into_iter().map(|(_, text)| text).collect(),
            money,
            forwarders,
        })
    }
}

pub(crate) fn is_test_path(path: &str) -> bool {
    let file = path.rsplit('/').next().unwrap_or(path);
    file == "tests.rs"
        || file.ends_with("_tests.rs")
        || path
            .split('/')
            .any(|part| part == "tests" || part.ends_with("_tests"))
}

fn is_str_reference(ty: &Type) -> bool {
    if let Type::Reference(reference) = ty
        && let Type::Path(path) = reference.elem.as_ref()
    {
        reference.mutability.is_none() && path.qself.is_none() && path.path.is_ident("str")
    } else {
        false
    }
}

fn literal_text(expr: &Expr) -> Option<String> {
    if let Expr::Lit(literal) = expr
        && let Lit::Str(text) = &literal.lit
    {
        Some(text.value())
    } else {
        None
    }
}

fn line_of<T: Spanned>(node: &T) -> usize {
    node.span().start().line
}

fn constant_findings(
    path: &str,
    constant: &syn::ItemConst,
    context: &Context,
) -> Vec<Finding<Structure>> {
    let Some(text) = literal_text(&constant.expr).filter(|_text| is_str_reference(&constant.ty))
    else {
        return vec![Finding::at_line(
            path,
            line_of(constant),
            Structure::ModuleItem,
        )];
    };
    let source = if context.migrations.contains(&text) {
        Source::Migration
    } else {
        Source::Query
    };
    statement::check(&text, source, &context.money)
        .into_iter()
        .map(|problem| {
            Finding::at_line(path, line_of(&constant.expr), Structure::Statement(problem))
        })
        .collect()
}

fn module_findings(path: &str, file: &syn::File, context: &Context) -> Vec<Finding<Structure>> {
    let mut findings = Vec::new();
    for item in &file.items {
        if let Item::Const(constant) = item {
            findings.extend(constant_findings(path, constant, context));
        } else if let Item::Macro(found) = item {
            findings.push(Finding::at_line(
                path,
                line_of(found),
                Structure::ModuleMacro,
            ));
        } else {
            findings.push(Finding::at_line(path, line_of(item), Structure::ModuleItem));
        }
    }
    findings
}

fn is_constant_name(segment: &syn::PathSegment) -> bool {
    let name = segment.ident.to_string();
    segment.arguments.is_none()
        && name.chars().any(|next| next.is_ascii_uppercase())
        && name
            .chars()
            .all(|next| next.is_ascii_uppercase() || next.is_ascii_digit() || next == '_')
}

fn is_constant_path(expr: &Expr) -> bool {
    if let Expr::Reference(reference) = expr {
        is_constant_path(&reference.expr)
    } else if let Expr::Paren(inner) = expr {
        is_constant_path(&inner.expr)
    } else if let Expr::Path(path) = expr {
        path.qself.is_none() && path.path.segments.last().is_some_and(is_constant_name)
    } else {
        false
    }
}

fn is_plain_path(expr: &Expr) -> bool {
    if let Expr::Path(path) = expr {
        path.qself.is_none()
    } else {
        false
    }
}

fn is_pragma_name(expr: &Expr) -> bool {
    literal_text(expr).is_some_and(|text| {
        !text.is_empty()
            && text
                .chars()
                .all(|next| next.is_ascii_lowercase() || next == '_')
    })
}

struct Calls<'a> {
    path: &'a str,
    forwarders: &'a BTreeSet<(String, String)>,
    function: Option<String>,
    findings: Vec<Finding<Structure>>,
}

impl Calls<'_> {
    fn forwarding(&self) -> bool {
        self.function.as_ref().is_some_and(|function| {
            self.forwarders
                .contains(&(String::from(self.path), function.clone()))
        })
    }
}

impl<'ast> Visit<'ast> for Calls<'_> {
    fn visit_item_fn(&mut self, i: &'ast ItemFn) {
        let previous = self.function.replace(i.sig.ident.to_string());
        syn::visit::visit_item_fn(self, i);
        self.function = previous;
    }

    fn visit_impl_item_fn(&mut self, i: &'ast syn::ImplItemFn) {
        let previous = self.function.replace(i.sig.ident.to_string());
        syn::visit::visit_impl_item_fn(self, i);
        self.function = previous;
    }

    fn visit_expr_method_call(&mut self, i: &'ast ExprMethodCall) {
        let method = i.method.to_string();
        let shape = SQL_METHODS
            .iter()
            .find(|(name, count, _)| *name == method && *count == i.args.len());
        if let Some((_, _, position)) = shape {
            let pragma = *position == 1;
            let accepted = i.args.iter().nth(*position).is_some_and(|argument| {
                is_constant_path(argument)
                    || (pragma && is_pragma_name(argument))
                    || (!pragma && self.forwarding() && is_plain_path(argument))
            });
            if !accepted {
                self.findings.push(Finding::at_line(
                    self.path,
                    line_of(&i.method),
                    Structure::CallArgument(method),
                ));
            }
        }
        syn::visit::visit_expr_method_call(self, i);
    }
}

struct Literals<'a> {
    path: &'a str,
    findings: Vec<Finding<Structure>>,
}

impl<'ast> Visit<'ast> for Literals<'_> {
    fn visit_lit_str(&mut self, i: &'ast syn::LitStr) {
        if recognize(&i.value()) == Recognition::Sql {
            self.findings.push(Finding::at_line(
                self.path,
                line_of(i),
                Structure::OutsideModule,
            ));
        }
    }
}

pub(crate) fn check_file(path: &str, text: &str, context: &Context) -> Vec<Finding<Structure>> {
    if !path.starts_with(STORE_SOURCE) || is_test_path(path) {
        return Vec::new();
    }
    let Ok(file) = syn::parse_file(text) else {
        return vec![Finding::whole_file(path, Structure::Unparsable)];
    };
    let in_module = path
        .strip_prefix(SQL_MODULE)
        .is_some_and(|rest| !rest.is_empty());
    let mut findings = if in_module {
        module_findings(path, &file, context)
    } else {
        let mut literals = Literals {
            path,
            findings: Vec::new(),
        };
        literals.visit_file(&file);
        literals.findings
    };
    let mut calls = Calls {
        path,
        forwarders: &context.forwarders,
        function: None,
        findings: Vec::new(),
    };
    calls.visit_file(&file);
    findings.extend(calls.findings);
    findings
}
