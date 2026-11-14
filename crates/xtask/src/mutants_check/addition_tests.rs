use super::{
    additions::{Additions, Change},
    graph::Sources,
};

fn files(source: &str) -> Sources {
    Sources::from([
        (
            String::from("crates/domain/src/lib.rs"),
            String::from("mod sample;"),
        ),
        (
            String::from("crates/domain/src/sample.rs"),
            String::from(source),
        ),
    ])
}

fn classified(before: &str, after: &str) -> Change {
    Additions::read(&files(before), &files(after))
        .expect("context")
        .classify("crates/domain/src/sample.rs")
}

#[test]
fn pure_items_and_new_nominal_impls_are_additive() {
    for added in [
        "pub fn novel() {}",
        "pub const NOVEL: usize = 1;",
        "pub static NOVEL: usize = 1;",
        "pub struct Novel; impl Novel { pub fn value(&self) {} }",
        "pub enum Novel { One }",
        "pub struct Novel<T>(T); impl<T> Novel<T> { pub fn value(&self) {} }",
        "pub type Novel = usize;",
        "pub trait Novel { fn value(&self); }",
    ] {
        assert_eq!(
            classified("fn old() {}", &format!("fn old() {{}} {added}")),
            Change::OnlyFile,
            "{added}"
        );
    }
}

#[test]
fn apparent_additions_that_change_old_resolution_are_rejected() {
    let base = "pub struct Old; pub trait Existing { fn score(&self); } impl Existing for Old { fn score(&self) {} } fn original() {} #[cfg(test)] mod tests { #[test] fn old_test() {} }";
    for changed in [
        format!("{base} impl Old {{ fn score(&self) {{}} }}"),
        format!("{base} pub struct Fresh; impl ::Fresh {{ fn score(&self) {{}} }}"),
        format!("{base} impl Clone for Old {{ fn clone(&self) -> Self {{ Old }} }}"),
        base.replace("fn original() {}", "fn original() { return; }"),
        base.replace("#[test] fn old_test() {}", ""),
        base.replace("#[test] fn old_test() {}", "#[test] fn novel_test() {}"),
        base.replace("fn old_test() {}", "fn old_test() { return; }"),
        format!(
            "{} fn novel() {{}}",
            base.replace("fn old_test() {}", "fn old_test() { return; }")
        ),
        base.replace("fn original()", "#[inline] fn original()"),
        format!("{base} use crate::*; fn novel() {{}}"),
        format!("{base} macro_rules! novel {{ () => {{}}; }}"),
        base.replace(
            "fn score(&self) {}",
            "fn score(&self) {} fn novel(&self) {}",
        ),
        base.replace(
            "#[cfg(test)] mod tests { #[test] fn old_test() {} }",
            "#[test] fn old_test() {}",
        ),
        format!("{base} type Alias = Old; impl Alias {{ fn score(&self) {{}} }}"),
        format!(
            "{base} use crate::sample::Existing as FreshTrait; fn novel(value: Old) {{ FreshTrait::score(&value); }}"
        ),
    ] {
        assert_eq!(classified(base, &changed), Change::Graph, "{changed}");
    }
}

#[test]
fn test_module_additions_keep_all_old_tokens() {
    let before = "#[cfg(test)] mod tests { #[test] fn old_test() {} }";
    assert_eq!(
        classified(
            before,
            "#[cfg(test)] mod tests { #[test] fn old_test() {} #[test] fn novel_test() {} fn novel_helper() {} }"
        ),
        Change::OnlyFile
    );
    assert_eq!(classified("fn old() {}", "fn old() { }"), Change::Graph);
}

fn select(
    before: &Sources,
    after: &Sources,
    names: &[&str],
) -> Result<std::collections::BTreeSet<String>, String> {
    super::scope::select_sources(
        before,
        after,
        &[String::from("crates/domain/src/**/*.rs")],
        &names
            .iter()
            .map(|name| format!("crates/domain/src/{name}.rs"))
            .collect(),
        "crates/domain",
    )
}

