use std::collections::BTreeSet;

use proc_macro2::{TokenStream, TokenTree};
use syn::visit::{self, Visit};
use syn::{
    ItemExternCrate, ItemImpl, ItemType, ItemUse, Macro, Path, UseGlob, UseName, UsePath,
    UseRename, Visibility,
};

use super::super::rules::SourceRules;
use super::super::tokens::flatten;
use super::super::{CrateName, Rule};

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
    fn visit_use_rename(&mut self, i: &'ast UseRename) {
        if i.ident == self.defining {
            self.aliases.insert(i.rename.to_string());
        }
        visit::visit_use_rename(self, i);
    }

    fn visit_item_extern_crate(&mut self, i: &'ast ItemExternCrate) {
        if i.ident == self.defining
            && let Some((_, rename)) = &i.rename
        {
            self.aliases.insert(rename.to_string());
        }
        visit::visit_item_extern_crate(self, i);
    }
}

#[derive(Debug)]
pub(super) struct AstRules<'context> {
    rules: &'context SourceRules,
    crate_name: &'context CrateName,
    presence_paths: BTreeSet<String>,
    prefix: Vec<String>,
    use_exposure: Exposure,
    pub(super) found: BTreeSet<Rule>,
}

fn is_process_command(first: &str, second: &str) -> Option<()> {
    (first == PROCESS_MODULE && second == PROCESS_COMMAND).then_some(())
}

fn collect_idents(trees: &[TokenTree], idents: &mut Vec<String>) {
    for tree in trees {
        match tree {
            TokenTree::Group(group) => collect_idents(&flatten(group), idents),
            TokenTree::Ident(ident) => idents.push(ident.to_string()),
            TokenTree::Punct(_) | TokenTree::Literal(_) => {}
        }
    }
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
            prefix: Vec::new(),
            use_exposure: Exposure::Private,
            found: BTreeSet::new(),
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

    fn last_prefix(&self) -> &str {
        self.prefix.last().map_or("", String::as_str)
    }

    fn exported_name(&mut self, name: &proc_macro2::Ident) {
        if self.use_exposure == Exposure::Exported && *name == self.rules.presence_trait {
            self.found.insert(Rule::UserPresenceReexport);
        }
    }

    fn glob_reexports_presence(&self) -> Option<()> {
        let from_defining_path = self
            .prefix
            .first()
            .is_some_and(|first| self.presence_paths.contains(first));
        let in_defining_crate = self.crate_name.as_str() == self.rules.presence_crate;
        (from_defining_path || in_defining_crate).then_some(())
    }

    fn macro_fallback(&mut self, tokens: &TokenStream) {
        let mut idents = Vec::new();
        collect_idents(&tokens.clone().into_iter().collect::<Vec<_>>(), &mut idents);
        let presence = self.rules.presence_trait.as_str();
        let names_trait = idents.iter().any(|ident| ident == presence);
        if names_trait
            && idents.iter().any(|ident| ident == "impl")
            && self.presence_impl_allowed().is_none()
        {
            self.found.insert(Rule::UserPresenceImpl);
        }
        let pairs: Vec<(&String, &String)> = idents.iter().zip(idents.iter().skip(1)).collect();
        if pairs
            .iter()
            .any(|(first, second)| *first == presence && *second == "as")
        {
            self.found.insert(Rule::UserPresenceAlias);
        }
        if pairs
            .iter()
            .any(|(first, second)| is_process_command(first, second).is_some())
        {
            self.process_used();
        }
    }
}

impl<'ast> Visit<'ast> for AstRules<'_> {
    fn visit_item_use(&mut self, i: &'ast ItemUse) {
        self.use_exposure = exposure(&i.vis);
        if self.use_exposure == Exposure::Exported {
            self.exported_item();
        }
        self.prefix.clear();
        visit::visit_item_use(self, i);
        self.prefix.clear();
        self.use_exposure = Exposure::Private;
    }

    fn visit_use_path(&mut self, i: &'ast UsePath) {
        self.prefix.push(i.ident.to_string());
        visit::visit_use_path(self, i);
        self.prefix.pop();
    }

    fn visit_use_name(&mut self, i: &'ast UseName) {
        self.exported_name(&i.ident);
        if is_process_command(self.last_prefix(), &i.ident.to_string()).is_some() {
            self.process_used();
        }
        visit::visit_use_name(self, i);
    }

    fn visit_use_rename(&mut self, i: &'ast UseRename) {
        self.exported_name(&i.ident);
        if i.ident == self.rules.presence_trait {
            self.found.insert(Rule::UserPresenceAlias);
        }
        let ident = i.ident.to_string();
        if ident == PROCESS_MODULE || is_process_command(self.last_prefix(), &ident).is_some() {
            self.process_used();
        }
        visit::visit_use_rename(self, i);
    }

    fn visit_use_glob(&mut self, i: &'ast UseGlob) {
        if self.use_exposure == Exposure::Exported && self.glob_reexports_presence().is_some() {
            self.found.insert(Rule::UserPresenceReexport);
        }
        if self.last_prefix() == PROCESS_MODULE {
            self.process_used();
        }
        visit::visit_use_glob(self, i);
    }

    fn visit_path(&mut self, i: &'ast Path) {
        let segments: Vec<String> = i
            .segments
            .iter()
            .map(|segment| segment.ident.to_string())
            .collect();
        if segments
            .iter()
            .zip(segments.iter().skip(1))
            .any(|(first, second)| is_process_command(first, second).is_some())
        {
            self.process_used();
        }
        visit::visit_path(self, i);
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
        visit::visit_item_type(self, i);
    }

    fn visit_item_impl(&mut self, i: &'ast ItemImpl) {
        let implements_presence = i
            .trait_
            .as_ref()
            .and_then(|(_, path, _)| path.segments.last())
            .is_some_and(|segment| segment.ident == self.rules.presence_trait);
        if implements_presence && self.presence_impl_allowed().is_none() {
            self.found.insert(Rule::UserPresenceImpl);
        }
        visit::visit_item_impl(self, i);
    }

    fn visit_macro(&mut self, i: &'ast Macro) {
        match syn::parse2::<syn::File>(i.tokens.clone()) {
            Ok(file) => self.visit_file(&file),
            Err(_) => self.macro_fallback(&i.tokens),
        }
        visit::visit_macro(self, i);
    }
}
