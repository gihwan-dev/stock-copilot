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
    let copy = generated_copy("t01-drift");
    let input = fs::read_to_string(copy.join(INPUT)).expect("spec");
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

fn generated_copy(name: &str) -> std::path::PathBuf {
    let root = repository_root();
    let copy = fresh_directory(&root, name);
    fs::create_dir_all(copy.join("docs")).expect("docs");
    fs::create_dir_all(copy.join(OUTPUT)).expect("output");
    fs::copy(root.join(INPUT), copy.join(INPUT)).expect("spec copy");
    crate::harness_workspace_tests::place_git_directory_outside(&copy, &["init"]);
    git_at(&copy, &["init", "--quiet"]);
    generate(&copy, Mode::Write).expect("generated copy");
    git_at(&copy, &["add", "crates/adapters/broker-toss-schema"]);
    copy
}

#[test]
fn every_generated_file_is_byte_checked() {
    let copy = generated_copy("t01-file-drift");
    for name in ["types.rs", "../Cargo.toml"] {
        let path = copy.join(OUTPUT).join(name);
        let original = fs::read_to_string(&path).expect("generated file");
        fs::write(&path, format!("{original}\n")).expect("mutate one byte");
        let error = generate(&copy, Mode::Check).expect_err("file drift rejected");
        assert!(error.contains("재생성 차이"), "{error}");
        fs::write(&path, original).expect("restore");
    }
}

#[test]
fn crate_rejects_handwritten_files_binaries_tests_and_manifest_root_override() {
    let copy = generated_copy("t01-inventory");
    let directory = copy.join(OUTPUT).parent().expect("crate").to_path_buf();
    for name in ["src/extra.rs", "src/bin/x.rs", "tests/t.rs", "ignored.data"] {
        let path = directory.join(name);
        fs::create_dir_all(path.parent().expect("parent")).expect("directory");
        fs::write(&path, "// handwritten\npub const X: f64 = 1.0;\n").expect("handwritten file");
        let _error = generate(&copy, Mode::Check).expect_err("extra file rejected");
        fs::remove_file(&path).expect("remove own file");
        if name.starts_with("src/bin/") {
            fs::remove_dir(directory.join("src/bin")).expect("remove bin");
        }
        if name.starts_with("tests/") {
            fs::remove_dir(directory.join("tests")).expect("remove tests");
        }
    }
    let path = directory.join("Cargo.toml");
    let original = fs::read_to_string(&path).expect("manifest");
    fs::write(
        &path,
        format!("{original}\n[lib]\npath = \"src/hand.rs\"\n"),
    )
    .expect("root override");
    let _error = generate(&copy, Mode::Check).expect_err("manifest override rejected");
}

#[test]
fn crate_rejects_extra_directories_and_symlinks() {
    let copy = generated_copy("t01-links");
    let directory = copy.join(OUTPUT).parent().expect("crate").to_path_buf();
    fs::create_dir_all(directory.join("empty")).expect("extra directory");
    let _error = generate(&copy, Mode::Check).expect_err("extra directory");
    fs::remove_dir(directory.join("empty")).expect("remove own directory");
    std::os::unix::fs::symlink("lib.rs", directory.join("src/link.rs")).expect("symlink");
    let _link_error = generate(&copy, Mode::Check).expect_err("symlink");
}

fn git_at(root: &std::path::Path, arguments: &[&str]) {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(root)
        .args(arguments)
        .output()
        .expect("git fixture");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn inventory_rejects_deleted_tracked_files_and_ignored_files() {
    let copy = generated_copy("t01-index-inventory");
    let path = copy.join("crates/adapters/broker-toss-schema/src/extra.rs");
    fs::write(&path, "pub struct Handwritten;\n").expect("extra");
    git_at(
        &copy,
        &["add", "crates/adapters/broker-toss-schema/src/extra.rs"],
    );
    fs::remove_file(path).expect("deleted tracked file");
    let error = generate(&copy, Mode::Check).expect_err("tracked extra retained in index");
    assert!(error.contains("추가 추적 파일"));
    git_at(
        &copy,
        &[
            "rm",
            "--cached",
            "crates/adapters/broker-toss-schema/src/extra.rs",
        ],
    );
    fs::write(copy.join(".gitignore"), "ignored.data\n").expect("ignore rule outside crate");
    fs::write(
        copy.join("crates/adapters/broker-toss-schema/ignored.data"),
        "ignored",
    )
    .expect("ignored file");
    git_at(
        &copy,
        &[
            "check-ignore",
            "crates/adapters/broker-toss-schema/ignored.data",
        ],
    );
    let _ignored_error = generate(&copy, Mode::Check).expect_err("ignored extra rejected");
}

#[test]
fn check_requires_all_three_generated_files_in_the_index() {
    let copy = generated_copy("t01-untracked-allowed");
    git_at(
        &copy,
        &[
            "rm",
            "--cached",
            "crates/adapters/broker-toss-schema/src/types.rs",
        ],
    );
    let error = generate(&copy, Mode::Check).expect_err("untracked generated file");
    assert!(error.contains("추적 파일 집합"));
}

#[test]
fn schema_check_ignores_redirecting_git_environment() {
    let root = repository_root();
    let directory = fresh_directory(&root, "t01-git-environment");
    let output = std::process::Command::new("cargo")
        .args([
            "run",
            "--locked",
            "--offline",
            "--quiet",
            "--package",
            "xtask",
            "--",
            "gen-schema",
            "--check",
        ])
        .current_dir(&root)
        .env("GIT_INDEX_FILE", directory.join("fake-index"))
        .env("GIT_DIR", directory.join("fake-git"))
        .output()
        .expect("schema CLI");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
