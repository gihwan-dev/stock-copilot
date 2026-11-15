use super::super::Rule;
use proc_macro2::{TokenStream, TokenTree};
use std::collections::BTreeSet;
use syn::ext::IdentExt;
use syn::visit::{self, Visit};
use syn::{LitByteStr, LitCStr, LitStr, Path};

const ROOT: &str = "apps/desktop/src-tauri/src/main.rs";
const ENDPOINT: &str = "crates/adapters/broker-toss/src/endpoint.rs";

fn production_reference(tokens: TokenStream) -> bool {
    tokens.into_iter().any(|token| match token {
        TokenTree::Group(group) => production_reference(group.stream()),
        TokenTree::Ident(ident) => ident.unraw() == "production",
        TokenTree::Punct(_) | TokenTree::Literal(_) => false,
    })
}

struct EndpointCheck<'a> {
    relative: &'a str,
    needle: &'a str,
    endpoint_impl: bool,
    constructor: bool,
    creation: bool,
    findings: BTreeSet<Rule>,
}

impl EndpointCheck<'_> {
    fn host(&mut self, value: &str) {
        if value.to_ascii_lowercase().contains(self.needle) {
            self.findings.insert(Rule::TossHost);
        }
    }
    fn literal(&mut self, value: &str) {
        if let Ok(text) = syn::parse_str::<LitStr>(value) {
            self.host(&text.value());
        } else if let Ok(text) = syn::parse_str::<LitByteStr>(value) {
            self.host(&String::from_utf8_lossy(&text.value()));
        } else if let Ok(text) = syn::parse_str::<LitCStr>(value) {
            self.host(&text.value().to_string_lossy());
        }
    }
    fn macro_tokens(&mut self, tokens: TokenStream) {
        for token in tokens {
            match token {
                TokenTree::Group(group) => self.macro_tokens(group.stream()),
                TokenTree::Literal(literal) => self.literal(&literal.to_string()),
                TokenTree::Ident(ident)
                    if self.relative != ROOT && ident.unraw() == "production" =>
                {
                    self.findings.insert(Rule::TossProduction);
                }
                TokenTree::Ident(ident)
                    if self.relative == ENDPOINT
                        && !self.constructor
                        && (ident.unraw() == "PRODUCTION_ORIGIN"
                            || ident.unraw() == "Production") =>
                {
                    self.findings.insert(Rule::TossProduction);
                }
                TokenTree::Ident(_) | TokenTree::Punct(_) => {}
            }
        }
    }
}

