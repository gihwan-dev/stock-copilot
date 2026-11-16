use crate::{
    BrokerRead, Clock, DeliveryReport, EventBus, PortError, SystemPortError, TransactionRead,
    WriteCompleted,
};
use domain::{
    AccountId, DomainEvent, Reconciliation, ReconciliationError, SettledRowSet, SettlementWindow,
    Timestamp, TransactionFilter,
};
use std::future::Future;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransactionSyncRecord {
    pub recorded_at: Timestamp,
    pub state: SettledRowSet,
    pub observation: Result<TransactionRead, PortError>,
    pub reconciliation: Option<Reconciliation>,
}

pub trait TransactionSyncRepository: Send + Sync {
    fn state(
        &self,
        account: &AccountId,
    ) -> impl Future<Output = Result<SettledRowSet, SystemPortError>> + Send;
    fn record(
        &self,
        account: &AccountId,
        previous: &SettledRowSet,
        record: &TransactionSyncRecord,
    ) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransactionSyncError {
    Repository(SystemPortError),
    Reconciliation(ReconciliationError),
    ResponseScopeMismatch,
}

#[derive(Debug)]
pub struct SyncTransactions<'a, Broker, Repository, Time> {
    pub broker: &'a Broker,
    pub repository: &'a Repository,
    pub clock: &'a Time,
    pub events: &'a EventBus,
}

impl<Broker: BrokerRead, Repository: TransactionSyncRepository, Time: Clock>
    SyncTransactions<'_, Broker, Repository, Time>
{
    pub async fn sync_window(
        &self,
        account: &AccountId,
        window: SettlementWindow,
    ) -> Result<DeliveryReport, TransactionSyncError> {
        let state = self
            .repository
            .state(account)
            .await
            .map_err(TransactionSyncError::Repository)?;
        if state.account_id() != account {
            return Err(TransactionSyncError::ResponseScopeMismatch);
        }
        let at = self.clock.now();
        if state.last_synchronized_at().is_some_and(|last| at < last) {
            return Err(TransactionSyncError::Reconciliation(
                ReconciliationError::InvalidHistory,
            ));
        }
        let filter = TransactionFilter::all();
        let observation = self.broker.transactions(account, window, &filter).await;
        let reconciliation = match &observation {
            Ok(TransactionRead::Complete(response)) => {
                if response.account_id() != account
                    || response.window() != window
                    || response.filter() != &filter
                {
                    return Err(TransactionSyncError::ResponseScopeMismatch);
                }
                Some(
                    state
                        .reconcile(response, at)
                        .map_err(TransactionSyncError::Reconciliation)?,
                )
            }
            Ok(TransactionRead::Incomplete(response)) => {
                if &response.account != account
                    || response.window != window
                    || response.filter != filter
                {
                    return Err(TransactionSyncError::ResponseScopeMismatch);
                }
                None
            }
            Err(_error) => None,
        };
        let next_state = reconciliation
            .as_ref()
            .map_or_else(|| state.clone(), |result| result.state().clone());
        let record = TransactionSyncRecord {
            recorded_at: at,
            state: next_state,
            observation,
            reconciliation,
        };
        self.repository
            .record(account, &state, &record)
            .await
            .map_err(TransactionSyncError::Repository)?;
        Ok(self
            .events
            .publish(DomainEvent::TransactionsSyncRecorded(account.clone())))
    }
}
