use super::graph::{Graph, Sources};
use std::collections::BTreeSet;

fn sources(caller: &str) -> Sources {
    Sources::from([
        (
            String::from("crates/domain/src/lib.rs"),
            String::from("mod caller; mod other; pub use other::Other;"),
        ),
        (
            String::from("crates/domain/src/caller.rs"),
            String::from(caller),
        ),
        (
            String::from("crates/domain/src/other.rs"),
            String::from("pub enum Other { Variant } pub fn f() {}"),
        ),
    ])
}

#[test]
fn weakened_macro_only_tests_keep_the_referenced_module() {
    for before in [
        "#[cfg(test)] mod tests { #[test] fn boundary() { assert_eq!(crate::other::f(), ()); } }",
        "proptest::proptest! { #[test] fn boundary(x in 0..1) { crate::other::f(); } }",
        "fn boundary() { let x = None; assert!(matches!(x, Some(crate::Other::Variant))); }",
    ] {
        let mut graph = Graph::read(&sources("fn boundary() {}")).expect("current graph");
        graph.extend(Graph::read(&sources(before)).expect("baseline graph"));
        let selected = graph
            .selected(&BTreeSet::from([String::from(
                "crates/domain/src/caller.rs",
            )]))
            .expect("graph selection");
        assert!(selected.contains("crates/domain/src/other.rs"), "{before}");
    }
}

#[test]
fn local_macro_body_in_another_file_is_a_dependency() {
    let mut files = sources("fn call() { shared!(); }");
    files.insert(
        String::from("crates/domain/src/macros.rs"),
        String::from("macro_rules! shared { () => { crate::other::f() }; }"),
    );
    let selected = Graph::read(&files)
        .expect("local macro graph")
        .selected(&BTreeSet::from([String::from(
            "crates/domain/src/caller.rs",
        )]))
        .expect("selected");
    assert!(selected.contains("crates/domain/src/other.rs"));
    assert!(selected.contains("crates/domain/src/macros.rs"));
}

#[test]
fn unknown_nested_macros_and_includes_remain_uncertain() {
    for source in [
        "fn call() { assert!(external!()); }",
        "fn call() { include!(\"generated.rs\"); }",
        "use external::hidden as verify; fn call() { verify!(); }",
    ] {
        assert!(Graph::read(&sources(source)).is_err(), "{source}");
    }
}

#[test]
fn unknown_derives_remain_uncertain() {
    assert!(
        Graph::read(&sources("#[derive(Custom)] struct Value;")).is_err(),
        "unknown derive"
    );
    Graph::read(&sources(
        "#[derive(Debug, Clone, Copy, PartialEq, Eq)] struct Value;",
    ))
    .expect("standard derives");
}

fn portfolio_sources() -> Sources {
    [
        (
            "lib",
            "mod portfolio_valuation; mod holding_valuation; mod exact_arithmetic; mod unrelated; pub use holding_valuation::compare_money;",
        ),
        (
            "portfolio_valuation",
            "#[cfg(test)] mod tests {
                #[test] fn boundary() {
                    assert_eq!(crate::compare_money(101, 100), true);
                    assert_eq!(crate::compare_money(99, 100), true);
                    assert_eq!(crate::compare_money(-101, -100), true);
                    assert_eq!(crate::compare_money(-1, 0), true);
                }
            }",
        ),
        (
            "holding_valuation",
            "pub fn compare_money(computed: i64, reported: i64) -> bool { crate::exact_arithmetic::compare_reported(computed, reported) }",
        ),
        (
            "exact_arithmetic",
            "pub fn compare_reported(computed: i64, reported: i64) -> bool { computed.abs_diff(reported) <= 1 }",
        ),
        ("unrelated", "pub fn untouched() {}"),
    ]
    .into_iter()
    .map(|(module, source)| (format!("crates/domain/src/{module}.rs"), String::from(source)))
    .collect()
}

