use super::{
    additions::{Additions, Change},
    graph::Sources,
};
use std::collections::BTreeSet;

fn sources(code: &str) -> Sources {
    Sources::from([
        (
            String::from("crates/domain/src/lib.rs"),
            String::from("mod constants; mod consumer;"),
        ),
        (
            String::from("crates/domain/src/constants.rs"),
            String::from(code),
        ),
        (
            String::from("crates/domain/src/consumer.rs"),
            String::from("pub fn above(x: usize) -> bool { x > crate::constants::LIMIT }"),
        ),
    ])
}
fn classify(before: &str, after: &str) -> Change {
    Additions::read(&sources(before), &sources(after))
        .expect("context")
        .classify("crates/domain/src/constants.rs")
}
fn selected(before: &Sources, after: &Sources, changed: &[&str]) -> BTreeSet<String> {
    match super::scope::select_sources(
        before,
        after,
        &[String::from("crates/domain/src/**/*.rs")],
        &changed.iter().map(|path| String::from(*path)).collect(),
        "crates/domain",
    ) {
        Ok(files) => files,
        Err(_) => after.keys().cloned().collect(),
    }
}

#[test]
fn unicode_changes_cannot_be_hidden_by_item_additions() {
    for literal in [
        "\"가\"",
        "\"😀\"",
        "\"e\u{301}\u{308}\"",
        "\"가\n나\"",
        "r#\"가\"#",
    ] {
        let before = format!("pub const LIMIT: usize = {literal}.len() + 17;");
        let after = format!("pub const LIMIT: usize = {literal}.len() + 18; pub fn novel() {{}}");
        assert_eq!(classify(&before, &after), Change::Graph, "{literal}");
        let chosen = selected(
            &sources(&before),
            &sources(&after),
            &["crates/domain/src/constants.rs"],
        );
        assert!(
            chosen.contains("crates/domain/src/consumer.rs"),
            "{literal}"
        );
    }
}

#[test]
fn unicode_exports_do_not_erase_changes_to_other_items() {
    let mut before = sources("pub const LIMIT: usize = 17;");
    before.insert(String::from("crates/domain/src/lib.rs"), String::from("pub const TEXT: &str = \"가😀e\u{301}\"; mod constants; mod consumer; pub use constants::LIMIT;"));
    let mut after = before.clone();
    after.insert(String::from("crates/domain/src/lib.rs"), String::from("pub const TEXT: &str = \"나😀e\u{301}\"; mod constants; mod consumer; pub use constants::LIMIT;"));
    super::scope::select_sources(
        &before,
        &after,
        &[String::from("crates/domain/src/**/*.rs")],
        &BTreeSet::from([String::from("crates/domain/src/lib.rs")]),
        "crates/domain",
    )
    .expect_err("lib constant changed");
}

#[test]
fn raw_and_plain_prelude_names_are_never_fresh() {
    for added in [
        "fn r#drop<T>(_: T) {}",
        "type r#Option = usize;",
        "trait r#Iterator {}",
        "fn drop<T>(_: T) {}",
        "struct Some;",
        "struct Future;",
        "struct IntoFuture;",
        "struct core;",
        "struct std;",
        "struct alloc;",
    ] {
        assert_eq!(
            classify("fn old() {}", &format!("fn old() {{}} {added}")),
            Change::Graph,
            "{added}"
        );
    }
}

#[test]
fn raw_reexports_share_graph_owners() {
    let before = Sources::from([
        (
            String::from("crates/domain/src/lib.rs"),
            String::from("mod constants; mod consumer; pub use constants::r#value;"),
        ),
        (
            String::from("crates/domain/src/constants.rs"),
            String::from("pub fn r#value() -> usize { 1 }"),
        ),
        (
            String::from("crates/domain/src/consumer.rs"),
            String::from("pub fn consumer() -> usize { crate::value() }"),
        ),
    ]);
    let closure = super::graph::Graph::read(&before)
        .expect("graph")
        .outward(&BTreeSet::from([String::from(
            "crates/domain/src/consumer.rs",
        )]))
        .expect("closure");
    assert!(closure.contains("crates/domain/src/constants.rs"));
}

