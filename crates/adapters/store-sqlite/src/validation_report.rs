use crate::{Database, sql::proposal as sql};
use application::{SystemPortError, ValidationReportRepository, WriteCompleted};
use domain::{CheckResult, ProposalId, ValidationCheck, ValidationReport, ValidationReportResults};
use rusqlite::{Row, params};

#[derive(Debug)]
pub struct ValidationReportStore<'database> {
    database: &'database Database,
}
impl<'database> ValidationReportStore<'database> {
    #[must_use]
    pub const fn new(database: &'database Database) -> Self {
        Self { database }
    }
}
fn tag(result: CheckResult) -> &'static str {
    match result {
        CheckResult::Passed => "passed",
        CheckResult::Failed => "failed",
        CheckResult::Unknown => "unknown",
    }
}
fn result(row: &Row<'_>, index: usize) -> rusqlite::Result<CheckResult> {
    match row.get::<_, String>(index)?.as_str() {
        "passed" => Ok(CheckResult::Passed),
        "failed" => Ok(CheckResult::Failed),
        "unknown" => Ok(CheckResult::Unknown),
        _ => Err(rusqlite::Error::InvalidQuery),
    }
}
fn decode(row: &Row<'_>) -> rusqlite::Result<ValidationReport> {
    Ok(ValidationReport::from_results(ValidationReportResults {
        instrument: result(row, 0)?,
        market_hours: result(row, 1)?,
        available_funds_or_quantity: result(row, 2)?,
        opposite_pending_order: result(row, 3)?,
        buy_warnings: result(row, 4)?,
        principles: result(row, 5)?,
        order_price: result(row, 6)?,
        market_order_permission: result(row, 7)?,
        account_match: result(row, 8)?,
        price_observation: result(row, 9)?,
        expiry: result(row, 10)?,
        price_range: result(row, 11)?,
    }))
}
impl ValidationReportRepository for ValidationReportStore<'_> {
    async fn append(
        &self,
        proposal: &ProposalId,
        reports: &[ValidationReport],
    ) -> Result<WriteCompleted, SystemPortError> {
        let owned_id = proposal.clone();
        let owned_reports = reports.to_vec();
        self.database
            .write(move |tx| {
                for owned_report in owned_reports {
                    tx.execute(
                        sql::INSERT_REPORT,
                        params![
                            owned_id.as_client_order_id().as_str(),
                            tag(owned_report.result(ValidationCheck::Instrument)),
                            tag(owned_report.result(ValidationCheck::MarketHours)),
                            tag(owned_report.result(ValidationCheck::AvailableFundsOrQuantity)),
                            tag(owned_report.result(ValidationCheck::OppositePendingOrder)),
                            tag(owned_report.result(ValidationCheck::BuyWarnings)),
                            tag(owned_report.result(ValidationCheck::Principles)),
                            tag(owned_report.result(ValidationCheck::OrderPrice)),
                            tag(owned_report.result(ValidationCheck::MarketOrderPermission)),
                            tag(owned_report.result(ValidationCheck::AccountMatch)),
                            tag(owned_report.result(ValidationCheck::PriceObservation)),
                            tag(owned_report.result(ValidationCheck::Expiry)),
                            tag(owned_report.result(ValidationCheck::PriceRange))
                        ],
                    )?;
                }
                Ok(WriteCompleted)
            })
            .await
    }
    async fn history(
        &self,
        proposal: &ProposalId,
    ) -> Result<Vec<ValidationReport>, SystemPortError> {
        let owned_id = proposal.clone();
        self.database
            .read(move |connection| {
                connection
                    .prepare(sql::REPORTS)?
                    .query_map([owned_id.as_client_order_id().as_str()], decode)?
                    .collect()
            })
            .await
    }
}
