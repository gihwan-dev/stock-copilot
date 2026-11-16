use crate::{CalendarDate, OffsetSeconds, Timestamp};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransactionSyncTimeError {
    OutOfRange,
    ClockReversed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DailyFullInspection {
    Required,
    CompletedToday,
}

pub fn daily_full_inspection(
    last_completed: Option<Timestamp>,
    now: Timestamp,
) -> Result<DailyFullInspection, TransactionSyncTimeError> {
    let today = kst_date(now)?;
    let Some(last) = last_completed else {
        return Ok(DailyFullInspection::Required);
    };
    let last_day = kst_date(last)?;
    if last > now {
        return Err(TransactionSyncTimeError::ClockReversed);
    }
    if last_day == today {
        Ok(DailyFullInspection::CompletedToday)
    } else {
        Ok(DailyFullInspection::Required)
    }
}

pub(crate) fn kst_date(value: Timestamp) -> Result<CalendarDate, TransactionSyncTimeError> {
    value
        .date_at_offset(OffsetSeconds::new(32400))
        .map_err(|_error| TransactionSyncTimeError::OutOfRange)
}

#[cfg(test)]
mod tests {
    use super::{DailyFullInspection, TransactionSyncTimeError, daily_full_inspection};
    use crate::recent_transaction_window;
    use crate::{
        CalendarDate, CalendarYear, DayNumber, MonthNumber, Nanosecond, OffsetSeconds, Timestamp,
        UnixSeconds,
    };
    use proptest::{prop_assert_eq, proptest, test_runner::Config as ProptestConfig};
    use time::{Date, Month, UtcOffset};

    fn date(year: i32, month: u8, day: u8) -> CalendarDate {
        CalendarDate::from_parts(
            CalendarYear::new(year),
            MonthNumber::new(month),
            DayNumber::new(day),
        )
        .expect("날짜")
    }
    fn at(year: i32, month: u8, day: u8, clock: (u8, u8, u8, u32)) -> Timestamp {
        let value = Date::from_calendar_date(year, Month::try_from(month).expect("월"), day)
            .expect("날짜")
            .with_hms_nano(clock.0, clock.1, clock.2, clock.3)
            .expect("시간")
            .assume_utc();
        Timestamp::from_parts(
            UnixSeconds::new(value.unix_timestamp()),
            Nanosecond::new(clock.3),
            OffsetSeconds::new(0),
        )
        .expect("시각")
    }

    #[test]
    fn kst_midnight_before_exact_after_and_utc_day() {
        for (now, from, to) in [
            (
                at(2026, 1, 31, (14, 59, 59, 999_999_999)),
                date(2026, 1, 18),
                date(2026, 1, 31),
            ),
            (
                at(2026, 1, 31, (15, 0, 0, 0)),
                date(2026, 1, 19),
                date(2026, 2, 1),
            ),
            (
                at(2026, 1, 31, (15, 0, 0, 1)),
                date(2026, 1, 19),
                date(2026, 2, 1),
            ),
            (
                at(2026, 12, 31, (15, 0, 0, 0)),
                date(2026, 12, 19),
                date(2027, 1, 1),
            ),
            (
                at(2024, 3, 1, (0, 0, 0, 0)),
                date(2024, 2, 17),
                date(2024, 3, 1),
            ),
            (
                at(2024, 2, 28, (15, 0, 0, 0)),
                date(2024, 2, 16),
                date(2024, 2, 29),
            ),
            (at(1, 1, 13, (15, 0, 0, 0)), date(1, 1, 1), date(1, 1, 14)),
            (
                at(9999, 12, 31, (14, 59, 59, 999_999_999)),
                date(9999, 12, 18),
                date(9999, 12, 31),
            ),
        ] {
            let window = recent_transaction_window(now).expect("창");
            assert_eq!(window.from(), from);
            assert_eq!(window.to(), to);
            assert_eq!(*from.days_until(to).expect("차이").as_i64(), 13);
            assert_eq!(
                window.coverage(from),
                crate::settled_row_set::WindowCoverage::Inside
            );
            assert_eq!(
                window.coverage(to),
                crate::settled_row_set::WindowCoverage::Inside
            );
        }
        let window = recent_transaction_window(at(2026, 1, 31, (15, 0, 0, 0))).expect("창");
        assert_eq!(
            window.coverage(date(2026, 1, 18)),
            crate::settled_row_set::WindowCoverage::Outside
        );
        assert_eq!(
            window.coverage(date(2026, 2, 2)),
            crate::settled_row_set::WindowCoverage::Outside
        );
    }

    #[test]
    fn daily_inspection_uses_calendar_day_and_rejects_future_even_one_nanosecond() {
        let midnight = at(2026, 12, 31, (15, 0, 0, 0));
        let before = at(2026, 12, 31, (14, 59, 59, 999_999_999));
        let after = at(2026, 12, 31, (15, 0, 0, 1));
        assert_eq!(
            daily_full_inspection(None, midnight),
            Ok(DailyFullInspection::Required)
        );
        assert_eq!(
            daily_full_inspection(Some(before), midnight),
            Ok(DailyFullInspection::Required)
        );
        assert_eq!(
            daily_full_inspection(Some(midnight), midnight),
            Ok(DailyFullInspection::CompletedToday)
        );
        assert_eq!(
            daily_full_inspection(Some(midnight), after),
            Ok(DailyFullInspection::CompletedToday)
        );
        assert_eq!(
            daily_full_inspection(Some(after), midnight),
            Err(TransactionSyncTimeError::ClockReversed)
        );
        assert_eq!(
            daily_full_inspection(Some(midnight), before),
            Err(TransactionSyncTimeError::ClockReversed)
        );
        assert_eq!(
            daily_full_inspection(
                Some(at(2026, 1, 1, (15, 0, 0, 0))),
                at(2026, 1, 2, (14, 59, 59, 0))
            ),
            Ok(DailyFullInspection::CompletedToday)
        );
        assert_eq!(
            daily_full_inspection(
                Some(at(2026, 1, 1, (14, 59, 59, 0))),
                at(2026, 1, 1, (15, 0, 0, 0))
            ),
            Ok(DailyFullInspection::Required)
        );
    }

    #[test]
    fn unrepresentable_windows_and_kst_dates_fail_closed() {
        for now in [
            at(1, 1, 1, (0, 0, 0, 0)),
            at(1, 1, 13, (14, 59, 59, 999_999_999)),
            at(9999, 12, 31, (15, 0, 0, 0)),
        ] {
            assert_eq!(
                recent_transaction_window(now),
                Err(TransactionSyncTimeError::OutOfRange)
            );
        }
        let beyond = at(9999, 12, 31, (15, 0, 0, 0));
        assert_eq!(
            daily_full_inspection(Some(beyond), at(9999, 12, 31, (14, 0, 0, 0))),
            Err(TransactionSyncTimeError::OutOfRange)
        );
        assert_eq!(
            daily_full_inspection(None, beyond),
            Err(TransactionSyncTimeError::OutOfRange)
        );
        assert_eq!(
            daily_full_inspection(Some(beyond), beyond),
            Err(TransactionSyncTimeError::OutOfRange)
        );
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(testkit::property_cases()))]
        #[test]
        fn independent_calendar_oracle_and_offset_invariance(seconds in -62_134_473_600_i64..253_402_214_400_i64, nanos in 0_u32..1_000_000_000_u32, offset in -1439_i32..=1439_i32, age in 0_i64..172_800_i64) {
            let now=Timestamp::from_parts(UnixSeconds::new(seconds),Nanosecond::new(nanos),OffsetSeconds::new(offset.checked_mul(60).expect("오프셋"))).expect("시각");
            let raw= time::OffsetDateTime::from_unix_timestamp(seconds).expect("시각").to_offset(UtcOffset::from_hms(9,0,0).expect("KST"));
            let end=raw.date();
            let start=Date::from_julian_day(end.to_julian_day().checked_sub(13).expect("날짜 차")).expect("시작");
            let window=recent_transaction_window(now).expect("창");
            prop_assert_eq!(window.from(), date(start.year(),start.month().into(),start.day()));
            prop_assert_eq!(window.to(), date(end.year(),end.month().into(),end.day()));
            let utc=Timestamp::from_parts(now.utc_seconds(),now.nanosecond(),OffsetSeconds::new(0)).expect("UTC");
            prop_assert_eq!(recent_transaction_window(now),recent_transaction_window(utc));
            let last=Timestamp::from_parts(UnixSeconds::new(seconds.checked_sub(age).expect("이전 초")),Nanosecond::new(nanos),OffsetSeconds::new(0)).expect("이전");
            let last_day=time::OffsetDateTime::from_unix_timestamp(seconds.checked_sub(age).expect("이전 초")).expect("이전").to_offset(UtcOffset::from_hms(9,0,0).expect("KST")).date();
            let expected=if last_day==end {DailyFullInspection::CompletedToday} else {DailyFullInspection::Required};
            prop_assert_eq!(daily_full_inspection(Some(last),now),Ok(expected));
            prop_assert_eq!(daily_full_inspection(Some(now),last),if age==0 {Ok(DailyFullInspection::CompletedToday)} else {Err(TransactionSyncTimeError::ClockReversed)});
        }
    }
}