#[test]
fn raw_definition_and_reference_spellings_share_root_owners() {
    for (definition, reference) in [("r#value", "value"), ("value", "r#value")] {
        let input = Sources::from([
            (
                String::from("crates/domain/src/lib.rs"),
                format!("pub fn {definition}() -> usize {{ 1 }}"),
            ),
            (
                String::from("crates/domain/src/consumer.rs"),
                format!("pub fn consumer() -> usize {{ crate::{reference}() }}"),
            ),
        ]);
        let closure = super::graph::Graph::read(&input)
            .expect("graph")
            .outward(&BTreeSet::from([String::from(
                "crates/domain/src/consumer.rs",
            )]))
            .expect("closure");
        assert!(closure.contains("crates/domain/src/lib.rs"));
    }
}

#[test]
fn test_diff_raw_calls_and_new_names_share_shadow_identity() {
    for call in ["drop", "r#drop"] {
        for added in ["drop", "r#drop"] {
            let before =
                format!("#[cfg(test)] mod tests {{ #[test] fn stable() {{ {call}(1); }} }}");
            let after = format!(
                "#[cfg(test)] mod tests {{ #[test] fn stable() {{ {call}(1); }} fn {added}<T>(_: T) {{}} }}"
            );
            let prior = crate::test_diff::Sources::from([(
                String::from("crates/domain/src/lib.rs"),
                before,
            )]);
            let current = crate::test_diff::Sources::from([(
                String::from("crates/domain/src/lib.rs"),
                after,
            )]);
            assert!(
                crate::test_diff::compare_sources(&prior, &current)
                    .iter()
                    .any(|finding| finding.rule.to_string().contains("shadowing")),
                "{call}/{added}"
            );
        }
    }
}

#[test]
fn test_diff_raw_support_names_keep_the_same_support_key() {
    let before =
        "#[cfg(test)] mod tests { #[test] fn stable() { drop(1); } fn r#drop<T>(_: T) {} }";
    let after = before.replace("fn r#drop", "fn drop");
    let prior = crate::test_diff::Sources::from([(
        String::from("crates/domain/src/lib.rs"),
        String::from(before),
    )]);
    let current =
        crate::test_diff::Sources::from([(String::from("crates/domain/src/lib.rs"), after)]);
    assert!(
        crate::test_diff::compare_sources(&prior, &current)
            .iter()
            .any(
                |finding| finding.rule.to_string().contains("support-modified")
                    && finding.rule.to_string().contains("fn drop")
            )
    );
}

#[test]
fn external_roots_and_bindings_cannot_be_registered_or_shadowed() {
    let before =
        sources("use time::Duration as ExternalSpan; fn old() {} pub const LIMIT: usize = 17;");
    for module in ["core", "std", "alloc", "time"] {
        let mut after = before.clone();
        after.insert(
            String::from("crates/domain/src/lib.rs"),
            format!("mod constants; mod consumer; mod {module};"),
        );
        after.insert(
            format!("crates/domain/src/{module}.rs"),
            String::from("pub fn novel() {}"),
        );
        assert_eq!(
            Additions::read(&before, &after)
                .expect("context")
                .classify("crates/domain/src/lib.rs"),
            Change::Graph,
            "{module}"
        );
    }
    assert_eq!(
        classify(
            "use time::Duration as ExternalSpan; fn old() {}",
            "use time::Duration as ExternalSpan; fn old() {} struct r#ExternalSpan;"
        ),
        Change::Graph
    );
}

#[test]
fn existing_location_macros_disable_addition_shortcuts() {
    for invocation in [
        "line!()",
        "file!()",
        "module_path!()",
        "std::line!()",
        "assert_eq!(line!(), 1)",
    ] {
        let before = format!("fn old() {{ let _ = {invocation}; }}");
        assert_eq!(
            classify(&before, &format!("fn novel() {{}} {before}")),
            Change::Graph,
            "{invocation}"
        );
    }
    assert_eq!(
        classify(
            "use std::line as location; fn old() { location!(); }",
            "use std::line as location; fn old() { location!(); } fn novel() {}"
        ),
        Change::Graph
    );
}

