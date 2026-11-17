use crate::{
    BrokerRead, Clock, DeliveryReport, EventBus, FullInspectionRepository, SyncTransactions,
    SystemPortError, TransactionSyncError, TransactionSyncRepository,
};
use domain::{
    AccountId, DailyFullInspection, FullInspectionCompletionError, FullInspectionError,
    FullInspectionPlan, FullInspectionProgress, InspectionRangeDays, InspectionRangeResult,
    InspectionReportError, SyncOutcome, Timestamp, TransactionSyncTimeError, daily_full_inspection,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FullInspectionRunError {
    Repository(SystemPortError),
    Time(TransactionSyncTimeError),
    Plan(FullInspectionError),
    Sync(TransactionSyncError),
    Report(InspectionReportError),
    Completion(FullInspectionCompletionError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FullInspectionRun {
    AlreadyCompletedToday,
    Completed { completed_at: Timestamp },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FullInspectionReport {
    pub outcome: Result<FullInspectionRun, FullInspectionRunError>,
    pub deliveries: Vec<DeliveryReport>,
}

#[derive(Debug)]
pub struct RunFullInspection<'a, Broker, Repository, Inspections, Time> {
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
> RunFullInspection<'_, Broker, Repository, Inspections, Time>
{
    pub async fn run_full_inspection(
        &self,
        account: &AccountId,
        limit: InspectionRangeDays,
    ) -> FullInspectionReport {
        let mut deliveries = Vec::new();
        let outcome = self.execute(account, limit, &mut deliveries).await;
        FullInspectionReport {
            outcome,
            deliveries,
        }
    }

    async fn execute(
        &self,
        account: &AccountId,
        limit: InspectionRangeDays,
        deliveries: &mut Vec<DeliveryReport>,
    ) -> Result<FullInspectionRun, FullInspectionRunError> {
        let previous = self
            .inspections
            .last_completed(account)
            .await
            .map_err(FullInspectionRunError::Repository)?;
        let started_at = self.clock.now();
        match daily_full_inspection(previous, started_at).map_err(FullInspectionRunError::Time)? {
            DailyFullInspection::CompletedToday => {
                return Ok(FullInspectionRun::AlreadyCompletedToday);
            }
            DailyFullInspection::Required => (),
        }
        let plan = FullInspectionPlan::new(None, started_at, limit)
            .map_err(FullInspectionRunError::Plan)?;
        let progress = self.inspect_ranges(account, &plan, deliveries).await?;
        let completed_at = self.clock.now();
        if completed_at < started_at {
            return Err(FullInspectionRunError::Time(
                TransactionSyncTimeError::ClockReversed,
            ));
        }
        let completion = progress
            .completion_at(completed_at)
            .map_err(FullInspectionRunError::Completion)?;
        self.inspections
            .record_completed(account, previous, completion.completed_at())
            .await
            .map_err(FullInspectionRunError::Repository)?;
        Ok(FullInspectionRun::Completed {
            completed_at: completion.completed_at(),
        })
    }
    async fn inspect_ranges(
        &self,
        account: &AccountId,
        plan: &FullInspectionPlan,
        deliveries: &mut Vec<DeliveryReport>,
    ) -> Result<FullInspectionProgress, FullInspectionRunError> {
        let mut progress = plan.begin();
        let sync = SyncTransactions {
            broker: self.broker,
            repository: self.repository,
            clock: self.clock,
            events: self.events,
        };
        for range in plan.ranges() {
            let (delivery, record) = sync
                .sync_window_record(account, *range)
                .await
                .map_err(FullInspectionRunError::Sync)?;
            deliveries.push(delivery);
            let summary = record.summary();
            let result = match summary.outcome() {
                SyncOutcome::Complete(_) => InspectionRangeResult::Succeeded,
                SyncOutcome::AccessBlocked => {
                    progress
                        .report(record.window(), InspectionRangeResult::Failed)
                        .map_err(FullInspectionRunError::Report)?;
                    return Err(FullInspectionRunError::Completion(
                        FullInspectionCompletionError::Incomplete,
                    ));
                }
                SyncOutcome::Partial(_) | SyncOutcome::Failed => InspectionRangeResult::Failed,
            };
            progress
                .report(record.window(), result)
                .map_err(FullInspectionRunError::Report)?;
        }
        Ok(progress)
    }
}
