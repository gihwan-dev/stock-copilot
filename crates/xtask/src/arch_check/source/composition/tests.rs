use super::{imports::Bindings, with_bindings};

fn forbidden(
    relative: &str,
    file: &syn::File,
    aliases: &std::collections::BTreeMap<String, Vec<String>>,
) -> bool {
    with_bindings(relative, file, Bindings::of(file, aliases))
}

#[test]
fn every_composition_fixture_enforces_the_constructor_boundary() {
    let text = std::fs::read_to_string("../../harness/arch-check/composition-cases.toml")
        .expect("고정 자료");
    let table: toml::Table = toml::from_str(&text).expect("TOML");
    for case in table["case"].as_array().expect("사례") {
        let name = case["name"].as_str().expect("이름");
        let file = syn::parse_file(case["source"].as_str().expect("소스")).expect("Rust");
        let aliases = std::collections::BTreeMap::from([(
            String::from("renamed_platform"),
            vec![String::from("platform_macos")],
        )]);
        let expected = case["forbidden"].as_bool().expect("기대값");
        assert_eq!(
            forbidden("apps/desktop/src-tauri/src/lib.rs", &file, &aliases),
            expected,
            "{name}"
        );
        assert!(
            !forbidden("apps/desktop/src-tauri/src/main.rs", &file, &aliases),
            "main: {name}"
        );
        assert!(
            !forbidden("crates/adapters/platform-macos/src/lib.rs", &file, &aliases),
            "adapter: {name}"
        );
    }
}
