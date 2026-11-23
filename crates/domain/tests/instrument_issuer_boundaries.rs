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
        let domain_path = fs::canonicalize(".").expect("domain 경로");
        let manifest = scratch.join("Cargo.toml");
        let escaped = domain_path
            .to_string_lossy()
            .replace('\\', "\\\\")
            .replace('"', "\\\"");
        fs::write(
            &manifest,
            format!(
                "[package]\nname = \"d55-domain-probe\"\nversion = \"0.1.0\"\nedition = \"2024\"\n[workspace]\n[dependencies]\ndomain = {{ path = \"{escaped}\" }}\n"
            ),
        )
        .expect("manifest");
        fs::copy("../../Cargo.lock", scratch.join("Cargo.lock")).expect("lockfile");
        manifest
    }

    const CASES: [(&str, &str, Option<&str>); 10] = [
            (
                "valid_independent_issuer",
                "use domain::{IndependentInstrumentIdIssuer, InstrumentMappings, InstrumentSource, IsinCode, KnownMarket, Symbol};
fn main() {
    let mut issuer = IndependentInstrumentIdIssuer::new();
    let candidate = issuer.issue().expect(\"issue\");
    let source = InstrumentSource::new(KnownMarket::Kr, Symbol::try_new(\"005930\").expect(\"s\"), IsinCode::try_new(\"KR7005930003\").expect(\"i\"));
    let mut mappings = InstrumentMappings::default();
    let _res = mappings.ensure(&source, candidate);
}",
                None,
            ),
            (
                "symbol_as_candidate",
                "use domain::{InstrumentMappings, InstrumentSource, IsinCode, KnownMarket, Symbol};
fn main() {
    let source = InstrumentSource::new(KnownMarket::Kr, Symbol::try_new(\"005930\").expect(\"s\"), IsinCode::try_new(\"KR7005930003\").expect(\"i\"));
    let mut mappings = InstrumentMappings::default();
    let _res = mappings.ensure(&source, source.symbol().clone());
}",
                Some("error[E0308]"),
            ),
            (
                "isin_as_candidate",
                "use domain::{InstrumentMappings, InstrumentSource, IsinCode, KnownMarket, Symbol};
fn main() {
    let source = InstrumentSource::new(KnownMarket::Kr, Symbol::try_new(\"005930\").expect(\"s\"), IsinCode::try_new(\"KR7005930003\").expect(\"i\"));
    let mut mappings = InstrumentMappings::default();
    let _res = mappings.ensure(&source, source.isin().clone());
}",
                Some("error[E0308]"),
            ),
            (
                "arbitrary_instrument_id_as_candidate",
                "use domain::{InstrumentId, InstrumentMappings, InstrumentSource, IsinCode, KnownMarket, Symbol};
fn main() {
    let source = InstrumentSource::new(KnownMarket::Kr, Symbol::try_new(\"005930\").expect(\"s\"), IsinCode::try_new(\"KR7005930003\").expect(\"i\"));
    let forged = InstrumentId::try_new(source.symbol().as_str()).expect(\"id\");
    let mut mappings = InstrumentMappings::default();
    let _res = mappings.ensure(&source, forged);
}",
                Some("error[E0308]"),
            ),
            (
                "forged_tuple_struct",
                "use domain::{InstrumentId, IssuedInstrumentId};
fn main() {
    let id = InstrumentId::try_new(\"005930\").expect(\"id\");
    let _forged = IssuedInstrumentId(id);
}",
                Some("error[E0423]"),
            ),
            (
                "no_try_new_on_issued",
                "use domain::IssuedInstrumentId;
fn main() {
    let _forged = IssuedInstrumentId::try_new(\"005930\");
}",
                Some("error[E0599]"),
            ),
            (
                "no_from_instrument_id",
                "use domain::{InstrumentId, IssuedInstrumentId};
fn main() {
    let id = InstrumentId::try_new(\"005930\").expect(\"id\");
    let _forged = IssuedInstrumentId::from(id);
}",
                Some("error[E0308]"),
            ),
            (
                "cannot_clone_issued_candidate",
                "use domain::IndependentInstrumentIdIssuer;
fn main() {
    let mut issuer = IndependentInstrumentIdIssuer::new();
    let candidate = issuer.issue().expect(\"issue\");
    let _dup = candidate.clone();
}",
                Some("error[E0599]"),
            ),
            (
                "cannot_reuse_consumed_candidate",
                "use domain::{IndependentInstrumentIdIssuer, InstrumentMappings, InstrumentSource, IsinCode, KnownMarket, Symbol};
fn main() {
    let mut issuer = IndependentInstrumentIdIssuer::new();
    let candidate = issuer.issue().expect(\"issue\");
    let first = InstrumentSource::new(KnownMarket::Kr, Symbol::try_new(\"005930\").expect(\"s\"), IsinCode::try_new(\"KR7005930003\").expect(\"i\"));
    let second = InstrumentSource::new(KnownMarket::Us, Symbol::try_new(\"AAPL\").expect(\"s\"), IsinCode::try_new(\"US0378331005\").expect(\"i\"));
    let mut mappings = InstrumentMappings::default();
    let _one = mappings.ensure(&first, candidate);
    let _two = mappings.ensure(&second, candidate);
}",
                Some("error[E0382]"),
            ),
            (
                "cannot_clone_issuer",
                "use domain::IndependentInstrumentIdIssuer;
fn main() {
    let issuer = IndependentInstrumentIdIssuer::new();
    let _dup = issuer.clone();
}",
                Some("error[E0599]"),
            ),
        ];

    #[test]
    fn domain_enforces_independent_issuer_type_boundary_at_compile_time() {
        let scratch = ScratchDir::new("domain-issuer-boundary");
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
