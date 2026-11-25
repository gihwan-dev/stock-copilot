use std::collections::BTreeSet;
use std::str::FromStr;

use proc_macro2::TokenStream;

use super::super::super::CrateName;
use super::super::super::rules::OrderCollectorRules;
use super::mentions_collector;

fn rules() -> OrderCollectorRules {
    OrderCollectorRules {
        collector_types: ["SealedAlphaCollector", "SealedBetaCollector"]
            .map(String::from)
            .into_iter()
            .collect(),
        allowed_crates: BTreeSet::from([CrateName::new("broker-toss")]),
        allowed_files: BTreeSet::from([String::from("crates/domain/src/order_query_proof.rs")]),
    }
}

fn flagged(crate_name: &str, relative: &str, source: &str) -> bool {
    let stream = TokenStream::from_str(source).expect("tokens");
    mentions_collector(&rules(), &CrateName::new(crate_name), relative, &stream)
}

const OUTSIDE: &str = "crates/application/src/sync.rs";

#[test]
fn direct_call_outside_allowed_locations_is_flagged() {
    assert!(flagged(
        "application",
        OUTSIDE,
        "fn f(s: S) { let _ = domain::SealedAlphaCollector::start(s); }"
    ));
    assert!(flagged(
        "application",
        OUTSIDE,
        "fn f(s: S) { let _ = r#SealedBetaCollector::start(s); }"
    ));
}

#[test]
fn aliases_strings_and_synthesized_names_are_flagged() {
    for source in [
        "use domain::SealedBetaCollector as Open; fn f(s: S) { let _ = Open::start(s); }",
        "use domain::{order_query_proof::{SealedAlphaCollector}};",
        "#[serde(default = \"domain::SealedBetaCollector::start\")] struct S;",
        "fn f() { m! { domain::[<SealedAlpha Collector>]::start(s) } }",
        "fn f() { m! { domain::[<Sealed \"Beta\" Collector>]::start(s) } }",
        "const N: &str = \"Sealed\\u{42}etaCollector\";",
    ] {
        assert!(flagged("application", OUTSIDE, source), "{source}");
    }
}

#[test]
fn allowed_crate_and_files_pass() {
    let call = "fn f(s: S) { let _ = SealedAlphaCollector::start(s); }";
    assert!(!flagged(
        "broker-toss",
        "crates/adapters/broker-toss/src/order_read.rs",
        call
    ));
    assert!(!flagged(
        "domain",
        "crates/domain/src/order_query_proof.rs",
        call
    ));
    assert!(flagged("domain", "crates/domain/src/other.rs", call));
    assert!(flagged(
        "broker-toss-trade",
        "crates/adapters/broker-toss-trade/src/lib.rs",
        call
    ));
}

#[test]
fn unrelated_source_passes() {
    assert!(!flagged(
        "application",
        OUTSIDE,
        "fn f() { let _ = SealedSet::default(); let _ = BetaCollector::start(); }"
    ));
}
