use application::Clock;
use domain::{Nanosecond, OffsetSeconds, TimeError, Timestamp, UnixSeconds};
use std::time::{SystemTime, UNIX_EPOCH};

const MIN_SECONDS: i64 = -62_135_596_800;
const MAX_SECONDS: i64 = 253_402_300_799;

#[derive(Debug)]
pub struct SystemClock {
    minimum: Timestamp,
    maximum: Timestamp,
}

impl SystemClock {
    pub fn new() -> Result<Self, TimeError> {
        Ok(Self {
            minimum: Timestamp::from_parts(
                UnixSeconds::new(MIN_SECONDS),
                Nanosecond::new(0),
                OffsetSeconds::new(0),
            )?,
            maximum: Timestamp::from_parts(
                UnixSeconds::new(MAX_SECONDS),
                Nanosecond::new(999_999_999),
                OffsetSeconds::new(0),
            )?,
        })
    }

    #[must_use]
    pub fn convert(&self, value: SystemTime) -> Timestamp {
        let (seconds, nanos) = match value.duration_since(UNIX_EPOCH) {
            Ok(duration) => (i128::from(duration.as_secs()), duration.subsec_nanos()),
            Err(error) => {
                let duration = error.duration();
                let seconds = i128::from(duration.as_secs()).saturating_neg();
                match duration.subsec_nanos() {
                    0 => (seconds, 0),
                    nanos => (
                        seconds.saturating_sub(1),
                        1_000_000_000_u32.saturating_sub(nanos),
                    ),
                }
            }
        };
        self.parts(seconds, nanos)
    }

    fn parts(&self, seconds: i128, nanos: u32) -> Timestamp {
        if seconds < i128::from(MIN_SECONDS) {
            return self.minimum;
        }
        if seconds > i128::from(MAX_SECONDS) {
            return self.maximum;
        }
        match i64::try_from(seconds).ok().and_then(|whole_seconds| {
            Timestamp::from_parts(
                UnixSeconds::new(whole_seconds),
                Nanosecond::new(nanos),
                OffsetSeconds::new(0),
            )
            .ok()
        }) {
            Some(value) => value,
            None => self.maximum,
        }
    }
}

impl Clock for SystemClock {
    fn now(&self) -> Timestamp {
        self.convert(SystemTime::now())
    }
}

#[cfg(test)]
mod tests;
