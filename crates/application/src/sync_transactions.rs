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
    recorded_at: Timestamp,
    window: SettlementWindow,
    state: SettledRowSet,
    observation: Result<TransactionRead, PortError>,
    reconciliation: Result<Option<Reconciliation>, TransactionSyncRejection>,
}

impl TransactionSyncRecord {
    #[must_use]
    pub fn from_observation(
        previous: &SettledRowSet,
        window: SettlementWindow,
        recorded_at: Timestamp,
        observation: Result<TransactionRead, PortError>,
    ) -> Self {
        let reconciliation = reconcile_observation(previous, window, recorded_at, &observation);
        let state = match &reconciliation {
            Ok(Some(result)) => result.state().clone(),
            Ok(None) | Err(_) => previous.clone(),
        };
        Self {
            recorded_at,
            window,
            state,
            observation,
            reconciliation,
        }
    }
    #[must_use]
    pub const fn recorded_at(&self) -> Timestamp {
        self.recorded_at
    }
    #[must_use]
    pub const fn window(&self) -> SettlementWindow {
        self.window
    }
    #[must_use]
    pub const fn state(&self) -> &SettledRowSet {
        &self.state
    }
    #[must_use]
    pub const fn observation(&self) -> &Result<TransactionRead, PortError> {
        &self.observation
    }
    #[must_use]
    pub const fn reconciliation(
        &self,
    ) -> &Result<Option<Reconciliation>, TransactionSyncRejection> {
        &self.reconciliation
    }
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
        previous_recorded_at: Option<Timestamp>,
        record: &TransactionSyncRecord,
    ) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransactionSyncError {
    Repository(SystemPortError),
    Reconciliation(ReconciliationError),
    StoredAccountMismatch,
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
        self.sync_window_record(account, window)
            .await
            .map(|(delivery, _record)| delivery)
    }

    pub(crate) async fn sync_window_record(
        &self,
        account: &AccountId,
        window: SettlementWindow,
    ) -> Result<(DeliveryReport, TransactionSyncRecord), TransactionSyncError> {
        let state = self
            .repository
            .state(account)
            .await
            .map_err(TransactionSyncError::Repository)?;
        if state.account_id() != account {
            return Err(TransactionSyncError::StoredAccountMismatch);
        }
        let latest = self
            .repository
            .latest_sync(account)
            .await
            .map_err(TransactionSyncError::Repository)?;
        if latest
            .as_ref()
            .is_some_and(|record| record.state.account_id() != account)
        {
            return Err(TransactionSyncError::StoredAccountMismatch);
        }
        let previous_recorded_at = latest.map(|record| record.recorded_at);
        let at = self.clock.now();
        if state.last_synchronized_at().is_some_and(|last| at < last)
            || previous_recorded_at.is_some_and(|last| at < last)
        {
            return Err(TransactionSyncError::Reconciliation(
                ReconciliationError::InvalidHistory,
            ));
        }
        let observation = self
            .broker
            .transactions(account, window, &TransactionFilter::all())
            .await;
        let record = TransactionSyncRecord::from_observation(&state, window, at, observation);
        self.repository
            .record_sync(account, &state, previous_recorded_at, &record)
            .await
            .map_err(TransactionSyncError::Repository)?;
        let delivery = self
            .events
            .publish(DomainEvent::TransactionsSyncRecorded(account.clone()));
        Ok((delivery, record))
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
            .reconcile(response.normalized(), at)
            .map(Some)
            .map_err(TransactionSyncRejection::Reconciliation)
    } else {
        Ok(None)
    }
}
