mod uses;

use std::collections::BTreeSet;

use proc_macro2::{TokenStream, TokenTree};
use syn::parse::Parser;
use syn::punctuated::Punctuated;
use syn::visit::{self, Visit};
use syn::{
    Expr, ExprCall, ExprMethodCall, ExprPath, ExprStruct, ImplItemConst, ImplItemFn, ImplItemType,
    ItemConst, ItemEnum, ItemExternCrate, ItemFn, ItemImpl, ItemStatic, ItemStruct, ItemTrait,
    ItemType, ItemUnion, ItemUse, LitByteStr, LitStr, Macro, Path, Token, TypePath, Visibility,
};

use super::super::rules::SourceRules;
use super::super::tokens::flatten;
use super::super::{CrateName, Rule};
use uses::{TypeNames, UseEntry, UseKind, has_pair, starts_with, unraw, use_entries};

const PROCESS_MODULE: &str = "process";
const PROCESS_COMMAND: &str = "Command";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Exposure {
    Private,
    Exported,
}

const fn exposure(visibility: &Visibility) -> Exposure {
    match visibility {
        Visibility::Inherited => Exposure::Private,
        Visibility::Public(_) | Visibility::Restricted(_) => Exposure::Exported,
    }
}

#[derive(Debug, Default)]
struct CrateAliases {
    defining: String,
    aliases: BTreeSet<String>,
}

impl<'ast> Visit<'ast> for CrateAliases {
    fn visit_item_use(&mut self, i: &'ast ItemUse) {
        for entry in use_entries(&i.tree) {
            if let UseKind::Rename(alias) = &entry.kind
                && entry.path == [self.defining.clone()]
            {
                self.aliases.insert(alias.clone());
            }
        }
    }

    fn visit_item_extern_crate(&mut self, i: &'ast ItemExternCrate) {
        if unraw(&i.ident) == self.defining
            && let Some((_, rename)) = &i.rename
        {
            self.aliases.insert(unraw(rename));
        }
        visit::visit_item_extern_crate(self, i);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ImplContext {
    Inherent,
    Trait,
}

#[derive(Debug, Default)]
pub(super) struct Hits {
    pub(super) write: Vec<String>,
    pub(super) socket: Vec<String>,
    pub(super) export: Vec<String>,
}

#[derive(Debug)]
pub(super) struct AstRules<'context> {
    rules: &'context SourceRules,
    crate_name: &'context CrateName,
    presence_paths: BTreeSet<String>,
    socket_modules: Vec<Vec<String>>,
    impl_context: ImplContext,
    pub(super) found: BTreeSet<Rule>,
    pub(super) hits: Hits,
}

fn collect_idents(trees: &[TokenTree], idents: &mut Vec<String>, literals: &mut Vec<String>) {
    for tree in trees {
        match tree {
            TokenTree::Group(group) => collect_idents(&flatten(group), idents, literals),
            TokenTree::Ident(ident) => idents.push(unraw(ident)),
            TokenTree::Literal(literal) => literals.push(literal.to_string()),
            TokenTree::Punct(_) => {}
        }
    }
}

fn segment_names(path: &Path) -> Vec<String> {
    path.segments
        .iter()
        .map(|segment| unraw(&segment.ident))
        .collect()
}

impl<'context> AstRules<'context> {
    pub(super) fn new(
        rules: &'context SourceRules,
        crate_name: &'context CrateName,
        file: &syn::File,
    ) -> Self {
        let defining = rules.presence_crate.replace('-', "_");
        let mut aliases = CrateAliases {
            defining: defining.clone(),
            aliases: BTreeSet::new(),
        };
        aliases.visit_file(file);
        let mut presence_paths = aliases.aliases;
        presence_paths.insert(defining);
        Self {
            rules,
            crate_name,
            presence_paths,
            socket_modules: rules
                .socket_modules
                .iter()
                .map(|module| module.split("::").map(String::from).collect())
                .collect(),
            impl_context: ImplContext::Inherent,
            found: BTreeSet::new(),
            hits: Hits::default(),
        }
    }

    fn presence_impl_allowed(&self) -> Option<()> {
        self.rules
            .presence_impl_crates
            .contains(self.crate_name)
            .then_some(())
    }

    fn exported_item(&mut self) {
        if self.rules.reexport_crates.contains(self.crate_name) {
            self.found.insert(Rule::Reexport);
        }
    }

    fn process_used(&mut self) {
        if !self.rules.process_crates.contains(self.crate_name) {
            self.found.insert(Rule::ProcessSpawn);
        }
    }

    fn write_name(&mut self, name: &str) {
        if self.rules.write_means.iter().any(|means| means == name) {
            self.hits.write.push(String::from(name));
        }
    }

    fn socket_name(&mut self, name: &str) {
        if self
            .rules
            .socket_identifiers
            .iter()
            .any(|means| means == name)
        {
            self.hits.socket.push(String::from(name));
        }
    }

