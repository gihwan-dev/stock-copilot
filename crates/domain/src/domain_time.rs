use time::{Date, OffsetDateTime};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Timestamp(OffsetDateTime);

impl Timestamp {
    #[must_use]
    pub const fn new(value: OffsetDateTime) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn as_offset_date_time(&self) -> OffsetDateTime {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CalendarDate(Date);

impl CalendarDate {
    #[must_use]
    pub const fn new(value: Date) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn as_date(&self) -> &Date {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct IntervalBounds {
    start: Timestamp,
    end: Timestamp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimeInterval(IntervalBounds);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeIntervalError {
    NonIncreasing,
}

impl TimeInterval {
    pub fn try_new(start: Timestamp, end: Timestamp) -> Result<Self, TimeIntervalError> {
        if end <= start {
            return Err(TimeIntervalError::NonIncreasing);
        }
        Ok(Self(IntervalBounds { start, end }))
    }

    #[must_use]
    pub const fn start(&self) -> Timestamp {
        self.0.start
    }

    #[must_use]
    pub const fn end(&self) -> Timestamp {
        self.0.end
    }
}

#[cfg(test)]
mod tests {
    use super::{CalendarDate, TimeInterval, TimeIntervalError, Timestamp};
    use time::{Date, Month, OffsetDateTime, UtcOffset};

    #[test]
    fn wrappers_preserve_dates_offsets_and_nanosecond_interval_boundaries() {
        let raw_date = Date::from_calendar_date(2026, Month::March, 9).expect("달력 날짜");
        assert_eq!(CalendarDate::new(raw_date).as_date(), &raw_date);
        let raw_start = raw_date
            .with_hms_nano(22, 30, 0, 1)
            .expect("나노초")
            .assume_offset(UtcOffset::from_hms(9, 0, 0).expect("KST"));
        let start = Timestamp::new(raw_start);
        assert_eq!(start.as_offset_date_time().offset(), raw_start.offset());
        assert_eq!(start, Timestamp::new(raw_start.to_offset(UtcOffset::UTC)));
        let end = Timestamp::new(raw_start.replace_nanosecond(2).expect("다음 나노초"));
        let interval = TimeInterval::try_new(start, end).expect("한 나노초");
        assert_eq!(interval.start(), start);
        assert_eq!(interval.end(), end);
        assert_eq!(
            TimeInterval::try_new(end, start),
            Err(TimeIntervalError::NonIncreasing)
        );
        assert_eq!(
            TimeInterval::try_new(start, Timestamp::new(raw_start.to_offset(UtcOffset::UTC))),
            Err(TimeIntervalError::NonIncreasing)
        );
        assert_ne!(Timestamp::new(OffsetDateTime::UNIX_EPOCH), start);
    }
}
