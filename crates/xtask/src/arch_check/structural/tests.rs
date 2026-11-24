use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use super::{
    AllowedStructuralSites, CommandStatus, ExpectedCanary, LineRange, check_structural,
    parse_clippy_jsonl, validate_parsed_output,
};
use crate::arch_check::{
    CrateName, Finding, RULES_PATH, Rule, check_sources, load_rules, load_workspace,
};
use crate::harness_workspace_tests::{fresh_directory, generate_lockfile, repository_root};

fn write_file(path: &Path, content: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create parent");
    }
    fs::write(path, content).expect("write fixture");
}

fn minimal_domain_source() -> &'static str {
    "#[derive(Debug, PartialEq, Eq)]
struct UserIdentityConfirmationSeal;

#[derive(Debug, PartialEq, Eq)]
pub struct UserIdentityConfirmation(UserIdentityConfirmationSeal);

impl UserIdentityConfirmation {
    #[must_use]
    pub const fn issue_after_user_confirmation() -> Self {
        Self(UserIdentityConfirmationSeal)
    }
}

#[cfg(test)]
mod tests {
    use super::UserIdentityConfirmation;

    #[test]
    fn canonical() {
        let first = UserIdentityConfirmation::issue_after_user_confirmation();
        let second = UserIdentityConfirmation::issue_after_user_confirmation();
        assert_eq!(first, second);
    }
}
"
}

