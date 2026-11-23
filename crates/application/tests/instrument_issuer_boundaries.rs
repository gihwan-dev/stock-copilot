#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    struct ScratchDir(PathBuf);

    impl ScratchDir {
        fn new(name: &str) -> Self {
            let path =
                Path::new("../../target/D-55").join(format!("{name}-{}", std::process::id()));
            fs::create_dir_all(path.join("src")).expect("임시 디렉터리");
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for ScratchDir {
        fn drop(&mut self) {
            let _removed = fs::remove_dir_all(&self.0);
        }
    }

    fn write_manifest(scratch: &Path) -> PathBuf {
        let application_path = fs::canonicalize(".").expect("application 경로");
        let domain_path = fs::canonicalize("../domain").expect("domain 경로");
        let manifest = scratch.join("Cargo.toml");
        let app_escaped = application_path
            .to_string_lossy()
            .replace('\\', "\\\\")
            .replace('"', "\\\"");
        let dom_escaped = domain_path
            .to_string_lossy()
            .replace('\\', "\\\\")
            .replace('"', "\\\"");
        fs::write(
            &manifest,
            format!(
                "[package]\nname = \"d55-app-probe\"\nversion = \"0.1.0\"\nedition = \"2024\"\n[workspace]\n[dependencies]\napplication = {{ path = \"{app_escaped}\" }}\ndomain = {{ path = \"{dom_escaped}\" }}\n"
            ),
        )
        .expect("manifest");
        fs::copy("../../Cargo.lock", scratch.join("Cargo.lock")).expect("lockfile");
        manifest
    }

    const CASES: [(&str, &str, Option<&str>); 4] = [
        (
            "valid_port_flow",
            "use application::{InstrumentIdIssuer, InstrumentMappingRepository};
use domain::InstrumentSource;
pub async fn assign<R: InstrumentMappingRepository, I: InstrumentIdIssuer>(repo: &R, issuer: &I, source: &InstrumentSource) {
    let candidate = issuer.issue().await.expect(\"issue\");
    let _assigned = repo.ensure(source, candidate).await;
}
fn main() {}",
            None,
        ),
        (
            "symbol_to_port_ensure",
            "use application::InstrumentMappingRepository;
use domain::InstrumentSource;
pub async fn assign<R: InstrumentMappingRepository>(repo: &R, source: &InstrumentSource) {
    let _assigned = repo.ensure(source, source.symbol().clone()).await;
}
fn main() {}",
            Some("error[E0308]"),
        ),
        (
            "instrument_id_to_port_ensure",
            "use application::InstrumentMappingRepository;
use domain::{InstrumentId, InstrumentSource};
pub async fn assign<R: InstrumentMappingRepository>(repo: &R, source: &InstrumentSource) {
    let forged = InstrumentId::try_new(source.symbol().as_str()).expect(\"id\");
    let _assigned = repo.ensure(source, forged).await;
}
fn main() {}",
            Some("error[E0308]"),
        ),
        (
            "reuse_port_candidate",
            "use application::{InstrumentIdIssuer, InstrumentMappingRepository};
use domain::InstrumentSource;
pub async fn assign<R: InstrumentMappingRepository, I: InstrumentIdIssuer>(repo: &R, issuer: &I, first: &InstrumentSource, second: &InstrumentSource) {
    let candidate = issuer.issue().await.expect(\"issue\");
    let _one = repo.ensure(first, candidate).await;
    let _two = repo.ensure(second, candidate).await;
}
fn main() {}",
            Some("error[E0382]"),
        ),
    ];

    #[test]
    fn mapping_repository_port_rejects_symbol_or_raw_instrument_id_candidates() {
        let scratch = ScratchDir::new("app-issuer-boundary");
        let manifest = write_manifest(scratch.path());
        for (name, source, expected_error) in CASES {
            fs::write(scratch.path().join("src/main.rs"), source).expect("probe");
            let output = testkit::check_consumer(&manifest, &scratch.path().join("target"))
                .expect("cargo check");
            let stderr = String::from_utf8_lossy(&output.stderr);
            match expected_error {
                None => assert!(output.status.success(), "{name}: {stderr}"),
                Some(code) => {
                    assert!(!output.status.success(), "{name} must fail to compile");
                    assert!(
                        stderr.contains(code),
                        "{name}: expected {code}, got:\n{stderr}"
                    );
                }
            }
        }
    }
}
