use time::{Date, Month, OffsetDateTime, SignedDuration, UtcOffset};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnixSeconds(i64);
impl UnixSeconds {
    #[must_use]
    pub const fn new(value: i64) -> Self {
        Self(value)
    }
    #[must_use]
    pub const fn as_i64(&self) -> &i64 {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Nanosecond(u32);
impl Nanosecond {
    #[must_use]
    pub const fn new(value: u32) -> Self {
        Self(value)
    }
    #[must_use]
    pub const fn as_u32(&self) -> &u32 {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OffsetSeconds(i32);
impl OffsetSeconds {
    #[must_use]
    pub const fn new(value: i32) -> Self {
        Self(value)
    }
    #[must_use]
    pub const fn as_i32(&self) -> &i32 {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpanSeconds(u64);
impl SpanSeconds {
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }
    #[must_use]
    pub const fn as_u64(&self) -> &u64 {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CalendarYear(i32);
impl CalendarYear {
    #[must_use]
    pub const fn new(value: i32) -> Self {
        Self(value)
    }
    #[must_use]
    pub const fn as_i32(&self) -> &i32 {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MonthNumber(u8);
impl MonthNumber {
    #[must_use]
    pub const fn new(value: u8) -> Self {
        Self(value)
    }
    #[must_use]
    pub const fn as_u8(&self) -> &u8 {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DayNumber(u8);
impl DayNumber {
    #[must_use]
    pub const fn new(value: u8) -> Self {
        Self(value)
    }
    #[must_use]
    pub const fn as_u8(&self) -> &u8 {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeError {
    InvalidComponent,
    OutOfRange,
    Overflow,
    NegativeSpan,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CalendarDate(Date);
impl CalendarDate {
    pub fn from_parts(
        year: CalendarYear,
        month: MonthNumber,
        day: DayNumber,
    ) -> Result<Self, TimeError> {
        if !(1_i32..=9999_i32).contains(&year.0) {
            return Err(TimeError::OutOfRange);
        }
        let calendar_month = Month::try_from(month.0).or(Err(TimeError::InvalidComponent))?;
        Date::from_calendar_date(year.0, calendar_month, day.0)
            .map(Self)
            .map_err(|_error| TimeError::InvalidComponent)
    }
    #[must_use]
    pub const fn year(self) -> CalendarYear {
        CalendarYear(self.0.year())
    }
    #[must_use]
    pub fn month(self) -> MonthNumber {
        MonthNumber(u8::from(self.0.month()))
    }
    #[must_use]
    pub const fn day(self) -> DayNumber {
        DayNumber(self.0.day())
    }
    pub fn days_until(self, other: Self) -> Result<CalendarDays, TimeError> {
        i64::from(other.0.to_julian_day())
            .checked_sub(i64::from(self.0.to_julian_day()))
            .map(CalendarDays)
            .ok_or(TimeError::Overflow)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CalendarDays(i64);
impl CalendarDays {
    #[must_use]
    pub const fn as_i64(&self) -> &i64 {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Timestamp(OffsetDateTime);
impl Timestamp {
    pub fn from_parts(
        seconds: UnixSeconds,
        nanosecond: Nanosecond,
        offset: OffsetSeconds,
    ) -> Result<Self, TimeError> {
        let validated_offset = UtcOffset::from_whole_seconds(offset.0)
            .map_err(|_error| TimeError::InvalidComponent)?;
        if !(-86_340_i32..=86_340_i32).contains(&offset.0)
            || offset.0.checked_rem(60) != Some(0_i32)
        {
            return Err(TimeError::InvalidComponent);
        }
        let utc = OffsetDateTime::from_unix_timestamp(seconds.0)
            .map_err(|_error| TimeError::OutOfRange)?
            .replace_nanosecond(nanosecond.0)
            .or(Err(TimeError::InvalidComponent))?;
        utc.checked_to_offset(validated_offset)
            .ok_or(TimeError::OutOfRange)
            .and_then(Self::try_new)
    }

    fn try_new(value: OffsetDateTime) -> Result<Self, TimeError> {
        let utc = value
            .checked_to_offset(UtcOffset::UTC)
            .ok_or(TimeError::OutOfRange)?;
        if !(1_i32..=9999_i32).contains(&utc.year()) || !(1_i32..=9999_i32).contains(&value.year())
        {
            return Err(TimeError::OutOfRange);
        }
        Ok(Self(value))
    }
    #[must_use]
    pub const fn utc_seconds(self) -> UnixSeconds {
        UnixSeconds(self.0.unix_timestamp())
    }
    #[must_use]
    pub const fn nanosecond(self) -> Nanosecond {
        Nanosecond(self.0.nanosecond())
    }
    #[must_use]
    pub const fn offset_seconds(self) -> OffsetSeconds {
        OffsetSeconds(self.0.offset().whole_seconds())
    }
    #[must_use]
    pub const fn offset_calendar_date(self) -> CalendarDate {
        CalendarDate(self.0.date())
    }

    pub fn checked_add(self, span: TimeSpan) -> Result<Self, TimeError> {
        Self::try_new(self.0.checked_add(span.0).ok_or(TimeError::Overflow)?)
    }
    pub fn checked_sub(self, span: TimeSpan) -> Result<Self, TimeError> {
        Self::try_new(self.0.checked_sub(span.0).ok_or(TimeError::Overflow)?)
    }
    pub fn date_at_offset(self, offset: OffsetSeconds) -> Result<CalendarDate, TimeError> {
        Self::from_parts(self.utc_seconds(), self.nanosecond(), offset)
            .map(Self::offset_calendar_date)
    }
    pub fn elapsed_until(self, end: Self) -> Result<TimeSpan, TimeError> {
        if end < self {
            return Err(TimeError::NegativeSpan);
        }
        let seconds = end.0.unix_timestamp().checked_sub(self.0.unix_timestamp());
        let nanos = i64::from(end.0.nanosecond()).checked_sub(i64::from(self.0.nanosecond()));
        seconds
            .zip(nanos)
            .and_then(|(whole_seconds, fraction)| {
                SignedDuration::seconds(whole_seconds)
                    .checked_add(SignedDuration::nanoseconds(fraction))
            })
            .map(TimeSpan)
            .ok_or(TimeError::Overflow)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct TimeSpan(SignedDuration);
pub(crate) const SIGNAL_HOLDING_MAX_AGE: TimeSpan = TimeSpan(SignedDuration::seconds(60));
impl TimeSpan {
    pub const DAY: Self = Self(SignedDuration::seconds(86400));
    pub const NANOSECOND: Self = Self(SignedDuration::nanoseconds(1));
    pub fn try_new(seconds: SpanSeconds, nanosecond: Nanosecond) -> Result<Self, TimeError> {
        if nanosecond.0 >= 1_000_000_000 {
            return Err(TimeError::InvalidComponent);
        }
        let whole_seconds = i64::try_from(seconds.0).map_err(|_error| TimeError::Overflow)?;
        SignedDuration::seconds(whole_seconds)
            .checked_add(SignedDuration::nanoseconds(i64::from(nanosecond.0)))
            .map(Self)
            .ok_or(TimeError::Overflow)
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
    pub const fn start(self) -> Timestamp {
        self.0.start
    }
    #[must_use]
    pub const fn end(self) -> Timestamp {
        self.0.end
    }
}

pub(crate) fn performance_day_difference(start: CalendarDate, end: CalendarDate) -> CalendarDays {
    CalendarDays(i64::from(
        end.0.to_julian_day().wrapping_sub(start.0.to_julian_day()),
    ))
}
pub(crate) fn performance_weekday(date: CalendarDate) -> DayNumber {
    DayNumber(date.0.weekday().number_from_monday())
}

pub(crate) fn performance_flow_days(
    start: Timestamp,
    end: Timestamp,
) -> crate::performance_arithmetic::Days {
    let first = start
        .utc_seconds()
        .as_i64()
        .wrapping_add(32400)
        .wrapping_div_euclid(86400);
    let last = end
        .utc_seconds()
        .as_i64()
        .wrapping_add(32400)
        .wrapping_div_euclid(86400);
    let [low0, low1, low2, low3, _, _, _, _] =
        last.wrapping_sub(first).unsigned_abs().to_le_bytes();
    crate::performance_arithmetic::Days::new(u32::from_le_bytes([low0, low1, low2, low3]))
}
#[cfg(test)]
mod tests {
    use super::{
        CalendarDate, CalendarYear, DayNumber, MonthNumber, Nanosecond, OffsetSeconds, SpanSeconds,
        TimeError, TimeInterval, TimeIntervalError, TimeSpan, Timestamp, UnixSeconds,
    };
    use time::{Date, Month, UtcOffset};

    fn date(year: i32, month: u8, day: u8) -> CalendarDate {
        CalendarDate::from_parts(
            CalendarYear::new(year),
            MonthNumber::new(month),
            DayNumber::new(day),
        )
        .expect("날짜")
    }

    fn at(year: i32, month: Month, day: u8, clock: (u8, u8, u8, u32)) -> Timestamp {
        let raw = Date::from_calendar_date(year, month, day)
            .expect("날짜")
            .with_hms_nano(clock.0, clock.1, clock.2, clock.3)
            .expect("시각")
            .assume_offset(UtcOffset::UTC);
        Timestamp::from_parts(
            UnixSeconds::new(raw.unix_timestamp()),
            Nanosecond::new(clock.3),
            OffsetSeconds::new(0),
        )
        .expect("시점")
    }

    #[test]
    fn equal_instants_have_zero_elapsed_time_and_zero_span_preserves_the_instant() {
        let instant = at(2026, Month::March, 25, (22, 30, 0, 42));
        let zero = TimeSpan::try_new(SpanSeconds::new(0), Nanosecond::new(0)).expect("0 기간");
        assert_eq!(instant.elapsed_until(instant), Ok(zero));
        assert_eq!(instant.checked_add(zero), Ok(instant));
        assert_eq!(instant.checked_sub(zero), Ok(instant));
        assert_eq!(date(1, 1, 1).year(), CalendarYear::new(1));
    }

    #[test]
    fn scalar_components_and_date_distances_round_trip() {
        assert_eq!(*UnixSeconds::new(-1).as_i64(), -1);
        assert_eq!(*Nanosecond::new(42).as_u32(), 42);
        assert_eq!(*OffsetSeconds::new(-3600).as_i32(), -3600_i32);
        assert_eq!(*SpanSeconds::new(2400).as_u64(), 2400);
        assert_eq!(*CalendarYear::new(2026).as_i32(), 2026_i32);
        assert_eq!(*MonthNumber::new(3).as_u8(), 3);
        assert_eq!(*DayNumber::new(25).as_u8(), 25);
        assert_eq!(
            *date(2026, 3, 25)
                .days_until(date(2026, 3, 26))
                .expect("날짜 차")
                .as_i64(),
            1
        );
    }

    #[test]
    fn validated_components_reject_invalid_dates_offsets_nanoseconds_and_years() {
        assert_eq!(date(2026_i32, 3, 25).year(), CalendarYear::new(2026));
        assert_eq!(date(2026_i32, 3, 25).month(), MonthNumber::new(3));
        assert_eq!(date(2026_i32, 3, 25).day(), DayNumber::new(25));
        for (year, month, day) in [
            (-9999_i32, 1, 1),
            (0_i32, 1, 1),
            (10000_i32, 1, 1),
            (2026_i32, 0, 1),
            (2026_i32, 13, 1),
            (2026_i32, 2, 29),
            (2026_i32, 1, 0),
        ] {
            CalendarDate::from_parts(
                CalendarYear::new(year),
                MonthNumber::new(month),
                DayNumber::new(day),
            )
            .expect_err("거부");
        }
        for offset in [-86_400_i32, 86_400_i32, 93_599_i32, 1_i32, i32::MAX] {
            Timestamp::from_parts(
                UnixSeconds::new(0),
                Nanosecond::new(0),
                OffsetSeconds::new(offset),
            )
            .expect_err("거부");
        }
        Timestamp::from_parts(
            UnixSeconds::new(i64::MAX),
            Nanosecond::new(0),
            OffsetSeconds::new(0),
        )
        .expect_err("거부");
        Timestamp::from_parts(
            UnixSeconds::new(0),
            Nanosecond::new(1_000_000_000),
            OffsetSeconds::new(0),
        )
        .expect_err("거부");
        Timestamp::from_parts(
            UnixSeconds::new(-62_167_219_200),
            Nanosecond::new(0),
            OffsetSeconds::new(0),
        )
        .expect_err("거부");
    }

    #[test]
    fn offset_date_is_explicit_while_equality_and_utc_components_are_instant_based() {
        let utc = at(2026, Month::March, 25, (23, 30, 0, 42));
        let kst = Timestamp::from_parts(
            utc.utc_seconds(),
            utc.nanosecond(),
            OffsetSeconds::new(32400),
        )
        .expect("KST");
        assert_eq!(utc, kst);
        assert_eq!(utc.offset_calendar_date(), date(2026, 3, 25));
        assert_eq!(kst.offset_calendar_date(), date(2026, 3, 26));
        assert_eq!(kst.offset_seconds(), OffsetSeconds::new(32400));
        assert_eq!(kst.nanosecond(), Nanosecond::new(42));
        for offset in [-86_340_i32, 86_340_i32] {
            let _accepted = Timestamp::from_parts(
                utc.utc_seconds(),
                utc.nanosecond(),
                OffsetSeconds::new(offset),
            )
            .expect("허용");
        }
        let last = at(9999, Month::December, 31, (23, 59, 59, 0));
        Timestamp::from_parts(
            last.utc_seconds(),
            last.nanosecond(),
            OffsetSeconds::new(60),
        )
        .expect_err("오프셋 날짜 범위");
        let first = at(1, Month::January, 1, (0, 0, 0, 0));
        Timestamp::from_parts(
            first.utc_seconds(),
            first.nanosecond(),
            OffsetSeconds::new(-60),
        )
        .expect_err("오프셋 날짜 범위");
    }

    #[test]
    fn checked_arithmetic_handles_schedule_offsets_fractional_borrow_and_overflow() {
        let start = at(2026_i32, Month::March, 25, (22, 30, 0, 0));
        let span = TimeSpan::try_new(SpanSeconds::new(2400), Nanosecond::new(0)).expect("40분");
        let before = start.checked_sub(span).expect("개장 전");
        assert_eq!(before, at(2026_i32, Month::March, 25, (21, 50, 0, 0)));
        assert_eq!(before.checked_add(span), Ok(start));
        assert_eq!(before.elapsed_until(start), Ok(span));
        assert_eq!(start.elapsed_until(before), Err(TimeError::NegativeSpan));
        let nano = TimeSpan::try_new(SpanSeconds::new(0), Nanosecond::new(1)).expect("나노초");
        let prior = start.checked_sub(nano).expect("나노초 빌림");
        assert_eq!(prior.elapsed_until(start), Ok(nano));
        assert_eq!(prior.checked_add(nano), Ok(start));
        let max = at(9999, Month::December, 31, (23, 59, 59, 999_999_999));
        let min = at(1, Month::January, 1, (0, 0, 0, 0));
        max.checked_add(nano).expect_err("거부");
        let shifted_max =
            Timestamp::from_parts(max.utc_seconds(), max.nanosecond(), OffsetSeconds::new(-60))
                .expect("UTC 상한의 음수 오프셋");
        shifted_max.checked_add(nano).expect_err("UTC 상한 넘침");
        min.checked_sub(nano).expect_err("거부");
        let huge = TimeSpan::try_new(
            SpanSeconds::new(9_223_372_036_854_775_807),
            Nanosecond::new(999_999_999),
        )
        .expect("기간 상한");
        assert_eq!(start.checked_add(huge), Err(TimeError::Overflow));
        assert_eq!(start.checked_sub(huge), Err(TimeError::Overflow));
        TimeSpan::try_new(SpanSeconds::new(u64::MAX), Nanosecond::new(0)).expect_err("거부");
        TimeSpan::try_new(SpanSeconds::new(0), Nanosecond::new(1_000_000_000)).expect_err("거부");
    }

    #[test]
    fn intervals_reject_empty_and_reversed_bounds_and_preserve_endpoints() {
        let start = at(2026_i32, Month::March, 25, (22, 30, 0, 0));
        let end = at(2026_i32, Month::March, 26, (5, 0, 0, 0));
        let interval = TimeInterval::try_new(start, end).expect("구간");
        assert_eq!(interval.start(), start);
        assert_eq!(interval.end(), end);
        assert_eq!(
            TimeInterval::try_new(end, start),
            Err(TimeIntervalError::NonIncreasing)
        );
        assert_eq!(
            TimeInterval::try_new(start, start),
            Err(TimeIntervalError::NonIncreasing)
        );
    }
}