impl<'ast> Visit<'ast> for EndpointCheck<'_> {
    fn visit_item(&mut self, i: &'ast syn::Item) {
        let outer = (self.constructor, self.creation);
        self.constructor = false;
        self.creation = false;
        visit::visit_item(self, i);
        (self.constructor, self.creation) = outer;
    }
    fn visit_expr_struct(&mut self, i: &'ast syn::ExprStruct) {
        if self.relative == ENDPOINT
            && !self.creation
            && i.path
                .segments
                .last()
                .is_some_and(|part| part.ident == "TossEndpoint" || part.ident == "Self")
        {
            self.findings.insert(Rule::TossProduction);
        }
        visit::visit_expr_struct(self, i);
    }
    fn visit_item_struct(&mut self, i: &'ast syn::ItemStruct) {
        if self.relative == ENDPOINT
            && i.ident == "TossEndpoint"
            && i.attrs.iter().any(|attr| !debug_derive(attr))
        {
            self.findings.insert(Rule::TossProduction);
        }
        visit::visit_item_struct(self, i);
    }
    fn visit_item_type(&mut self, i: &'ast syn::ItemType) {
        if self.relative == ENDPOINT {
            self.findings.insert(Rule::TossProduction);
        }
        visit::visit_item_type(self, i);
    }
    fn visit_use_rename(&mut self, i: &'ast syn::UseRename) {
        if self.relative == ENDPOINT {
            self.findings.insert(Rule::TossProduction);
        }
        visit::visit_use_rename(self, i);
    }
    fn visit_item_use(&mut self, i: &'ast syn::ItemUse) {
        if self.relative == ENDPOINT && !self.constructor {
            use quote::ToTokens;
            self.macro_tokens(i.to_token_stream());
        }
        visit::visit_item_use(self, i);
    }
    fn visit_item_impl(&mut self, i: &'ast syn::ItemImpl) {
        let outer = self.endpoint_impl;
        self.endpoint_impl = i.trait_.is_none()
            && if let syn::Type::Path(value) = i.self_ty.as_ref() {
                value.path.is_ident("TossEndpoint")
            } else {
                false
            };
        if self.relative == ENDPOINT
            && let syn::Type::Path(value) = i.self_ty.as_ref()
            && value.path.is_ident("TossEndpoint")
            && (i.trait_.is_some() || i.items.iter().any(|item| !closed_endpoint_method(item)))
        {
            self.findings.insert(Rule::TossProduction);
        }
        visit::visit_item_impl(self, i);
        self.endpoint_impl = outer;
    }
    fn visit_impl_item_fn(&mut self, i: &'ast syn::ImplItemFn) {
        let outer = (self.constructor, self.creation);
        self.creation =
            self.endpoint_impl && (i.sig.ident == "production" || i.sig.ident == "local");
        self.constructor = self.endpoint_impl && i.sig.ident.unraw() == "production";
        visit::visit_impl_item_fn(self, i);
        (self.constructor, self.creation) = outer;
    }
    fn visit_lit_str(&mut self, i: &'ast LitStr) {
        self.host(&i.value());
    }
    fn visit_lit_byte_str(&mut self, i: &'ast LitByteStr) {
        self.host(&String::from_utf8_lossy(&i.value()));
    }
    fn visit_lit_cstr(&mut self, i: &'ast LitCStr) {
        self.host(&i.value().to_string_lossy());
    }
    fn visit_macro(&mut self, i: &'ast syn::Macro) {
        self.macro_tokens(i.tokens.clone());
    }
    fn visit_path(&mut self, i: &'ast Path) {
        if self.relative != ROOT
            && i.segments
                .iter()
                .any(|segment| segment.ident.unraw() == "production")
        {
            self.findings.insert(Rule::TossProduction);
        }
        if self.relative == ENDPOINT
            && !self.constructor
            && i.segments.iter().any(|part| {
                part.ident.unraw() == "PRODUCTION_ORIGIN" || part.ident.unraw() == "Production"
            })
        {
            self.findings.insert(Rule::TossProduction);
        }
        visit::visit_path(self, i);
    }
}

fn debug_derive(attr: &syn::Attribute) -> bool {
    if !attr.path().is_ident("derive") {
        return false;
    }
    attr.parse_args_with(syn::punctuated::Punctuated::<Path, syn::Token![,]>::parse_terminated)
        .is_ok_and(|paths| {
            paths.len() == 1 && paths.first().is_some_and(|path| path.is_ident("Debug"))
        })
}

fn closed_endpoint_method(item: &syn::ImplItem) -> bool {
    use quote::ToTokens;
    let syn::ImplItem::Fn(method) = item else {
        return false;
    };
    let signature = method.sig.to_token_stream().to_string();
    [
        "fn production () -> Self",
        "fn local (port : u16) -> Result < Self , TossHttpError >",
        "fn origin (& self) -> & str",
        "const fn mode (& self) -> EndpointMode",
    ]
    .contains(&signature.as_str())
}

fn origin_constant(item: &syn::Item) -> bool {
    let syn::Item::Const(value) = item else {
        return false;
    };
    if value.ident != "PRODUCTION_ORIGIN" {
        return false;
    }
    let syn::Visibility::Inherited = &value.vis else {
        return false;
    };
    let syn::Expr::Lit(expression) = value.expr.as_ref() else {
        return false;
    };
    let syn::Lit::Str(_) = &expression.lit else {
        return false;
    };
    true
}