#[test]
fn new_files_and_pure_registration_do_not_expand() {
    let before = files("pub fn old() {}");
    let mut after = before.clone();
    after.insert(
        String::from("crates/domain/src/fresh.rs"),
        String::from(
            "pub struct Fresh; impl Fresh { pub fn value(&self) { crate::sample::old(); } }",
        ),
    );
    after.insert(
        String::from("crates/domain/src/lib.rs"),
        String::from("mod sample; mod fresh; pub use fresh::Fresh;"),
    );
    let additions = Additions::read(&before, &after).expect("context");
    assert_eq!(
        additions.classify("crates/domain/src/fresh.rs"),
        Change::OnlyFile
    );
    assert_eq!(
        additions.classify("crates/domain/src/lib.rs"),
        Change::Registration
    );
    assert_eq!(
        select(&before, &after, &["fresh", "lib"]).expect("selection"),
        std::collections::BTreeSet::from([String::from("crates/domain/src/fresh.rs")])
    );
}

#[test]
fn registration_rejects_globs_collisions_old_modules_and_traits() {
    let mut before = files("pub struct Old; pub trait Existing {}");
    before.insert(
        String::from("crates/domain/src/lib.rs"),
        String::from("mod sample; pub use sample::Old;"),
    );
    for addition in [
        "pub use sample::*;",
        "pub use sample::Existing as Old;",
        "pub use sample::Old as Old;",
        "pub use sample::Existing as FreshTrait;",
        "mod sample;",
        "mod fresh { pub fn value() {} }",
        "pub use sample::Old as _;",
    ] {
        let mut after = before.clone();
        after.insert(
            String::from("crates/domain/src/lib.rs"),
            format!("mod sample; pub use sample::Old; {addition}"),
        );
        assert_eq!(
            Additions::read(&before, &after)
                .expect("context")
                .classify("crates/domain/src/lib.rs"),
            Change::Graph,
            "{addition}"
        );
    }
}

#[test]
fn crate_wide_fresh_names_and_safe_imports_are_required() {
    let mut before = files("pub fn old() {}");
    before.insert(String::from("crates/domain/src/other.rs"), String::from("pub struct Other; pub fn duplicated() {} pub trait Existing {} pub use crate::other::Existing as Alias; pub use crate::other::Other as PublicOther;"));
    for (addition, expected) in [
        ("pub fn duplicated() {}", Change::Graph),
        (
            "use crate::other::Other; pub fn novel(_: Other) {}",
            Change::OnlyFile,
        ),
        ("use crate::other::Other;", Change::Graph),
        ("use crate::other::Other; pub fn novel() {}", Change::Graph),
        (
            "use crate::other::PublicOther; pub fn novel(_: PublicOther) {}",
            Change::OnlyFile,
        ),
        (
            "use std::fmt::Display as Fresh; pub fn novel(_: impl Fresh) {}",
            Change::Graph,
        ),
        (
            "use crate::other::Alias as Fresh; pub fn novel(_: impl Fresh) {}",
            Change::Graph,
        ),
        (
            "use crate::other::Other as old; pub fn novel(_: old) {}",
            Change::Graph,
        ),
    ] {
        let mut after = before.clone();
        after.insert(
            String::from("crates/domain/src/sample.rs"),
            format!("pub fn old() {{}} {addition}"),
        );
        assert_eq!(
            Additions::read(&before, &after)
                .expect("context")
                .classify("crates/domain/src/sample.rs"),
            expected,
            "{addition}"
        );
    }
}

#[test]
fn new_files_cannot_add_existing_type_impls_or_global_macros() {
    let before = files("pub struct Old;");
    for source in [
        "use crate::sample::Old; impl Old { fn novel(&self) {} }",
        "use crate::sample::Old; impl Clone for Old { fn clone(&self) -> Self { Old } }",
        "macro_rules! novel { () => {}; }",
        "#![cfg(test)] pub fn novel() {}",
        "#[path = \"hidden.rs\"] mod hidden;",
        "pub struct Fresh; mod nested { type Alias = crate::sample::Old; impl Alias { fn novel(&self) {} } }",
        "pub struct Fresh; mod nested { type Fresh = crate::sample::Old; impl Fresh { fn novel(&self) {} } }",
    ] {
        let mut after = before.clone();
        after.insert(
            String::from("crates/domain/src/fresh.rs"),
            String::from(source),
        );
        assert_eq!(
            Additions::read(&before, &after)
                .expect("context")
                .classify("crates/domain/src/fresh.rs"),
            Change::Graph,
            "{source}"
        );
    }
}

