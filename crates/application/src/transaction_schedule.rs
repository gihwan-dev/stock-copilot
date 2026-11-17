use crate::{
    BrokerRead, Clock, DeliveryReport, EventBus, SyncTransactions, SystemPortError,
    TransactionSyncError, TransactionSyncRepository, WriteCompleted,
};
use domain::{
    AccountId, DailyFullInspection, Timestamp, TransactionSyncTimeError, daily_full_inspection,
    recent_transaction_window,
};
use std::future::Future;

pub trait FullInspectionRepository: Send + Sync {
    fn last_completed(
        &self,
        account: &AccountId,
    ) -> impl Future<Output = Result<Option<Timestamp>, SystemPortError>> + Send;
    fn record_completed(
        &self,
        account: &AccountId,
        previous: Option<Timestamp>,
        completed_at: Timestamp,
    ) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransactionScheduleError {
    Repository(SystemPortError),
    Time(TransactionSyncTimeError),
    Sync(TransactionSyncError),
}

#[derive(Debug)]
pub struct TransactionSchedule<'a, Broker, Repository, Inspections, Time> {
    pub broker: &'a Broker,
    pub repository: &'a Repository,
    pub inspections: &'a Inspections,
    pub clock: &'a Time,
    pub events: &'a EventBus,
}

impl<
    Broker: BrokerRead,
    Repository: TransactionSyncRepository,
    Inspections: FullInspectionRepository,
    Time: Clock,
> TransactionSchedule<'_, Broker, Repository, Inspections, Time>
{
    pub async fn sync_recent(
        &self,
        account: &AccountId,
    ) -> Result<DeliveryReport, TransactionScheduleError> {
        let at = self.clock.now();
        let window = recent_transaction_window(at).map_err(TransactionScheduleError::Time)?;
        SyncTransactions {
            broker: self.broker,
            repository: self.repository,
            clock: &SampledClock(at),
            events: self.events,
        }
        .sync_window(account, window)
        .await
        .map_err(TransactionScheduleError::Sync)
    }

    pub async fn full_inspection_status(
        &self,
        account: &AccountId,
    ) -> Result<DailyFullInspection, TransactionScheduleError> {
        let last = self
            .inspections
            .last_completed(account)
            .await
            .map_err(TransactionScheduleError::Repository)?;
        daily_full_inspection(last, self.clock.now()).map_err(TransactionScheduleError::Time)
    }

    pub async fn record_full_inspection_completed(
        &self,
        account: &AccountId,
    ) -> Result<WriteCompleted, TransactionScheduleError> {
        let previous = self
            .inspections
            .last_completed(account)
            .await
            .map_err(TransactionScheduleError::Repository)?;
        let at = self.clock.now();
        daily_full_inspection(previous, at).map_err(TransactionScheduleError::Time)?;
        self.inspections
            .record_completed(account, previous, at)
            .await
            .map_err(TransactionScheduleError::Repository)
    }
}

struct SampledClock(Timestamp);
impl Clock for SampledClock {
    fn now(&self) -> Timestamp {
        self.0
    }
}
