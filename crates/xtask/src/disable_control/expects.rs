use std::collections::BTreeSet;

use proc_macro2::{Delimiter, TokenStream, TokenTree};
use syn::ext::IdentExt;
use syn::parse::{ParseStream, Parser};
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::visit::{self, Visit};
use syn::{
    AttrStyle, Attribute, Expr, Field, ForeignItemFn, ForeignItemMacro, ForeignItemStatic,
    ForeignItemType, ImplItemConst, ImplItemFn, ImplItemMacro, ImplItemType, ItemConst, ItemEnum,
    ItemExternCrate, ItemFn, ItemForeignMod, ItemImpl, ItemMacro, ItemMod, ItemStatic, ItemStruct,
    ItemTrait, ItemTraitAlias, ItemType, ItemUnion, ItemUse, Local, Macro, Meta, Stmt, Token,
    TraitItemConst, TraitItemFn, TraitItemMacro, TraitItemType, Variant,
};

use super::allowlist::local_text;

const ATTRIBUTE_ROOTS: [&str; 3] = ["std", "core", "alloc"];

const EXPECT: &str = "expect";

const REASON: &str = "reason";

const BLOCK_KEYWORDS: [&str; 9] = [
    "if", "match", "loop", "while", "for", "unsafe", "async", "const", "try",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Target {
    Function,
    Statement,
    Field,
    Variant,
    Item,
    Module,
    Impl,
    Trait,
    ForeignBlock,
    Macro,
    Block,
    Inner,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Collected {
    pub(crate) line: usize,
    pub(crate) lints: Vec<String>,
    pub(crate) reason: String,
    pub(crate) target: Target,
    pub(crate) statement: Option<String>,
}

#[derive(Debug, Default)]
pub(crate) struct ExpectCollector {
    handled: BTreeSet<(usize, usize)>,
    pending: Vec<Target>,
    pub(crate) found: Vec<Collected>,
}

fn is_expect_path(path: &syn::Path) -> Option<()> {
    let segments: Vec<String> = path
        .segments
        .iter()
        .map(|segment| segment.ident.unraw().to_string())
        .collect();
    let expect = match segments.as_slice() {
        [only] => only == EXPECT,
        [root, .., last] => ATTRIBUTE_ROOTS.contains(&root.as_str()) && last == EXPECT,
        [] => false,
    };
    expect.then_some(())
}

fn lint_name(path: &syn::Path) -> String {
    path.segments
        .iter()
        .map(|segment| segment.ident.unraw().to_string())
        .collect::<Vec<_>>()
        .join("::")
}

fn arguments(tokens: TokenStream) -> (Vec<String>, String) {
    let parser = Punctuated::<Meta, Token![,]>::parse_terminated;
    let Ok(items) = parser.parse2(tokens) else {
        return (Vec::new(), String::new());
    };
    let mut lints = Vec::new();
    let mut reason = String::new();
    for item in items {
        match item {
            Meta::Path(path) => lints.push(lint_name(&path)),
            Meta::NameValue(pair) if pair.path.is_ident(REASON) => {
                if let Expr::Lit(literal) = &pair.value
                    && let syn::Lit::Str(text) = &literal.lit
                {
                    reason = text.value();
                }
            }
            Meta::NameValue(_) | Meta::List(_) => {}
        }
    }
    (lints, reason)
}

fn expect_arguments(meta: &Meta) -> Option<(Vec<String>, String)> {
    is_expect_path(meta.path())?;
    match meta {
        Meta::List(list) => Some(arguments(list.tokens.clone())),
        Meta::Path(_) | Meta::NameValue(_) => Some((Vec::new(), String::new())),
    }
}

fn outer_attributes(input: ParseStream<'_>) -> syn::Result<(Vec<Attribute>, TokenStream)> {
    let attributes = input.call(Attribute::parse_outer)?;
    let rest: TokenStream = input.parse()?;
    Ok((attributes, rest))
}

fn is_block_like(rest: &TokenStream) -> Option<()> {
    let block_like = match rest.clone().into_iter().next() {
        Some(TokenTree::Group(group)) => group.delimiter() == Delimiter::Brace,
        Some(TokenTree::Ident(ident)) => BLOCK_KEYWORDS.contains(&ident.to_string().as_str()),
        Some(TokenTree::Punct(punct)) => punct.as_char() == '\'',
        Some(TokenTree::Literal(_)) | None => false,
    };
    block_like.then_some(())
}

impl ExpectCollector {
    fn record(&mut self, attributes: &[Attribute], target: Target, statement: Option<&String>) {
        for attribute in attributes {
            let start = attribute.pound_token.span.start();
            let Some((lints, reason)) = expect_arguments(&attribute.meta) else {
                continue;
            };
            if !self.handled.insert((start.line, start.column)) {
                continue;
            }
            let placed = match attribute.style {
                AttrStyle::Inner(_) => Target::Inner,
                AttrStyle::Outer => target,
            };
            self.found.push(Collected {
                line: start.line,
                lints,
                reason,
                target: placed,
                statement: statement.cloned(),
            });
        }
    }

    fn expression_statement(&mut self, expression: &Expr) {
        let Ok((attributes, rest)) = outer_attributes.parse2(source_tokens(expression)) else {
            return;
        };
        let target = if is_block_like(&rest).is_some() {
            Target::Block
        } else {
            Target::Statement
        };
        self.pending = vec![target; attributes.len()];
    }

    fn scan_tokens(&mut self, tokens: TokenStream) {
        let trees: Vec<TokenTree> = tokens.into_iter().collect();
        let mut previous: Option<&TokenTree> = None;
        for tree in &trees {
            if let TokenTree::Group(group) = tree {
                self.scan_group(group, previous);
            }
            previous = Some(tree);
        }
    }

    fn scan_group(&mut self, group: &proc_macro2::Group, previous: Option<&TokenTree>) {
        if group.delimiter() == Delimiter::Bracket && is_pound(previous).is_some() {
            self.bracket_attribute(group);
        }
        self.scan_tokens(group.stream());
    }

    fn bracket_attribute(&mut self, group: &proc_macro2::Group) {
        let Ok(meta) = syn::parse2::<Meta>(group.stream()) else {
            return;
        };
        let Some((lints, reason)) = expect_arguments(&meta) else {
            return;
        };
        let start = group.span_open().start();
        if self.handled.insert((start.line, start.column)) {
            self.found.push(Collected {
                line: start.line,
                lints,
                reason,
                target: Target::Macro,
                statement: None,
            });
        }
    }
}

fn is_pound(tree: Option<&TokenTree>) -> Option<()> {
    match tree {
        Some(TokenTree::Punct(punct)) => (punct.as_char() == '#').then_some(()),
        Some(TokenTree::Group(_) | TokenTree::Ident(_) | TokenTree::Literal(_)) | None => None,
    }
}

fn source_tokens(expression: &Expr) -> TokenStream {
    match expression
        .span()
        .source_text()
        .map(|text| text.parse::<TokenStream>())
    {
        Some(Ok(stream)) => stream,
        Some(Err(_)) | None => TokenStream::new(),
    }
}

impl<'ast> Visit<'ast> for ExpectCollector {
    fn visit_item_fn(&mut self, i: &'ast ItemFn) {
        self.record(&i.attrs, Target::Function, None);
        visit::visit_item_fn(self, i);
    }

    fn visit_impl_item_fn(&mut self, i: &'ast ImplItemFn) {
        self.record(&i.attrs, Target::Function, None);
        visit::visit_impl_item_fn(self, i);
    }

    fn visit_trait_item_fn(&mut self, i: &'ast TraitItemFn) {
        self.record(&i.attrs, Target::Function, None);
        visit::visit_trait_item_fn(self, i);
    }

    fn visit_foreign_item_fn(&mut self, i: &'ast ForeignItemFn) {
        self.record(&i.attrs, Target::Function, None);
        visit::visit_foreign_item_fn(self, i);
    }

    fn visit_local(&mut self, i: &'ast Local) {
        let statement = local_text(i);
        self.record(&i.attrs, Target::Statement, statement.as_ref());
        visit::visit_local(self, i);
    }

    fn visit_stmt(&mut self, i: &'ast Stmt) {
        match i {
            Stmt::Expr(expression, _) => self.expression_statement(expression),
            Stmt::Macro(statement) => self.record(&statement.attrs, Target::Macro, None),
            Stmt::Local(_) | Stmt::Item(_) => {}
        }
        visit::visit_stmt(self, i);
        self.pending.clear();
    }

    fn visit_field(&mut self, i: &'ast Field) {
        self.record(&i.attrs, Target::Field, None);
        visit::visit_field(self, i);
    }

    fn visit_variant(&mut self, i: &'ast Variant) {
        self.record(&i.attrs, Target::Variant, None);
        visit::visit_variant(self, i);
    }

    fn visit_item_struct(&mut self, i: &'ast ItemStruct) {
        self.record(&i.attrs, Target::Item, None);
        visit::visit_item_struct(self, i);
    }

    fn visit_item_enum(&mut self, i: &'ast ItemEnum) {
        self.record(&i.attrs, Target::Item, None);
        visit::visit_item_enum(self, i);
    }

    fn visit_item_union(&mut self, i: &'ast ItemUnion) {
        self.record(&i.attrs, Target::Item, None);
        visit::visit_item_union(self, i);
    }

    fn visit_item_const(&mut self, i: &'ast ItemConst) {
        self.record(&i.attrs, Target::Item, None);
        visit::visit_item_const(self, i);
    }

    fn visit_item_static(&mut self, i: &'ast ItemStatic) {
        self.record(&i.attrs, Target::Item, None);
        visit::visit_item_static(self, i);
    }

    fn visit_item_type(&mut self, i: &'ast ItemType) {
        self.record(&i.attrs, Target::Item, None);
        visit::visit_item_type(self, i);
    }

    fn visit_item_use(&mut self, i: &'ast ItemUse) {
        self.record(&i.attrs, Target::Item, None);
        visit::visit_item_use(self, i);
    }

    fn visit_item_extern_crate(&mut self, i: &'ast ItemExternCrate) {
        self.record(&i.attrs, Target::Item, None);
        visit::visit_item_extern_crate(self, i);
    }

    fn visit_impl_item_const(&mut self, i: &'ast ImplItemConst) {
        self.record(&i.attrs, Target::Item, None);
        visit::visit_impl_item_const(self, i);
    }

    fn visit_impl_item_type(&mut self, i: &'ast ImplItemType) {
        self.record(&i.attrs, Target::Item, None);
        visit::visit_impl_item_type(self, i);
    }

    fn visit_trait_item_const(&mut self, i: &'ast TraitItemConst) {
        self.record(&i.attrs, Target::Item, None);
        visit::visit_trait_item_const(self, i);
    }

    fn visit_trait_item_type(&mut self, i: &'ast TraitItemType) {
        self.record(&i.attrs, Target::Item, None);
        visit::visit_trait_item_type(self, i);
    }

    fn visit_foreign_item_static(&mut self, i: &'ast ForeignItemStatic) {
        self.record(&i.attrs, Target::Item, None);
        visit::visit_foreign_item_static(self, i);
    }

    fn visit_foreign_item_type(&mut self, i: &'ast ForeignItemType) {
        self.record(&i.attrs, Target::Item, None);
        visit::visit_foreign_item_type(self, i);
    }

    fn visit_item_mod(&mut self, i: &'ast ItemMod) {
        self.record(&i.attrs, Target::Module, None);
        visit::visit_item_mod(self, i);
    }

    fn visit_item_impl(&mut self, i: &'ast ItemImpl) {
        self.record(&i.attrs, Target::Impl, None);
        visit::visit_item_impl(self, i);
    }

    fn visit_item_trait(&mut self, i: &'ast ItemTrait) {
        self.record(&i.attrs, Target::Trait, None);
        visit::visit_item_trait(self, i);
    }

    fn visit_item_trait_alias(&mut self, i: &'ast ItemTraitAlias) {
        self.record(&i.attrs, Target::Trait, None);
        visit::visit_item_trait_alias(self, i);
    }

    fn visit_item_foreign_mod(&mut self, i: &'ast ItemForeignMod) {
        self.record(&i.attrs, Target::ForeignBlock, None);
        visit::visit_item_foreign_mod(self, i);
    }

    fn visit_item_macro(&mut self, i: &'ast ItemMacro) {
        self.record(&i.attrs, Target::Macro, None);
        visit::visit_item_macro(self, i);
    }

    fn visit_impl_item_macro(&mut self, i: &'ast ImplItemMacro) {
        self.record(&i.attrs, Target::Macro, None);
        visit::visit_impl_item_macro(self, i);
    }

    fn visit_trait_item_macro(&mut self, i: &'ast TraitItemMacro) {
        self.record(&i.attrs, Target::Macro, None);
        visit::visit_trait_item_macro(self, i);
    }

    fn visit_foreign_item_macro(&mut self, i: &'ast ForeignItemMacro) {
        self.record(&i.attrs, Target::Macro, None);
        visit::visit_foreign_item_macro(self, i);
    }

    fn visit_attribute(&mut self, i: &'ast Attribute) {
        let target = self.pending.pop().unwrap_or(Target::Other);
        self.record(std::slice::from_ref(i), target, None);
        visit::visit_attribute(self, i);
    }

    fn visit_macro(&mut self, i: &'ast Macro) {
        self.scan_tokens(i.tokens.clone());
        visit::visit_macro(self, i);
    }
}