#[test]
fn portfolio_boundary_deletion_reaches_exact_arithmetic_through_graph() {
    let before = portfolio_sources();
    let mut after = before.clone();
    let file = String::from("crates/domain/src/portfolio_valuation.rs");
    let source = after.get_mut(&file).expect("portfolio fixture");
    for row in [
        "assert_eq!(crate::compare_money(101, 100), true);",
        "assert_eq!(crate::compare_money(99, 100), true);",
        "assert_eq!(crate::compare_money(-101, -100), true);",
        "assert_eq!(crate::compare_money(-1, 0), true);",
    ] {
        assert!(source.contains(row), "{row}");
        *source = source.replacen(row, "", 1);
    }
    let seeds = BTreeSet::from([file]);
    let mut graph = Graph::read(&after).expect("weakened fixture graph, no fallback");
    assert!(
        !graph
            .outward(&seeds)
            .expect("current closure")
            .contains("crates/domain/src/exact_arithmetic.rs")
    );
    graph.extend(Graph::read(&before).expect("baseline fixture graph, no fallback"));
    let selected = graph.outward(&seeds).expect("outgoing graph closure");
    assert!(selected.contains("crates/domain/src/holding_valuation.rs"));
    assert!(selected.contains("crates/domain/src/exact_arithmetic.rs"));
    assert!(!selected.contains("crates/domain/src/unrelated.rs"));
    assert!(!selected.contains("crates/domain/src/lib.rs"));
}

#[test]
fn removed_compare_call_keeps_baseline_dependency_through_graph() {
    let before = portfolio_sources();
    let mut after = before.clone();
    let file = String::from("crates/domain/src/holding_valuation.rs");
    after.insert(file.clone(), String::from("pub fn compare_money(computed: i64, reported: i64) -> bool { local_compare(computed, reported) } fn local_compare(computed: i64, reported: i64) -> bool { computed.abs_diff(reported) <= 1 }"));
    let seeds = BTreeSet::from([file]);
    let mut graph = Graph::read(&after).expect("replacement fixture graph, no fallback");
    assert!(
        !graph
            .outward(&seeds)
            .expect("current closure")
            .contains("crates/domain/src/exact_arithmetic.rs")
    );
    graph.extend(Graph::read(&before).expect("baseline fixture graph, no fallback"));
    let selected = graph.outward(&seeds).expect("outgoing graph closure");
    assert!(selected.contains("crates/domain/src/exact_arithmetic.rs"));
    assert!(!selected.contains("crates/domain/src/unrelated.rs"));
}

#[test]
fn formatting_captures_are_references_and_constructed_captures_expand() {
    let mut files = sources(&["fn call() { format!(\"{", "SECRET", ":?}\"); }"].concat());
    files.insert(
        String::from("crates/domain/src/other.rs"),
        String::from("pub const SECRET: usize = 1;"),
    );
    let selected = Graph::read(&files)
        .expect("capture graph")
        .outward(&BTreeSet::from([String::from(
            "crates/domain/src/caller.rs",
        )]))
        .expect("outward");
    assert!(selected.contains("crates/domain/src/other.rs"));
    files.insert(
        String::from("crates/domain/src/caller.rs"),
        String::from("fn call() { format!(concat!(\"{SEC\", \"RET}\")); }"),
    );
    assert!(Graph::read(&files).is_err(), "constructed identifier");
    assert!(
        Graph::read(&sources(
            "use external as std; fn call() { std::assert!(true); }"
        ))
        .is_err(),
        "untrusted std alias"
    );
}

#[test]
fn imported_macro_aliases_and_globs_preserve_argument_references() {
    for source in [
        "use proptest::prop_assert_eq as verify; fn call() { verify!(crate::other::f(), ()); }",
        "use proptest::prelude::*; fn call() { prop_assert!(crate::other::f()); }",
    ] {
        let selected = Graph::read(&sources(source))
            .expect("known import")
            .outward(&BTreeSet::from([String::from(
                "crates/domain/src/caller.rs",
            )]))
            .expect("outward");
        assert!(selected.contains("crates/domain/src/other.rs"));
    }
    assert!(
        Graph::read(&sources(
            "use external::Debug; #[derive(Debug)] struct Value;"
        ))
        .is_err(),
        "custom derive shadow"
    );
}

