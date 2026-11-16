use crate::{
    BrokerRead, Clock, DeliveryReport, EventBus, PortError, PortErrorKind, SystemPortError,
    TransactionRead, TransactionRepository, WriteCompleted,
};
use domain::{
    AccountId, DomainEvent, Reconciliation, ReconciliationError, SettledRowSet, SettlementWindow,
    SyncOutcome, SyncRecord, Timestamp, TransactionFilter,
};
use std::future::Future;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransactionSyncRejection {
    ResponseScopeMismatch,
    Reconciliation(ReconciliationError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransactionSyncRecord {
    pub recorded_at: Timestamp,
    pub window: SettlementWindow,
    pub state: SettledRowSet,
    pub observation: Result<TransactionRead, PortError>,
    pub reconciliation: Result<Option<Reconciliation>, TransactionSyncRejection>,
}

impl TransactionSyncRecord {
    #[must_use]
    pub fn summary(&self) -> SyncRecord<SettledRowSet> {
        if self.reconciliation.is_err() {
            return SyncRecord::new(self.recorded_at, SyncOutcome::Failed);
        }
        let outcome = match &self.observation {
            Ok(TransactionRead::Complete(_response)) => SyncOutcome::Complete(self.state.clone()),
            Ok(TransactionRead::Incomplete(_response)) => SyncOutcome::Partial(self.state.clone()),
            Err(error) => {
                if let PortErrorKind::AccessDenied(_denial) = error.kind() {
                    SyncOutcome::AccessBlocked
                } else {
                    SyncOutcome::Failed
                }
            }
        };
        SyncRecord::new(self.recorded_at, outcome)
    }
}

pub trait TransactionSyncRepository: TransactionRepository {
    fn state(
        &self,
        account: &AccountId,
    ) -> impl Future<Output = Result<SettledRowSet, SystemPortError>> + Send;
    fn latest_sync(
        &self,
        account: &AccountId,
    ) -> impl Future<Output = Result<Option<TransactionSyncRecord>, SystemPortError>> + Send;
    fn record_sync(
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
        let observation = self
            .broker
            .transactions(account, window, &TransactionFilter::all())
            .await;
        let reconciliation = reconcile_observation(&state, window, at, &observation);
        let next_state = match &reconciliation {
            Ok(Some(result)) => result.state().clone(),
            Ok(None) | Err(_) => state.clone(),
        };
        let record = TransactionSyncRecord {
            recorded_at: at,
            window,
            state: next_state,
            observation,
            reconciliation,
        };
        self.repository
            .record_sync(account, &state, &record)
            .await
            .map_err(TransactionSyncError::Repository)?;
        Ok(self
            .events
            .publish(DomainEvent::TransactionsSyncRecorded(account.clone())))
    }
}

fn reconcile_observation(
    state: &SettledRowSet,
    window: SettlementWindow,
    at: Timestamp,
    observation: &Result<TransactionRead, PortError>,
) -> Result<Option<Reconciliation>, TransactionSyncRejection> {
    let (account, received_window, filter) = match observation {
        Ok(TransactionRead::Complete(response)) => {
            (response.account_id(), response.window(), response.filter())
        }
        Ok(TransactionRead::Incomplete(response)) => {
            (&response.account, response.window, &response.filter)
        }
        Err(_error) => return Ok(None),
    };
    if account != state.account_id()
        || received_window != window
        || filter != &TransactionFilter::all()
    {
        return Err(TransactionSyncRejection::ResponseScopeMismatch);
    }
    if let Ok(TransactionRead::Complete(response)) = observation {
        state
            .reconcile(response, at)
            .map(Some)
            .map_err(TransactionSyncRejection::Reconciliation)
    } else {
        Ok(None)
    }
}
