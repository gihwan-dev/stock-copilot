use std::collections::BTreeSet;

use proc_macro2::{Delimiter, TokenStream, TokenTree};
use syn::ext::IdentExt;
use syn::parse::{ParseStream, Parser};
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::visit::{self, Visit};
use syn::{
    AttrStyle, Attribute, Block, Expr, Field, ForeignItemFn, ForeignItemMacro, ForeignItemStatic,
    ForeignItemType, ImplItemConst, ImplItemFn, ImplItemMacro, ImplItemType, Item, ItemConst,
    ItemEnum, ItemExternCrate, ItemFn, ItemForeignMod, ItemImpl, ItemMacro, ItemMod, ItemStatic,
    ItemStruct, ItemTrait, ItemTraitAlias, ItemType, ItemUnion, ItemUse, Local, Macro, Meta, Stmt,
    Token, TraitItemConst, TraitItemFn, TraitItemMacro, TraitItemType, Variant,
};

use super::allowlist::local_text;
use super::scoped::{Site, TauriImports, function_sites, local_sites};

const EXPECT: &str = "expect";

const REASON: &str = "reason";

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
    NestedItem,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DisableRules {
    pub(crate) attribute_roots: BTreeSet<String>,
    pub(crate) block_keywords: BTreeSet<String>,
}

const fn holds_one_item(target: Target) -> Option<()> {
    match target {
        Target::Function | Target::Statement | Target::Field | Target::Variant | Target::Item => {
            Some(())
        }
        Target::Module
        | Target::Impl
        | Target::Trait
        | Target::ForeignBlock
        | Target::Macro
        | Target::Block
        | Target::Inner
        | Target::Other
        | Target::NestedItem => None,
    }
}

#[derive(Debug, Default)]
struct NestedItems {
    found: Option<()>,
}

impl<'ast> Visit<'ast> for NestedItems {
    fn visit_item(&mut self, i: &'ast Item) {
        if let Item::Use(_) = i {
            return;
        }
        self.found = Some(());
    }

    fn visit_macro(&mut self, i: &'ast Macro) {
        if macro_may_define_items(&i.tokens).is_some() {
            self.found = Some(());
        }
    }
}

fn macro_may_define_items(tokens: &TokenStream) -> Option<()> {
    let mut nested = NestedItems::default();
    if let Ok(file) = syn::parse2::<syn::File>(tokens.clone()) {
        for item in &file.items {
            nested.visit_item(item);
        }
    } else if let Ok(statements) = Block::parse_within.parse2(tokens.clone()) {
        for statement in &statements {
            nested.visit_stmt(statement);
        }
    } else if let Ok(expressions) =
        Punctuated::<Expr, Token![,]>::parse_terminated.parse2(tokens.clone())
    {
        for expression in &expressions {
            nested.visit_expr(expression);
        }
    } else {
        nested.found = Some(());
    }
    nested.found
}

fn nested<F: FnOnce(&mut NestedItems)>(walk: F) -> Option<()> {
    let mut items = NestedItems::default();
    walk(&mut items);
    items.found
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Collected {
    pub(crate) line: usize,
    pub(crate) lints: Vec<String>,
    pub(crate) reason: String,
    pub(crate) target: Target,
    pub(crate) statement: Option<String>,
    pub(crate) sites: BTreeSet<Site>,
}

#[derive(Debug)]
pub(crate) struct ExpectCollector<'rules> {
    rules: &'rules DisableRules,
    handled: BTreeSet<(usize, usize)>,
    pending: Vec<Target>,
    next_sites: BTreeSet<Site>,
    imports: TauriImports,
    pub(crate) found: Vec<Collected>,
}

