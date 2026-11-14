use super::{INPUT, Mode, OUTPUT, generate, render};
use crate::harness_workspace_tests::{fresh_directory, repository_root};
use std::fs;

#[test]
fn generation_is_deterministic_and_preserves_constraints() {
    let input = fs::read_to_string(repository_root().join(INPUT)).expect("spec");
    let first = render(&input).expect("first generation");
    assert_eq!(first, render(&input).expect("second generation"));
    let [_, types] = first;
    assert!(types.contains("Regex::new"));
    assert!(types.contains(".map_err(|error| error.to_string())?"));
    assert!(!types.contains(".unwrap()"));
    assert!(!types.contains("f64"));
}

#[test]
fn regeneration_rejects_a_one_character_spec_copy_change() {
    let root = repository_root();
    let copy = fresh_directory(&root, "t01-drift");
    fs::create_dir_all(copy.join("docs")).expect("docs");
    fs::create_dir_all(copy.join(OUTPUT)).expect("output");
    let input = fs::read_to_string(root.join(INPUT)).expect("spec");
    fs::write(copy.join(INPUT), &input).expect("copy spec");
    generate(&copy, Mode::Write).expect("generate baseline");
    generate(&copy, Mode::Check).expect("baseline check");
    fs::write(copy.join(INPUT), format!("{input}\n")).expect("one character mutation");
    let error = generate(&copy, Mode::Check).expect_err("drift must fail");
    assert!(error.contains("재생성 차이"), "{error}");
}

#[test]
fn committed_generation_matches_local_spec() {
    generate(&repository_root(), Mode::Check).expect("committed generated files");
}

#[test]
fn schema_default_change_selects_the_new_variant() {
    let input = fs::read_to_string(repository_root().join(INPUT)).expect("spec");
    let changed = input.replace("\"default\": \"DAY\"", "\"default\": \"CLS\"");
    assert_ne!(input, changed);
    let [_, source] = render(&changed).expect("changed default");
    assert!(source.contains("#[default]\n    Cls"));
    assert!(!source.contains("#[default]\n    Day"));
}