pub(super) fn check(
    relative: &str,
    file: &syn::File,
    tokens: &TokenStream,
    needle: &str,
) -> BTreeSet<Rule> {
    let mut scan = EndpointCheck {
        relative,
        needle,
        endpoint_impl: false,
        constructor: false,
        creation: false,
        findings: BTreeSet::new(),
    };
    for item in &file.items {
        if relative != ENDPOINT || !origin_constant(item) {
            scan.visit_item(item);
        }
    }
    if relative != ROOT && relative != ENDPOINT && production_reference(tokens.clone()) {
        scan.findings.insert(Rule::TossProduction);
    }
    scan.findings
}

#[cfg(test)]
mod tests {
    use super::{ENDPOINT, ROOT, check};
    use crate::arch_check::Rule;
    use std::collections::BTreeSet;

    fn findings(path: &str, source: &str) -> BTreeSet<Rule> {
        check(
            path,
            &syn::parse_file(&source.replace("HOST", &["tossinvest", ".com"].concat()))
                .expect("parse"),
            &source
                .replace("HOST", &["tossinvest", ".com"].concat())
                .parse()
                .expect("tokens"),
            &["tossinvest", ".com"].concat(),
        )
    }

    #[test]
    fn calls_aliases_and_function_values_require_the_composition_root() {
        for source in [
            "fn f() { let endpoint = broker_toss::TossEndpoint::production(); }",
            "use broker_toss::TossEndpoint as E; fn f() { let endpoint = E::production(); }",
            "fn f() { let factory = broker_toss::TossEndpoint::production; }",
            "use broker_toss as b; fn f() { let factory = b::TossEndpoint::r#production; }",
            "type E = broker_toss::TossEndpoint; fn f() { let factory = E::production; }",
            "fn f() { check!(broker_toss::TossEndpoint::production()); }",
        ] {
            assert_eq!(
                findings("crates/testkit/src/test.rs", source),
                BTreeSet::from([Rule::TossProduction])
            );
            assert!(findings(ROOT, source).is_empty());
        }
        assert!(
            findings(
                "crates/testkit/src/test.rs",
                "fn f() { let endpoint = broker_toss::TossEndpoint::local(1234); }"
            )
            .is_empty()
        );
    }

    #[test]
    fn only_the_named_origin_constant_can_contain_the_host() {
        let correct = "const PRODUCTION_ORIGIN: &str = \"https://openapi.HOST\";";
        assert!(findings(ENDPOINT, correct).is_empty());
        assert_eq!(findings(ROOT, correct), BTreeSet::from([Rule::TossHost]));
        assert_eq!(
            findings(ENDPOINT, "const OTHER: &str = \"https://openapi.HOST\";"),
            BTreeSet::from([Rule::TossHost])
        );
        assert_eq!(
            findings(ENDPOINT, "fn f() { let url = b\"https://openapi.HOST\"; }"),
            BTreeSet::from([Rule::TossHost])
        );
        assert!(
            findings(
                ENDPOINT,
                "impl TossEndpoint { pub fn production() -> Self { Self {} } }"
            )
            .is_empty()
        );
        assert_eq!(
            findings(
                ENDPOINT,
                "fn f() { let factory = TossEndpoint::production; }"
            ),
            BTreeSet::from([Rule::TossProduction])
        );
    }
    #[test]
    fn macros_escapes_c_strings_and_nested_constants_cannot_copy_the_host() {
        for source in [
            "fn f() { log!(\"https://openapi.HOST\"); }",
            "fn f() { let value = c\"https://openapi.HOST\"; }",
            "fn f() { const PRODUCTION_ORIGIN: &str = \"https://openapi.HOST\"; }",
            "pub const PRODUCTION_ORIGIN: &str = \"https://openapi.HOST\";",
        ] {
            assert_eq!(findings(ENDPOINT, source), BTreeSet::from([Rule::TossHost]));
        }
        let escaped = [
            "fn f() { log!(\"https://openapi.",
            "\\x74",
            "ossinvest.com\"); }",
        ]
        .concat();
        assert_eq!(
            findings("crates/testkit/src/test.rs", &escaped),
            BTreeSet::from([Rule::TossHost])
        );
        assert_eq!(
            findings(ENDPOINT, "fn f() { custom!(TossEndpoint::production()); }"),
            BTreeSet::from([Rule::TossProduction])
        );
    }
    #[test]
    fn literal_contract_locations_are_independent_of_the_checker_constants() {
        let reference = "fn f() { let factory = broker_toss::TossEndpoint::production; }";
        assert!(findings("apps/desktop/src-tauri/src/main.rs", reference).is_empty());
        assert_eq!(
            findings("apps/desktop/src-tauri/src/lib.rs", reference),
            BTreeSet::from([Rule::TossProduction])
        );
        let origin = "const PRODUCTION_ORIGIN: &str = \"https://openapi.HOST\";";
        assert!(findings("crates/adapters/broker-toss/src/endpoint.rs", origin).is_empty());
        assert_eq!(
            findings("crates/adapters/broker-toss/src/other.rs", origin),
            BTreeSet::from([Rule::TossHost])
        );
    }

