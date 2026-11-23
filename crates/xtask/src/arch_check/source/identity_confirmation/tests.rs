use std::collections::{BTreeMap, BTreeSet};

use super::{CheckContext, check};
use crate::arch_check::rules::IdentityConfirmationRules;
use crate::arch_check::{CrateName, Rule};

fn rules() -> IdentityConfirmationRules {
    IdentityConfirmationRules {
        confirmation_type: String::from("UserIdentityConfirmation"),
        seal_type: String::from("UserIdentityConfirmationSeal"),
        issuer_function: String::from("issue_after_user_confirmation"),
        defining_file: String::from("crates/domain/src/instrument_mapping_change.rs"),
        defining_test_file: String::from(
            "crates/domain/src/instrument_mapping_change/history_cases.rs",
        ),
        use_case_file: String::from("crates/application/src/confirm_instrument_identity.rs"),
        issuance_allowed_files: BTreeSet::from([
            String::from("crates/application/src/confirm_instrument_identity.rs"),
            String::from("crates/testkit-ports/src/scripted_identity_confirmation.rs"),
        ]),
        prompt_trait: String::from("IdentityConfirmationPrompt"),
        prompt_module: String::from("identity_confirmation_prompt"),
        prompt_defining_crate: String::from("application"),
        prompt_impl_crates: BTreeSet::from([CrateName::new("platform-macos")]),
        prompt_impl_files: BTreeSet::from([String::from(
            "crates/testkit-ports/src/scripted_identity_confirmation.rs",
        )]),
    }
}

fn scan(crate_name: &str, path: &str, source: &str) -> BTreeSet<Rule> {
    scan_with_aliases(crate_name, &BTreeMap::new(), path, source)
}

fn scan_with_aliases(
    crate_name: &str,
    aliases: &BTreeMap<String, Vec<String>>,
    path: &str,
    source: &str,
) -> BTreeSet<Rule> {
    let parsed = syn::parse_file(source).expect("parse");
    let tokens = source.parse().expect("tokens");
    let krate = CrateName::new(crate_name);
    check(
        &rules(),
        &CheckContext {
            crate_name: &krate,
            source_aliases: aliases,
            relative: path,
            text: source,
            file: &parsed,
            tokens: &tokens,
        },
    )
}

#[test]
fn unauthorized_adapters_and_indirect_calls_are_rejected() {
    for (krate, path, source) in [
        (
            "store-sqlite",
            "crates/adapters/store-sqlite/src/lib.rs",
            "fn f() { let _ = domain::UserIdentityConfirmation::issue_after_user_confirmation(); }",
        ),
        (
            "broker-toss",
            "crates/adapters/broker-toss/src/lib.rs",
            "use domain::UserIdentityConfirmation as C; fn f() { let _ = C::issue_after_user_confirmation(); }",
        ),
        (
            "ai-claude-code",
            "crates/adapters/ai-claude-code/src/lib.rs",
            "fn f() { let ptr = domain::UserIdentityConfirmation::r#issue_after_user_confirmation; let _ = ptr(); }",
        ),
        (
            "mcp-server",
            "crates/mcp-server/src/lib.rs",
            "fn f() { let _ = vec![domain::UserIdentityConfirmation::issue_after_user_confirmation()]; }",
        ),
        (
            "scheduler",
            "crates/scheduler/src/lib.rs",
            "fn call<F: Fn() -> domain::UserIdentityConfirmation>(f: F) -> domain::UserIdentityConfirmation { f() } fn g() { let _ = call(domain::UserIdentityConfirmation::issue_after_user_confirmation); }",
        ),
        (
            "store-sqlite",
            "crates/adapters/store-sqlite/src/lib.rs",
            "fn f() { let _ = UserIdentityConfirmationSeal; }",
        ),
    ] {
        assert_eq!(
            scan(krate, path, source),
            BTreeSet::from([Rule::IdentityConfirmationIssuance]),
            "{krate} {path}"
        );
    }
}