    fn means_name(&mut self, name: &str) {
        self.write_name(name);
        self.socket_name(name);
    }

    fn socket_path(&mut self, path: &[String]) {
        if self
            .socket_modules
            .iter()
            .any(|module| starts_with(path, module).is_some())
        {
            self.hits.socket.push(path.join("::"));
        }
    }

    fn glob_reexports_presence(&self, path: &[String]) -> Option<()> {
        let from_defining_path = path
            .first()
            .is_some_and(|first| self.presence_paths.contains(first));
        let in_defining_crate = self.crate_name.as_str() == self.rules.presence_crate;
        (from_defining_path || in_defining_crate).then_some(())
    }

    fn signature_names(&mut self, names: &TypeNames) {
        let leaked: Vec<String> = names
            .names
            .iter()
            .filter(|name| self.rules.write_means.contains(name))
            .cloned()
            .collect();
        self.hits.export.extend(leaked);
    }

    fn exported_signature<F: FnOnce(&mut TypeNames)>(
        &mut self,
        visibility: &Visibility,
        visit_types: F,
    ) {
        let exported =
            exposure(visibility) == Exposure::Exported || self.impl_context == ImplContext::Trait;
        if exported {
            let mut names = TypeNames::default();
            visit_types(&mut names);
            self.signature_names(&names);
        }
    }

    fn exported_glob(&mut self, entry: &UseEntry) {
        self.hits
            .export
            .push(format!("{}::*", entry.path.join("::")));
        if self.glob_reexports_presence(&entry.path).is_some() {
            self.found.insert(Rule::UserPresenceReexport);
        }
    }

    fn exported_name(&mut self, imported: &str) {
        if imported == self.rules.presence_trait {
            self.found.insert(Rule::UserPresenceReexport);
        }
        if self.rules.write_means.iter().any(|means| means == imported) {
            self.hits.export.push(String::from(imported));
        }
    }

    fn use_entry(&mut self, entry: &UseEntry, exposed: Exposure) {
        let imported = String::from(entry.imported());
        if has_pair(&entry.path, PROCESS_MODULE, PROCESS_COMMAND).is_some() {
            self.process_used();
        }
        self.socket_path(&entry.path);
        match (&entry.kind, exposed) {
            (UseKind::Glob, _) if imported == PROCESS_MODULE => self.process_used(),
            (UseKind::Glob, Exposure::Private) | (UseKind::Name, _) => {}
            (UseKind::Rename(_), _) => {
                if imported == PROCESS_MODULE {
                    self.process_used();
                }
                if imported == self.rules.presence_trait {
                    self.found.insert(Rule::UserPresenceAlias);
                }
            }
            (UseKind::Glob, Exposure::Exported) => self.exported_glob(entry),
        }
        match (&entry.kind, exposed) {
            (UseKind::Glob, _) => {}
            (UseKind::Name | UseKind::Rename(_), Exposure::Exported) => {
                self.means_name(&imported);
                self.exported_name(&imported);
            }
            (UseKind::Name | UseKind::Rename(_), Exposure::Private) => self.means_name(&imported),
        }
    }

    fn macro_fallback(&mut self, tokens: &TokenStream) {
        let mut idents = Vec::new();
        let mut literals = Vec::new();
        collect_idents(
            &tokens.clone().into_iter().collect::<Vec<_>>(),
            &mut idents,
            &mut literals,
        );
        let presence = self.rules.presence_trait.as_str();
        if idents.iter().any(|ident| ident == presence)
            && idents.iter().any(|ident| ident == "impl")
            && self.presence_impl_allowed().is_none()
        {
            self.found.insert(Rule::UserPresenceImpl);
        }
        if has_pair(&idents, presence, "as").is_some() {
            self.found.insert(Rule::UserPresenceAlias);
        }
        if has_pair(&idents, PROCESS_MODULE, PROCESS_COMMAND).is_some() {
            self.process_used();
        }
        for ident in &idents {
            self.means_name(ident);
        }
        for literal in &literals {
            if let Ok(text) = syn::parse_str::<LitStr>(literal) {
                self.write_name(&text.value());
            }
        }
    }
}

impl<'ast> Visit<'ast> for AstRules<'_> {
    fn visit_item_use(&mut self, i: &'ast ItemUse) {
        let exposed = exposure(&i.vis);
        if exposed == Exposure::Exported {
            self.exported_item();
        }
        for entry in use_entries(&i.tree) {
            self.use_entry(&entry, exposed);
        }
    }

    fn visit_path(&mut self, i: &'ast Path) {
        let names = segment_names(i);
        if has_pair(&names, PROCESS_MODULE, PROCESS_COMMAND).is_some() {
            self.process_used();
        }
        if names.len() > 1 {
            for name in &names {
                self.means_name(name);
            }
            self.socket_path(&names);
        }
        visit::visit_path(self, i);
    }

    fn visit_type_path(&mut self, i: &'ast TypePath) {
        for name in segment_names(&i.path) {
            self.means_name(&name);
        }
        visit::visit_type_path(self, i);
    }