#[test]
fn local_macro_reexport_aliases_are_read_without_fallback() {
    let mut files = sources("fn call() { crate::Alias!(); }");
    files.insert(
        String::from("crates/domain/src/lib.rs"),
        String::from("mod caller; mod other; mod macros; pub use crate::shared as Alias;"),
    );
    files.insert(
        String::from("crates/domain/src/macros.rs"),
        String::from("#[macro_export] macro_rules! shared { () => { crate::other::f() }; }"),
    );
    let selected = Graph::read(&files)
        .expect("macro reexport graph")
        .outward(&BTreeSet::from([String::from(
            "crates/domain/src/caller.rs",
        )]))
        .expect("outward");
    assert!(selected.contains("crates/domain/src/other.rs"));
    assert!(selected.contains("crates/domain/src/macros.rs"));
    assert!(
        Graph::read(&sources("#[external_attr] fn call() {}")).is_err(),
        "unknown attribute"
    );
    assert!(
        Graph::read(&sources(
            "use external::hidden as verify; fn call() { verify!(); }"
        ))
        .is_err(),
        "unknown external alias"
    );
}

#[test]
fn local_macro_references_are_merged_into_the_call_tokens() {
    let mut files = sources("fn call() { shared!(); }");
    files.insert(
        String::from("crates/domain/src/macros.rs"),
        String::from("macro_rules! shared { () => { crate::other::f() }; }"),
    );
    let parsed = files
        .iter()
        .map(|(path, source)| (path.clone(), syn::parse_file(source).expect("source")))
        .collect();
    let macros = super::macro_tokens::Macros::read(&parsed, &files).expect("macro table");
    let ids = macros.expanded_ids("shared!()".parse().expect("call tokens"));
    assert!(ids.contains("other"));
    assert!(ids.contains("f"));
    assert!(
        Graph::read(&sources(
            "macro_rules! generated { () => { pub fn emitted() {} }; } generated!();"
        ))
        .is_err(),
        "item generation is unresolved"
    );
}

#[test]
fn outward_closure_excludes_incoming_only_files() {
    let graph =
        Graph::read(&sources("fn call() { assert_eq!(crate::other::f(), ()); }")).expect("graph");
    let seeds = BTreeSet::from([String::from("crates/domain/src/caller.rs")]);
    assert!(
        !graph
            .outward(&seeds)
            .expect("outward")
            .contains("crates/domain/src/lib.rs")
    );
    assert!(
        graph
            .selected(&seeds)
            .expect("selected")
            .contains("crates/domain/src/lib.rs")
    );
}

#[test]
fn ordinary_glob_imports_keep_token_references_but_glob_reexports_expand() {
    let graph = Graph::read(&sources(
        "use crate::other::*; fn call() { assert_eq!(f(), ()); }",
    ))
    .expect("ordinary glob");
    assert!(
        graph
            .outward(&BTreeSet::from([String::from(
                "crates/domain/src/caller.rs"
            )]))
            .expect("outward")
            .contains("crates/domain/src/other.rs")
    );
    assert!(
        Graph::read(&sources("pub use crate::other::*;")).is_err(),
        "ambiguous reexport"
    );
}

#[test]
fn proptest_config_is_only_a_known_helper_inside_macro_tokens() {
    Graph::read(&sources("proptest::proptest! { #![proptest_config(Config::with_cases(1))] #[test] fn check(x in 0..1) { crate::other::f(); } }")).expect("known helper");
    assert!(
        Graph::read(&sources(
            "#[proptest_config(Config::with_cases(1))] fn check() {}"
        ))
        .is_err(),
        "not an external attribute allowlist"
    );
    assert!(Graph::read(&sources("use external::evil as proptest_config; proptest::proptest! { #[proptest_config] fn check(x in 0..1) {} }")).is_err(), "unknown helper binding");
}

#[test]
fn familiar_macro_names_from_other_namespaces_are_not_trusted() {
    for source in [
        "use external::proptest as test_it; fn call() { test_it!{} }",
        "use external::std as std; fn call() { std::assert!(true); }",
        "use external::std::Debug; #[derive(Debug)] struct Value;",
        "pub(crate) use crate::other::*;",
    ] {
        assert!(Graph::read(&sources(source)).is_err(), "{source}");
    }
}
