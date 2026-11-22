use crate::{Database, sql::proposal as sql};
use application::{
    ConsumptionOutcome, ConsumptionStatus, ProposalConsumptionRepository, SystemPortError,
};
use domain::{ClientOrderId, ProposalId};
use rusqlite::{Connection, OptionalExtension, Transaction};

#[derive(Debug)]
pub struct ProposalConsumptionStore<'database> {
    database: &'database Database,
}
impl<'database> ProposalConsumptionStore<'database> {
    #[must_use]
    pub const fn new(database: &'database Database) -> Self {
        Self { database }
    }
}
fn status(connection: &Connection, proposal: &ProposalId) -> rusqlite::Result<ConsumptionStatus> {
    let stored: Option<String> = connection
        .query_row(sql::FIND, [proposal.as_client_order_id().as_str()], |row| {
            row.get(0)
        })
        .optional()?;
    let Some(raw) = stored else {
        return Ok(ConsumptionStatus::NotConsumed);
    };
    let id = ProposalId::new(
        ClientOrderId::try_new(&raw).map_err(|_error| rusqlite::Error::InvalidQuery)?,
    );
    if id != *proposal {
        return Err(rusqlite::Error::InvalidQuery);
    }
    Ok(ConsumptionStatus::Consumed)
}
fn consume(
    transaction: &Transaction<'_>,
    proposal: &ProposalId,
) -> rusqlite::Result<ConsumptionOutcome> {
    if status(transaction, proposal)? == ConsumptionStatus::Consumed {
        return Ok(ConsumptionOutcome::AlreadyConsumed);
    }
    match transaction.execute(sql::INSERT, [proposal.as_client_order_id().as_str()]) {
        Ok(1) => Ok(ConsumptionOutcome::Recorded),
        Ok(_) => Err(rusqlite::Error::InvalidQuery),
        Err(error) => Err(error),
    }
}
impl ProposalConsumptionRepository for ProposalConsumptionStore<'_> {
    async fn consume(&self, proposal: &ProposalId) -> Result<ConsumptionOutcome, SystemPortError> {
        let owned_id = proposal.clone();
        self.database
            .write(move |transaction| consume(transaction, &owned_id))
            .await
    }
    async fn status(&self, proposal: &ProposalId) -> Result<ConsumptionStatus, SystemPortError> {
        let owned_id = proposal.clone();
        self.database
            .read(move |connection| status(connection, &owned_id))
            .await
    }
}