#[test]
fn non_target_additions_expand_the_crate_without_other_checks() {
    let before = sources("pub const LIMIT: usize = 17;");
    for path in [
        "crates/domain/build.rs",
        "crates/domain/examples/demo.rs",
        "crates/domain/benches/speed.rs",
        "crates/domain/settings.json",
    ] {
        let mut after = before.clone();
        after.insert(String::from(path), String::from("fn main() {}"));
        super::scope::select_sources(
            &before,
            &after,
            &[String::from("crates/domain/src/**/*.rs")],
            &BTreeSet::from([String::from(path)]),
            "crates/domain",
        )
        .expect_err(path);
    }
}

#[test]
fn blanket_impls_and_trait_objects_expand_uncertain_dispatch() {
    for implementation in [
        "impl<U: crate::Bound> crate::Evaluate for U { fn evaluate(&self) {} }",
        "pub fn dispatch(value: &dyn crate::Evaluate) { value.evaluate(); }",
    ] {
        let input = Sources::from([
            (
                String::from("crates/domain/src/lib.rs"),
                String::from("pub trait Bound {} pub trait Evaluate { fn evaluate(&self); }"),
            ),
            (
                String::from("crates/domain/src/dispatch.rs"),
                String::from(implementation),
            ),
        ]);
        super::graph::Graph::read(&input)
            .err()
            .expect("uncertain dispatch expands");
    }
}

#[test]
fn trait_names_reach_their_implementation_files() {
    let input = Sources::from([
        (
            String::from("crates/domain/src/lib.rs"),
            String::from("pub mod traits; pub mod model; mod implementation; pub mod consumer;"),
        ),
        (
            String::from("crates/domain/src/traits.rs"),
            String::from("pub trait Evaluate { fn evaluate(&self); }"),
        ),
        (
            String::from("crates/domain/src/model.rs"),
            String::from("pub struct Model;"),
        ),
        (
            String::from("crates/domain/src/implementation.rs"),
            String::from(
                "impl crate::traits::Evaluate for crate::model::Model { fn evaluate(&self) {} }",
            ),
        ),
        (
            String::from("crates/domain/src/consumer.rs"),
            String::from(
                "use super::traits::Evaluate; pub fn run<T: Evaluate>(value: T) { value.evaluate(); }",
            ),
        ),
    ]);
    let closure = super::graph::Graph::read(&input)
        .expect("graph")
        .outward(&BTreeSet::from([String::from(
            "crates/domain/src/consumer.rs",
        )]))
        .expect("closure");
    assert!(closure.contains("crates/domain/src/implementation.rs"));
}

#[test]
fn import_origin_and_data_only_guards_are_observable() {
    let mut before = sources("fn old() {} pub const LIMIT: usize = 17;");
    before.insert(String::from("crates/domain/src/foreign.rs"), String::from("pub struct Data; pub trait Mixed {} pub mod duplicate { pub struct Mixed; } pub use Mixed as Alias;"));
    for added in [
        "use external::Data as Fresh; fn novel(_: Fresh) {}",
        "use crate::foreign::Mixed as Fresh; fn novel(_: impl Fresh) {}",
        "use crate::foreign::Alias as Fresh; fn novel(_: impl Fresh) {}",
    ] {
        let mut after = before.clone();
        after.insert(
            String::from("crates/domain/src/constants.rs"),
            format!("fn old() {{}} pub const LIMIT: usize = 17; {added}"),
        );
        assert_eq!(
            Additions::read(&before, &after)
                .expect("context")
                .classify("crates/domain/src/constants.rs"),
            Change::Graph,
            "{added}"
        );
    }
}

