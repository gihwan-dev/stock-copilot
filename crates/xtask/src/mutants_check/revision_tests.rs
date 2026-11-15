use super::{
    additions::{Additions, Change},
    graph::Sources,
};
use std::collections::BTreeSet;

fn fixture(code: &str) -> Sources {
    Sources::from([
        (
            String::from("crates/domain/src/lib.rs"),
            String::from("mod old;"),
        ),
        (String::from("crates/domain/src/old.rs"), String::from(code)),
        (
            String::from("crates/domain/tests/x/mod.rs"),
            String::from("pub fn integration_helper() {}"),
        ),
    ])
}

fn added(before: &Sources, source: &str) -> Sources {
    let mut after = before.clone();
    after.insert(
        String::from("crates/domain/src/lib.rs"),
        String::from("mod old; mod fresh;"),
    );
    after.insert(
        String::from("crates/domain/src/fresh.rs"),
        String::from(source),
    );
    after
}

fn select(before: &Sources, after: &Sources, changed: &[&str]) -> Result<BTreeSet<String>, String> {
    super::scope::select_sources(
        before,
        after,
        &[String::from("crates/domain/src/**/*.rs")],
        &changed.iter().map(|path| String::from(*path)).collect(),
        "crates/domain",
    )
}

#[test]
fn integration_support_and_line_bindings_do_not_expand_new_files() {
    let before = fixture("fn old() { let _ = Some(1).filter(|line| *line > 0); }");
    let after = added(
        &before,
        "pub fn novel() {} #[cfg(test)] mod tests { #[test] #[should_panic(expected = \"expected\")] fn fresh_panic() { panic!(\"expected\"); } }",
    );
    assert_eq!(
        select(
            &before,
            &after,
            &["crates/domain/src/lib.rs", "crates/domain/src/fresh.rs"]
        )
        .expect("src graph"),
        BTreeSet::from([String::from("crates/domain/src/fresh.rs")])
    );
}

#[test]
fn integration_changes_and_prior_names_still_widen() {
    let mut before = fixture("fn old() {}");
    before.insert(
        String::from("crates/domain/tests/x/mod.rs"),
        String::from("fn check() { ShadowName(); }"),
    );
    let mut after = added(&before, "pub struct ShadowName;");
    after.insert(
        String::from("crates/domain/src/lib.rs"),
        String::from("mod old; mod fresh; pub use fresh::ShadowName;"),
    );
    select(
        &before,
        &after,
        &["crates/domain/src/lib.rs", "crates/domain/src/fresh.rs"],
    )
    .expect_err("integration identifiers are prior names");
    for path in [
        "crates/domain/tests/x/mod.rs",
        "crates/domain/tests/other.rs",
        "crates/domain/build.rs",
        "crates/domain/tests/renamed.rs",
        "crates/domain/tests/deleted.rs",
        "crates/domain/tests/nested/data.rs",
        "crates/domain/examples/demo.rs",
        "crates/domain/benches/speed.rs",
        "crates/domain/Cargo.toml",
        "crates/domain/settings.json",
    ] {
        select(&before, &after, &[path]).expect_err("support change remains crate-wide");
    }
}

#[test]
fn source_cannot_reach_integration_support_through_path_or_include() {
    let before = fixture("fn old() {}");
    for code in [
        "#[path = \"../tests/x/mod.rs\"] mod hidden;",
        "include!(\"../tests/x/mod.rs\");",
    ] {
        select(
            &before,
            &added(&before, code),
            &["crates/domain/src/lib.rs", "crates/domain/src/fresh.rs"],
        )
        .expect_err("hidden source remains uncertain");
    }
    let root = crate::harness_workspace_tests::repository_root();
    let findings = crate::source_check::file_findings(
        &root,
        "crates/domain/src/valuation_snapshot.rs",
        "include!(\"../tests/x/mod.rs\");",
    )
    .expect("source check")
    .expect("applicable source");
    assert!(
        findings
            .iter()
            .any(|finding| finding.to_string().contains("macro:include"))
    );
}

#[test]
fn ordinary_location_names_do_not_disable_additions() {
    for code in [
        "fn old() { let line = 1; let _ = line; }",
        "fn old() { let line = 1; let _ = line != 0; }",
        "fn old(Location: usize) { let _ = Location; }",
        "fn old() { let file = 1; let module_path = file; }",
        "fn old() { let _ = Some(1).filter(|line| *line > 0); }",
    ] {
        let before = fixture(code);
        let mut after = before.clone();
        after.insert(
            String::from("crates/domain/src/old.rs"),
            format!("{code} pub fn novel() {{}}"),
        );
        assert_eq!(
            Additions::read(&before, &after)
                .expect("context")
                .classify("crates/domain/src/old.rs"),
            Change::OnlyFile,
            "{code}"
        );
    }
}

