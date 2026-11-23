#[cfg(test)]
mod tests {
    use domain::{CheckResult, ValidationCheck, ValidationReport, ValidationReportResults};

    #[test]
    fn stored_check_results_are_preserved_without_creating_validation_authority() {
        let report = ValidationReport::from_results(ValidationReportResults {
            instrument: CheckResult::Passed,
            market_hours: CheckResult::Failed,
            available_funds_or_quantity: CheckResult::Unknown,
            opposite_pending_order: CheckResult::Passed,
            buy_warnings: CheckResult::Failed,
            principles: CheckResult::Unknown,
            order_price: CheckResult::Passed,
            market_order_permission: CheckResult::Failed,
            account_match: CheckResult::Unknown,
            price_observation: CheckResult::Passed,
            expiry: CheckResult::Failed,
            price_range: CheckResult::Unknown,
        });
        for (check, expected) in [
            (ValidationCheck::Instrument, CheckResult::Passed),
            (ValidationCheck::MarketHours, CheckResult::Failed),
            (
                ValidationCheck::AvailableFundsOrQuantity,
                CheckResult::Unknown,
            ),
            (ValidationCheck::OppositePendingOrder, CheckResult::Passed),
            (ValidationCheck::BuyWarnings, CheckResult::Failed),
            (ValidationCheck::Principles, CheckResult::Unknown),
            (ValidationCheck::OrderPrice, CheckResult::Passed),
            (ValidationCheck::MarketOrderPermission, CheckResult::Failed),
            (ValidationCheck::AccountMatch, CheckResult::Unknown),
            (ValidationCheck::PriceObservation, CheckResult::Passed),
            (ValidationCheck::Expiry, CheckResult::Failed),
            (ValidationCheck::PriceRange, CheckResult::Unknown),
        ] {
            assert_eq!(report.result(check), expected, "{check:?}");
        }
    }
}