#[test]
fn only_exact_test_modules_may_extend_unchanged_headers() {
    for (before, after) in [
        (
            "#[anything(test)] mod cases { fn old() {} }",
            "#[anything(test)] mod cases { fn old() {} fn novel() {} }",
        ),
        (
            "#[cfg(feature = \"test\")] mod cases { fn old() {} }",
            "#[cfg(feature = \"test\")] mod cases { fn old() {} fn novel() {} }",
        ),
        (
            "#[cfg(test)] pub(crate) mod cases { fn old() {} }",
            "#[cfg(test)] mod cases { fn old() {} fn novel() {} }",
        ),
        (
            "#[cfg(test)] #[allow(dead_code)] mod cases { fn old() {} }",
            "#[cfg(test)] #[allow(dead_code)] mod cases { fn old() {} fn novel() {} }",
        ),
    ] {
        assert_eq!(classify(before, after), Change::Graph, "{after}");
    }
}

#[test]
fn prelude_export_inventory_and_raw_modules_are_protected() {
    for name in "Send Sized Sync Unpin Drop Fn FnMut FnOnce AsyncFn AsyncFnMut AsyncFnOnce drop align_of align_of_val size_of size_of_val AsMut AsRef From Into DoubleEndedIterator ExactSizeIterator Extend IntoIterator Iterator Option Some None Result Ok Err Clone Copy Debug Default Eq Hash Ord PartialEq PartialOrd ToOwned Box String ToString Vec FromIterator TryFrom TryInto Future IntoFuture core std alloc time mem collections".split_whitespace() {
        let before = sources("pub const LIMIT: usize = 17;");
        let mut after = before.clone();
        after.insert(String::from("crates/domain/src/lib.rs"), format!("mod constants; mod consumer; mod r#{name};"));
        after.insert(format!("crates/domain/src/{name}.rs"), String::from("pub fn novel() {}"));
        assert_eq!(Additions::read(&before, &after).expect("context").classify("crates/domain/src/lib.rs"), Change::Graph, "{name}");
    }
    let before = sources("pub const LIMIT: usize = 17;");
    let mut after = before.clone();
    after.insert(
        String::from("crates/domain/src/lib.rs"),
        String::from("mod constants; mod consumer; mod r#fresh;"),
    );
    after.insert(
        String::from("crates/domain/src/fresh.rs"),
        String::from("pub fn novel() {}"),
    );
    assert_eq!(
        Additions::read(&before, &after)
            .expect("context")
            .classify("crates/domain/src/lib.rs"),
        Change::Registration
    );
}

#[test]
fn new_non_target_rust_files_expand_partial_crate_targets() {
    let before = sources("pub const LIMIT: usize = 17;");
    let mut after = before.clone();
    after.insert(
        String::from("crates/domain/src/added.rs"),
        String::from("pub fn novel() {}"),
    );
    super::scope::select_sources(
        &before,
        &after,
        &[String::from("crates/domain/src/consumer.rs")],
        &BTreeSet::from([String::from("crates/domain/src/added.rs")]),
        "crates/domain",
    )
    .expect_err("new non-target input");
}

#[test]
fn external_aliases_and_fully_qualified_roots_are_not_modules() {
    for (code, modules) in [
        (
            "use sdk::Remote as ExternalSpan; fn old() {}",
            "sdk Remote ExternalSpan",
        ),
        ("fn old() { sdk::value(); }", "sdk"),
        ("extern crate sdk;", "sdk"),
        ("extern crate sdk as Bridge;", "sdk Bridge"),
    ] {
        for module in modules.split_whitespace() {
            let before = sources(code);
            let mut after = before.clone();
            after.insert(
                String::from("crates/domain/src/lib.rs"),
                format!("mod constants; mod consumer; mod {module};"),
            );
            after.insert(
                format!("crates/domain/src/{module}.rs"),
                String::from("pub fn novel() {}"),
            );
            assert_eq!(
                Additions::read(&before, &after)
                    .expect("context")
                    .classify("crates/domain/src/lib.rs"),
                Change::Graph,
                "{code}/{module}"
            );
        }
    }
}

