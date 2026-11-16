use application::Clock;
use domain::{Nanosecond, OffsetSeconds, TimeError, Timestamp, UnixSeconds};

#[derive(Debug)]
pub struct FixedClock(Timestamp);
impl Clock for FixedClock {
    fn now(&self) -> Timestamp {
        self.0
    }
}
pub fn fixed_clock() -> Result<FixedClock, TimeError> {
    Timestamp::from_parts(
        UnixSeconds::new(1_800_000_000),
        Nanosecond::new(0),
        OffsetSeconds::new(0),
    )
    .map(FixedClock)
}
