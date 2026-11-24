use crate::script::State;
use application::Clock;
use domain::{TimeError, TimeSpan, Timestamp};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClockStepError {
    Backwards,
}
#[derive(Debug)]
pub struct SteppedClock {
    at: State<Timestamp>,
    calls: State<Vec<Timestamp>>,
}
impl SteppedClock {
    #[must_use]
    pub const fn new(at: Timestamp) -> Self {
        Self {
            at: State::new(at),
            calls: State::new(Vec::new()),
        }
    }
    #[must_use]
    pub fn calls(&self) -> Vec<Timestamp> {
        self.calls.write().clone()
    }
    pub fn advance_to(&self, at: Timestamp) -> Result<(), ClockStepError> {
        let mut current = self.at.write();
        if at < *current {
            return Err(ClockStepError::Backwards);
        }
        *current = at;
        Ok(())
    }
    pub fn advance_by(&self, span: TimeSpan) -> Result<Timestamp, TimeError> {
        let mut current = self.at.write();
        let next = current.checked_add(span)?;
        *current = next;
        Ok(next)
    }
}
impl Clock for SteppedClock {
    fn now(&self) -> Timestamp {
        let at = *self.at.write();
        self.calls.write().push(at);
        at
    }
}