#[test]
fn uncertain_unicode_identifiers_expand_instead_of_guessing_nfc() {
    let input = Sources::from([
        (
            String::from("crates/domain/src/lib.rs"),
            String::from("pub fn é() {}"),
        ),
        (
            String::from("crates/domain/src/consumer.rs"),
            String::from("pub fn call() { crate::e\u{301}(); }"),
        ),
    ]);
    super::graph::Graph::read(&input)
        .err()
        .expect("normalization uncertainty");
    assert_eq!(
        classify("fn old() {}", "fn old() {} fn é() {}"),
        Change::Graph
    );
    let before = "#[cfg(test)] mod tests { #[test] fn stable() { é(); } }";
    let after = "#[cfg(test)] mod tests { #[test] fn stable() { é(); } fn e\u{301}() {} }";
    let prior = crate::test_diff::Sources::from([(
        String::from("crates/domain/src/lib.rs"),
        String::from(before),
    )]);
    let current = crate::test_diff::Sources::from([(
        String::from("crates/domain/src/lib.rs"),
        String::from(after),
    )]);
    assert!(
        crate::test_diff::compare_sources(&prior, &current)
            .iter()
            .any(|finding| finding.rule.to_string().contains("shadowing"))
    );
}

#[test]
fn registration_import_origin_and_mixed_kinds_are_observable() {
    let mut before = sources("pub const LIMIT: usize = 17;");
    before.insert(
        String::from("crates/domain/src/foreign.rs"),
        String::from("pub struct Data; pub trait Mixed {} pub mod duplicate { pub struct Mixed; }"),
    );
    for import in [
        "pub use external::Data as Fresh;",
        "pub use crate::foreign::Mixed as Fresh;",
    ] {
        let mut after = before.clone();
        after.insert(
            String::from("crates/domain/src/lib.rs"),
            format!("mod constants; mod consumer; {import}"),
        );
        assert_eq!(
            Additions::read(&before, &after)
                .expect("context")
                .classify("crates/domain/src/lib.rs"),
            Change::Graph,
            "{import}"
        );
    }
}

#[test]
fn existing_build_scripts_are_non_rust_execution_inputs() {
    let mut before = sources("pub const LIMIT: usize = 17;");
    before.insert(
        String::from("crates/domain/build.rs"),
        String::from("fn main() {}"),
    );
    let mut after = before.clone();
    after.insert(
        String::from("crates/domain/build.rs"),
        String::from("fn main() {} fn novel() {}"),
    );
    super::scope::select_sources(
        &before,
        &after,
        &[String::from("crates/domain/src/**/*.rs")],
        &BTreeSet::from([String::from("crates/domain/build.rs")]),
        "crates/domain",
    )
    .expect_err("build script is not a production module");
}

#[test]
fn prelude_reexports_cannot_replace_implicit_names() {
    let mut before = sources("pub const LIMIT: usize = 17;");
    before.insert(
        String::from("crates/domain/src/foreign.rs"),
        String::from("pub struct Data;"),
    );
    for binding in ["r#Option", "Iterator", "drop"] {
        let mut after = before.clone();
        after.insert(
            String::from("crates/domain/src/lib.rs"),
            format!("mod constants; mod consumer; pub use crate::foreign::Data as {binding};"),
        );
        assert_eq!(
            Additions::read(&before, &after)
                .expect("context")
                .classify("crates/domain/src/lib.rs"),
            Change::Graph,
            "{binding}"
        );
    }
}

#[test]
fn declared_child_module_imports_are_not_external_preludes() {
    let mut before = sources("pub const LIMIT: usize = 17;");
    before.insert(
        String::from("crates/domain/src/container.rs"),
        String::from("mod evidence; pub use evidence::Evidence;"),
    );
    before.insert(
        String::from("crates/domain/src/container/evidence.rs"),
        String::from("pub struct Evidence;"),
    );
    let mut after = before.clone();
    after.insert(
        String::from("crates/domain/src/fresh.rs"),
        String::from("#[cfg(test)] mod tests { fn evidence() {} }"),
    );
    assert_eq!(
        Additions::read(&before, &after)
            .expect("context")
            .classify("crates/domain/src/fresh.rs"),
        Change::OnlyFile
    );
}
