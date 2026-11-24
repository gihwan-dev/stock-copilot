#[cfg(test)]
mod tests {
    use std::{fs, path::Path, process::Output};

    fn run(directory: &Path, source: &str) -> Output {
        fs::write(directory.join("src/main.rs"), source).expect("소스");
        testkit::check_consumer(&directory.join("Cargo.toml"), &directory.join("target"))
            .expect("컴파일러")
    }

    type CompileCase = (&'static str, &'static str, Option<&'static str>);

    fn sell_boundary_cases() -> [CompileCase; 4] {
        [
            (
                "control_with_history",
                "use domain::*; fn check(live: &SellSignal, stored: &HistoricalSignal, facts: &SellDraftQuantityFacts<'_>, now: Timestamp, cal: &[MarketCalendar]) -> SellDraftQuantityDecision { calculate_sell_draft_quantity_with_history(live, stored, facts, now, cal) } fn main() { let _ = check; }",
                None,
            ),
            (
                "historical_sell_signal_accessor_private",
                "use domain::*; fn check(stored: &HistoricalSignal) { let _ = stored.sell_signal(); } fn main() { let _ = check; }",
                Some("error[E0624]"),
            ),
            (
                "historical_signal_not_sell_signal",
                "use domain::*; fn check(stored: &HistoricalSignal, facts: &SellDraftQuantityFacts<'_>, now: Timestamp, cal: &[MarketCalendar]) { let _ = calculate_sell_draft_quantity(stored, facts, now, cal); } fn main() { let _ = check; }",
                Some("error[E0308]"),
            ),
            (
                "historical_signal_not_live_in_with_history",
                "use domain::*; fn check(stored: &HistoricalSignal, facts: &SellDraftQuantityFacts<'_>, now: Timestamp, cal: &[MarketCalendar]) { let _ = calculate_sell_draft_quantity_with_history(stored, stored, facts, now, cal); } fn main() { let _ = check; }",
                Some("error[E0308]"),
            ),
        ]
    }

    #[test]
    fn historical_signal_cannot_bypass_live_sell_draft_quantity_boundary() {
        let root = std::env::current_dir().expect("도메인 경로");
        let directory = root
            .join("../../target/D-57")
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
                "[package]\nname = \"d57-boundary\"\nversion = \"0.0.0\"\nedition = \"2024\"\n[workspace]\n[dependencies]\ndomain = {{ path = \"{dependency}\" }}\n"
            ),
        )
        .expect("명세");
        for (name, source, expected) in sell_boundary_cases() {
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
