use crate::{Clock, PortError, RecordOutcome, SystemPortError};
use domain::{
    AccountId, CalendarDate, SnapshotError, Timestamp, ValuationCalendarDay, ValuationDayInputs,
    ValuationDayResult, ValuationGapPlan,
};
use std::collections::BTreeMap;
use std::future::Future;

pub trait ValuationCalendarSource: Send + Sync {
    fn days(
        &self,
        last: CalendarDate,
        now: Timestamp,
    ) -> impl Future<Output = Result<Vec<ValuationCalendarDay>, PortError>> + Send;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValuationGapError {
    Calendar(PortError),
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValuationInputError {
    Missing(SnapshotError),
    Port(PortError),
}

pub trait ValuationDayInputSource: Send + Sync {
    fn inputs(
        &self,
        account: &AccountId,
        date: CalendarDate,
    ) -> impl Future<Output = Result<ValuationDayInputs, ValuationInputError>> + Send;
}

pub trait ValuationDaySuccessSource: Send + Sync {
    fn succeeded(
        &self,
        account: &AccountId,
        dates: &[CalendarDate],
    ) -> impl Future<Output = Result<Vec<CalendarDate>, SystemPortError>> + Send;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValuationBackfillError {
    Gap(ValuationGapError),
    Inputs(PortError),
    Repository(SystemPortError),
}

#[derive(Debug)]
pub struct BackfillValuationDays<'a, Calendars, Time, Inputs, Successes, Days> {
    pub calendars: &'a Calendars,
    pub clock: &'a Time,
    pub inputs: &'a Inputs,
    pub successes: &'a Successes,
    pub days: &'a Days,
}

impl<Calendars, Time, Inputs, Successes, Days>
    BackfillValuationDays<'_, Calendars, Time, Inputs, Successes, Days>
where
    Calendars: ValuationCalendarSource,
    Time: Clock,
    Inputs: ValuationDayInputSource,
    Successes: ValuationDaySuccessSource,
    Days: ValuationDayRepository,
{
    pub async fn execute(
        &self,
        account: &AccountId,
        last: CalendarDate,
    ) -> Result<Vec<ValuationDayResult>, ValuationBackfillError> {
        let plan = PlanValuationGap {
            calendars: self.calendars,
            clock: self.clock,
        }
        .execute(last)
        .await
        .map_err(ValuationBackfillError::Gap)?;
        let planned: Vec<CalendarDate> = plan.dates().collect();
        if planned.is_empty() {
            return Ok(Vec::new());
        }
        let filled = self
            .successes
            .succeeded(account, &planned)
            .await
            .map_err(ValuationBackfillError::Repository)?;
        let remaining = plan.excluding(&filled);
        let mut collected = BTreeMap::new();
        for date in remaining.dates() {
            let inputs = match self.inputs.inputs(account, date).await {
                Ok(value) => Ok(value),
                Err(ValuationInputError::Missing(reason)) => Err(reason),
                Err(ValuationInputError::Port(error)) => {
                    return Err(ValuationBackfillError::Inputs(error));
                }
            };
            collected.insert(date, inputs);
        }
        let results = remaining.resolve(|date, calendar| match collected.get(&date) {
            Some(Ok(inputs)) => inputs.recompute(date, &calendar),
            Some(Err(reason)) => Err(*reason),
            None => Err(SnapshotError::MissingHistory),
        });
        for result in &results {
            self.days
                .record(account, result)
                .await
                .map_err(ValuationBackfillError::Repository)?;
        }
        Ok(results)
    }
}
