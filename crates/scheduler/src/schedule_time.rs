use crate::types::{KstTimeOfDay, KstWeekday, ScheduleRule};
use domain::{Nanosecond, OffsetSeconds, SpanSeconds, TimeError, TimeSpan, Timestamp, UnixSeconds};

pub const KST_OFFSET_SECONDS: i32 = 32_400;
const SECONDS_PER_DAY: i64 = 86_400;
const SECONDS_PER_HOUR: i64 = 3_600;
const SECONDS_PER_MINUTE: i64 = 60;
const EPOCH_THURSDAY_OFFSET: i64 = 3;
const DAYS_PER_WEEK: i64 = 7;
const SKIP_GRACE_SECONDS: u64 = 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScheduleTimeError {
    ZeroInterval,
    Time(TimeError),
    Overflow,
}

pub fn next_due_after(
    after: Timestamp,
    rule: ScheduleRule,
) -> Result<Option<Timestamp>, ScheduleTimeError> {
    match rule {
        ScheduleRule::DailyKst(time)
        | ScheduleRule::AfterMarketClose { fallback: time, .. }
        | ScheduleRule::BeforeMarketOpen { fallback: time, .. } => {
            next_daily_kst(after, time).map(Some)
        }
        ScheduleRule::WeeklyKst { weekday, time } => {
            next_weekly_kst(after, weekday, time).map(Some)
        }
        ScheduleRule::Interval(span) => next_interval(after, span).map(Some),
        ScheduleRule::EventTriggered => Ok(None),
    }
}

pub(crate) fn initial_due(
    started_at: Timestamp,
    rule: ScheduleRule,
) -> Result<Option<Timestamp>, ScheduleTimeError> {
    match rule {
        ScheduleRule::DailyKst(time)
        | ScheduleRule::AfterMarketClose { fallback: time, .. }
        | ScheduleRule::BeforeMarketOpen { fallback: time, .. } => {
            daily_kst_at_or_after(started_at, time).map(Some)
        }
        ScheduleRule::WeeklyKst { weekday, time } => {
            weekly_kst_at_or_after(started_at, weekday, time).map(Some)
        }
        ScheduleRule::Interval(span) => next_interval(started_at, span).map(Some),
        ScheduleRule::EventTriggered => Ok(None),
    }
}

#[must_use]
pub fn is_within_skip_grace(scheduled_for: Timestamp, now: Timestamp) -> bool {
    let Ok(grace) = TimeSpan::try_new(SpanSeconds::new(SKIP_GRACE_SECONDS), Nanosecond::new(0))
    else {
        return false;
    };
    match scheduled_for.elapsed_until(now) {
        Ok(delta) => delta <= grace,
        Err(_) => false,
    }
}

fn next_interval(after: Timestamp, span: TimeSpan) -> Result<Timestamp, ScheduleTimeError> {
    let (seconds, nanos) = span.as_parts();
    if *seconds.as_u64() == 0 && *nanos.as_u32() == 0 {
        return Err(ScheduleTimeError::ZeroInterval);
    }
    after.checked_add(span).map_err(ScheduleTimeError::Time)
}

fn kst_day_index(instant: Timestamp) -> Result<i64, ScheduleTimeError> {
    let kst_seconds = instant
        .utc_seconds()
        .as_i64()
        .checked_add(i64::from(KST_OFFSET_SECONDS))
        .ok_or(ScheduleTimeError::Overflow)?;
    kst_seconds
        .checked_div_euclid(SECONDS_PER_DAY)
        .ok_or(ScheduleTimeError::Overflow)
}

fn kst_timestamp(day_index: i64, time: KstTimeOfDay) -> Result<Timestamp, ScheduleTimeError> {
    let day_start = day_index
        .checked_mul(SECONDS_PER_DAY)
        .ok_or(ScheduleTimeError::Overflow)?;
    let hour_secs = i64::from(time.hour())
        .checked_mul(SECONDS_PER_HOUR)
        .ok_or(ScheduleTimeError::Overflow)?;
    let minute_secs = i64::from(time.minute())
        .checked_mul(SECONDS_PER_MINUTE)
        .ok_or(ScheduleTimeError::Overflow)?;
    let kst_total = day_start
        .checked_add(hour_secs)
        .and_then(|value| value.checked_add(minute_secs))
        .ok_or(ScheduleTimeError::Overflow)?;
    let utc_seconds = kst_total
        .checked_sub(i64::from(KST_OFFSET_SECONDS))
        .ok_or(ScheduleTimeError::Overflow)?;
    Timestamp::from_parts(
        UnixSeconds::new(utc_seconds),
        Nanosecond::new(0),
        OffsetSeconds::new(KST_OFFSET_SECONDS),
    )
    .map_err(ScheduleTimeError::Time)
}

fn daily_kst_at_or_after(
    instant: Timestamp,
    time: KstTimeOfDay,
) -> Result<Timestamp, ScheduleTimeError> {
    let day_index = kst_day_index(instant)?;
    let today_target = kst_timestamp(day_index, time)?;
    if today_target >= instant {
        Ok(today_target)
    } else {
        let next_day = day_index
            .checked_add(1)
            .ok_or(ScheduleTimeError::Overflow)?;
        kst_timestamp(next_day, time)
    }
}

fn next_daily_kst(after: Timestamp, time: KstTimeOfDay) -> Result<Timestamp, ScheduleTimeError> {
    let day_index = kst_day_index(after)?;
    let today_target = kst_timestamp(day_index, time)?;
    if today_target > after {
        Ok(today_target)
    } else {
        let next_day = day_index
            .checked_add(1)
            .ok_or(ScheduleTimeError::Overflow)?;
        kst_timestamp(next_day, time)
    }
}

fn weekday_from_day_index(day_index: i64) -> Result<i64, ScheduleTimeError> {
    day_index
        .checked_add(EPOCH_THURSDAY_OFFSET)
        .and_then(|shifted| shifted.checked_rem_euclid(DAYS_PER_WEEK))
        .ok_or(ScheduleTimeError::Overflow)
}

fn weekly_kst_at_or_after(
    instant: Timestamp,
    weekday: KstWeekday,
    time: KstTimeOfDay,
) -> Result<Timestamp, ScheduleTimeError> {
    let day_index = kst_day_index(instant)?;
    let current_weekday = weekday_from_day_index(day_index)?;
    let delta = weekday
        .number_from_monday()
        .checked_sub(current_weekday)
        .and_then(|diff| diff.checked_rem_euclid(DAYS_PER_WEEK))
        .ok_or(ScheduleTimeError::Overflow)?;
    let candidate_day = day_index
        .checked_add(delta)
        .ok_or(ScheduleTimeError::Overflow)?;
    let candidate = kst_timestamp(candidate_day, time)?;
    if candidate >= instant {
        Ok(candidate)
    } else {
        let next_week = candidate_day
            .checked_add(DAYS_PER_WEEK)
            .ok_or(ScheduleTimeError::Overflow)?;
        kst_timestamp(next_week, time)
    }
}

fn next_weekly_kst(
    after: Timestamp,
    weekday: KstWeekday,
    time: KstTimeOfDay,
) -> Result<Timestamp, ScheduleTimeError> {
    let candidate = weekly_kst_at_or_after(after, weekday, time)?;
    if candidate > after {
        Ok(candidate)
    } else {
        let candidate_day = kst_day_index(candidate)?;
        let next_week = candidate_day
            .checked_add(DAYS_PER_WEEK)
            .ok_or(ScheduleTimeError::Overflow)?;
        kst_timestamp(next_week, time)
    }
}
