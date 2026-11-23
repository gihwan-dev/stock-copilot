use crate::{
    Database, DatabasePath, ValidationReportStore,
    tests::{Fixture, clock, key, runtime},
};
use application::ValidationReportRepository;
use domain::{
    CheckResult, ClientOrderId, ProposalId, ValidationCheck, ValidationReport,
    ValidationReportResults,
};

fn report(check: ValidationCheck, picked: CheckResult) -> ValidationReport {
    let value = |item| {
        if item == check {
            picked
        } else {
            CheckResult::Unknown
        }
    };
    ValidationReport::from_results(ValidationReportResults {
        instrument: value(ValidationCheck::Instrument),
        market_hours: value(ValidationCheck::MarketHours),
        available_funds_or_quantity: value(ValidationCheck::AvailableFundsOrQuantity),
        opposite_pending_order: value(ValidationCheck::OppositePendingOrder),
        buy_warnings: value(ValidationCheck::BuyWarnings),
        principles: value(ValidationCheck::Principles),
        order_price: value(ValidationCheck::OrderPrice),
        market_order_permission: value(ValidationCheck::MarketOrderPermission),
        account_match: value(ValidationCheck::AccountMatch),
        price_observation: value(ValidationCheck::PriceObservation),
        expiry: value(ValidationCheck::Expiry),
        price_range: value(ValidationCheck::PriceRange),
    })
}
fn proposal(value: &str) -> ProposalId {
    ProposalId::new(ClientOrderId::try_new(value).expect("id"))
}

#[test]
fn validation_reports_preserve_each_check_and_are_scoped_by_proposal() {
    let fixture = Fixture::new();
    let rt = runtime();
    let path = DatabasePath::new(fixture.path());
    let id = proposal("report-1");
    let other = proposal("report-2");
    let expected: Vec<_> = [
        ValidationCheck::Instrument,
        ValidationCheck::MarketHours,
        ValidationCheck::AvailableFundsOrQuantity,
        ValidationCheck::OppositePendingOrder,
        ValidationCheck::BuyWarnings,
        ValidationCheck::Principles,
        ValidationCheck::OrderPrice,
        ValidationCheck::MarketOrderPermission,
        ValidationCheck::AccountMatch,
        ValidationCheck::PriceObservation,
        ValidationCheck::Expiry,
        ValidationCheck::PriceRange,
    ]
    .into_iter()
    .map(|check| report(check, CheckResult::Passed))
    .chain([report(ValidationCheck::AccountMatch, CheckResult::Failed)])
    .collect();
    {
        let db = Database::open(&path, &key(), &clock()).expect("open");
        let store = ValidationReportStore::new(&db);
        rt.block_on(store.append(&id, &expected)).expect("append");
        assert_eq!(rt.block_on(store.history(&other)), Ok(vec![]));
    };
    let db = Database::open(&path, &key(), &clock()).expect("reopen");
    let store = ValidationReportStore::new(&db);
    assert_eq!(rt.block_on(store.history(&id)), Ok(expected));
    assert_eq!(rt.block_on(store.history(&other)), Ok(vec![]));
}

#[test]
fn report_batch_rolls_back_on_second_insert_failure() {
    let fixture = Fixture::new();
    let rt = runtime();
    let db = Database::open(&DatabasePath::new(fixture.path()), &key(), &clock()).expect("open");
    let store = ValidationReportStore::new(&db);
    let id = proposal("report-1");
    rt.block_on(db.write(|tx| tx.execute_batch(crate::sql::proposal_tests::FAIL_REPORT)))
        .expect("fixture");
    let reports = [
        report(ValidationCheck::Instrument, CheckResult::Passed),
        report(ValidationCheck::Instrument, CheckResult::Failed),
    ];
    rt.block_on(store.append(&id, &reports))
        .expect_err("injected failure");
    assert_eq!(rt.block_on(store.history(&id)), Ok(vec![]));
}

#[test]
fn corrupted_report_fails_closed_without_affecting_other_proposals() {
    let fixture = Fixture::new();
    let rt = runtime();
    let db = Database::open(&DatabasePath::new(fixture.path()), &key(), &clock()).expect("open");
    let store = ValidationReportStore::new(&db);
    let bad = proposal("report-1");
    let good = proposal("report-2");
    let expected = [report(ValidationCheck::PriceRange, CheckResult::Failed)];
    rt.block_on(store.append(&bad, &expected)).expect("append");
    rt.block_on(store.append(&good, &expected)).expect("append");
    rt.block_on(db.write(|tx| {
        tx.execute_batch(crate::sql::proposal_tests::IGNORE_CHECKS_ON)?;
        tx.execute(crate::sql::proposal_tests::CORRUPT_REPORT, ["report-1"])?;
        tx.execute_batch(crate::sql::proposal_tests::IGNORE_CHECKS_OFF)?;
        Ok(())
    }))
    .expect("corrupt fixture");
    rt.block_on(store.history(&bad))
        .expect_err("invalid result");
    assert_eq!(rt.block_on(store.history(&good)), Ok(expected.to_vec()));
}