#[test]
fn mixed_changes_union_additive_files_with_graph_and_keep_wide_guards() {
    let mut before = files("pub fn old() -> usize { 1 }");
    before.insert(
        String::from("crates/domain/src/consumer.rs"),
        String::from("pub fn consumer() -> usize { crate::sample::old() }"),
    );
    let mut after = before.clone();
    after.insert(
        String::from("crates/domain/src/sample.rs"),
        String::from("pub fn old() -> usize { 2 }"),
    );
    after.insert(
        String::from("crates/domain/src/fresh.rs"),
        String::from("pub fn novel() {}"),
    );
    let chosen = select(&before, &after, &["sample", "fresh"]).expect("selection");
    for name in ["sample", "consumer", "fresh"] {
        assert!(
            chosen.contains(&format!("crates/domain/src/{name}.rs")),
            "{name}"
        );
    }
    after.remove("crates/domain/src/sample.rs");
    select(&before, &after, &["sample", "fresh"]).expect_err("deletion widens");
    super::scope::select_sources(
        &before,
        &after,
        &[String::from("crates/domain/src/**/*.rs")],
        &std::collections::BTreeSet::from([String::from("crates/domain/Cargo.toml")]),
        "crates/domain",
    )
    .expect_err("manifest widens");
}

#[test]
fn actual_shadowing_attacks_use_graph_instead_of_addition_shortcuts() {
    let base = "pub struct Value; pub trait Old { fn read(&self) -> usize; } impl Old for Value { fn read(&self) -> usize { crate::victim::boundary() } } pub fn score() -> usize { Value.read() }";
    for attack in [
        "impl Value { pub fn read(&self) -> usize { 1 } }",
        "impl Value { pub const SCORE: usize = 1; }",
    ] {
        assert_eq!(classified(base, &format!("{base} {attack}")), Change::Graph);
    }
    let mut before = files(base);
    before.insert(String::from("crates/domain/src/victim.rs"), String::from("pub fn boundary() -> usize { 1 } pub trait FreshTrait { fn read(&self) -> usize; } impl FreshTrait for crate::sample::Value { fn read(&self) -> usize { 1 } }"));
    let mut after = before.clone();
    after.insert(String::from("crates/domain/src/sample.rs"), format!("{base} use crate::victim::FreshTrait as ScopeFresh; pub fn novel(value: Value) {{ ScopeFresh::read(&value); }}"));
    assert_eq!(
        Additions::read(&before, &after)
            .expect("context")
            .classify("crates/domain/src/sample.rs"),
        Change::Graph
    );
    assert!(
        select(&before, &after, &["sample"])
            .expect("graph")
            .contains("crates/domain/src/victim.rs")
    );
}

#[test]
fn diff_command_requires_test_diff_at_the_resolved_base() {
    use crate::git_base::{git, output};
    let root = crate::harness_workspace_tests::fresh_directory(
        &crate::harness_workspace_tests::repository_root(),
        "h43-addition-gate",
    );
    for (path, source) in [
        (
            "Cargo.lock",
            "version = 4\n[[package]]\nname='domain'\nversion='0.1.0'\n",
        ),
        (
            "Cargo.toml",
            "[workspace]\nmembers=['crates/domain']\nresolver='2'\n",
        ),
        (
            "crates/domain/Cargo.toml",
            "[package]\nname='domain'\nversion='0.1.0'\nedition='2024'\n",
        ),
        (
            ".cargo/mutants.toml",
            "examine_globs=['crates/domain/src/**/*.rs']\n",
        ),
        ("crates/domain/src/lib.rs", "pub mod sample;"),
        (
            "crates/domain/src/sample.rs",
            "pub fn value() -> usize { 1 } #[cfg(test)] mod tests { #[test] fn stable() { assert_eq!(super::value(), 1); } }",
        ),
    ] {
        let full = root.join(path);
        std::fs::create_dir_all(full.parent().expect("parent")).expect("directory");
        std::fs::write(full, source).expect("source");
    }
    for args in [
        vec!["init"],
        vec!["add", "."],
        vec![
            "-c",
            "user.name=Harness",
            "-c",
            "user.email=harness@example.invalid",
            "commit",
            "--no-verify",
            "-m",
            "fixture",
        ],
    ] {
        let mut command = git(&root);
        command.args(args);
        output(command, "fixture").expect("git");
    }
    let args = [String::from("--in-diff"), String::from("HEAD")];
    super::checked_scope(&root, &args).expect("baseline passes");
    std::fs::write(root.join("crates/domain/src/sample.rs"), "pub fn value() -> usize { 1 } #[cfg(test)] mod tests { #[test] fn stable() { assert!(true); } }").expect("weaken test");
    let error = super::checked_scope(&root, &args).expect_err("test-diff required");
    assert!(error.contains("test-diff 실패"), "{error}");
    assert!(error.contains("test-diff:modified"), "{error}");
}

