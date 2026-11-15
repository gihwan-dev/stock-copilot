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
                TokenTree::Ident(_) | TokenTree::Punct(_) => {}
            }
        }
    }
}

impl<'ast> Visit<'ast> for EndpointCheck<'_> {
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
        visit::visit_path(self, i);
    }
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
}
