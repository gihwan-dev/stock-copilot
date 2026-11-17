use std::collections::BTreeSet;

use std::fs;

use crate::harness_workspace_tests::{copy_tree, repository_root, unique_directory};

use super::{TAURI_DIRECTORY, check_u03_directory, check_u03_wiring};

#[test]
fn u03_requires_csp_prototype_freezing_and_event_subscription_permissions() {
    let root = repository_root();
    let findings: BTreeSet<String> = check_u03_directory(&root.join(TAURI_DIRECTORY))
        .into_iter()
        .map(|finding| finding.rule.to_string())
        .collect();
    assert!(findings.is_empty(), "U-03 설정 위반: {findings:?}");
}

#[test]
fn u03_security_fixtures_detect_each_regression() {
    let root = repository_root();
    let fixtures = root.join("harness/capability-check/u03");
    let table: toml::Table = fs::read_to_string(fixtures.join("cases.toml"))
        .expect("위반 사례 읽기")
        .parse()
        .expect("위반 사례 TOML");
    let cases = table["case"].as_array().expect("사례 배열");
    assert!(!cases.is_empty(), "위반 사례가 있어야 한다");
    for case in cases {
        let name = case["name"].as_str().expect("사례 이름");
        let file = case["file"].as_str().expect("대상 파일");
        let before = case["before"].as_str().expect("바꿀 문자열");
        let after = case["after"].as_str().expect("바뀐 문자열");
        let expected = case["expected"].as_str().expect("기대 규칙");
        let directory = unique_directory(&root, "u03-capability");
        copy_tree(&fixtures.join("base"), &directory);
        let path = directory.join(file);
        let original = fs::read_to_string(&path).expect("기준 설정 읽기");
        assert_eq!(original.matches(before).count(), 1, "{name}: 교체 대상");
        fs::write(&path, original.replacen(before, after, 1)).expect("위반 설정 쓰기");
        let findings: BTreeSet<String> = check_u03_directory(&directory)
            .into_iter()
            .map(|finding| finding.rule.to_string())
            .collect();
        assert_eq!(findings, BTreeSet::from([String::from(expected)]), "{name}");
    }
}

#[test]
fn u03_security_fixture_baseline_is_accepted() {
    let root = repository_root();
    let directory = root.join("harness/capability-check/u03/base");
    assert!(check_u03_directory(&directory).is_empty());
}

#[test]
fn u03_navigation_wiring_fixtures_detect_each_regression() {
    let root = repository_root();
    let fixtures = root.join("harness/capability-check/u03/wiring");
    assert!(check_u03_wiring(&fixtures.join("base")).is_empty());
    let cases: toml::Table = fs::read_to_string(fixtures.join("cases.toml"))
        .expect("탐색 사례 읽기")
        .parse()
        .expect("탐색 사례 TOML");
    for case in cases["case"].as_array().expect("탐색 사례 배열") {
        let name = case["name"].as_str().expect("이름");
        let directory = unique_directory(&root, "u03-wiring");
        copy_tree(&fixtures.join("base"), &directory);
        let path = directory.join(case["file"].as_str().expect("대상"));
        let source = fs::read_to_string(&path).expect("기준 소스");
        let before = case["before"].as_str().expect("교체 전");
        let after = case["after"].as_str().expect("교체 후");
        assert_eq!(source.matches(before).count(), 1, "{name}: 교체 대상");
        fs::write(path, source.replacen(before, after, 1)).expect("변형 쓰기");
        let rules: BTreeSet<String> = check_u03_wiring(&directory)
            .into_iter()
            .map(|finding| finding.rule.to_string())
            .collect();
        assert_eq!(
            rules,
            BTreeSet::from([String::from("capability:u03-navigation")]),
            "{name}"
        );
    }
}