#[test]
fn location_calls_paths_imports_and_opaque_tokens_keep_graph_rules() {
    for code in [
        "fn old() { line!(); }",
        "fn old() { file!(); }",
        "fn old() { module_path!(); }",
        "fn old() { column!(); }",
        "fn old() { std::line!(); }",
        "fn old() { assert_eq!(column!(), 1); }",
        "use std::line as l; fn old() { l!(); }",
        "pub use core::column as c; fn old() { c!(); }",
        "use std::{line as l}; fn old() { l!(); }",
        "use std::panic::Location as L; fn old() { L::caller(); }",
        "use std::panic as p; fn old() { p::Location::caller(); }",
        "use std::panic::{self as p}; fn old() { p::Location::caller(); }",
        "use core::r#line as l; fn old() { l!(); }",
        "use std as s; fn old() { s::panic::Location::caller(); }",
        "use std::*; fn old() { line!(); }",
        "fn old() { std::panic::Location::caller(); }",
        "#[track_caller] fn old() {}",
        "macro_rules! local { () => { line!() }; } fn old() { local!(); }",
        "macro_rules! local { () => { use std::line as l; l!(); }; } fn old() {}",
        "macro_rules! local { ($p:path) => { use $p as l; l!(); }; } fn old() {}",
        "pub use std::line as first; pub use first as second; fn old() { second!(); }",
    ] {
        assert!(
            super::location_tokens::sensitive(code.parse().expect("tokens")),
            "{code}"
        );
        let before = fixture(code);
        let mut after = before.clone();
        after.insert(
            String::from("crates/domain/src/old.rs"),
            format!("{code} pub fn novel() {{}}"),
        );
        assert_eq!(
            Additions::read(&before, &after)
                .expect("context")
                .classify("crates/domain/src/old.rs"),
            Change::Graph,
            "{code}"
        );
    }
}

#[test]
fn suffixed_test_attribute_literals_require_fallback() {
    for attribute in [
        "should_panic(expected = \"a\"suffix)",
        "should_panic(expected = r#\"a\"#suffix)",
        "ignore = \"a\"suffix",
    ] {
        let before = fixture(&format!(
            "#[cfg(test)] mod tests {{ #[test] #[{attribute}] fn old() {{}} }}"
        ));
        select(
            &before,
            &added(&before, "pub struct Fresh;"),
            &["crates/domain/src/lib.rs", "crates/domain/src/fresh.rs"],
        )
        .expect_err(attribute);
        let meta = syn::parse_str::<syn::Meta>(attribute).expect("meta");
        assert!(!super::macro_tokens::attribute_known(&meta), "{attribute}");
    }
}

#[test]
fn attributes_in_new_files_and_appended_test_functions_are_narrow() {
    let before = fixture("fn old() {} #[cfg(test)] mod tests { #[test] fn stable() {} }");
    for attribute in [
        "should_panic",
        "should_panic(expected = \"expected\")",
        "ignore",
        "ignore = \"reason\"",
    ] {
        let new_test =
            format!("#[test] #[{attribute}] fn novel_test() {{ panic!(\"expected\"); }}");
        let fresh = added(
            &before,
            &format!("pub struct Fresh; #[cfg(test)] mod tests {{ {new_test} }}"),
        );
        assert_eq!(
            select(
                &before,
                &fresh,
                &["crates/domain/src/lib.rs", "crates/domain/src/fresh.rs"]
            )
            .expect(attribute),
            BTreeSet::from([String::from("crates/domain/src/fresh.rs")])
        );
        let mut appended = before.clone();
        appended.insert(
            String::from("crates/domain/src/old.rs"),
            format!(
                "fn old() {{}} #[cfg(test)] mod tests {{ #[test] fn stable() {{}} {new_test} }}"
            ),
        );
        assert_eq!(
            select(&before, &appended, &["crates/domain/src/old.rs"]).expect(attribute),
            BTreeSet::from([String::from("crates/domain/src/old.rs")])
        );
    }
}

#[test]
fn cross_file_location_reexports_keep_existing_code_in_graph_scope() {
    let mut before = fixture("use crate::bridge::first as second; fn old() { second!(); }");
    before.insert(
        String::from("crates/domain/src/bridge.rs"),
        String::from("pub use std::line as first;"),
    );
    let mut after = before.clone();
    after.insert(
        String::from("crates/domain/src/old.rs"),
        String::from(
            "use crate::bridge::first as second; fn old() { second!(); } pub fn novel() {}",
        ),
    );
    assert_eq!(
        Additions::read(&before, &after)
            .expect("context")
            .classify("crates/domain/src/old.rs"),
        Change::Graph
    );
}
