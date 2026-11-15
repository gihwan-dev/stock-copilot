use std::collections::{BTreeMap, BTreeSet};

use syn::visit::Visit;

use super::finding::Rule;

struct KeyCalls<'a> {
    allowed: &'a BTreeSet<String>,
    counts: BTreeMap<usize, usize>,
}

impl<'ast> Visit<'ast> for KeyCalls<'_> {
    fn visit_expr_call(&mut self, i: &'ast syn::ExprCall) {
        let call = i;
        if let syn::Expr::Path(path) = call.func.as_ref()
            && path.qself.is_none()
            && path
                .path
                .segments
                .iter()
                .map(|segment| segment.ident.to_string())
                .collect::<Vec<_>>()
                == ["std", "env", "var"]
            && call.args.len() == 1
            && let Some(syn::Expr::Lit(argument)) = call.args.first()
            && let syn::Lit::Str(key) = &argument.lit
            && self
                .allowed
                .contains(&format!("clock-env:var:{}", key.value()))
            && let Some(function) = path.path.segments.last()
        {
            let count = self
                .counts
                .entry(function.ident.span().start().line)
                .or_default();
            *count = count.saturating_add(1);
        }
        syn::visit::visit_expr_call(self, call);
    }
}

pub(super) fn filter(
    source: &str,
    rules: Vec<(usize, Rule)>,
    allowed: Option<&BTreeSet<String>>,
) -> Vec<(usize, Rule)> {
    let Some(names) = allowed else {
        return rules;
    };
    if !names.iter().any(|name| name.starts_with("clock-env:var:")) {
        return rules;
    }
    let Ok(file) = syn::parse_file(source) else {
        return rules;
    };
    let mut calls = KeyCalls {
        allowed: names,
        counts: BTreeMap::new(),
    };
    calls.visit_file(&file);
    rules
        .into_iter()
        .filter_map(|(line, rule)| {
            if let Rule::ClockOrEnvironment(name) = &rule
                && name == "var"
                && let Some(remaining) = calls.counts.get_mut(&line)
                && *remaining > 0
            {
                *remaining = remaining.saturating_sub(1);
                return None;
            }
            Some((line, rule))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::filter;
    use crate::source_check::tokens::{ClockScope, FloatScope, Scope, token_rules};
    use std::collections::BTreeSet;

    fn found(source: &str, allowed: &BTreeSet<String>) -> Vec<String> {
        let scope = Scope {
            clock: ClockScope::Forbidden,
            float: FloatScope::Elsewhere,
        };
        filter(
            source,
            token_rules(source.parse().expect("tokens"), scope),
            Some(allowed),
        )
        .into_iter()
        .map(|(_, rule)| rule.to_string())
        .collect()
    }

    fn keys() -> BTreeSet<String> {
        BTreeSet::from([String::from("clock-env:var:STOCK_COPILOT_BASE_USE")])
    }

    #[test]
    fn h44_review_environment_permission_is_exact() {
        let allowed = keys();
        assert_eq!(
            found(
                "fn main() { std::env::var(\"STOCK_COPILOT_BASE_USE\"); }",
                &allowed
            ),
            Vec::<String>::new()
        );
        for source in [
            "fn main() { std::env::var(\"HOME\"); }",
            "fn main() { let key = \"STOCK_COPILOT_BASE_USE\"; std::env::var(key); }",
            "fn main() { other::var(\"STOCK_COPILOT_BASE_USE\"); }",
            "fn main() { any!(std::env::var(\"STOCK_COPILOT_BASE_USE\")); }",
            "std::env::var(\"STOCK_COPILOT_BASE_USE\");",
        ] {
            assert_eq!(found(source, &allowed), ["clock-env:var"], "{source}");
        }
        assert_eq!(
            found(
                "fn main() { std::env::var(\"STOCK_COPILOT_BASE_USE\"); }",
                &BTreeSet::new()
            ),
            ["clock-env:var"]
        );
    }

    #[test]
    fn h44_review_environment_permission_does_not_hide_neighbor_or_alias_calls() {
        let allowed = keys();
        for source in [
            "fn main() { std::env::var(\"STOCK_COPILOT_BASE_USE\"); std::env::var(\"HOME\"); }",
            "fn main() { std::env::var(\"HOME\"); std::env::var(\"STOCK_COPILOT_BASE_USE\"); }",
        ] {
            assert_eq!(found(source, &allowed), ["clock-env:var"], "{source}");
        }
        assert_ne!(
            found(
                "use std::env::var as read; fn main() { read(\"STOCK_COPILOT_BASE_USE\"); }",
                &allowed
            ),
            Vec::<String>::new()
        );
    }
}
