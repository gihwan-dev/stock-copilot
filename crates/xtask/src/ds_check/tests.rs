use std::fs;
use std::path::{Path, PathBuf};

use super::{
    BASELINE_DIR, CATALOG_DIR, INDEX_FILE, LOCK_FILE, Rule, STRESS_DIR, check_repository,
    exported_components,
};
use crate::check_support::workspace_root;
use crate::private_temp::PrivateDirectory;

fn write(root: &Path, relative: &str, text: &[u8]) {
    let path = root.join(relative);
    fs::create_dir_all(path.parent().expect("부모")).expect("폴더");
    fs::write(path, text).expect("쓰기");
}

fn fixture(directory: &PrivateDirectory) -> PathBuf {
    let root = directory.child("repo").expect("저장소");
    write(
        &root,
        INDEX_FILE,
        b"export { Button } from \"./Button\";\nexport { AppShell, type PanelState } from \"./AppShell\";\nexport type { ChartPoint } from \"./chart-types\";\n",
    );
    write(
        &root,
        LOCK_FILE,
        b"{\"components\":[\"Button\",\"AppShell\"]}",
    );
    for name in ["Button", "AppShell"] {
        write(
            &root,
            &format!("{CATALOG_DIR}/{name}.stories.mjs"),
            b"export const Light0 = {};\nexport const Dark0 = {};\n",
        );
        for theme in ["light", "dark"] {
            write(
                &root,
                &format!("{BASELINE_DIR}/{name}-{theme}-chromium-linux-arm64.png"),
                b"png",
            );
        }
    }
    write(
        &root,
        &format!("{STRESS_DIR}/Stress.stories.tsx"),
        b"[{ component: \"Button\" }, { component: \"AppShell\" }]",
    );
    root
}

fn rules(root: &Path) -> Vec<Rule> {
    check_repository(root)
        .expect("검사")
        .into_iter()
        .map(|finding| finding.rule)
        .collect()
}

#[test]
fn exports_skip_type_only_names() {
    assert_eq!(
        exported_components(
            "export { A, type B } from \"./a\";\nexport type { C } from \"./c\";\nexport { D as E } from \"./d\";"
        ),
        vec![String::from("A"), String::from("E")]
    );
}

#[test]
fn complete_fixture_passes_and_each_missing_piece_fails() {
    let directory = PrivateDirectory::create("ds-check-").expect("임시");
    let root = fixture(&directory);
    assert_eq!(rules(&root), Vec::<Rule>::new());

    fs::remove_file(root.join(format!(
        "{BASELINE_DIR}/Button-dark-chromium-linux-arm64.png"
    )))
    .expect("지움");
    assert_eq!(
        rules(&root),
        vec![Rule::MissingBaseline(String::from(
            "Button-dark-chromium-linux-arm64.png"
        ))]
    );
    write(
        &root,
        &format!("{BASELINE_DIR}/Button-dark-chromium-linux-arm64.png"),
        b"",
    );
    assert_eq!(rules(&root).len(), 1);
    write(
        &root,
        &format!("{BASELINE_DIR}/Button-dark-chromium-linux-arm64.png"),
        b"png",
    );

    write(
        &root,
        &format!("{CATALOG_DIR}/AppShell.stories.mjs"),
        b"export const Light0 = {};\n",
    );
    assert_eq!(
        rules(&root),
        vec![Rule::MissingCatalogEntry(String::from("AppShell"))]
    );
    write(
        &root,
        &format!("{CATALOG_DIR}/AppShell.stories.mjs"),
        b"export const Light0 = {};\nexport const Dark0 = {};\n",
    );

    write(
        &root,
        &format!("{STRESS_DIR}/Stress.stories.tsx"),
        b"[{ component: \"Button\" }]",
    );
    assert_eq!(
        rules(&root),
        vec![Rule::MissingStateStory(String::from("AppShell"))]
    );
    directory.remove().expect("정리");
}

#[test]
fn index_edits_outside_the_locked_list_fail() {
    let directory = PrivateDirectory::create("ds-check-").expect("임시");
    let root = fixture(&directory);
    write(&root, LOCK_FILE, b"{\"components\":[\"Button\",\"Card\"]}");
    assert_eq!(
        rules(&root),
        vec![
            Rule::PublicListChanged(String::from("added:AppShell")),
            Rule::PublicListChanged(String::from("removed:Card")),
        ]
    );
    directory.remove().expect("정리");
}

#[test]
fn repository_design_system_is_complete() {
    let root = workspace_root().expect("작업 공간");
    let findings: Vec<String> = check_repository(&root)
        .expect("검사")
        .iter()
        .map(ToString::to_string)
        .collect();
    assert_eq!(findings, Vec::<String>::new());
}
