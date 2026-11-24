#[cfg(test)]
mod tests {
    use std::{fs, path::Path, process::Output};

    fn run(directory: &Path, source: &str) -> Output {
        fs::write(directory.join("src/main.rs"), source).expect("소스");
        testkit::check_consumer(&directory.join("Cargo.toml"), &directory.join("target"))
            .expect("컴파일러")
    }

    type CompileCase = (&'static str, &'static str, Option<&'static str>);

    fn proof_boundary_cases() -> [CompileCase; 7] {
        [
            (
                "control_adapter_collector_and_proven_d38",
                "use domain::*; fn check(account: AccountId, window: TimeInterval, observed: Timestamp) -> Result<ClosedOrderSet, NetPurchaseInputError> { let scope = ClosedOrderQueryScope::unfiltered(account.clone(), window); let mut collector = ClosedOrderPageCollector::start(scope); collector.record_page_boundary(OrderPageBoundary::LastPage); let proven = collector.finish(observed)?; let _ = proven_net_purchases(&proven, window); let _ = proven_net_purchase_amount(&proven, window, NetPurchaseScope::Currency(Currency::Krw)); let closed = proven.into_closed_order_set(); let mut open = OpenOrderCollector::start(OpenOrderQueryScope::unfiltered(account)); open.record_all_pages_received(); let _ = open.finish(observed); Ok(closed) } fn main() { let _ = check; }",
                None,
            ),
            (
                "unproven_closed_order_set_rejected_by_proven_d38",
                "use domain::*; fn bypass(orders: ClosedOrderSet, window: TimeInterval) { let _ = proven_net_purchase_amount(&orders, window, NetPurchaseScope::Currency(Currency::Krw)); } fn main() { let _ = bypass; }",
                Some("error[E0308]"),
            ),
            (
                "forged_completeness_proof_struct_rejected",
                "use domain::*; fn forge(scope: ClosedOrderQueryScope) -> ClosedOrderCompletenessProof { ClosedOrderCompletenessProof { scope, exhaustion: OrderPageExhaustion::Exhausted, completeness: ClosedOrderCompleteness::Complete } } fn main() { let _ = forge; }",
                Some("error[E0451]"),
            ),
            (
                "forged_proven_closed_order_set_rejected",
                "use domain::*; fn forge(orders: ClosedOrderSet, proof: ClosedOrderCompletenessProof) -> ProvenClosedOrderSet { ProvenClosedOrderSet { orders, proof } } fn main() { let _ = forge; }",
                Some("error[E0451]"),
            ),
            (
                "private_from_verified_proof_rejected",
                "use domain::*; fn forge(account: AccountId, covered: TimeInterval, observed: Timestamp, proof: ClosedOrderCompletenessProof) { let _ = ClosedOrderSet::from_verified_proof(account, std::collections::BTreeMap::new(), covered, observed, proof); } fn main() { let _ = forge; }",
                Some("error[E0624]"),
            ),
            (
                "closed_scope_field_mutation_rejected",
                "use domain::*; fn tamper(mut scope: ClosedOrderQueryScope) { scope.date_filter = OrderDateFilter::Unfiltered; } fn main() { let _ = tamper; }",
                Some("error[E0616]"),
            ),
            (
                "open_scope_direct_construction_rejected",
                "use domain::*; fn forge(account_id: AccountId) -> OpenOrderQueryScope { OpenOrderQueryScope { account_id, date_filter: OrderDateFilter::Unfiltered, symbol_filter: OrderSymbolFilter::AllSymbols } } fn main() { let _ = forge; }",
                Some("error[E0451]"),
            ),
        ]
    }

    #[test]
    fn completeness_proof_types_seal_direct_construction_and_proven_d38_rejects_unproven_sets() {
        let root = std::env::current_dir().expect("도메인 경로");
        let directory = root
            .join("../../target/T-15")
            .join(format!("compile-boundary-{}", std::process::id()));
        fs::create_dir_all(directory.join("src")).expect("임시 폴더");
        let dependency = root
            .canonicalize()
            .expect("도메인 경로")
            .to_string_lossy()
            .replace('\\', "\\\\")
            .replace('"', "\\\"");
        fs::write(
            directory.join("Cargo.toml"),
            format!(
                "[package]\nname = \"t15-boundary\"\nversion = \"0.0.0\"\nedition = \"2024\"\n[workspace]\n[dependencies]\ndomain = {{ path = \"{dependency}\" }}\n"
            ),
        )
        .expect("명세");
        for (name, source, expected) in proof_boundary_cases() {
            let output = run(&directory, source);
            let diagnostics = String::from_utf8_lossy(&output.stderr);
            match expected {
                None => assert!(output.status.success(), "{name}: {diagnostics}"),
                Some(code) => {
                    assert!(!output.status.success(), "{name} unexpectedly compiled");
                    assert!(diagnostics.contains(code), "{name}: {diagnostics}");
                }
            }
        }
        fs::remove_dir_all(directory).expect("임시 폴더 정리");
    }
}