#[test]
fn unauthorized_internal_files_and_domain_siblings_are_rejected() {
    for (krate, path, source) in [
        (
            "application",
            "crates/application/src/sync_portfolio.rs",
            "fn f() { let _ = domain::UserIdentityConfirmation::issue_after_user_confirmation(); }",
        ),
        (
            "testkit-ports",
            "crates/testkit-ports/src/memory.rs",
            "fn f() { let _ = domain::UserIdentityConfirmation::issue_after_user_confirmation(); }",
        ),
        (
            "domain",
            "crates/domain/src/instrument_mapping.rs",
            "fn f() { let _ = crate::UserIdentityConfirmation::issue_after_user_confirmation(); }",
        ),
        (
            "domain",
            "crates/domain/src/other.rs",
            "type Alias = crate::UserIdentityConfirmation;",
        ),
        (
            "domain",
            "crates/domain/src/other.rs",
            "impl crate::UserIdentityConfirmation { pub fn mint() {} }",
        ),
    ] {
        assert_eq!(
            scan(krate, path, source),
            BTreeSet::from([Rule::IdentityConfirmationIssuance]),
            "{krate} {path}"
        );
    }
}

#[test]
fn defining_file_accepts_canonical_issuer_and_rejects_backdoors() {
    let defining = "crates/domain/src/instrument_mapping_change.rs";
    let valid = "
#[derive(Debug, PartialEq, Eq)] struct UserIdentityConfirmationSeal;
#[derive(Debug, PartialEq, Eq)] pub struct UserIdentityConfirmation(UserIdentityConfirmationSeal);
impl UserIdentityConfirmation { #[must_use] pub const fn issue_after_user_confirmation() -> Self { Self(UserIdentityConfirmationSeal) } }
mod history_cases;
#[cfg(test)] mod tests { use super::UserIdentityConfirmation; #[test] fn ok() { let _ = UserIdentityConfirmation::issue_after_user_confirmation(); } }
";
    assert!(scan("domain", defining, valid).is_empty());
    for source in [
        "#[derive(Debug, Clone, PartialEq, Eq)] pub struct UserIdentityConfirmation(UserIdentityConfirmationSeal);",
        "#[derive(Debug, Default, PartialEq, Eq)] pub struct UserIdentityConfirmation(UserIdentityConfirmationSeal);",
        "#[derive(Debug, PartialEq, Eq)] pub struct UserIdentityConfirmation(pub UserIdentityConfirmationSeal);",
        "#[derive(Debug, PartialEq, Eq)] pub struct UserIdentityConfirmationSeal;",
        "impl UserIdentityConfirmation { pub const fn issue_after_user_confirmation() -> Self { Self(UserIdentityConfirmationSeal) } pub fn second() -> Self { Self(UserIdentityConfirmationSeal) } }",
        "impl UserIdentityConfirmation { pub const fn issue_after_user_confirmation() -> Self { Self(UserIdentityConfirmationSeal) } pub fn extra(&self) {} }",
        "impl Default for UserIdentityConfirmation { fn default() -> Self { Self(UserIdentityConfirmationSeal) } }",
        "impl UserIdentityConfirmationSeal { fn mint() -> Self { Self } }",
        "pub fn sneak() -> UserIdentityConfirmation { UserIdentityConfirmation(UserIdentityConfirmationSeal) }",
        "pub fn sneak() -> UserIdentityConfirmation { UserIdentityConfirmation::issue_after_user_confirmation() }",
        "impl UserIdentityConfirmation { pub const fn issue_after_user_confirmation() -> Self { pub fn nested() -> UserIdentityConfirmation { UserIdentityConfirmation(UserIdentityConfirmationSeal) } Self(UserIdentityConfirmationSeal) } }",
        "mod sneak;",
        "#[cfg(test)] mod tests { pub fn leak() -> super::UserIdentityConfirmation { super::UserIdentityConfirmation::issue_after_user_confirmation() } }",
        "type Alias = UserIdentityConfirmation;",
        "use self::UserIdentityConfirmationSeal as S;",
        "fn f() { custom!(UserIdentityConfirmationSeal); }",
    ] {
        assert_eq!(
            scan("domain", defining, source),
            BTreeSet::from([Rule::IdentityConfirmationIssuance]),
            "{source}"
        );
    }
}

#[test]
fn defining_file_children_and_test_module_are_closed() {
    for (path, source) in [
        (
            "crates/domain/src/instrument_mapping_change/sneak.rs",
            "pub fn f() {}",
        ),
        (
            "crates/domain/src/instrument_mapping_change/history_cases.rs",
            "pub fn leaked() {}",
        ),
        (
            "crates/domain/src/instrument_mapping_change/history_cases.rs",
            "#[cfg(test)] mod tests { fn call() { let _ = super::UserIdentityConfirmation::issue_after_user_confirmation(); } }",
        ),
    ] {
        assert_eq!(
            scan("domain", path, source),
            BTreeSet::from([Rule::IdentityConfirmationIssuance]),
            "{path}"
        );
    }
}

#[test]
fn use_case_file_requires_prompt_gated_confirmed_arm_and_blocks_wrappers() {
    let path = "crates/application/src/confirm_instrument_identity.rs";
    let valid = "
use crate::Clock;
use crate::identity_confirmation_prompt::{IdentifierChangePrompt, IdentityConfirmationDecision, IdentityConfirmationPrompt};
use domain::{ConfirmedIdentifierChange, IdentityChangeConfirmation, IdentityConfirmationOrigin, UserIdentityConfirmation};
pub enum IdentityConfirmationOutcome<T> { Confirmed(T), Denied, Unavailable }
pub struct ConfirmInstrumentIdentity<'a, Prompt, Time> { prompt: &'a Prompt, clock: &'a Time }
impl<'a, Prompt: IdentityConfirmationPrompt, Time: Clock> ConfirmInstrumentIdentity<'a, Prompt, Time> {
    pub async fn confirm_identifier_change(&self, request: IdentifierChangePrompt) -> IdentityConfirmationOutcome<ConfirmedIdentifierChange> {
        match self.prompt.confirm_identifier_change(&request).await {
            IdentityConfirmationDecision::Confirmed => {
                let seal = UserIdentityConfirmation::issue_after_user_confirmation();
                let confirmed_at = self.clock.now();
                let (id, prev, next, ev) = request.into_parts();
                IdentityConfirmationOutcome::Confirmed(ConfirmedIdentifierChange::new(id, prev, next, IdentityChangeConfirmation::new(IdentityConfirmationOrigin::UserConfirmed(seal), ev, confirmed_at)))
            }
            IdentityConfirmationDecision::Denied => IdentityConfirmationOutcome::Denied,
            IdentityConfirmationDecision::Unavailable => IdentityConfirmationOutcome::Unavailable,
        }
    }
}
";
    assert!(scan("application", path, valid).is_empty());
    for source in [
        "pub fn forge() -> domain::UserIdentityConfirmation { domain::UserIdentityConfirmation::issue_after_user_confirmation() }",
        "pub const F: fn() -> domain::UserIdentityConfirmation = domain::UserIdentityConfirmation::issue_after_user_confirmation;",
        "pub struct ConfirmInstrumentIdentity<'a, Prompt, Time> { prompt: &'a Prompt, clock: &'a Time } impl<'a, Prompt: IdentityConfirmationPrompt, Time: Clock> ConfirmInstrumentIdentity<'a, Prompt, Time> { pub async fn bypass(&self) { let _ = UserIdentityConfirmation::issue_after_user_confirmation(); } }",
        "pub struct ConfirmInstrumentIdentity<'a, Prompt, Time> { prompt: &'a Prompt, clock: &'a Time } impl<'a, Prompt: IdentityConfirmationPrompt, Time: Clock> ConfirmInstrumentIdentity<'a, Prompt, Time> { pub async fn wrong_arm(&self, req: &IdentifierChangePrompt) { match self.prompt.confirm_identifier_change(req).await { IdentityConfirmationDecision::Denied => { let _ = UserIdentityConfirmation::issue_after_user_confirmation(); } _ => {} } } }",
        "pub struct ConfirmInstrumentIdentity<'a, Prompt, Time> { prompt: &'a Prompt, clock: &'a Time } impl<'a, Prompt: IdentityConfirmationPrompt, Time: Clock> ConfirmInstrumentIdentity<'a, Prompt, Time> { pub async fn double_mint(&self, req: &IdentifierChangePrompt) { match self.prompt.confirm_identifier_change(req).await { IdentityConfirmationDecision::Confirmed => { let _a = UserIdentityConfirmation::issue_after_user_confirmation(); let _b = UserIdentityConfirmation::issue_after_user_confirmation(); } _ => {} } } }",
        "pub struct ConfirmInstrumentIdentity<'a, Prompt, Time> { prompt: &'a Prompt, clock: &'a Time } impl<'a, Prompt: IdentityConfirmationPrompt, Time: Clock> ConfirmInstrumentIdentity<'a, Prompt, Time> { pub async fn loop_mint(&self, req: &IdentifierChangePrompt) { match self.prompt.confirm_identifier_change(req).await { IdentityConfirmationDecision::Confirmed => { for _ in 0..2 { let _ = UserIdentityConfirmation::issue_after_user_confirmation(); } } _ => {} } } }",
        "pub struct ConfirmInstrumentIdentity<'a, Prompt, Time> { prompt: &'a Prompt, clock: &'a Time } impl<'a, Prompt: IdentityConfirmationPrompt, Time: Clock> ConfirmInstrumentIdentity<'a, Prompt, Time> { pub async fn closure_mint(&self, req: &IdentifierChangePrompt) { match self.prompt.confirm_identifier_change(req).await { IdentityConfirmationDecision::Confirmed => { let _f = || UserIdentityConfirmation::issue_after_user_confirmation(); } _ => {} } } }",
        "pub struct ConfirmInstrumentIdentity<'a, Prompt, Time> { prompt: &'a Prompt, clock: &'a Time } impl<'a, Prompt, Time> ConfirmInstrumentIdentity<'a, Prompt, Time> { pub async fn unbounded(&self, req: &IdentifierChangePrompt) { match self.prompt.confirm_identifier_change(req).await { IdentityConfirmationDecision::Confirmed => { let _ = UserIdentityConfirmation::issue_after_user_confirmation(); } _ => {} } } }",
        "use crate::identity_confirmation_prompt::IdentityConfirmationDecision::Denied as Confirmed;",
        "pub use domain::UserIdentityConfirmation;",
        "mod sneak;",
        "enum IdentityConfirmationDecision { Confirmed }",
    ] {
        assert_eq!(
            scan("application", path, source),
            BTreeSet::from([Rule::IdentityConfirmationIssuance]),
            "{source}"
        );
    }
    assert_eq!(
        scan(
            "application",
            "crates/application/src/confirm_instrument_identity/sneak.rs",
            "pub fn f() {}"
        ),
        BTreeSet::from([Rule::IdentityConfirmationIssuance])
    );
}

#[test]
fn prompt_trait_impl_alias_and_reexport_are_enforced_across_crates() {
    assert!(scan("platform-macos", "crates/adapters/platform-macos/src/lib.rs", "impl application::identity_confirmation_prompt::IdentityConfirmationPrompt for Native {}").is_empty());
    assert!(
        scan(
            "testkit-ports",
            "crates/testkit-ports/src/scripted_identity_confirmation.rs",
            "impl IdentityConfirmationPrompt for Scripted {}"
        )
        .is_empty()
    );
    for (krate, path, source, expected) in [
        (
            "testkit-ports",
            "crates/testkit-ports/src/lib.rs",
            "impl IdentityConfirmationPrompt for Sneaky {}",
            Rule::IdentityConfirmationPromptImpl,
        ),
        (
            "store-sqlite",
            "crates/adapters/store-sqlite/src/lib.rs",
            "impl application::identity_confirmation_prompt::r#IdentityConfirmationPrompt for Fake {}",
            Rule::IdentityConfirmationPromptImpl,
        ),
        (
            "mcp-server",
            "crates/mcp-server/src/lib.rs",
            "use application::identity_confirmation_prompt::IdentityConfirmationPrompt as P;",
            Rule::IdentityConfirmationPromptAlias,
        ),
        (
            "broker-toss",
            "crates/adapters/broker-toss/src/lib.rs",
            "pub use application::identity_confirmation_prompt::IdentityConfirmationPrompt;",
            Rule::IdentityConfirmationPromptReexport,
        ),
        (
            "ai-claude-code",
            "crates/adapters/ai-claude-code/src/lib.rs",
            "use application::identity_confirmation_prompt as p; pub use p::*;",
            Rule::IdentityConfirmationPromptReexport,
        ),
    ] {
        assert_eq!(
            scan(krate, path, source),
            BTreeSet::from([expected]),
            "{krate} {path}"
        );
    }
    assert_eq!(
        scan_with_aliases(
            "secrets-keychain",
            &BTreeMap::from([(String::from("app"), vec![String::from("application")])]),
            "crates/adapters/secrets-keychain/src/lib.rs",
            "pub use app::*;",
        ),
        BTreeSet::from([Rule::IdentityConfirmationPromptReexport])
    );
}

#[test]
fn spoofed_prompt_imports_nested_structs_literal_matches_and_parens_are_rejected() {
    let use_case = "crates/application/src/confirm_instrument_identity.rs";
    for bad in [
        "use crate::fake_prompt::{IdentityConfirmationDecision, IdentityConfirmationPrompt};
pub struct ConfirmInstrumentIdentity<'a, P> { prompt: &'a P }
impl<'a, P: IdentityConfirmationPrompt> ConfirmInstrumentIdentity<'a, P> {
    pub async fn confirm(&self) {
        match self.prompt.confirm_identifier_change().await {
            IdentityConfirmationDecision::Confirmed => { let _ = UserIdentityConfirmation::issue_after_user_confirmation(); }
            _ => {}
        }
    }
}",
        "use crate::identity_confirmation_prompt::{IdentityConfirmationDecision, IdentityConfirmationPrompt};
pub struct ConfirmInstrumentIdentity<'a, P> { prompt: &'a P }
impl<'a, P: other::IdentityConfirmationPrompt> ConfirmInstrumentIdentity<'a, P> {
    pub async fn confirm(&self) {
        match self.prompt.confirm_identifier_change().await {
            IdentityConfirmationDecision::Confirmed => { let _ = UserIdentityConfirmation::issue_after_user_confirmation(); }
            _ => {}
        }
    }
}",
        "use crate::identity_confirmation_prompt::IdentityConfirmationDecision;
pub struct ConfirmInstrumentIdentity<'a, P> { prompt: &'a P }
fn hide() { struct ConfirmInstrumentIdentity<'a, P> { prompt: &'a P } }",
    ] {
        assert_eq!(
            scan("application", use_case, bad),
            BTreeSet::from([Rule::IdentityConfirmationIssuance]),
            "{bad}"
        );
    }
    assert_eq!(
        scan(
            "testkit-ports",
            "crates/testkit-ports/src/scripted_identity_confirmation.rs",
            "fn forge() { match IdentityConfirmationDecision::Confirmed { IdentityConfirmationDecision::Confirmed => { let _ = UserIdentityConfirmation::issue_after_user_confirmation(); } _ => {} } }",
        ),
        BTreeSet::from([Rule::IdentityConfirmationIssuance])
    );
    assert_eq!(
        scan(
            "domain",
            "crates/domain/src/other.rs",
            "impl (crate::UserIdentityConfirmation) { pub fn mint() {} }",
        ),
        BTreeSet::from([Rule::IdentityConfirmationIssuance])
    );
}

