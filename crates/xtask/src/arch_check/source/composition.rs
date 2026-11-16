mod imports;
#[cfg(test)]
mod tests;

use imports::Bindings;
use proc_macro2::{TokenStream, TokenTree};
use syn::ext::IdentExt;
use syn::parse::Parser;
use syn::visit::{self, Visit};
use syn::{ExprPath, ExprStruct, File, Macro};

const ROOT: &str = "apps/desktop/src-tauri/src/main.rs";
const ADAPTERS: &[&str] = &[
    "broker_toss",
    "broker_toss_trade",
    "sources",
    "ai_claude_code",
    "ai_codex",
    "store_sqlite",
    "secrets_keychain",
    "platform_macos",
];

struct Construction {
    bindings: Bindings,
    found: bool,
}
impl Construction {
    fn path(&mut self, path: &syn::Path) {
        self.segments(
            &path
                .segments
                .iter()
                .map(|segment| segment.ident.unraw().to_string())
                .collect::<Vec<_>>(),
        );
    }
    fn segments(&mut self, segments: &[String]) {
        if self.bindings.adapter(segments) {
            self.found = true;
        }
    }
    fn tokens(&mut self, tokens: TokenStream) {
        let parser = syn::punctuated::Punctuated::<syn::Expr, syn::Token![,]>::parse_terminated;
        if let Ok(expressions) = parser.parse2(tokens.clone()) {
            for expression in expressions {
                self.visit_expr(&expression);
            }
        }
        let mut trees = tokens.into_iter().peekable();
        while let Some(tree) = trees.next() {
            match tree {
                TokenTree::Group(group) => self.tokens(group.stream()),
                TokenTree::Ident(ident) => {
                    self.segments(&token_path(&ident, &mut trees));
                }
                TokenTree::Punct(_) | TokenTree::Literal(_) => {}
            }
        }
    }
}
fn token_path(
    ident: &proc_macro2::Ident,
    trees: &mut std::iter::Peekable<proc_macro2::token_stream::IntoIter>,
) -> Vec<String> {
    let mut path = vec![ident.unraw().to_string()];
    while trees.peek().is_some_and(matches_colon) {
        trees.next();
        if trees.peek().is_some_and(matches_colon) {
            trees.next();
        }
        if let Some(TokenTree::Ident(segment)) = trees.peek() {
            path.push(segment.unraw().to_string());
            trees.next();
        } else {
            break;
        }
    }
    path
}

fn matches_colon(token: &TokenTree) -> bool {
    if let TokenTree::Punct(punctuation) = token {
        punctuation.as_char() == ':'
    } else {
        false
    }
}
impl<'ast> Visit<'ast> for Construction {
    fn visit_expr_path(&mut self, i: &'ast ExprPath) {
        self.path(&i.path);
        if let Some(qself) = &i.qself {
            if let syn::Type::Path(qualified) = qself.ty.as_ref() {
                self.path(&qualified.path);
            }
            if qself.as_token.is_some()
                && i.path.segments.last().is_some_and(|last| {
                    ["new", "try_new", "open", "default", "production"]
                        .contains(&last.ident.unraw().to_string().as_str())
                })
            {
                self.found = true;
            }
        }
        visit::visit_expr_path(self, i);
    }
    fn visit_expr_struct(&mut self, i: &'ast ExprStruct) {
        self.path(&i.path);
        visit::visit_expr_struct(self, i);
    }
    fn visit_item_type(&mut self, i: &'ast syn::ItemType) {
        if let syn::Type::Path(value) = i.ty.as_ref() {
            self.path(&value.path);
            if value.qself.is_some() {
                self.found = true;
            }
        }
        visit::visit_item_type(self, i);
    }
    fn visit_macro(&mut self, i: &'ast Macro) {
        self.tokens(i.tokens.clone());
    }
}

pub(super) fn forbidden(
    relative: &str,
    file: &File,
    aliases: &std::collections::BTreeMap<String, Vec<String>>,
) -> bool {
    if !relative.starts_with("apps/desktop/src-tauri/src/") || relative == ROOT {
        return false;
    }
    let bindings = Bindings::of(file, aliases);
    let mut visitor = Construction {
        found: bindings.prohibited,
        bindings,
    };
    visitor.visit_file(file);
    visitor.found
}
