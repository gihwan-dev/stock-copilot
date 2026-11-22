use crate::tests::Fixture;
use std::path::{Path, PathBuf};
use std::process::Output;

fn write_manifest(directory: &Path, root: &Path) {
    let application = root
        .join("../../application")
        .canonicalize()
        .expect("application");
    let domain = root.join("../../domain").canonicalize().expect("domain");
    let mut manifest = String::from(
        "[package]\nname = \"proposal-boundary-probe\"\nversion = \"0.0.0\"\nedition = \"2024\"\n[workspace]\n[dependencies]\napplication = { path = \"",
    );
    let application_path = application
        .to_string_lossy()
        .replace('\\', "\\\\")
        .replace('\"', "\\\"");
    let domain_path = domain
        .to_string_lossy()
        .replace('\\', "\\\\")
        .replace('\"', "\\\"");
    manifest.push_str(&application_path);
    manifest.push_str("\" }\ndomain = { path = \"");
    manifest.push_str(&domain_path);
    manifest.push_str("\" }\n");
    std::fs::write(directory.join("Cargo.toml"), manifest).expect("manifest");
}
fn check(directory: &Path, source: &str) -> Output {
    std::fs::write(directory.join("src/main.rs"), source).expect("source");
    testkit::check_consumer(&directory.join("Cargo.toml"), &directory.join("target"))
        .expect("cargo")
}
fn failed_with_type_mismatch(output: &Output) {
    let diagnostics = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "negative probe must fail");
    assert!(diagnostics.contains("error[E0308]"), "{diagnostics}");
    assert!(diagnostics.contains("Validated"), "{diagnostics}");
    assert!(diagnostics.contains("Draft"), "{diagnostics}");
}
fn failed_with_missing_method(output: &Output, method: &str) {
    let diagnostics = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "missing method probe must fail");
    assert!(diagnostics.contains("error[E0599]"), "{diagnostics}");
    assert!(diagnostics.contains(method), "{diagnostics}");
}
#[test]
fn proposal_repository_never_accepts_or_returns_validated_proposals() {
    let fixture = Fixture::new();
    let directory: PathBuf = fixture.path().with_file_name("compile-check");
    std::fs::create_dir_all(directory.join("src")).expect("directory");
    write_manifest(&directory, &std::env::current_dir().expect("package root"));
    let control = std::fs::read_to_string("tests/proposal-boundaries/draft.txt").expect("control");
    let stored_validated = std::fs::read_to_string("tests/proposal-boundaries/store-validated.txt")
        .expect("store probe");
    let restored_validated =
        std::fs::read_to_string("tests/proposal-boundaries/load-validated.txt")
            .expect("load probe");
    let forged_report =
        std::fs::read_to_string("tests/proposal-boundaries/report-to-validated.txt")
            .expect("report probe");
    let no_store = std::fs::read_to_string("tests/proposal-boundaries/no-store-method.txt")
        .expect("missing store probe");
    let no_load = std::fs::read_to_string("tests/proposal-boundaries/no-load-method.txt")
        .expect("missing load probe");
    let valid = check(&directory, &control);
    assert!(
        valid.status.success(),
        "{}",
        String::from_utf8_lossy(&valid.stderr)
    );
    failed_with_type_mismatch(&check(&directory, &stored_validated));
    failed_with_type_mismatch(&check(&directory, &restored_validated));
    let denied_report = check(&directory, &forged_report);
    let diagnostics = String::from_utf8_lossy(&denied_report.stderr);
    assert!(!denied_report.status.success(), "report probe must fail");
    assert!(diagnostics.contains("error[E0624]"), "{diagnostics}");
    assert!(diagnostics.contains("finish_validation"), "{diagnostics}");
    failed_with_missing_method(&check(&directory, &no_store), "store_validated");
    failed_with_missing_method(&check(&directory, &no_load), "validated");
}