#[test]
fn test_module_impl_async_block_and_local_trait_are_rejected() {
    let use_case = "crates/application/src/confirm_instrument_identity.rs";
    assert_eq!(
        scan(
            "sources",
            "crates/adapters/sources/src/lib.rs",
            "expand!([impl IdentityConfirmationPrompt for Hidden {}]);",
        ),
        BTreeSet::from([Rule::IdentityConfirmationPromptImpl])
    );
    assert_eq!(
        scan(
            "domain",
            "crates/domain/src/instrument_mapping_change/history_cases.rs",
            "#[cfg(test)] mod tests { impl super::super::UserIdentityConfirmation { pub fn backdoor() {} } }",
        ),
        BTreeSet::from([Rule::IdentityConfirmationIssuance])
    );
    assert_eq!(
        scan("application", use_case, "trait Helper {}"),
        BTreeSet::from([Rule::IdentityConfirmationIssuance])
    );
    assert_eq!(
        scan(
            "application",
            use_case,
            "use crate::identity_confirmation_prompt::{IdentityConfirmationDecision, IdentityConfirmationPrompt};
pub struct ConfirmInstrumentIdentity<'a, P> { prompt: &'a P }
impl<'a, P: IdentityConfirmationPrompt> ConfirmInstrumentIdentity<'a, P> {
    pub async fn confirm(&self) {
        match self.prompt.confirm_identifier_change().await {
            IdentityConfirmationDecision::Confirmed => { let _ = async { UserIdentityConfirmation::issue_after_user_confirmation() }; }
            _ => {}
        }
    }
}",
        ),
        BTreeSet::from([Rule::IdentityConfirmationIssuance])
    );
    assert_eq!(
        scan(
            "application",
            use_case,
            "use crate::identity_confirmation_prompt::{IdentityConfirmationDecision, IdentityConfirmationPrompt};
pub struct ConfirmInstrumentIdentity<'a, P> { prompt: &'a P }
impl<'a, P: IdentityConfirmationPrompt> ConfirmInstrumentIdentity<'a, P> {
    pub async fn leak_raw(&self) -> Option<UserIdentityConfirmation> {
        match self.prompt.confirm_identifier_change().await {
            IdentityConfirmationDecision::Confirmed => Some(UserIdentityConfirmation::issue_after_user_confirmation()),
            _ => None,
        }
    }
}",
        ),
        BTreeSet::from([Rule::IdentityConfirmationIssuance])
    );
}
