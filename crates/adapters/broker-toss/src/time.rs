use chrono::{DateTime, Datelike, NaiveDate, Timelike};
use domain::{
    CalendarDate, CalendarYear, DayNumber, MonthNumber, Nanosecond, OffsetSeconds, TimeError,
    Timestamp, UnixSeconds,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TossTimeError {
    InvalidSyntax,
    LeapSecond,
    UnknownOffset,
    PrecisionLoss,
    Domain(TimeError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TossTimestamp(Timestamp);

impl TryFrom<&str> for TossTimestamp {
    type Error = TossTimeError;
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        if value.ends_with("-00:00") {
            return Err(TossTimeError::UnknownOffset);
        }
        if value.split_once('.').is_some_and(|(_, fraction)| {
            fraction.chars().take_while(char::is_ascii_digit).count() > 9
        }) {
            return Err(TossTimeError::PrecisionLoss);
        }
        let parsed =
            DateTime::parse_from_rfc3339(value).map_err(|_error| TossTimeError::InvalidSyntax)?;
        if parsed.nanosecond() >= 1_000_000_000_u32 {
            return Err(TossTimeError::LeapSecond);
        }
        Timestamp::from_parts(
            UnixSeconds::new(parsed.timestamp()),
            Nanosecond::new(parsed.nanosecond()),
            OffsetSeconds::new(parsed.offset().local_minus_utc()),
        )
        .map(Self)
        .map_err(TossTimeError::Domain)
    }
}

impl TossTimestamp {
    #[must_use]
    pub fn into_domain(self) -> Timestamp {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TossDate(CalendarDate);

impl TryFrom<&str> for TossDate {
    type Error = TossTimeError;
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let parsed = NaiveDate::parse_from_str(value, "%Y-%m-%d")
            .map_err(|_error| TossTimeError::InvalidSyntax)?;
        let month = u8::try_from(parsed.month()).map_err(|_error| TossTimeError::InvalidSyntax)?;
        let day = u8::try_from(parsed.day()).map_err(|_error| TossTimeError::InvalidSyntax)?;
        CalendarDate::from_parts(
            CalendarYear::new(parsed.year()),
            MonthNumber::new(month),
            DayNumber::new(day),
        )
        .map(Self)
        .map_err(TossTimeError::Domain)
    }
}

impl TossDate {
    #[must_use]
    pub fn into_domain(self) -> CalendarDate {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::{TossDate, TossTimeError, TossTimestamp};

    #[test]
    fn market_time_preserves_offset_and_absolute_time() {
        let parsed = TossTimestamp::try_from("2026-03-25T22:30:00.123456789+09:00")
            .expect("market time")
            .into_domain();
        assert_eq!(*parsed.nanosecond().as_u32(), 123_456_789_u32);
        assert_eq!(*parsed.offset_seconds().as_i32(), 32_400_i32);
        assert_eq!(
            parsed,
            TossTimestamp::try_from("2026-03-25T13:30:00.123456789Z")
                .expect("UTC")
                .into_domain()
        );
        assert_eq!(
            *TossDate::try_from("2026-03-25")
                .expect("date")
                .into_domain()
                .day()
                .as_u8(),
            25_u8
        );
    }

    #[test]
    fn leap_unknown_offset_and_precision_loss_are_rejected_without_normalization() {
        assert_eq!(
            TossTimestamp::try_from("2016-12-31T23:59:60Z"),
            Err(TossTimeError::LeapSecond)
        );
        assert_eq!(
            TossTimestamp::try_from("2026-03-25T13:30:00-00:00"),
            Err(TossTimeError::UnknownOffset)
        );
        assert_eq!(
            TossTimestamp::try_from("2026-03-25T13:30:00.1234567891Z"),
            Err(TossTimeError::PrecisionLoss)
        );
        for invalid in [
            "",
            "2026-03-25T13:30:00",
            "2026-03-25T13:30:00+25:00",
            "0000-01-01T00:00:00Z",
        ] {
            let _error = TossTimestamp::try_from(invalid).expect_err("invalid timestamp");
        }
        let _error = TossDate::try_from("2026-02-30").expect_err("invalid date");
    }
}