    #[test]
    fn secondary_constructors_and_origin_exports_are_rejected() {
        for source in [
            "impl Default for TossEndpoint { fn default() -> Self { Self { origin: PRODUCTION_ORIGIN.to_owned(), mode: EndpointMode::Production } } }",
            "use self::PRODUCTION_ORIGIN as O; pub fn leaked() -> &'static str { O }",
            "use self::EndpointMode::Production as P; impl Default for TossEndpoint { fn default() -> Self { Self { origin: String::new(), mode: P } } }",
            "pub fn real_origin() -> &'static str { PRODUCTION_ORIGIN }",
            "impl TossEndpoint { pub fn extra() -> Self { Self { origin: PRODUCTION_ORIGIN.to_owned(), mode: EndpointMode::Production } } }",
        ] {
            assert_eq!(
                findings(ENDPOINT, source),
                BTreeSet::from([Rule::TossProduction])
            );
        }
        assert!(findings(ENDPOINT, "impl TossEndpoint { pub fn production() -> Self { Self { origin: PRODUCTION_ORIGIN.to_owned(), mode: EndpointMode::Production } } }").is_empty());
    }
    #[test]
    fn nested_items_and_arbitrary_origin_factories_cannot_inherit_creation_rights() {
        for source in [
            "impl TossEndpoint { pub fn production() -> Self { impl TossEndpoint { pub const MAKE: fn() -> TossEndpoint = || TossEndpoint { origin: PRODUCTION_ORIGIN.to_owned(), mode: EndpointMode::Production }; } Self { origin: PRODUCTION_ORIGIN.to_owned(), mode: EndpointMode::Production } } }",
            "impl TossEndpoint { pub fn any(o: &str) -> Self { Self { origin: o.to_owned(), mode: EndpointMode::Loopback } } }",
            "pub fn any(o: &str) -> TossEndpoint { TossEndpoint { origin: o.to_owned(), mode: EndpointMode::Loopback } }",
            "#[derive(Debug, serde::Deserialize)] pub struct TossEndpoint { origin: String, mode: EndpointMode }",
            "type Other = TossEndpoint;",
            "use self::TossEndpoint as Other;",
            "impl From<&str> for TossEndpoint { fn from(o: &str) -> Self { Self { origin: o.to_owned(), mode: EndpointMode::Loopback } } }",
        ] {
            assert_eq!(
                findings(ENDPOINT, source),
                BTreeSet::from([Rule::TossProduction])
            );
        }
    }
}
