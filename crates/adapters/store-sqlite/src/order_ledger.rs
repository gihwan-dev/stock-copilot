use application::{
    AuditEntry, AuditKind, ConsumptionOutcome, ConsumptionStatus, OrderRateRepository,
    ProposalConsumptionRepository, SystemPort, SystemPortError, ValidationReportRepository,
    WriteCompleted,
};
use domain::{ClientOrderId, HourlyOrderLimit, ProposalId, Timestamp, ValidationReport};

use crate::shared::opened;
use crate::sql::order_rate as rate_sql;
use crate::{OrderRateStore, ProposalConsumptionStore, SqliteRepositories, ValidationReportStore};

const REPOSITORY: SystemPort = SystemPort::Repository;

impl OrderRateRepository for SqliteRepositories {
    async fn hourly_limit(&self) -> Result<Option<HourlyOrderLimit>, SystemPortError> {
        let guard = self.shared().read().await;
        let database = opened(guard.as_ref(), REPOSITORY)?;
        OrderRateStore::new(database).hourly_limit().await
    }

    async fn recent_submission_times(
        &self,
        since: Timestamp,
    ) -> Result<Vec<Timestamp>, SystemPortError> {
        let guard = self.shared().read().await;
        let database = opened(guard.as_ref(), REPOSITORY)?;
        OrderRateStore::new(database)
            .recent_submission_times(since)
            .await
    }
}

impl ProposalConsumptionRepository for SqliteRepositories {
    async fn consume(&self, proposal: &ProposalId) -> Result<ConsumptionOutcome, SystemPortError> {
        let guard = self.shared().read().await;
        let database = opened(guard.as_ref(), REPOSITORY)?;
        ProposalConsumptionRepository::consume(&ProposalConsumptionStore::new(database), proposal)
            .await
    }

    async fn status(&self, proposal: &ProposalId) -> Result<ConsumptionStatus, SystemPortError> {
        let guard = self.shared().read().await;
        let database = opened(guard.as_ref(), REPOSITORY)?;
        ProposalConsumptionRepository::status(&ProposalConsumptionStore::new(database), proposal)
            .await
    }
}

impl ValidationReportRepository for SqliteRepositories {
    async fn append(
        &self,
        proposal: &ProposalId,
        reports: &[ValidationReport],
    ) -> Result<WriteCompleted, SystemPortError> {
        let guard = self.shared().read().await;
        let database = opened(guard.as_ref(), REPOSITORY)?;
        ValidationReportRepository::append(&ValidationReportStore::new(database), proposal, reports)
            .await
    }

    async fn history(
        &self,
        proposal: &ProposalId,
    ) -> Result<Vec<ValidationReport>, SystemPortError> {
        let guard = self.shared().read().await;
        let database = opened(guard.as_ref(), REPOSITORY)?;
        ValidationReportRepository::history(&ValidationReportStore::new(database), proposal).await
    }
}

pub async fn commit_prepared_submission(
    repositories: &SqliteRepositories,
    proposal: &ProposalId,
    client_order_id: &ClientOrderId,
    prepared_at: Timestamp,
) -> Result<ConsumptionOutcome, SystemPortError> {
    let guard = repositories.shared().read().await;
    let database = opened(guard.as_ref(), SystemPort::AuditLog)?;
    let owned = proposal.clone();
    let encoded = crate::audit::encode(&AuditEntry::new(
        prepared_at,
        AuditKind::SubmissionPrepared(client_order_id.clone()),
    ));
    let seconds = *prepared_at.utc_seconds().as_i64();
    let nanos = i64::from(*prepared_at.nanosecond().as_u32());
    let offset = i64::from(*prepared_at.offset_seconds().as_i32());
    database
        .write(move |transaction| {
            match crate::proposal_consumption::consume(transaction, &owned)? {
                ConsumptionOutcome::AlreadyConsumed => Ok(ConsumptionOutcome::AlreadyConsumed),
                ConsumptionOutcome::Recorded => {
                    let _audit = crate::audit::append(transaction, &encoded)?;
                    match transaction.execute(
                        rate_sql::INSERT_SUBMISSION,
                        rusqlite::params![seconds, nanos, offset],
                    )? {
                        1 => Ok(ConsumptionOutcome::Recorded),
                        _ => Err(rusqlite::Error::InvalidQuery),
                    }
                }
            }
        })
        .await
}

pub async fn proposal_instrument(
    repositories: &SqliteRepositories,
    proposal: &ProposalId,
) -> Result<Option<domain::InstrumentId>, SystemPortError> {
    let guard = repositories.shared().read().await;
    let database = opened(guard.as_ref(), REPOSITORY)?;
    let wanted = String::from(proposal.as_client_order_id().as_str());
    let stored: Option<String> = database
        .read(move |connection| {
            rusqlite::OptionalExtension::optional(connection.query_row(
                crate::sql::signals::DRAFT_INSTRUMENT_BY_ID,
                [wanted],
                |row| row.get(0),
            ))
        })
        .await?;
    stored
        .map(|text| {
            domain::InstrumentId::try_new(&text).map_err(|_error| {
                SystemPortError::new(REPOSITORY, application::SystemFailure::InvalidData)
            })
        })
        .transpose()
}

#[cfg(test)]
mod tests;