    fn visit_expr_struct(&mut self, i: &'ast ExprStruct) {
        for name in segment_names(&i.path) {
            self.means_name(&name);
        }
        visit::visit_expr_struct(self, i);
    }

    fn visit_expr_call(&mut self, i: &'ast ExprCall) {
        if let Expr::Path(ExprPath { path, .. }) = &*i.func {
            for name in segment_names(path) {
                self.means_name(&name);
            }
        }
        visit::visit_expr_call(self, i);
    }

    fn visit_expr_method_call(&mut self, i: &'ast ExprMethodCall) {
        self.means_name(&unraw(&i.method));
        visit::visit_expr_method_call(self, i);
    }

    fn visit_lit_str(&mut self, i: &'ast LitStr) {
        self.write_name(&i.value());
        visit::visit_lit_str(self, i);
    }

    fn visit_lit_byte_str(&mut self, i: &'ast LitByteStr) {
        self.write_name(&String::from_utf8_lossy(&i.value()));
        visit::visit_lit_byte_str(self, i);
    }

    fn visit_item_extern_crate(&mut self, i: &'ast ItemExternCrate) {
        if exposure(&i.vis) == Exposure::Exported {
            self.exported_item();
        }
        visit::visit_item_extern_crate(self, i);
    }

    fn visit_item_type(&mut self, i: &'ast ItemType) {
        if exposure(&i.vis) == Exposure::Exported {
            self.exported_item();
        }
        self.exported_signature(&i.vis, |names| names.visit_type(&i.ty));
        visit::visit_item_type(self, i);
    }

    fn visit_item_fn(&mut self, i: &'ast ItemFn) {
        self.exported_signature(&i.vis, |names| names.visit_signature(&i.sig));
        let outer = self.impl_context;
        self.impl_context = ImplContext::Inherent;
        visit::visit_item_fn(self, i);
        self.impl_context = outer;
    }

    fn visit_item_struct(&mut self, i: &'ast ItemStruct) {
        self.exported_signature(&i.vis, |names| names.visit_item_struct(i));
        visit::visit_item_struct(self, i);
    }

    fn visit_item_enum(&mut self, i: &'ast ItemEnum) {
        self.exported_signature(&i.vis, |names| names.visit_item_enum(i));
        visit::visit_item_enum(self, i);
    }

    fn visit_item_union(&mut self, i: &'ast ItemUnion) {
        self.exported_signature(&i.vis, |names| names.visit_item_union(i));
        visit::visit_item_union(self, i);
    }

    fn visit_item_const(&mut self, i: &'ast ItemConst) {
        self.exported_signature(&i.vis, |names| names.visit_type(&i.ty));
        visit::visit_item_const(self, i);
    }

    fn visit_item_static(&mut self, i: &'ast ItemStatic) {
        self.exported_signature(&i.vis, |names| names.visit_type(&i.ty));
        visit::visit_item_static(self, i);
    }

    fn visit_item_trait(&mut self, i: &'ast ItemTrait) {
        self.exported_signature(&i.vis, |names| names.visit_item_trait(i));
        visit::visit_item_trait(self, i);
    }

    fn visit_impl_item_fn(&mut self, i: &'ast ImplItemFn) {
        self.exported_signature(&i.vis, |names| names.visit_signature(&i.sig));
        visit::visit_impl_item_fn(self, i);
    }

    fn visit_impl_item_const(&mut self, i: &'ast ImplItemConst) {
        self.exported_signature(&i.vis, |names| names.visit_type(&i.ty));
        visit::visit_impl_item_const(self, i);
    }

    fn visit_impl_item_type(&mut self, i: &'ast ImplItemType) {
        self.exported_signature(&i.vis, |names| names.visit_type(&i.ty));
        visit::visit_impl_item_type(self, i);
    }

    fn visit_item_impl(&mut self, i: &'ast ItemImpl) {
        let implements_presence = i
            .trait_
            .as_ref()
            .and_then(|(_, path, _)| path.segments.last())
            .is_some_and(|segment| unraw(&segment.ident) == self.rules.presence_trait);
        if implements_presence && self.presence_impl_allowed().is_none() {
            self.found.insert(Rule::UserPresenceImpl);
        }
        let outer = self.impl_context;
        self.impl_context = match i.trait_ {
            Some(_) => ImplContext::Trait,
            None => ImplContext::Inherent,
        };
        visit::visit_item_impl(self, i);
        self.impl_context = outer;
    }

    fn visit_macro(&mut self, i: &'ast Macro) {
        if let Ok(file) = syn::parse2::<syn::File>(i.tokens.clone()) {
            self.visit_file(&file);
        } else if let Ok(expressions) =
            Punctuated::<Expr, Token![,]>::parse_terminated.parse2(i.tokens.clone())
        {
            for expression in &expressions {
                self.visit_expr(expression);
            }
        } else {
            self.macro_fallback(&i.tokens);
        }
        visit::visit_macro(self, i);
    }
}
