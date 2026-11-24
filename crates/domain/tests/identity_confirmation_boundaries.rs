#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    struct ScratchDir(PathBuf);

    impl ScratchDir {
        fn new(name: &str) -> Self {
            let path =
                Path::new("../../target/H-64").join(format!("{name}-{}", std::process::id()));
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
                "[package]\nname = \"h64-domain-probe\"\nversion = \"0.1.0\"\nedition = \"2024\"\n[workspace]\n[dependencies]\ndomain = {{ path = \"{escaped}\" }}\n"
            ),
        )
        .expect("manifest");
        fs::copy("../../Cargo.lock", scratch.join("Cargo.lock")).expect("lockfile");
        manifest
    }

    const CASES: [(&str, &str, Option<&str>); 7] = [
        (
            "valid_issued_user_confirmation_consumed_once",
            "use domain::{IdentityConfirmationOrigin, UserIdentityConfirmation};
fn main() {
    let confirmation = UserIdentityConfirmation::issue_after_user_confirmation();
    let _origin = IdentityConfirmationOrigin::UserConfirmed(confirmation);
}",
            None,
        ),
        (
            "cannot_clone_issued_user_confirmation",
            "use domain::UserIdentityConfirmation;
fn main() {
    let confirmation = UserIdentityConfirmation::issue_after_user_confirmation();
    let _dup = confirmation.clone();
}",
            Some("error[E0599]"),
        ),
        (
            "cannot_reuse_moved_user_confirmation",
            "use domain::{IdentityConfirmationOrigin, UserIdentityConfirmation};
fn main() {
    let confirmation = UserIdentityConfirmation::issue_after_user_confirmation();
    let _first = IdentityConfirmationOrigin::UserConfirmed(confirmation);
    let _second = IdentityConfirmationOrigin::UserConfirmed(confirmation);
}",
            Some("error[E0382]"),
        ),
        (
            "cannot_clone_confirmed_retirement_correction",
            "use domain::ConfirmedRetirementCorrection;
fn dup(correction: ConfirmedRetirementCorrection) {
    let _dup = correction.clone();
}
fn main() {}",
            Some("error[E0599]"),
        ),
        (
            "cannot_reuse_confirmed_retirement_correction",
            "use domain::{ConfirmedRetirementCorrection, InstrumentMappings};
fn reuse(mappings: &mut InstrumentMappings, correction: ConfirmedRetirementCorrection) {
    let _first = mappings.correct_retirement(correction);
    let _second = mappings.correct_retirement(correction);
}
fn main() {}",
            Some("error[E0382]"),
        ),
        (
            "cannot_clone_confirmed_link_correction",
            "use domain::ConfirmedLinkCorrection;
fn dup(correction: ConfirmedLinkCorrection) {
    let _dup = correction.clone();
}
fn main() {}",
            Some("error[E0599]"),
        ),
        (
            "cannot_reuse_confirmed_link_correction",
            "use domain::{ConfirmedLinkCorrection, InstrumentMappings};
fn reuse(mappings: &mut InstrumentMappings, correction: ConfirmedLinkCorrection) {
    let _first = mappings.correct_link(correction);
    let _second = mappings.correct_link(correction);
}
fn main() {}",
            Some("error[E0382]"),
        ),
    ];

    #[test]
    fn issued_user_identity_confirmations_remain_one_time_at_compile_time() {
        let scratch = ScratchDir::new("identity-confirmation-boundary");
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
