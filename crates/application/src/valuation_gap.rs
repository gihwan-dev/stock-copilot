use crate::{Clock, RecordOutcome, SystemPortError};
use domain::{
    AccountId, CalendarDate, SnapshotError, Timestamp, ValuationCalendarDay, ValuationDayResult,
    ValuationGapPlan,
};
use std::future::Future;

pub trait ValuationCalendarSource: Send + Sync {
    fn days(
        &self,
        last: CalendarDate,
        now: Timestamp,
    ) -> impl Future<Output = Result<Vec<ValuationCalendarDay>, SystemPortError>> + Send;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValuationGapError {
    Calendar(SystemPortError),
    Plan(SnapshotError),
}

#[derive(Debug)]
pub struct PlanValuationGap<'a, Calendars, Time> {
    pub calendars: &'a Calendars,
    pub clock: &'a Time,
}
impl<Calendars: ValuationCalendarSource, Time: Clock> PlanValuationGap<'_, Calendars, Time> {
    pub async fn execute(&self, last: CalendarDate) -> Result<ValuationGapPlan, ValuationGapError> {
        let now = self.clock.now();
        let days = self
            .calendars
            .days(last, now)
            .await
            .map_err(ValuationGapError::Calendar)?;
        ValuationGapPlan::new(last, now, &days).map_err(ValuationGapError::Plan)
    }
}

pub trait ValuationDayRepository: Send + Sync {
    fn record(
        &self,
        account: &AccountId,
        result: &ValuationDayResult,
    ) -> impl Future<Output = Result<RecordOutcome, SystemPortError>> + Send;
}
