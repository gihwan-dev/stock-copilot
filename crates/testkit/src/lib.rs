#[must_use]
pub fn property_cases() -> u32 {
    cases_for(
        std::env::var("STOCK_COPILOT_PROPTEST_CASES")
            .as_deref()
            .ok(),
    )
}

#[must_use]
pub fn property_cases_at_least(minimum: u32) -> u32 {
    property_cases().max(minimum)
}

fn cases_for(ci: Option<&str>) -> u32 {
    match ci {
        None => 256,
        Some(_) => 4096,
    }
}

#[cfg(test)]
mod tests {
    use super::cases_for;
    #[test]
    fn local_and_full_counts_are_fixed() {
        assert_eq!(cases_for(None), 256);
        assert_eq!(cases_for(Some("256")), 4096);
        assert_eq!(cases_for(Some("4096")), 4096);
    }
}