#[test]
fn addition_shortcuts_keep_graph_uncertainty_and_file_attributes() {
    let before = files("fn old() {}");
    for source in [
        "pub fn novel() { crate::missing::call(); }",
        "pub fn novel() { unknown!(); }",
        "#[path = \"hidden.rs\"] mod hidden;",
    ] {
        let mut after = before.clone();
        after.insert(
            String::from("crates/domain/src/fresh.rs"),
            String::from(source),
        );
        select(&before, &after, &["fresh"]).expect_err("uncertainty widens");
    }
    assert_eq!(
        classified("fn old() {}", "#![cfg(test)] fn old() {} fn novel() {}"),
        Change::Graph
    );
    assert_eq!(
        classified("fn old() {}", "fn old() {} assert_eq!(1, 1);"),
        Change::Graph
    );
}

#[test]
fn nested_aliases_cannot_borrow_another_scopes_new_type_name() {
    let before = "pub struct Old; #[cfg(test)] mod tests { #[test] fn stable() {} }";
    for after in [
        "pub struct Old; #[cfg(test)] mod tests { #[test] fn stable() {} use crate::sample::Old as Fresh; impl Fresh { fn score(&self) {} } } pub struct Fresh;",
        "pub struct Old; #[cfg(test)] mod tests { #[test] fn stable() {} pub struct Fresh; } use crate::sample::Old as Fresh; impl Fresh { fn score(&self) {} }",
    ] {
        assert_eq!(classified(before, after), Change::Graph, "{after}");
    }
}

#[test]
fn generic_parameters_and_cfg_types_cannot_prove_new_impl_ownership() {
    let before = "pub struct Old; pub trait Candidate { fn read(self); }";
    for addition in [
        "pub struct Fresh; impl<Fresh> Candidate for Fresh { fn read(self) {} }",
        "#[cfg(not(test))] pub struct Fresh; #[cfg(test)] pub type Fresh = Old; impl Fresh { fn read(&self) {} }",
    ] {
        assert_eq!(
            classified(before, &format!("{before} {addition}")),
            Change::Graph
        );
    }
}

#[test]
fn existing_glob_scopes_cannot_certify_fresh_bindings() {
    let base = "use std::borrow::*; fn old() {}";
    assert_eq!(
        classified(base, &format!("{base} pub struct Borrow;")),
        Change::Graph
    );
    let mut before = files(base);
    before.insert(
        String::from("crates/domain/src/lib.rs"),
        String::from("mod sample;"),
    );
    let mut after = before.clone();
    after.insert(
        String::from("crates/domain/src/fresh.rs"),
        String::from("pub struct Borrow;"),
    );
    after.insert(
        String::from("crates/domain/src/lib.rs"),
        String::from("mod sample; mod fresh; pub use fresh::Borrow;"),
    );
    assert_eq!(
        Additions::read(&before, &after)
            .expect("context")
            .classify("crates/domain/src/lib.rs"),
        Change::Graph
    );
}

#[test]
fn opaque_macro_items_cannot_hide_impls_or_imports() {
    let base = "pub struct Old; fn old() {}";
    assert_eq!(
        classified(
            base,
            &format!(
                "{base} pub fn novel() {{ assert!({{ impl Old {{ fn read(&self) {{}} }} true }}); }}"
            )
        ),
        Change::Graph
    );
    assert_eq!(
        classified(
            base,
            &format!("{base} pub fn novel() {{ assert_eq!(crate::sample::old(), ()); }}")
        ),
        Change::OnlyFile
    );
    let opaque = "fn old() { assert!({ use std::borrow::*; true }); }";
    assert_eq!(
        classified(opaque, &format!("{opaque} pub struct Borrow;")),
        Change::Graph
    );
    let before = files(base);
    let mut after = before.clone();
    after.insert(
        String::from("crates/domain/src/fresh.rs"),
        String::from(
            "pub fn novel() { assert!({ impl crate::sample::Old { fn read(&self) {} } true }); }",
        ),
    );
    assert_eq!(
        Additions::read(&before, &after)
            .expect("context")
            .classify("crates/domain/src/fresh.rs"),
        Change::Graph
    );
}
