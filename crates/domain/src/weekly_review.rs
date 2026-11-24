use crate::{CalendarDate, OffsetSeconds, TimeError, Timestamp};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeeklyReviewSlot {
    Due(CalendarDate),
    NotDue,
}

pub fn weekly_review_slot(now: Timestamp) -> Result<WeeklyReviewSlot, TimeError> {
    let date = now.date_at_offset(OffsetSeconds::new(32_400))?;
    let local = now
        .utc_seconds()
        .as_i64()
        .checked_add(32_400_i64)
        .ok_or(TimeError::Overflow)?
        .rem_euclid(86_400_i64);
    let saturday = *crate::domain_time::performance_weekday(date).as_u8() == 6;
    if saturday && (21_600_i64..43_200_i64).contains(&local) {
        Ok(WeeklyReviewSlot::Due(date))
    } else {
        Ok(WeeklyReviewSlot::NotDue)
    }
}

#[cfg(test)]
mod tests {
    use super::{WeeklyReviewSlot, weekly_review_slot};
    use crate::{
        CalendarDate, CalendarYear, DayNumber, MonthNumber, Nanosecond, OffsetSeconds, Timestamp,
        UnixSeconds,
    };

    fn kst(day: u8, hour: i64, minute: i64) -> Timestamp {
        let base = CalendarDate::from_parts(
            CalendarYear::new(2026),
            MonthNumber::new(11),
            DayNumber::new(1),
        )
        .expect("날짜");
        let days = i64::from(day.checked_sub(1).expect("일"));
        let midnight_utc = crate::valuation_time(base)
            .utc_seconds()
            .as_i64()
            .checked_sub(25_200)
            .expect("자정");
        Timestamp::from_parts(
            UnixSeconds::new(
                midnight_utc
                    .checked_add(days.checked_mul(86_400).expect("일"))
                    .and_then(|value| value.checked_add(hour.checked_mul(3_600)?))
                    .and_then(|value| value.checked_add(minute.checked_mul(60)?))
                    .and_then(|value| value.checked_sub(32_400))
                    .expect("시각"),
            ),
            Nanosecond::new(0),
            OffsetSeconds::new(0),
        )
        .expect("시각")
    }
    fn date(day: u8) -> CalendarDate {
        CalendarDate::from_parts(
            CalendarYear::new(2026),
            MonthNumber::new(11),
            DayNumber::new(day),
        )
        .expect("날짜")
    }

    #[test]
    fn only_saturday_morning_kst_is_due() {
        assert_eq!(
            weekly_review_slot(kst(28, 6, 0)),
            Ok(WeeklyReviewSlot::Due(date(28)))
        );
        assert_eq!(
            weekly_review_slot(kst(28, 11, 59)),
            Ok(WeeklyReviewSlot::Due(date(28)))
        );
        for (day, hour, minute) in [(28, 5, 59), (28, 12, 0), (27, 9, 0), (29, 9, 0)] {
            assert_eq!(
                weekly_review_slot(kst(day, hour, minute)),
                Ok(WeeklyReviewSlot::NotDue),
                "{day} {hour}:{minute}"
            );
        }
    }
}