fn is_expect_path(path: &syn::Path, roots: &BTreeSet<String>) -> Option<()> {
    let segments: Vec<String> = path
        .segments
        .iter()
        .map(|segment| segment.ident.unraw().to_string())
        .collect();
    let expect = match segments.as_slice() {
        [only] => only == EXPECT,
        [root, .., last] => roots.contains(root) && last == EXPECT,
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

fn expect_arguments(meta: &Meta, roots: &BTreeSet<String>) -> Option<(Vec<String>, String)> {
    is_expect_path(meta.path(), roots)?;
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

fn is_block_like(rest: &TokenStream, keywords: &BTreeSet<String>) -> Option<()> {
    let block_like = match rest.clone().into_iter().next() {
        Some(TokenTree::Group(group)) => group.delimiter() == Delimiter::Brace,
        Some(TokenTree::Ident(ident)) => keywords.contains(&ident.to_string()),
        Some(TokenTree::Punct(punct)) => punct.as_char() == '\'',
        Some(TokenTree::Literal(_)) | None => false,
    };
    block_like.then_some(())
}

impl<'rules> ExpectCollector<'rules> {
    pub(crate) fn new(rules: &'rules DisableRules, imports: TauriImports) -> Self {
        Self {
            rules,
            handled: BTreeSet::new(),
            pending: Vec::new(),
            next_sites: BTreeSet::new(),
            imports,
            found: Vec::new(),
        }
    }

    fn record(
        &mut self,
        attributes: &[Attribute],
        target: Target,
        statement: Option<&String>,
        inside: Option<()>,
    ) {
        let sites = self.next_sites.clone();
        self.next_sites.clear();
        let checked = if holds_one_item(target).is_some() && inside.is_some() {
            Target::NestedItem
        } else {
            target
        };
        for attribute in attributes {
            let start = attribute.pound_token.span.start();
            let Some((lints, reason)) =
                expect_arguments(&attribute.meta, &self.rules.attribute_roots)
            else {
                continue;
            };
            if !self.handled.insert((start.line, start.column)) {
                continue;
            }
            let placed = match attribute.style {
                AttrStyle::Inner(_) => Target::Inner,
                AttrStyle::Outer => checked,
            };
            self.found.push(Collected {
                line: start.line,
                lints,
                reason,
                target: placed,
                statement: statement.cloned(),
                sites: sites.clone(),
            });
        }
    }

    fn expression_statement(&mut self, expression: &Expr) {
        let Ok((attributes, rest)) = outer_attributes.parse2(source_tokens(expression)) else {
            return;
        };
        let target = if is_block_like(&rest, &self.rules.block_keywords).is_some() {
            Target::Block
        } else if nested(|items| items.visit_expr(expression)).is_some() {
            Target::NestedItem
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
        let Some((lints, reason)) = expect_arguments(&meta, &self.rules.attribute_roots) else {
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
                sites: BTreeSet::new(),
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

impl<'ast> Visit<'ast> for ExpectCollector<'_> {
    fn visit_item_fn(&mut self, i: &'ast ItemFn) {
        self.next_sites = function_sites(i, &self.imports);
        self.record(
            &i.attrs,
            Target::Function,
            None,
            nested(|items| visit::visit_item_fn(items, i)),
        );
        visit::visit_item_fn(self, i);
    }

    fn visit_impl_item_fn(&mut self, i: &'ast ImplItemFn) {
        self.record(
            &i.attrs,
            Target::Function,
            None,
            nested(|items| visit::visit_impl_item_fn(items, i)),
        );
        visit::visit_impl_item_fn(self, i);
    }

    fn visit_trait_item_fn(&mut self, i: &'ast TraitItemFn) {
        self.record(
            &i.attrs,
            Target::Function,
            None,
            nested(|items| visit::visit_trait_item_fn(items, i)),
        );
        visit::visit_trait_item_fn(self, i);
    }

    fn visit_foreign_item_fn(&mut self, i: &'ast ForeignItemFn) {
        self.record(
            &i.attrs,
            Target::Function,
            None,
            nested(|items| visit::visit_foreign_item_fn(items, i)),
        );
        visit::visit_foreign_item_fn(self, i);
    }

    fn visit_local(&mut self, i: &'ast Local) {
        let statement = local_text(i);
        self.next_sites = local_sites(i);
        self.record(
            &i.attrs,
            Target::Statement,
            statement.as_ref(),
            nested(|items| visit::visit_local(items, i)),
        );
        visit::visit_local(self, i);
    }

    fn visit_stmt(&mut self, i: &'ast Stmt) {
        match i {
            Stmt::Expr(expression, _) => self.expression_statement(expression),
            Stmt::Macro(statement) => self.record(&statement.attrs, Target::Macro, None, None),
            Stmt::Local(_) | Stmt::Item(_) => {}
        }
        visit::visit_stmt(self, i);
        self.pending.clear();
    }

    fn visit_field(&mut self, i: &'ast Field) {
        self.record(
            &i.attrs,
            Target::Field,
            None,
            nested(|items| visit::visit_field(items, i)),
        );
        visit::visit_field(self, i);
    }

    fn visit_variant(&mut self, i: &'ast Variant) {
        self.record(
            &i.attrs,
            Target::Variant,
            None,
            nested(|items| visit::visit_variant(items, i)),
        );
        visit::visit_variant(self, i);
    }

    fn visit_item_struct(&mut self, i: &'ast ItemStruct) {
        self.record(
            &i.attrs,
            Target::Item,
            None,
            nested(|items| visit::visit_item_struct(items, i)),
        );
        visit::visit_item_struct(self, i);
    }

    fn visit_item_enum(&mut self, i: &'ast ItemEnum) {
        self.record(
            &i.attrs,
            Target::Item,
            None,
            nested(|items| visit::visit_item_enum(items, i)),
        );
        visit::visit_item_enum(self, i);
    }

    fn visit_item_union(&mut self, i: &'ast ItemUnion) {
        self.record(
            &i.attrs,
            Target::Item,
            None,
            nested(|items| visit::visit_item_union(items, i)),
        );
        visit::visit_item_union(self, i);
    }

    fn visit_item_const(&mut self, i: &'ast ItemConst) {
        self.record(
            &i.attrs,
            Target::Item,
            None,
            nested(|items| visit::visit_item_const(items, i)),
        );
        visit::visit_item_const(self, i);
    }

    fn visit_item_static(&mut self, i: &'ast ItemStatic) {
        self.record(
            &i.attrs,
            Target::Item,
            None,
            nested(|items| visit::visit_item_static(items, i)),
        );
        visit::visit_item_static(self, i);
    }

    fn visit_item_type(&mut self, i: &'ast ItemType) {
        self.record(
            &i.attrs,
            Target::Item,
            None,
            nested(|items| visit::visit_item_type(items, i)),
        );
        visit::visit_item_type(self, i);
    }

    fn visit_item_use(&mut self, i: &'ast ItemUse) {
        self.record(
            &i.attrs,
            Target::Item,
            None,
            nested(|items| visit::visit_item_use(items, i)),
        );
        visit::visit_item_use(self, i);
    }

    fn visit_item_extern_crate(&mut self, i: &'ast ItemExternCrate) {
        self.record(
            &i.attrs,
            Target::Item,
            None,
            nested(|items| visit::visit_item_extern_crate(items, i)),
        );
        visit::visit_item_extern_crate(self, i);
    }

    fn visit_impl_item_const(&mut self, i: &'ast ImplItemConst) {
        self.record(
            &i.attrs,
            Target::Item,
            None,
            nested(|items| visit::visit_impl_item_const(items, i)),
        );
        visit::visit_impl_item_const(self, i);
    }

    fn visit_impl_item_type(&mut self, i: &'ast ImplItemType) {
        self.record(
            &i.attrs,
            Target::Item,
            None,
            nested(|items| visit::visit_impl_item_type(items, i)),
        );
        visit::visit_impl_item_type(self, i);
    }

    fn visit_trait_item_const(&mut self, i: &'ast TraitItemConst) {
        self.record(
            &i.attrs,
            Target::Item,
            None,
            nested(|items| visit::visit_trait_item_const(items, i)),
        );
        visit::visit_trait_item_const(self, i);
    }

    fn visit_trait_item_type(&mut self, i: &'ast TraitItemType) {
        self.record(
            &i.attrs,
            Target::Item,
            None,
            nested(|items| visit::visit_trait_item_type(items, i)),
        );
        visit::visit_trait_item_type(self, i);
    }

    fn visit_foreign_item_static(&mut self, i: &'ast ForeignItemStatic) {
        self.record(
            &i.attrs,
            Target::Item,
            None,
            nested(|items| visit::visit_foreign_item_static(items, i)),
        );
        visit::visit_foreign_item_static(self, i);
    }

    fn visit_foreign_item_type(&mut self, i: &'ast ForeignItemType) {
        self.record(
            &i.attrs,
            Target::Item,
            None,
            nested(|items| visit::visit_foreign_item_type(items, i)),
        );
        visit::visit_foreign_item_type(self, i);
    }

    fn visit_item_mod(&mut self, i: &'ast ItemMod) {
        self.record(&i.attrs, Target::Module, None, None);
        visit::visit_item_mod(self, i);
    }

    fn visit_item_impl(&mut self, i: &'ast ItemImpl) {
        self.record(&i.attrs, Target::Impl, None, None);
        visit::visit_item_impl(self, i);
    }

    fn visit_item_trait(&mut self, i: &'ast ItemTrait) {
        self.record(&i.attrs, Target::Trait, None, None);
        visit::visit_item_trait(self, i);
    }

    fn visit_item_trait_alias(&mut self, i: &'ast ItemTraitAlias) {
        self.record(&i.attrs, Target::Trait, None, None);
        visit::visit_item_trait_alias(self, i);
    }

    fn visit_item_foreign_mod(&mut self, i: &'ast ItemForeignMod) {
        self.record(&i.attrs, Target::ForeignBlock, None, None);
        visit::visit_item_foreign_mod(self, i);
    }

    fn visit_item_macro(&mut self, i: &'ast ItemMacro) {
        self.record(&i.attrs, Target::Macro, None, None);
        visit::visit_item_macro(self, i);
    }

    fn visit_impl_item_macro(&mut self, i: &'ast ImplItemMacro) {
        self.record(&i.attrs, Target::Macro, None, None);
        visit::visit_impl_item_macro(self, i);
    }

    fn visit_trait_item_macro(&mut self, i: &'ast TraitItemMacro) {
        self.record(&i.attrs, Target::Macro, None, None);
        visit::visit_trait_item_macro(self, i);
    }

    fn visit_foreign_item_macro(&mut self, i: &'ast ForeignItemMacro) {
        self.record(&i.attrs, Target::Macro, None, None);
        visit::visit_foreign_item_macro(self, i);
    }

    fn visit_attribute(&mut self, i: &'ast Attribute) {
        let target = self.pending.pop().unwrap_or(Target::Other);
        self.record(std::slice::from_ref(i), target, None, None);
        visit::visit_attribute(self, i);
    }

    fn visit_macro(&mut self, i: &'ast Macro) {
        self.scan_tokens(i.tokens.clone());
        visit::visit_macro(self, i);
    }
}
