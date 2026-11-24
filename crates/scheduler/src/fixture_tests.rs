use crate::{
    JobConfig, JobEnablement, JobFuture, JobKind, JobRunDetail, KST_OFFSET_SECONDS,
    MarketCalendarSource, MissedPolicy, ScheduleRule, ScheduledJob, ScheduledJobId, StartupPolicy,
};
use domain::{
    CalendarDate, CalendarYear, DayNumber, KnownMarket, MarketCalendar, MarketSession, MonthNumber,
    Nanosecond, OffsetSeconds, SessionData, SessionInformation, SpanSeconds, TimeInterval,
    TimeSpan, Timestamp, UnixSeconds,
};
use std::sync::Arc;
use tokio::sync::RwLock;

pub(crate) fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .expect("테스트 런타임")
}

pub(crate) fn date(year: i32, month: u8, day: u8) -> CalendarDate {
    CalendarDate::from_parts(
        CalendarYear::new(year),
        MonthNumber::new(month),
        DayNumber::new(day),
    )
    .expect("날짜")
}

pub(crate) fn kst(year: i32, month: u8, day: u8, hour: u8, minute: u8) -> Timestamp {
    let days = *date(1970, 1, 1)
        .days_until(date(year, month, day))
        .expect("일수")
        .as_i64();
    let total_kst = days
        .checked_mul(86_400)
        .and_then(|value| value.checked_add(i64::from(hour).checked_mul(3_600)?))
        .and_then(|value| value.checked_add(i64::from(minute).checked_mul(60)?))
        .expect("KST 초");
    let utc = total_kst
        .checked_sub(i64::from(KST_OFFSET_SECONDS))
        .expect("UTC 초");
    Timestamp::from_parts(
        UnixSeconds::new(utc),
        Nanosecond::new(0),
        OffsetSeconds::new(KST_OFFSET_SECONDS),
    )
    .expect("시각")
}

pub(crate) fn span_secs(seconds: u64) -> TimeSpan {
    TimeSpan::try_new(SpanSeconds::new(seconds), Nanosecond::new(0)).expect("기간")
}

pub(crate) fn code_config(schedule: ScheduleRule, missed_policy: MissedPolicy) -> JobConfig {
    JobConfig {
        kind: JobKind::CodeOnly,
        schedule,
        missed_policy,
        enabled: JobEnablement::Enabled,
    }
}

pub(crate) struct RecordingJob {
    pub(crate) id: ScheduledJobId,
    pub(crate) config: JobConfig,
    pub(crate) startup: StartupPolicy,
    pub(crate) runs: Arc<RwLock<Vec<Timestamp>>>,
}

impl RecordingJob {
    pub(crate) fn new(
        id: ScheduledJobId,
        config: JobConfig,
        startup: StartupPolicy,
    ) -> (Self, Arc<RwLock<Vec<Timestamp>>>) {
        let runs = Arc::new(RwLock::new(Vec::new()));
        (
            Self {
                id,
                config,
                startup,
                runs: Arc::clone(&runs),
            },
            runs,
        )
    }
}

impl ScheduledJob for RecordingJob {
    fn id(&self) -> ScheduledJobId {
        self.id
    }

    fn default_config(&self) -> JobConfig {
        self.config
    }

    fn startup_policy(&self) -> StartupPolicy {
        self.startup
    }

    fn run(&self, now: Timestamp) -> JobFuture<'_> {
        let runs = Arc::clone(&self.runs);
        Box::pin(async move {
            runs.write().await.push(now);
            Ok(JobRunDetail::new("ok", 1))
        })
    }
}

pub(crate) struct FixedCalendar(pub(crate) MarketCalendar);

impl MarketCalendarSource for FixedCalendar {
    fn calendar(&self, market: KnownMarket) -> Option<MarketCalendar> {
        match market {
            KnownMarket::Kr => Some(self.0.clone()),
            KnownMarket::Us => None,
        }
    }
}

fn kr_day(year: i32, month: u8, day: u8, close_hour: u8, close_minute: u8) -> MarketSession {
    let regular = SessionData::try_new(
        TimeInterval::try_new(
            kst(year, month, day, 9, 0),
            kst(year, month, day, close_hour, close_minute),
        )
        .expect("정규장"),
        None,
    )
    .expect("세션");
    MarketSession::try_new(
        date(year, month, day),
        SessionInformation::Kr {
            pre_market: None,
            regular_market: Some(regular),
            after_market: None,
        },
    )
    .expect("국내 장")
}

pub(crate) fn kr_calendar_with_late_close() -> MarketCalendar {
    MarketCalendar::try_new(
        kr_day(2026, 11, 24, 15, 30),
        kr_day(2026, 11, 25, 16, 30),
        kr_day(2026, 11, 26, 15, 30),
    )
    .expect("달력")
}
