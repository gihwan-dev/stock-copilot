use crate::tests::Fixture;
use std::path::{Path, PathBuf};
use std::process::Output;

fn dependency_line(name: &str, path: &Path) -> String {
    let canonical = path.canonicalize().expect("dependency");
    let escaped = canonical
        .to_string_lossy()
        .replace('\\', "\\\\")
        .replace('\"', "\\\"");
    let mut line = String::from(name);
    line.push_str(" = { path = \"");
    line.push_str(&escaped);
    line.push_str("\" }\n");
    line
}

fn write_manifest(directory: &Path, root: &Path) {
    let mut manifest = String::from(
        "[package]\nname = \"signal-store-boundary-probe\"\nversion = \"0.0.0\"\nedition = \"2024\"\n[workspace]\n[dependencies]\n",
    );
    manifest.push_str(&dependency_line(
        "application",
        &root.join("../../application"),
    ));
    manifest.push_str(&dependency_line("domain", &root.join("../../domain")));
    manifest.push_str(&dependency_line("store-sqlite", root));
    std::fs::write(directory.join("Cargo.toml"), manifest).expect("manifest");
}

fn check(directory: &Path, source: &str) -> Output {
    std::fs::write(directory.join("src/main.rs"), source).expect("source");
    testkit::check_consumer(&directory.join("Cargo.toml"), &directory.join("target"))
        .expect("cargo")
}

#[test]
fn signal_store_never_accepts_or_returns_validated_proposals() {
    let fixture = Fixture::new();
    let directory: PathBuf = fixture.path().with_file_name("signal-store-probe");
    std::fs::create_dir_all(directory.join("src")).expect("directory");
    write_manifest(&directory, &std::env::current_dir().expect("package root"));
    let control = std::fs::read_to_string("tests/proposal-boundaries/signal-store-draft.txt")
        .expect("control");
    let negative = std::fs::read_to_string("tests/proposal-boundaries/signal-store-validated.txt")
        .expect("negative");
    let valid = check(&directory, &control);
    assert!(
        valid.status.success(),
        "{}",
        String::from_utf8_lossy(&valid.stderr)
    );
    let denied = check(&directory, &negative);
    let diagnostics = String::from_utf8_lossy(&denied.stderr);
    assert!(!denied.status.success(), "negative probe must fail");
    assert_eq!(
        diagnostics.matches("error[E0308]").count(),
        2,
        "{diagnostics}"
    );
    assert!(diagnostics.contains("Validated"), "{diagnostics}");
    assert!(diagnostics.contains("Draft"), "{diagnostics}");
    assert!(diagnostics.contains("error[E0599]"), "{diagnostics}");
    assert!(diagnostics.contains("store_validated"), "{diagnostics}");
    assert!(diagnostics.contains("`validated`"), "{diagnostics}");
}
