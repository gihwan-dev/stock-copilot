use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use quote::ToTokens;
use syn::{Block, Expr, ExprMethodCall, Item, ItemFn, Stmt, parse_quote};

use crate::check_support::Finding;

use super::{Findings, Rule};

fn function<'a>(source: &'a syn::File, name: &str) -> Option<&'a ItemFn> {
    let mut matches = source.items.iter().filter_map(|item| {
        if let Item::Fn(candidate) = item
            && candidate.sig.ident == name
        {
            Some(candidate)
        } else {
            None
        }
    });
    let found = matches.next()?;
    matches.next().is_none().then_some(found)
}

fn same_tokens(left: &impl ToTokens, right: &impl ToTokens) -> bool {
    left.to_token_stream().to_string() == right.to_token_stream().to_string()
}

fn protected_setup(call: &ExprMethodCall) -> bool {
    if call.method != "setup" || call.args.len() != 1 {
        return false;
    }
    let Some(Expr::Closure(closure)) = call.args.first() else {
        return false;
    };
    let Expr::Block(body) = closure.body.as_ref() else {
        return false;
    };
    let events: Stmt = parse_quote!(ipc::registry::specta_builder::<R>().mount_events(app););
    let security: Stmt = parse_quote!(security::create_windows(app)?;);
    body.block
        .stmts
        .first()
        .is_some_and(|first| same_tokens(first, &events))
        && body
            .block
            .stmts
            .get(1)
            .is_some_and(|second| same_tokens(second, &security))
}

fn protected_build(statement: &Stmt) -> bool {
    let Stmt::Expr(Expr::MethodCall(build), None) = statement else {
        return false;
    };
    if build.method != "build" || build.args.len() != 1 {
        return false;
    }
    let Some(Expr::Path(argument)) = build.args.first() else {
        return false;
    };
    let Expr::MethodCall(setup) = build.receiver.as_ref() else {
        return false;
    };
    argument.path.is_ident("context") && protected_setup(setup)
}

fn assembly_is_protected(source: &syn::File) -> bool {
    let Some(build) = function(source, "build_app") else {
        return false;
    };
    let expected: Stmt = parse_quote! {
        for window in &mut context.config_mut().app.windows {
            window.visible = false;
            window.create = false;
        }
    };
    build.block.stmts.len() == 2
        && build
            .block
            .stmts
            .first()
            .is_some_and(|first| same_tokens(first, &expected))
        && build.block.stmts.last().is_some_and(protected_build)
}

fn security_is_protected(source: &syn::File) -> bool {
    let imports: syn::ItemUse = parse_quote!(
        use tauri::{App, Runtime, Url, WebviewWindowBuilder, webview::NewWindowResponse};
    );
    let navigation: Block = parse_quote!({
        url.scheme() == "tauri"
            && url.host_str() == Some("localhost")
            && url.port().is_none()
            && url.username().is_empty()
            && url.password().is_none()
    });
    let new_window: Block = parse_quote!({ NewWindowResponse::Deny });
    let creation: Block = parse_quote!({
        for config in &app.config().app.windows {
            WebviewWindowBuilder::from_config(app, config)?
                .on_navigation(allows_navigation)
                .on_new_window(|_, _| new_window_response())
                .build()?;
        }
        Ok(())
    });
    source.items.len() == 4
        && source.items.iter().any(|item| {
            if let Item::Use(actual) = item {
                same_tokens(actual, &imports)
            } else {
                false
            }
        })
        && [
            ("allows_navigation", &navigation),
            ("new_window_response", &new_window),
            ("create_windows", &creation),
        ]
        .into_iter()
        .all(|(name, expected)| {
            function(source, name).is_some_and(|item| same_tokens(item.block.as_ref(), expected))
        })
}

fn parsed_file(directory: &Path, relative: &str) -> Option<syn::File> {
    let text = fs::read_to_string(directory.join(relative)).ok()?;
    syn::parse_file(&text).ok()
}

pub(super) fn check_u03_wiring(directory: &Path) -> BTreeSet<Finding<Rule>> {
    let mut findings = Findings {
        items: BTreeSet::new(),
    };
    if !parsed_file(directory, "src/lib.rs").is_some_and(|source| assembly_is_protected(&source)) {
        findings.add("src/lib.rs", Rule::U03Navigation);
    }
    if !parsed_file(directory, "src/security.rs")
        .is_some_and(|source| security_is_protected(&source))
    {
        findings.add("src/security.rs", Rule::U03Navigation);
    }
    findings.items
}
