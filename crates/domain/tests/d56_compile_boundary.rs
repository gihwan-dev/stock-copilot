#[cfg(test)]
mod tests {
    use std::{fs, path::Path, process::Output};

    fn run(directory: &Path, source: &str) -> Output {
        fs::write(directory.join("src/main.rs"), source).expect("소스");
        testkit::check_consumer(&directory.join("Cargo.toml"), &directory.join("target"))
            .expect("컴파일러")
    }

    type CompileCase = (&'static str, &'static str, Option<&'static str>);

    fn draft_cases() -> [CompileCase; 7] {
        [
            (
                "control",
                "use domain::*; fn check(e: &DraftRestoreEvidence, m: &MasterInstrument) -> Result<Proposal<Draft>, DraftRestoreError> { e.restore(m) } fn main() { let _ = check; }",
                None,
            ),
            (
                "validated",
                "use domain::*; fn check(e: &DraftRestoreEvidence, m: &MasterInstrument) -> Result<Proposal<Validated>, DraftRestoreError> { e.restore(m) } fn main() { let _ = check; }",
                Some("error[E0308]"),
            ),
            (
                "execution",
                "use domain::*; fn check(p: Proposal<Draft>, t: Timestamp, o: PriceObservation) { let _ = p.into_execution(t, o); } fn main() { let _ = check; }",
                Some("error[E0599]"),
            ),
            (
                "approval",
                "use domain::*; fn check(p: Proposal<Draft>, i: InstrumentNameFact, a: AccountDisplayFact, f: EstimatedOrderFee, r: OrderFxRates) { let _ = PendingOrderConfirmation::from_execution(p, i, a, f, r); } fn main() { let _ = check; }",
                Some("error[E0308]"),
            ),
            (
                "approval_control",
                "use domain::*; fn check(p: ExecutionToken, i: InstrumentNameFact, a: AccountDisplayFact, f: EstimatedOrderFee, r: OrderFxRates) { let _ = PendingOrderConfirmation::from_execution(p, i, a, f, r); } fn main() { let _ = check; }",
                None,
            ),
            (
                "draft_order_quantity",
                "use domain::*; fn check(e: &DraftRestoreEvidence, m: &MasterInstrument) -> Option<WholeOrderQuantity> { let p = e.restore(m).ok()?; match p.intent().terms() { HistoricalOrderTerms::UsMarketBuy { quantity } => Some(quantity), _ => None } } fn main() { let _ = check; }",
                Some("error[E0308]"),
            ),
            (
                "historical_sell_quantity",
                "use domain::*; fn check(e: &StoredSignalEvidence) -> Option<SellOrderQuantity> { match e.expected.direction { HistoricalSignalDirection::Sell { quantity, .. } => SellOrderQuantity::try_new(quantity).ok(), _ => None } } fn main() { let _ = check; }",
                Some("error[E0308]"),
            ),
        ]
    }

    fn history_cases() -> [CompileCase; 7] {
        [
            (
                "historical_holding_quantity",
                "use domain::*; fn check(p: &HistoricalPortfolio) -> Option<SellOrderQuantity> { let quantity = p.facts().holdings.first()?.quantity; SellOrderQuantity::try_new(quantity).ok() } fn main() { let _ = check; }",
                Some("error[E0308]"),
            ),
            (
                "historical_display_control",
                "use domain::*; fn check(h: HistoricalSignal) -> SignalDisplayFacts { h.display_facts() } fn main() { let _ = check; }",
                None,
            ),
            (
                "current_buy_signal",
                "use domain::*; fn check(h: HistoricalSignal, r: BuySignalRecord) { let _ = CurrentBuySignal::try_new(h, r); } fn main() { let _ = check; }",
                Some("error[E0308]"),
            ),
            (
                "historical_occurrence",
                "use domain::*; fn check(s: BuySignal, h: HistoricalSignalOccurrence) { let _ = CurrentBuySignal::try_new(s, h); } fn main() { let _ = check; }",
                Some("error[E0308]"),
            ),
            (
                "quantity",
                "use domain::*; fn check(h: &HistoricalSignal, f: &BuyQuantityFacts<'_>, n: Timestamp, c: &[MarketCalendar], r: &[BuySignalRecord]) { let _ = calculate_buy_quantity(h, f, n, c, r); } fn main() { let _ = check; }",
                Some("error[E0308]"),
            ),
            (
                "past_portfolio",
                "use domain::*; fn check(h: &HistoricalPortfolio) -> &Portfolio { h.portfolio() } fn main() { let _ = check; }",
                Some("error[E0624]"),
            ),
            (
                "past_classifications",
                "use domain::*; fn check(h: &HistoricalClassifications) { let _ = h.replay(); } fn main() { let _ = check; }",
                Some("error[E0624]"),
            ),
        ]
    }

    #[test]
    fn restored_data_cannot_become_validation_execution_or_current_quantity_authority() {
        let root = std::env::current_dir().expect("도메인 경로");
        let directory = root
            .join("../../target/D-56")
            .join(format!("compile-boundary-{}", std::process::id()));
        fs::create_dir_all(directory.join("src")).expect("임시 폴더");
        let dependency = root
            .canonicalize()
            .expect("도메인 경로")
            .to_string_lossy()
            .replace('\\', "\\\\")
            .replace('"', "\\\"");
        fs::write(directory.join("Cargo.toml"), format!("[package]\nname = \"d56-boundary\"\nversion = \"0.0.0\"\nedition = \"2024\"\n[workspace]\n[dependencies]\ndomain = {{ path = \"{dependency}\" }}\n")).expect("명세");
        for (name, source, expected) in draft_cases().into_iter().chain(history_cases()) {
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
