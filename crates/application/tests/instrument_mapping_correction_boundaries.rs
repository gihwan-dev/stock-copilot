#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    struct ScratchDir(PathBuf);

    impl ScratchDir {
        fn new(name: &str) -> Self {
            let path =
                Path::new("../../target/S-24-probe").join(format!("{name}-{}", std::process::id()));
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
                "[package]\nname = \"s24-app-probe\"\nversion = \"0.1.0\"\nedition = \"2024\"\n[workspace]\n[dependencies]\napplication = {{ path = \"{app_escaped}\" }}\ndomain = {{ path = \"{dom_escaped}\" }}\n"
            ),
        )
        .expect("manifest");
        fs::copy("../../Cargo.lock", scratch.join("Cargo.lock")).expect("lockfile");
        manifest
    }

    const CASES: [(&str, &str, Option<&str>); 7] = [
        (
            "single_use_calls_compile",
            "use application::InstrumentMappingCorrectionRepository;
use domain::{ConfirmedIdentifierChange, ConfirmedLinkCorrection, ConfirmedRetirementCorrection};
async fn once<P: InstrumentMappingCorrectionRepository>(
    port: &P,
    c1: ConfirmedIdentifierChange,
    c2: ConfirmedRetirementCorrection,
    c3: ConfirmedLinkCorrection,
) {
    let _ = port.link_confirmed_change(c1).await;
    let _ = port.correct_retirement(c2).await;
    let _ = port.correct_link(c3).await;
}
fn main() { let _ = once::<Never>; }
struct Never;
impl InstrumentMappingCorrectionRepository for Never {
    async fn link_confirmed_change(&self, _: ConfirmedIdentifierChange) -> Result<domain::InstrumentId, application::MappingTransitionStoreError> { unimplemented!() }
    async fn correct_retirement(&self, _: ConfirmedRetirementCorrection) -> Result<domain::InstrumentId, application::MappingTransitionStoreError> { unimplemented!() }
    async fn correct_link(&self, _: ConfirmedLinkCorrection) -> Result<domain::InstrumentId, application::MappingTransitionStoreError> { unimplemented!() }
}",
            None,
        ),
        (
            "cannot_reuse_confirmed_change_on_port",
            "use application::InstrumentMappingCorrectionRepository;
use domain::ConfirmedIdentifierChange;
async fn twice<P: InstrumentMappingCorrectionRepository>(port: &P, change: ConfirmedIdentifierChange) {
    let _ = port.link_confirmed_change(change).await;
    let _ = port.link_confirmed_change(change).await;
}
fn main() {}",
            Some("error[E0382]"),
        ),
        (
            "cannot_reuse_confirmed_retirement_on_port",
            "use application::InstrumentMappingCorrectionRepository;
use domain::ConfirmedRetirementCorrection;
async fn twice<P: InstrumentMappingCorrectionRepository>(port: &P, corr: ConfirmedRetirementCorrection) {
    let _ = port.correct_retirement(corr).await;
    let _ = port.correct_retirement(corr).await;
}
fn main() {}",
            Some("error[E0382]"),
        ),
        (
            "cannot_reuse_confirmed_link_on_port",
            "use application::InstrumentMappingCorrectionRepository;
use domain::ConfirmedLinkCorrection;
async fn twice<P: InstrumentMappingCorrectionRepository>(port: &P, corr: ConfirmedLinkCorrection) {
    let _ = port.correct_link(corr).await;
    let _ = port.correct_link(corr).await;
}
fn main() {}",
            Some("error[E0382]"),
        ),
        (
            "cannot_pass_unconfirmed_change_prompt_to_port",
            "use application::{IdentifierChangePrompt, InstrumentMappingCorrectionRepository};
async fn bypass<P: InstrumentMappingCorrectionRepository>(port: &P, prompt: IdentifierChangePrompt) {
    let _ = port.link_confirmed_change(prompt).await;
}
fn main() {}",
            Some("error[E0308]"),
        ),
        (
            "cannot_pass_unconfirmed_retirement_prompt_to_port",
            "use application::{InstrumentMappingCorrectionRepository, RetirementCorrectionPrompt};
async fn bypass<P: InstrumentMappingCorrectionRepository>(port: &P, prompt: RetirementCorrectionPrompt) {
    let _ = port.correct_retirement(prompt).await;
}
fn main() {}",
            Some("error[E0308]"),
        ),
        (
            "cannot_pass_unconfirmed_link_prompt_to_port",
            "use application::{InstrumentMappingCorrectionRepository, LinkCorrectionPrompt};
async fn bypass<P: InstrumentMappingCorrectionRepository>(port: &P, prompt: LinkCorrectionPrompt) {
    let _ = port.correct_link(prompt).await;
}
fn main() {}",
            Some("error[E0308]"),
        ),
    ];

    #[test]
    fn correction_port_enforces_single_use_and_rejects_unconfirmed_prompts_at_compile_time() {
        let scratch = ScratchDir::new("app-correction-boundary");
        let manifest = write_manifest(scratch.path());
        let target_dir = scratch.path().join("target");
        for (name, source, expected_error) in CASES {
            fs::write(scratch.path().join("src/main.rs"), source).expect("write probe");
            let output = testkit::check_consumer(&manifest, &target_dir).expect("cargo check");
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
