use std::collections::BTreeSet;
use std::fs;

use crate::harness_workspace_tests::repository_root;

#[test]
fn every_component_has_a_committed_generated_type() {
    let root = repository_root();
    let input = fs::read_to_string(root.join("docs/toss-openapi-1.2.28.json")).expect("spec");
    let spec = crate::capability_check::json::parse(&input).expect("JSON");
    let source = fs::read_to_string(root.join("crates/adapters/broker-toss-schema/src/types.rs"))
        .expect("committed generated types");
    let file = syn::parse_file(&source).expect("generated Rust");
    let names: BTreeSet<String> = file
        .items
        .iter()
        .filter_map(|item| {
            if let syn::Item::Struct(structure) = item {
                return Some(structure.ident.to_string());
            }
            if let syn::Item::Enum(enumeration) = item {
                return Some(enumeration.ident.to_string());
            }
            if let syn::Item::Type(alias) = item {
                return Some(alias.ident.to_string());
            }
            None
        })
        .collect();
    let crate::capability_check::json::Json::Object(schemas) = spec
        .get("components")
        .and_then(|value| value.get("schemas"))
        .expect("components.schemas")
    else {
        panic!("schema object");
    };
    assert_eq!(schemas.len(), 109);
    for name in schemas.keys() {
        assert!(names.contains(name), "missing {name}");
    }
    assert!(!source.contains("f64"));
    assert!(!source.contains("chrono::"));
    assert!(!source.contains(".unwrap()"));
}
