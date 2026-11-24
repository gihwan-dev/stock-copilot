#[cfg(test)]
mod tests {
    use std::{fs, path::Path, process::Output};

    fn run(directory: &Path, source: &str) -> Output {
        fs::write(directory.join("src/main.rs"), source).expect("소스");
        testkit::check_consumer(&directory.join("Cargo.toml"), &directory.join("target"))
            .expect("컴파일러")
    }

    type CompileCase = (&'static str, &'static str, Option<&'static str>);

    fn cases() -> [CompileCase; 6] {
        [
            (
                "proven_set_feeds_net_purchase_and_daily_profit",
                "use domain::*; fn check(proven: &ProvenClosedOrderSet, window: TimeInterval, today: &ValuationSnapshot, rows: &SettledRowSet) { let _ = proven_net_purchases(proven, window); let _ = holdings_daily_profit_with_orders(today, None, proven, Currency::Krw); let _ = compare_holdings_daily_profit_with_orders(today, None, proven, rows, None); } fn main() { let _ = check; }",
                None,
            ),
            (
                "public_closed_order_set_constructor_removed",
                "use domain::*; fn forge(account: AccountId, covered: TimeInterval, observed: Timestamp) { let _ = ClosedOrderSet::new(account, Vec::new(), covered, observed, ClosedOrderCompleteness::Complete); } fn main() { let _ = forge; }",
                Some("error[E0599]"),
            ),
            (
                "unproven_net_purchases_not_public",
                "use domain::*; fn bypass(orders: &ClosedOrderSet, window: TimeInterval) { let _ = net_purchases(orders, window); } fn main() { let _ = bypass; }",
                Some("error[E0425]"),
            ),
            (
                "unproven_net_purchase_amount_not_public",
                "use domain::*; fn bypass(orders: &ClosedOrderSet, window: TimeInterval) { let _ = net_purchase_amount(orders, window, NetPurchaseScope::Currency(Currency::Krw)); } fn main() { let _ = bypass; }",
                Some("error[E0425]"),
            ),
            (
                "unproven_daily_profit_rejected",
                "use domain::*; fn bypass(orders: &ClosedOrderSet, today: &ValuationSnapshot) { let _ = holdings_daily_profit_with_orders(today, None, orders, Currency::Krw); } fn main() { let _ = bypass; }",
                Some("error[E0308]"),
            ),
            (
                "unproven_daily_comparison_rejected",
                "use domain::*; fn bypass(orders: &ClosedOrderSet, today: &ValuationSnapshot, rows: &SettledRowSet) { let _ = compare_holdings_daily_profit_with_orders(today, None, orders, rows, None); } fn main() { let _ = bypass; }",
                Some("error[E0308]"),
            ),
        ]
    }

    #[test]
    fn net_purchase_calls_without_completeness_proof_do_not_compile() {
        let root = std::env::current_dir().expect("도메인 경로");
        let directory = root
            .join("../../target/D-59")
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
                "[package]\nname = \"d59-boundary\"\nversion = \"0.0.0\"\nedition = \"2024\"\n[workspace]\n[dependencies]\ndomain = {{ path = \"{dependency}\" }}\n"
            ),
        )
        .expect("명세");
        for (name, source, expected) in cases() {
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
