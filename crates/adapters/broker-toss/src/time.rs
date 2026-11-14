use domain::{
    CalendarDate, CalendarYear, DayNumber, MonthNumber, Nanosecond, OffsetSeconds, TimeError,
    Timestamp, UnixSeconds,
};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

fn separated(value: &str, length: usize, separators: &[(usize, char)]) -> bool {
    value.chars().count() == length
        && value.chars().enumerate().all(|(position, character)| {
            separators
                .iter()
                .find(|(index, _)| *index == position)
                .map_or_else(
                    || character.is_ascii_digit(),
                    |(_, expected)| character == *expected,
                )
        })
}
fn date_syntax(value: &str) -> bool {
    separated(value, 10, &[(4, '-'), (7, '-')])
}
fn timestamp_syntax(value: &str) -> Result<(), TossTimeError> {
    let (date, tail) = value.split_once('T').ok_or(TossTimeError::InvalidSyntax)?;
    if !date_syntax(date) {
        return Err(TossTimeError::InvalidSyntax);
    }
    let clock = if let Some(clock) = tail.strip_suffix('Z') {
        clock
    } else {
        let position = tail.rfind(['+', '-']).ok_or(TossTimeError::InvalidSyntax)?;
        let offset = tail.get(position..).ok_or(TossTimeError::InvalidSyntax)?;
        if !separated(
            offset.get(1..).ok_or(TossTimeError::InvalidSyntax)?,
            5,
            &[(2, ':')],
        ) {
            return Err(TossTimeError::InvalidSyntax);
        }
        tail.get(..position).ok_or(TossTimeError::InvalidSyntax)?
    };
    let (seconds, fraction) = clock
        .split_once('.')
        .map_or((clock, None), |(whole, fraction)| (whole, Some(fraction)));
    if !separated(seconds, 8, &[(2, ':'), (5, ':')])
        || fraction.is_some_and(|digits| {
            digits.is_empty() || !digits.chars().all(|character| character.is_ascii_digit())
        })
    {
        return Err(TossTimeError::InvalidSyntax);
    }
    if seconds.get(6..8) == Some("60") {
        return Err(TossTimeError::LeapSecond);
    }
    Ok(())
}

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
        timestamp_syntax(value)?;
        let parsed = OffsetDateTime::parse(value, &Rfc3339)
            .map_err(|_error| TossTimeError::InvalidSyntax)?;
        Timestamp::from_parts(
            UnixSeconds::new(parsed.unix_timestamp()),
            Nanosecond::new(parsed.nanosecond()),
            OffsetSeconds::new(parsed.offset().whole_seconds()),
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
        if !date_syntax(value) {
            return Err(TossTimeError::InvalidSyntax);
        }
        let mut parts = value.split('-');
        let year = parts
            .next()
            .ok_or(TossTimeError::InvalidSyntax)?
            .parse::<i32>()
            .map_err(|_error| TossTimeError::InvalidSyntax)?;
        let month = parts
            .next()
            .ok_or(TossTimeError::InvalidSyntax)?
            .parse::<u8>()
            .map_err(|_error| TossTimeError::InvalidSyntax)?;
        let day = parts
            .next()
            .ok_or(TossTimeError::InvalidSyntax)?
            .parse::<u8>()
            .map_err(|_error| TossTimeError::InvalidSyntax)?;
        CalendarDate::from_parts(
            CalendarYear::new(year),
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
        for invalid in ["2026-02-30", "2026-3-5", "+2026-03-05"] {
            let _error = TossDate::try_from(invalid).expect_err("invalid date");
        }
        for invalid in [
            "2026-03-25 13:30:00Z",
            "2026-03-25t13:30:00Z",
            "2026-03-25T13:30:00z",
        ] {
            let _error = TossTimestamp::try_from(invalid).expect_err("strict RFC3339");
        }
    }
}
