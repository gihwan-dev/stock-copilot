use super::super::Rule;
use proc_macro2::{TokenStream, TokenTree};
use std::collections::BTreeSet;
use syn::ext::IdentExt;
use syn::visit::{self, Visit};
use syn::{LitByteStr, LitCStr, LitStr, Path};

const ROOT: &str = "apps/desktop/src-tauri/src/main.rs";
const SMOKE_ROOT: &str = "crates/toss-smoke/src/main.rs";
const ENDPOINT: &str = "crates/adapters/broker-toss/src/endpoint.rs";
const HTTP: &str = "crates/adapters/broker-toss/src/http.rs";

fn is_root(relative: &str) -> bool {
    relative == ROOT || relative == SMOKE_ROOT
}

fn production_reference(tokens: TokenStream) -> bool {
    tokens.into_iter().any(|token| match token {
        TokenTree::Group(group) => production_reference(group.stream()),
        TokenTree::Ident(ident) => ident.unraw() == "production",
        TokenTree::Punct(_) | TokenTree::Literal(_) => false,
    })
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Target {
    Other,
    Endpoint,
    Client,
    WsConnector,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Factory {
    Forbidden,
    Endpoint,
    Client,
    WsConnector,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Scope {
    Product,
    Test,
}

struct EndpointCheck<'a> {
    relative: &'a str,
    needle: &'a str,
    target: Target,
    constructor: bool,
    factory: Factory,
    scope: Scope,
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
                    if !is_root(self.relative) && ident.unraw() == "production" =>
                {
                    self.findings.insert(Rule::TossProduction);
                }
                TokenTree::Ident(ident)
                    if self.relative == ENDPOINT
                        && !self.constructor
                        && (ident.unraw() == "PRODUCTION_ORIGIN"
                            || ident.unraw() == "PRODUCTION_WS_ORIGIN"
                            || ident.unraw() == "Production") =>
                {
                    self.findings.insert(Rule::TossProduction);
                }
                TokenTree::Ident(ident)
                    if (self.relative == ENDPOINT || self.relative == HTTP)
                        && [
                            "TossEndpoint",
                            "TossWsEndpoint",
                            "TossHttpClient",
                            "TossWsConnector",
                            "Self",
                        ]
                        .iter()
                        .any(|name| ident.unraw() == name) =>
                {
                    self.findings.insert(Rule::TossProduction);
                }
                TokenTree::Ident(ident)
                    if (self.relative == ENDPOINT || self.relative == HTTP)
                        && ([
                            "impl", "struct", "mod", "type", "const", "static", "fn", "use",
                        ]
                        .iter()
                        .any(|name| ident.unraw() == name)
                            || (self.scope == Scope::Product
                                && ["origin", "mode", "client"]
                                    .iter()
                                    .any(|name| ident.unraw() == name))) =>
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
        let outer = (self.constructor, self.factory);
        self.constructor = false;
        self.factory = Factory::Forbidden;
        visit::visit_item(self, i);
        (self.constructor, self.factory) = outer;
    }
    fn visit_expr_struct(&mut self, i: &'ast syn::ExprStruct) {
        let endpoint = i.path.segments.last().is_some_and(|part| {
            part.ident.unraw() == "TossEndpoint"
                || part.ident.unraw() == "TossWsEndpoint"
                || (part.ident.unraw() == "Self" && (self.target == Target::Endpoint))
        });
        let client = i.path.segments.last().is_some_and(|part| {
            part.ident.unraw() == "TossHttpClient"
                || (part.ident.unraw() == "Self" && (self.target == Target::Client))
        });
        let ws_connector = i.path.segments.last().is_some_and(|part| {
            part.ident.unraw() == "TossWsConnector"
                || (part.ident.unraw() == "Self" && (self.target == Target::WsConnector))
        });
        let hidden = (self.relative == ENDPOINT || self.relative == HTTP)
            && self.factory == Factory::Forbidden
            && i.fields.iter().any(|field| {
                if let syn::Member::Named(name) = &field.member {
                    ["origin", "mode", "client"]
                        .iter()
                        .any(|value| name.unraw() == value)
                } else {
                    false
                }
            });
        if hidden
            || (endpoint && !(self.relative == ENDPOINT && (self.factory == Factory::Endpoint)))
            || (client && !(self.relative == HTTP && (self.factory == Factory::Client)))
            || (ws_connector && !(self.relative == HTTP && (self.factory == Factory::WsConnector)))
        {
            self.findings.insert(Rule::TossProduction);
        }
        visit::visit_expr_struct(self, i);
    }
    fn visit_item_mod(&mut self, i: &'ast syn::ItemMod) {
        let outer = self.scope;
        if self.relative == ENDPOINT || self.relative == HTTP {
            use quote::ToTokens;
            if i.ident.unraw() != "tests"
                || !private(&i.vis)
                || i.content.is_none()
                || i.attrs.len() != 1
                || i.attrs
                    .first()
                    .is_none_or(|attr| attr.meta.to_token_stream().to_string() != "cfg (test)")
                || outer == Scope::Test
            {
                self.findings.insert(Rule::TossProduction);
            }
            self.scope = Scope::Test;
        }
        visit::visit_item_mod(self, i);
        self.scope = outer;
    }
    fn visit_item_fn(&mut self, i: &'ast syn::ItemFn) {
        if self.scope == Scope::Product
            && (self.relative == ENDPOINT
                || (self.relative == HTTP
                    && ![
                        "sensitive_header",
                        "header",
                        "classify_status",
                        "configured_client",
                        "configured_ws_client",
                        "validate_loopback",
                        "receive",
                    ]
                    .iter()
                    .any(|name| i.sig.ident == name)))
        {
            self.findings.insert(Rule::TossProduction);
        }
        visit::visit_item_fn(self, i);
    }
    fn visit_item_struct(&mut self, i: &'ast syn::ItemStruct) {
        if self.relative == ENDPOINT
            && (i.ident.unraw() == "TossEndpoint" || i.ident.unraw() == "TossWsEndpoint")
            && i.attrs.iter().any(|attr| !debug_derive(attr))
        {
            self.findings.insert(Rule::TossProduction);
        }
        if (i.ident.unraw() == "TossEndpoint"
            || i.ident.unraw() == "TossWsEndpoint"
            || i.ident.unraw() == "TossHttpClient"
            || i.ident.unraw() == "TossWsConnector")
            && i.fields.iter().any(|field| !private(&field.vis))
        {
            self.findings.insert(Rule::TossProduction);
        }
        visit::visit_item_struct(self, i);
    }
    fn visit_item_type(&mut self, i: &'ast syn::ItemType) {
        if self.relative == ENDPOINT || self.relative == HTTP {
            self.findings.insert(Rule::TossProduction);
        }
        visit::visit_item_type(self, i);
    }
    fn visit_use_rename(&mut self, i: &'ast syn::UseRename) {
        if self.relative == ENDPOINT || self.relative == HTTP {
            self.findings.insert(Rule::TossProduction);
        }
        visit::visit_use_rename(self, i);
    }
    fn visit_use_name(&mut self, i: &'ast syn::UseName) {
        if self.relative == ENDPOINT
            && (i.ident.unraw() == "PRODUCTION_ORIGIN"
                || i.ident.unraw() == "PRODUCTION_WS_ORIGIN"
                || i.ident.unraw() == "Production")
        {
            self.findings.insert(Rule::TossProduction);
        }
        visit::visit_use_name(self, i);
    }
    fn visit_item_impl(&mut self, i: &'ast syn::ItemImpl) {
        let outer = self.target;
        let endpoint =
            type_named(&i.self_ty, "TossEndpoint") || type_named(&i.self_ty, "TossWsEndpoint");
        let client = type_named(&i.self_ty, "TossHttpClient");
        let ws_connector = type_named(&i.self_ty, "TossWsConnector");
        self.target = if i.trait_.is_some() {
            Target::Other
        } else if endpoint {
            Target::Endpoint
        } else if client {
            Target::Client
        } else if ws_connector {
            Target::WsConnector
        } else {
            Target::Other
        };
        if endpoint
            && (self.relative != ENDPOINT
                || i.trait_.is_some()
                || i.items.iter().any(|item| !closed_endpoint_method(item)))
        {
            self.findings.insert(Rule::TossProduction);
        }
        if client
            && (self.relative != HTTP
                || match &i.trait_ {
                    Some((_, path, _)) => path
                        .segments
                        .last()
                        .is_none_or(|part| part.ident != "Debug"),
                    None => i.items.iter().any(|item| !closed_client_method(item)),
                })
        {
            self.findings.insert(Rule::TossProduction);
        }
        if ws_connector
            && (self.relative != HTTP
                || match &i.trait_ {
                    Some((_, path, _)) => path
                        .segments
                        .last()
                        .is_none_or(|part| part.ident != "Debug"),
                    None => i.items.iter().any(|item| !closed_ws_connector_method(item)),
                })
        {
            self.findings.insert(Rule::TossProduction);
        }
        visit::visit_item_impl(self, i);
        self.target = outer;
    }
    fn visit_impl_item_fn(&mut self, i: &'ast syn::ImplItemFn) {
        let outer = (self.constructor, self.factory);
        self.factory = if self.target == Target::Endpoint
            && (i.sig.ident == "production" || i.sig.ident == "local")
        {
            Factory::Endpoint
        } else if self.target == Target::Client && i.sig.ident == "new" {
            Factory::Client
        } else if self.target == Target::WsConnector && i.sig.ident == "new" {
            Factory::WsConnector
        } else {
            Factory::Forbidden
        };
        self.constructor = self.target == Target::Endpoint && i.sig.ident.unraw() == "production";
        visit::visit_impl_item_fn(self, i);
        (self.constructor, self.factory) = outer;
    }
    fn visit_expr_field(&mut self, i: &'ast syn::ExprField) {
        if self.scope == Scope::Product
            && let syn::Member::Named(name) = &i.member
            && ((self.relative == ENDPOINT
                && self.target != Target::Endpoint
                && (name.unraw() == "origin" || name.unraw() == "mode"))
                || (self.relative == HTTP
                    && self.target != Target::Client
                    && self.target != Target::WsConnector
                    && (name.unraw() == "origin" || name.unraw() == "client")))
        {
            self.findings.insert(Rule::TossProduction);
        }
        visit::visit_expr_field(self, i);
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
        if self.scope == Scope::Product
            && (self.relative == ENDPOINT || self.relative == HTTP)
            && i.path
                .segments
                .last()
                .is_none_or(|part| part.ident.unraw() != "format")
        {
            self.findings.insert(Rule::TossProduction);
        }
        self.macro_tokens(i.tokens.clone());
    }
    fn visit_path(&mut self, i: &'ast Path) {
        if !is_root(self.relative)
            && i.segments
                .iter()
                .any(|segment| segment.ident.unraw() == "production")
        {
            self.findings.insert(Rule::TossProduction);
        }
        if self.relative == ENDPOINT
            && !self.constructor
            && i.segments.iter().any(|part| {
                part.ident.unraw() == "PRODUCTION_ORIGIN"
                    || part.ident.unraw() == "PRODUCTION_WS_ORIGIN"
                    || part.ident.unraw() == "Production"
            })
        {
            self.findings.insert(Rule::TossProduction);
        }
        visit::visit_path(self, i);
    }
}

fn private(vis: &syn::Visibility) -> bool {
    match vis {
        syn::Visibility::Inherited => true,
        syn::Visibility::Public(_) | syn::Visibility::Restricted(_) => false,
    }
}

fn type_named(ty: &syn::Type, name: &str) -> bool {
    if let syn::Type::Path(value) = ty {
        value
            .path
            .segments
            .last()
            .is_some_and(|part| part.ident.unraw() == name)
    } else {
        false
    }
}

fn closed_client_method(item: &syn::ImplItem) -> bool {
    use quote::ToTokens;
    let syn::ImplItem::Fn(method) = item else {
        return false;
    };
    let mut actual = method.sig.clone();
    actual.inputs = actual.inputs.into_iter().collect();
    [
        "fn new(endpoint: &TossEndpoint, clock: Arc<dyn Clock>, wait: Arc<dyn crate::RequestWait>) -> Result<Self, TossHttpError> {}",
        "async fn get(&self, request: &ReadRequest, token: &SecretValue) -> Result<RawResponse, TossHttpError> {}",
        "async fn send_token(&self, body: SecretValue) -> Result<RawResponse, TossHttpError> {}",
        "async fn get_public_doc(&self, route: PublicDocsRoute) -> Result<RawResponse, TossHttpError> {}",
    ].iter().any(|text| syn::parse_str::<syn::ImplItemFn>(text).is_ok_and(|expected| expected.sig.to_token_stream().to_string() == actual.to_token_stream().to_string()))
}

fn closed_ws_connector_method(item: &syn::ImplItem) -> bool {
    use quote::ToTokens;
    let syn::ImplItem::Fn(method) = item else {
        return false;
    };
    let mut actual = method.sig.clone();
    actual.inputs = actual.inputs.into_iter().collect();
    [
        "fn new(endpoint: &TossWsEndpoint) -> Result<Self, TossHttpError> {}",
        "async fn open_upgrade(&self, token: &SecretValue, ws_key: &str, expected_accept: &str) -> Result<WsUpgradeOutcome, TossHttpError> {}",
    ]
    .iter()
    .any(|text| {
        syn::parse_str::<syn::ImplItemFn>(text).is_ok_and(|expected| {
            expected.sig.to_token_stream().to_string() == actual.to_token_stream().to_string()
        })
    })
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
    if value.ident != "PRODUCTION_ORIGIN" && value.ident != "PRODUCTION_WS_ORIGIN" {
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
        target: Target::Other,
        constructor: false,
        factory: Factory::Forbidden,
        scope: Scope::Product,
        findings: BTreeSet::new(),
    };
    if relative.starts_with("crates/adapters/broker-toss/src/endpoint/")
        || relative.starts_with("crates/adapters/broker-toss/src/http/")
    {
        scan.findings.insert(Rule::TossProduction);
    }
    for item in &file.items {
        if relative != ENDPOINT || !origin_constant(item) {
            scan.visit_item(item);
        }
    }
    if !is_root(relative) && relative != ENDPOINT && production_reference(tokens.clone()) {
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
            BTreeSet::from([Rule::TossHost, Rule::TossProduction])
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
        ] {
            assert_eq!(
                findings(ENDPOINT, source),
                BTreeSet::from([Rule::TossHost, Rule::TossProduction])
            );
        }
        assert_eq!(
            findings(
                ENDPOINT,
                "pub const PRODUCTION_ORIGIN: &str = \"https://openapi.HOST\";"
            ),
            BTreeSet::from([Rule::TossHost])
        );
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
    #[test]
    fn child_modules_retargeting_and_qualified_factories_are_rejected() {
        for source in [
            "mod sneak;",
            "pub const RETARGET: fn(&mut TossEndpoint, String) = |e, o| e.origin = o;",
            "mod sneak { impl super::TossEndpoint { pub fn any(o: &str) -> Self { Self { origin: o.to_owned(), mode: super::EndpointMode::Production } } } }",
            "pub fn retarget(e: &mut TossEndpoint, o: &str) { e.origin = o.to_owned(); }",
            "impl self::TossEndpoint { pub fn any(o: &str) -> Self { Self { origin: o.to_owned(), mode: EndpointMode::Loopback } } }",
            "impl self::r#TossEndpoint { pub fn any(o: &str) -> Self { Self { origin: o.to_owned(), mode: EndpointMode::Loopback } } }",
            "impl crate::E { pub fn any(o: &str) -> Self { Self { origin: o.to_owned(), mode: mode() } } }",
            "std::thread_local! { pub static RETARGET: fn(&mut TossEndpoint, String) = |e, o| e.origin = o; }",
        ] {
            assert_eq!(
                findings(ENDPOINT, source),
                BTreeSet::from([Rule::TossProduction])
            );
        }
        assert_eq!(
            findings(
                "crates/adapters/broker-toss/src/endpoint/sneak.rs",
                "impl super::TossEndpoint { pub fn any(o: &str) -> Self { Self { origin: o.to_owned(), mode: super::EndpointMode::Production } } }"
            ),
            BTreeSet::from([Rule::TossProduction])
        );
    }
    #[test]
    fn client_construction_is_private_and_requires_the_canonical_constructor() {
        let http = "crates/adapters/broker-toss/src/http.rs";
        for source in [
            "impl TossHttpClient { pub fn from_url(u: &str) -> Self { Self { client: client(), origin: parse(u) } } }",
            "impl self::TossHttpClient { pub fn from_url(u: &str) -> Self { Self { client: client(), origin: parse(u) } } }",
            "impl self::r#TossHttpClient { pub fn from_url(u: &str) -> Self { Self { client: client(), origin: parse(u) } } }",
            "format!(\"{}\", { impl TossHttpClient { pub fn from_url(u: &str) -> Self { Self { client: client(), origin: parse(u) } } } 0 });",
            "impl TossHttpClient { pub fn raw(&self) -> &Client { &self.client } }",
            "impl crate::E { pub fn from_url(u: &str) -> Self { Self { client: client(), origin: parse(u) } } }",
            "pub fn retarget(c: &mut TossHttpClient, u: &str) { c.origin = parse(u); }",
            "pub const RETARGET: fn(&mut TossHttpClient, Url) = |c, u| c.origin = u;",
            "impl RawResponse { pub fn raw(c: &TossHttpClient) -> &Client { &c.client } }",
            "mod sneak;",
            "pub struct TossHttpClient { pub(crate) client: Client, origin: Url }",
            "type Other = TossHttpClient;",
        ] {
            assert_eq!(
                findings(http, source),
                BTreeSet::from([Rule::TossProduction])
            );
        }
        assert_eq!(
            findings(
                "crates/adapters/broker-toss/src/other.rs",
                "fn f() { let c = TossHttpClient { client: client(), origin: url() }; }"
            ),
            BTreeSet::from([Rule::TossProduction])
        );
        assert!(
            findings(
                http,
                "pub struct TossHttpClient { client: Client, origin: Url }"
            )
            .is_empty()
        );
        assert!(findings(http, "#[cfg(test)] mod tests { #[test] fn f() {} }").is_empty());
        assert!(findings(http, "impl TossHttpClient { pub fn new(endpoint: &TossEndpoint, clock: Arc<dyn Clock>, wait: Arc<dyn crate::RequestWait>) -> Result<Self, TossHttpError> { Ok(Self { client: client(), origin: url() }) } }").is_empty());
        assert!(findings(http, "impl self::TossHttpClient { pub fn new(endpoint: &TossEndpoint, clock: Arc<dyn Clock>, wait: Arc<dyn crate::RequestWait>) -> Result<Self, TossHttpError> { Ok(Self { client: client(), origin: url() }) } }").is_empty());
    }
    #[test]
    fn old_client_constructor_cannot_omit_clock_and_wait() {
        assert_eq!(
            findings(
                "crates/adapters/broker-toss/src/http.rs",
                "impl TossHttpClient { pub fn new(endpoint: &TossEndpoint) -> Result<Self, TossHttpError> { Ok(Self { client: client(), origin: url() }) } }"
            ),
            BTreeSet::from([Rule::TossProduction])
        );
    }
    #[test]
    fn toss_smoke_main_allows_production_while_other_toss_smoke_files_and_host_literals_fail() {
        for source in [
            "fn f() { let endpoint = broker_toss::TossEndpoint::production(); }",
            "use broker_toss::TossEndpoint as E; fn f() { let endpoint = E::production(); }",
            "fn f() { let factory = broker_toss::TossEndpoint::production; }",
            "use broker_toss as b; fn f() { let factory = b::TossEndpoint::r#production; }",
            "type E = broker_toss::TossEndpoint; fn f() { let factory = E::production; }",
            "fn f() { check!(broker_toss::TossEndpoint::production()); }",
        ] {
            assert!(findings("crates/toss-smoke/src/main.rs", source).is_empty());
            for forbidden in [
                "crates/toss-smoke/src/lib.rs",
                "crates/toss-smoke/src/cli.rs",
                "crates/toss-smoke/tests/smoke.rs",
            ] {
                assert_eq!(
                    findings(forbidden, source),
                    BTreeSet::from([Rule::TossProduction])
                );
            }
        }
        assert_eq!(
            findings(
                "crates/toss-smoke/src/main.rs",
                "const PRODUCTION_ORIGIN: &str = \"https://openapi.HOST\";"
            ),
            BTreeSet::from([Rule::TossHost])
        );
    }
}