fn create_base_fixture(name: &str) -> PathBuf {
    let root = fresh_directory(&repository_root(), name);
    write_file(
        &root.join("Cargo.toml"),
        "[workspace]\nresolver = \"3\"\nmembers = [\n    \"crates/domain\",\n    \"crates/application\",\n    \"crates/synth-macro\",\n    \"crates/helper-reexport\",\n    \"crates/attacker\",\n]\n",
    );
    write_file(
        &root.join("crates/domain/Cargo.toml"),
        "[package]\nname = \"domain\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    );
    write_file(
        &root.join("crates/domain/src/lib.rs"),
        "pub mod instrument_mapping_change;\npub use instrument_mapping_change::UserIdentityConfirmation;\n",
    );
    write_file(
        &root.join("crates/domain/src/instrument_mapping_change.rs"),
        minimal_domain_source(),
    );
    write_file(
        &root.join("crates/application/Cargo.toml"),
        "[package]\nname = \"application\"\nversion = \"0.1.0\"\nedition = \"2024\"\n[dependencies]\ndomain = { path = \"../domain\" }\n",
    );
    write_file(
        &root.join("crates/application/src/lib.rs"),
        "pub mod confirm_instrument_identity;\n",
    );
    write_file(
        &root.join("crates/application/src/confirm_instrument_identity.rs"),
        "use domain::UserIdentityConfirmation;\npub fn allowed() -> UserIdentityConfirmation {\n    UserIdentityConfirmation::issue_after_user_confirmation()\n}\n",
    );
    write_file(
        &root.join("crates/synth-macro/Cargo.toml"),
        "[package]\nname = \"synth-macro\"\nversion = \"0.1.0\"\nedition = \"2024\"\n[lib]\nproc-macro = true\n",
    );
    write_file(
        &root.join("crates/synth-macro/src/lib.rs"),
        "extern crate proc_macro;\nuse proc_macro::TokenStream;\n#[proc_macro]\npub fn brand_new_unlisted_macro(_input: TokenStream) -> TokenStream {\n    let part_a = \"domain::UserIdentity\";\n    let part_b = \"Confirmation::issue_\";\n    let part_c = \"after_user_confirmation()\";\n    let code = format!(\"{{ #[allow(clippy::all, clippy::disallowed_methods, warnings)] {{ {part_a}{part_b}{part_c} }} }}\");\n    code.parse().expect(\"tokens\")\n}\n",
    );
    write_file(
        &root.join("crates/helper-reexport/Cargo.toml"),
        "[package]\nname = \"helper-reexport\"\nversion = \"0.1.0\"\nedition = \"2024\"\n[dependencies]\ndomain = { path = \"../domain\" }\n",
    );
    write_file(
        &root.join("crates/helper-reexport/src/lib.rs"),
        "pub use domain::UserIdentityConfirmation as AliasConf;\npub fn forward_call() -> AliasConf {\n    AliasConf::issue_after_user_confirmation()\n}\n",
    );
    write_file(
        &root.join("crates/attacker/Cargo.toml"),
        "[package]\nname = \"attacker\"\nversion = \"0.1.0\"\nedition = \"2024\"\n[dependencies]\ndomain = { path = \"../domain\" }\nsynth-macro = { path = \"../synth-macro\" }\nhelper-reexport = { path = \"../helper-reexport\" }\n",
    );
    write_file(
        &root.join("crates/attacker/build.rs"),
        "use std::fs;\nuse std::path::PathBuf;\nfn main() {\n    let out = PathBuf::from(std::env::var(\"OUT_DIR\").expect(\"out\"));\n    fs::write(out.join(\"generated.rs\"), \"pub fn from_build() -> domain::UserIdentityConfirmation { domain::UserIdentityConfirmation::issue_after_user_confirmation() }\\n\").expect(\"write\");\n}\n",
    );
    write_file(
        &root.join("crates/attacker/src/lib.rs"),
        "pub fn clean() {}\n",
    );
    root
}

fn populate_attack_modules(root: &Path) {
    let base = root.join("crates/attacker");
    write_file(
        &base.join("src/via_new_proc_macro.rs"),
        "pub fn sneak() -> domain::UserIdentityConfirmation {\n    synth_macro::brand_new_unlisted_macro!()\n}\n",
    );
    write_file(
        &base.join("src/via_macro_rules.rs"),
        "macro_rules! mk {\n    () => { domain::UserIdentityConfirmation::issue_after_user_confirmation() };\n}\npub fn sneak() -> domain::UserIdentityConfirmation { mk!() }\n",
    );
    write_file(
        &base.join("src/via_fn_pointer.rs"),
        "pub fn sneak() -> domain::UserIdentityConfirmation {\n    let ptr: fn() -> domain::UserIdentityConfirmation = domain::UserIdentityConfirmation::issue_after_user_confirmation;\n    ptr()\n}\n",
    );
    write_file(
        &base.join("src/via_trait_object.rs"),
        "pub fn sneak() -> domain::UserIdentityConfirmation {\n    let cb: &dyn Fn() -> domain::UserIdentityConfirmation = &domain::UserIdentityConfirmation::issue_after_user_confirmation;\n    cb()\n}\n",
    );
    write_file(
        &base.join("src/via_generic.rs"),
        "fn run<F: Fn() -> domain::UserIdentityConfirmation>(cb: F) -> domain::UserIdentityConfirmation { cb() }\npub fn sneak() -> domain::UserIdentityConfirmation {\n    run(domain::UserIdentityConfirmation::issue_after_user_confirmation)\n}\n",
    );
    write_file(
        &base.join("src/via_const_static.rs"),
        "pub const CONST_VAL: domain::UserIdentityConfirmation = domain::UserIdentityConfirmation::issue_after_user_confirmation();\npub static STATIC_VAL: domain::UserIdentityConfirmation = domain::UserIdentityConfirmation::issue_after_user_confirmation();\n",
    );
    write_file(
        &base.join("src/via_assoc_type.rs"),
        "trait Alias { type Out; }\nstruct Holder;\nimpl Alias for Holder { type Out = domain::UserIdentityConfirmation; }\npub fn sneak() -> domain::UserIdentityConfirmation {\n    <Holder as Alias>::Out::issue_after_user_confirmation()\n}\n",
    );
    write_file(
        &base.join("src/via_reexport_and_allow.rs"),
        "#[allow(clippy::all, clippy::disallowed_methods, warnings)]\npub fn sneak_allow() -> helper_reexport::AliasConf {\n    helper_reexport::AliasConf::issue_after_user_confirmation()\n}\n#[expect(clippy::disallowed_methods, reason = \"try bypass\")]\npub fn sneak_expect() -> helper_reexport::AliasConf {\n    helper_reexport::AliasConf::issue_after_user_confirmation()\n}\n",
    );
    write_file(
        &base.join("src/included_call.inc"),
        "pub fn from_include() -> domain::UserIdentityConfirmation {\n    domain::UserIdentityConfirmation::issue_after_user_confirmation()\n}\n",
    );
    write_file(
        &base.join("src/via_include.rs"),
        "include!(\"included_call.inc\");\n",
    );
    write_file(
        &base.join("src/via_build_script.rs"),
        "include!(concat!(env!(\"OUT_DIR\"), \"/generated.rs\"));\n",
    );
    write_file(
        &base.join("src/via_cross_crate_include.rs"),
        "include!(\"../../application/src/confirm_instrument_identity.rs\");\n",
    );
    write_file(
        &base.join("tests/integration_attack.rs"),
        "#[test]\nfn sneak_in_test() {\n    let _val = synth_macro::brand_new_unlisted_macro!();\n}\n",
    );
    write_file(
        &base.join("src/lib.rs"),
        "pub mod via_new_proc_macro;\npub mod via_macro_rules;\npub mod via_fn_pointer;\npub mod via_trait_object;\npub mod via_generic;\npub mod via_const_static;\npub mod via_assoc_type;\npub mod via_reexport_and_allow;\npub mod via_include;\npub mod via_build_script;\npub mod via_cross_crate_include;\n",
    );
}

#[test]
fn real_workspace_passes_structural_check() {
    let root = repository_root();
    let rules = load_rules(&root.join(RULES_PATH)).expect("load rules");
    let workspace = load_workspace(&root.join("Cargo.toml"), &[]).expect("load workspace");
    let findings = check_structural(&workspace, &rules).expect("structural check");
    assert!(findings.is_empty(), "{findings:?}");
}

#[test]
fn structural_check_catches_all_unauthorized_issuance_paths_without_denylist() {
    let rules = load_rules(&repository_root().join(RULES_PATH)).expect("load rules");
    let root = create_base_fixture("h66-structural-attacks");
    populate_attack_modules(&root);
    generate_lockfile(&root.join("Cargo.toml"));
    let workspace = load_workspace(&root.join("Cargo.toml"), &[]).expect("load fixture");

    let source_findings = check_sources(&rules.source, &workspace).expect("source check");
    assert!(
        !source_findings.contains(&Finding::new(
            Rule::IdentityConfirmationIssuance,
            String::from("crates/attacker/src/via_new_proc_macro.rs"),
        )),
        "source-text check cannot know brand_new_unlisted_macro without a denylist"
    );

    let structural_findings = check_structural(&workspace, &rules).expect("structural check");
    let has_build_script_finding = structural_findings.iter().any(|finding| {
        finding.rule == Rule::IdentityConfirmationIssuance
            && finding
                .subject
                .starts_with("attacker:target/arch-check-clippy/")
            && finding.subject.ends_with("/out/generated.rs")
    });
    assert!(
        has_build_script_finding,
        "missing build.rs generated finding in {structural_findings:?}"
    );
    let non_build: BTreeSet<Finding> = structural_findings
        .into_iter()
        .filter(|finding| !finding.subject.ends_with("/out/generated.rs"))
        .collect();
    let expected: BTreeSet<Finding> = [
        "crates/helper-reexport/src/lib.rs",
        "crates/attacker/src/via_new_proc_macro.rs",
        "crates/attacker/src/via_macro_rules.rs",
        "crates/attacker/src/via_fn_pointer.rs",
        "crates/attacker/src/via_trait_object.rs",
        "crates/attacker/src/via_generic.rs",
        "crates/attacker/src/via_const_static.rs",
        "crates/attacker/src/via_assoc_type.rs",
        "crates/attacker/src/via_reexport_and_allow.rs",
        "crates/attacker/src/included_call.inc",
        "attacker:crates/application/src/confirm_instrument_identity.rs",
        "crates/attacker/tests/integration_attack.rs",
    ]
    .into_iter()
    .map(|path| Finding::new(Rule::IdentityConfirmationIssuance, String::from(path)))
    .collect();
    assert_eq!(non_build, expected);
}

#[test]
fn structural_check_catches_defining_and_allowed_file_misuse() {
    let rules = load_rules(&repository_root().join(RULES_PATH)).expect("load rules");
    let root = create_base_fixture("h66-structural-allowed-misuse");
    write_file(
        &root.join("crates/helper-reexport/src/lib.rs"),
        "pub fn clean() {}\n",
    );
    write_file(
        &root.join("crates/domain/src/instrument_mapping_change.rs"),
        &format!(
            "{}\npub fn backdoor() -> UserIdentityConfirmation {{ UserIdentityConfirmation::issue_after_user_confirmation() }}\ntrait SealForge {{ type Seal; }}\nstruct Forge;\nimpl SealForge for Forge {{ type Seal = UserIdentityConfirmationSeal; }}\n",
            minimal_domain_source()
        ),
    );
    write_file(
        &root.join("crates/application/Cargo.toml"),
        "[package]\nname = \"application\"\nversion = \"0.1.0\"\nedition = \"2024\"\n[dependencies]\ndomain = { path = \"../domain\" }\nsynth-macro = { path = \"../synth-macro\" }\n",
    );
    write_file(
        &root.join("crates/application/src/confirm_instrument_identity.rs"),
        "use domain::UserIdentityConfirmation;\npub fn allowed() -> UserIdentityConfirmation {\n    UserIdentityConfirmation::issue_after_user_confirmation()\n}\npub fn macro_in_allowed() -> UserIdentityConfirmation {\n    synth_macro::brand_new_unlisted_macro!()\n}\npub fn fn_ptr_in_allowed() -> fn() -> UserIdentityConfirmation {\n    UserIdentityConfirmation::issue_after_user_confirmation\n}\n",
    );
    generate_lockfile(&root.join("Cargo.toml"));
    let workspace = load_workspace(&root.join("Cargo.toml"), &[]).expect("load fixture");
    let findings = check_structural(&workspace, &rules).expect("structural check");
    let expected = BTreeSet::from([
        Finding::new(
            Rule::IdentityConfirmationIssuance,
            String::from("crates/domain/src/instrument_mapping_change.rs"),
        ),
        Finding::new(
            Rule::IdentityConfirmationIssuance,
            String::from("crates/application/src/confirm_instrument_identity.rs"),
        ),
    ]);
    assert_eq!(findings, expected);
}

fn synthetic_sites() -> AllowedStructuralSites {
    AllowedStructuralSites {
        defining_file: String::from("crates/domain/src/instrument_mapping_change.rs"),
        defining_test_file: String::from(
            "crates/domain/src/instrument_mapping_change/history_cases.rs",
        ),
        defining_test_ranges: vec![LineRange {
            start: 489,
            end: 499,
        }],
        seal_struct_ranges: vec![LineRange { start: 6, end: 7 }],
        confirmation_struct_ranges: vec![LineRange { start: 9, end: 10 }],
        defining_test_file_ranges: vec![LineRange { start: 1, end: 20 }],
        allowed_direct_call_ranges: BTreeMap::from([(
            String::from("crates/application/src/confirm_instrument_identity.rs"),
            vec![LineRange { start: 40, end: 40 }],
        )]),
        expected_issuer_canary: ExpectedCanary::Expected,
    }
}

const SAMPLE_ARTIFACT: &str =
    r#"{"reason":"compiler-artifact","manifest_path":"/workspace/crates/domain/Cargo.toml"}"#;
const SAMPLE_SEAL: &str = r#"{"reason":"compiler-message","manifest_path":"/workspace/crates/domain/Cargo.toml","target":{"kind":["lib"]},"message":{"level":"warning","code":{"code":"clippy::disallowed_types"},"spans":[{"file_name":"crates/domain/src/instrument_mapping_change.rs","line_start":10,"line_end":10,"is_primary":true,"expansion":null}]}}"#;
const SAMPLE_ISSUER: &str = r#"{"reason":"compiler-message","manifest_path":"/workspace/crates/application/Cargo.toml","target":{"kind":["lib"]},"message":{"level":"warning","code":{"code":"clippy::disallowed_methods"},"spans":[{"file_name":"crates/application/src/confirm_instrument_identity.rs","line_start":40,"line_end":40,"is_primary":true,"expansion":null}]}}"#;

fn run_synthetic_validation(
    status: CommandStatus,
    stderr: &[u8],
    jsonl: &str,
) -> Result<BTreeSet<Finding>, crate::arch_check::CheckFailure> {
    let root = Path::new("/workspace");
    let dir_map = BTreeMap::from([
        (
            PathBuf::from("/workspace/crates/domain"),
            CrateName::new("domain"),
        ),
        (
            PathBuf::from("/workspace/crates/application"),
            CrateName::new("application"),
        ),
        (
            PathBuf::from("/workspace/crates/attacker"),
            CrateName::new("attacker"),
        ),
    ]);
    let targets = BTreeSet::from([CrateName::new("domain")]);
    let sites = synthetic_sites();
    let parsed = parse_clippy_jsonl(jsonl.as_bytes(), root, &dir_map, &sites)?;
    validate_parsed_output(status, stderr, &targets, &sites, parsed)
}

#[test]
fn structural_parser_fails_closed_on_errors_missing_artifacts_and_canary() {
    let clean = format!("{SAMPLE_ARTIFACT}\n{SAMPLE_SEAL}\n{SAMPLE_ISSUER}\n");
    assert!(
        run_synthetic_validation(CommandStatus::Succeeded, b"", &clean)
            .expect("valid")
            .is_empty()
    );
    let status_err = run_synthetic_validation(CommandStatus::Failed, b"boom", &clean)
        .expect_err("failed status must fail closed");
    assert!(status_err.to_string().contains("boom"));

    let missing_artifact = format!("{SAMPLE_SEAL}\n{SAMPLE_ISSUER}\n");
    let artifact_err = run_synthetic_validation(CommandStatus::Succeeded, b"", &missing_artifact)
        .expect_err("missing artifact must fail closed");
    assert!(artifact_err.to_string().contains("domain"));

    let silent_seal = format!("{SAMPLE_ARTIFACT}\n{SAMPLE_ISSUER}\n");
    let seal_err = run_synthetic_validation(CommandStatus::Succeeded, b"", &silent_seal)
        .expect_err("silent seal canary must fail closed");
    assert!(seal_err.to_string().contains("봉인 타입"));

    let silent_issuer = format!("{SAMPLE_ARTIFACT}\n{SAMPLE_SEAL}\n");
    let issuer_err = run_synthetic_validation(CommandStatus::Succeeded, b"", &silent_issuer)
        .expect_err("silent issuer canary must fail closed");
    assert!(issuer_err.to_string().contains("발급 함수"));
}

#[test]
fn structural_parser_reports_empty_spans_as_violation() {
    let empty_span = r#"{"reason":"compiler-message","message":{"level":"warning","code":{"code":"clippy::disallowed_methods"},"spans":[]}}"#;
    let jsonl = format!("{SAMPLE_ARTIFACT}\n{SAMPLE_SEAL}\n{empty_span}\n");
    let findings =
        run_synthetic_validation(CommandStatus::Succeeded, b"", &jsonl).expect("finding reported");
    assert_eq!(
        findings,
        BTreeSet::from([Finding::new(
            Rule::IdentityConfirmationIssuance,
            String::from("(structural:missing-span)"),
        )])
    );
}

#[test]
fn structural_parser_rejects_cross_package_and_non_lib_target_spoofing() {
    let cross_pkg = r#"{"reason":"compiler-message","manifest_path":"/workspace/crates/attacker/Cargo.toml","target":{"kind":["lib"]},"message":{"level":"warning","code":{"code":"clippy::disallowed_methods"},"spans":[{"file_name":"crates/application/src/confirm_instrument_identity.rs","line_start":40,"line_end":40,"is_primary":true,"expansion":null}]}}"#;
    let non_lib = r#"{"reason":"compiler-message","manifest_path":"/workspace/crates/application/Cargo.toml","target":{"kind":["test"]},"message":{"level":"warning","code":{"code":"clippy::disallowed_methods"},"spans":[{"file_name":"crates/application/src/confirm_instrument_identity.rs","line_start":40,"line_end":40,"is_primary":true,"expansion":null}]}}"#;
    let jsonl = format!("{SAMPLE_ARTIFACT}\n{SAMPLE_SEAL}\n{cross_pkg}\n{non_lib}\n");
    let findings = run_synthetic_validation(CommandStatus::Succeeded, b"", &jsonl)
        .expect("spoofing findings reported");
    assert_eq!(
        findings,
        BTreeSet::from([
            Finding::new(
                Rule::IdentityConfirmationIssuance,
                String::from("attacker:crates/application/src/confirm_instrument_identity.rs"),
            ),
            Finding::new(
                Rule::IdentityConfirmationIssuance,
                String::from("crates/application/src/confirm_instrument_identity.rs"),
            ),
        ])
    );
}

#[test]
fn structural_parser_isolates_expansion_defining_line_allowed_line_and_seal_line_guards() {
    let macro_on_allowed_line = r#"{"reason":"compiler-message","manifest_path":"/workspace/crates/application/Cargo.toml","target":{"kind":["lib"]},"message":{"level":"warning","code":{"code":"clippy::disallowed_methods"},"spans":[{"file_name":"crates/application/src/confirm_instrument_identity.rs","line_start":40,"line_end":40,"is_primary":true,"expansion":{"span":{"file_name":"crates/application/src/confirm_instrument_identity.rs","line_start":40,"line_end":40,"expansion":null}}}]}}"#;
    let non_direct_line_in_allowed = r#"{"reason":"compiler-message","manifest_path":"/workspace/crates/application/Cargo.toml","target":{"kind":["lib"]},"message":{"level":"warning","code":{"code":"clippy::disallowed_methods"},"spans":[{"file_name":"crates/application/src/confirm_instrument_identity.rs","line_start":41,"line_end":41,"is_primary":true,"expansion":null}]}}"#;
    let product_line_in_defining = r#"{"reason":"compiler-message","manifest_path":"/workspace/crates/domain/Cargo.toml","target":{"kind":["lib"]},"message":{"level":"warning","code":{"code":"clippy::disallowed_methods"},"spans":[{"file_name":"crates/domain/src/instrument_mapping_change.rs","line_start":25,"line_end":25,"is_primary":true,"expansion":null}]}}"#;
    let extra_seal_line_in_defining = r#"{"reason":"compiler-message","manifest_path":"/workspace/crates/domain/Cargo.toml","target":{"kind":["lib"]},"message":{"level":"warning","code":{"code":"clippy::disallowed_types"},"spans":[{"file_name":"crates/domain/src/instrument_mapping_change.rs","line_start":25,"line_end":25,"is_primary":true,"expansion":null}]}}"#;

    for (case_line, expected_subject) in [
        (
            macro_on_allowed_line,
            "crates/application/src/confirm_instrument_identity.rs",
        ),
        (
            non_direct_line_in_allowed,
            "crates/application/src/confirm_instrument_identity.rs",
        ),
        (
            product_line_in_defining,
            "crates/domain/src/instrument_mapping_change.rs",
        ),
        (
            extra_seal_line_in_defining,
            "crates/domain/src/instrument_mapping_change.rs",
        ),
    ] {
        let jsonl = format!("{SAMPLE_ARTIFACT}\n{SAMPLE_SEAL}\n{SAMPLE_ISSUER}\n{case_line}\n");
        let findings = run_synthetic_validation(CommandStatus::Succeeded, b"", &jsonl)
            .expect("isolated guard must report finding");
        assert_eq!(
            findings,
            BTreeSet::from([Finding::new(
                Rule::IdentityConfirmationIssuance,
                String::from(expected_subject),
            )]),
            "{case_line}"
        );
    }
}
