use crate::{EstimateTask, MarketCalendar, NoSignalReason, SignalUnavailable, Timestamp};

#[derive(Clone, Copy)]
pub struct SignalTiming<'facts> {
    now: Timestamp,
    calendars: &'facts [MarketCalendar],
}
impl<'facts> SignalTiming<'facts> {
    #[must_use]
    pub const fn new(now: Timestamp, calendars: &'facts [MarketCalendar]) -> Self {
        Self { now, calendars }
    }
    #[must_use]
    pub const fn now(self) -> Timestamp {
        self.now
    }
    pub(crate) fn check(
        self,
        task: &EstimateTask,
        submitted_at: Timestamp,
    ) -> Result<(), NoSignalReason> {
        if self.now < submitted_at {
            return Err(NoSignalReason::Unavailable(
                SignalUnavailable::SubmissionFromFuture,
            ));
        }
        let interval = crate::estimate_outcome::signal_task_interval(task, self.calendars).or(
            Err(NoSignalReason::Unavailable(SignalUnavailable::Calendar)),
        )?;
        if self.now >= interval.end() {
            return Err(NoSignalReason::Expired);
        }
        Ok(())
    }
    pub(crate) fn check_holding(
        self,
        task: &EstimateTask,
        observed_at: Timestamp,
    ) -> Result<(), NoSignalReason> {
        let elapsed = observed_at
            .elapsed_until(self.now)
            .or(Err(NoSignalReason::Unavailable(
                SignalUnavailable::StaleHolding,
            )))?;
        if observed_at < task.created_at() || elapsed > crate::domain_time::SIGNAL_HOLDING_MAX_AGE {
            return Err(NoSignalReason::Unavailable(SignalUnavailable::StaleHolding));
        }
        Ok(())
    }
}
impl std::fmt::Debug for SignalTiming<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SignalTiming(..)")
    }
}
