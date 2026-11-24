use domain::{KnownMarket, TimeSpan, Timestamp};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ScheduledJobId {
    PortfolioSync,
    TransactionsSync,
    NewsCollection,
    MorningBriefing,
    ClosingReview,
    DailyValuationSnapshot,
    EstimateResolution,
    PreUsMarketCheck,
    EstimateCycle,
    CandidateDiscovery,
    WeeklyReview,
    IntradayCompaction,
    TossSpecWatch,
    DailyBackup,
    UsEstimateResolution,
    IntradayWatch,
}

impl ScheduledJobId {
    #[must_use]
    pub const fn as_code(self) -> &'static str {
        match self {
            Self::PortfolioSync => "portfolio_sync",
            Self::TransactionsSync => "transactions_sync",
            Self::NewsCollection => "news_collection",
            Self::MorningBriefing => "morning_briefing",
            Self::ClosingReview => "closing_review",
            Self::DailyValuationSnapshot => "daily_valuation_snapshot",
            Self::EstimateResolution => "estimate_resolution",
            Self::PreUsMarketCheck => "pre_us_market_check",
            Self::EstimateCycle => "estimate_cycle",
            Self::CandidateDiscovery => "candidate_discovery",
            Self::WeeklyReview => "weekly_review",
            Self::IntradayCompaction => "intraday_compaction",
            Self::TossSpecWatch => "toss_spec_watch",
            Self::DailyBackup => "daily_backup",
            Self::UsEstimateResolution => "us_estimate_resolution",
            Self::IntradayWatch => "intraday_watch",
        }
    }

    #[must_use]
    pub fn from_code(code: &str) -> Option<Self> {
        match code {
            "portfolio_sync" => Some(Self::PortfolioSync),
            "transactions_sync" => Some(Self::TransactionsSync),
            "news_collection" => Some(Self::NewsCollection),
            "morning_briefing" => Some(Self::MorningBriefing),
            "closing_review" => Some(Self::ClosingReview),
            "daily_valuation_snapshot" => Some(Self::DailyValuationSnapshot),
            "estimate_resolution" => Some(Self::EstimateResolution),
            "pre_us_market_check" => Some(Self::PreUsMarketCheck),
            "estimate_cycle" => Some(Self::EstimateCycle),
            "candidate_discovery" => Some(Self::CandidateDiscovery),
            "weekly_review" => Some(Self::WeeklyReview),
            "intraday_compaction" => Some(Self::IntradayCompaction),
            "toss_spec_watch" => Some(Self::TossSpecWatch),
            "daily_backup" => Some(Self::DailyBackup),
            "us_estimate_resolution" => Some(Self::UsEstimateResolution),
            "intraday_watch" => Some(Self::IntradayWatch),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobKind {
    CodeOnly,
    AiBackground,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MissedPolicy {
    CatchUpOnce,
    Skip,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartupPolicy {
    WaitForSchedule,
    RunAtStartup,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobEnablement {
    Enabled,
    Disabled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KstTimeError {
    HourOutOfRange,
    MinuteOutOfRange,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct KstTimeOfDay {
    hour: u8,
    minute: u8,
}

impl KstTimeOfDay {
    pub fn try_new(hour: u8, minute: u8) -> Result<Self, KstTimeError> {
        if hour > 23 {
            return Err(KstTimeError::HourOutOfRange);
        }
        if minute > 59 {
            return Err(KstTimeError::MinuteOutOfRange);
        }
        Ok(Self { hour, minute })
    }

    #[must_use]
    pub const fn hour(self) -> u8 {
        self.hour
    }

    #[must_use]
    pub const fn minute(self) -> u8 {
        self.minute
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum KstWeekday {
    Monday,
    Tuesday,
    Wednesday,
    Thursday,
    Friday,
    Saturday,
    Sunday,
}

impl KstWeekday {
    #[must_use]
    pub const fn number_from_monday(self) -> i64 {
        match self {
            Self::Monday => 0,
            Self::Tuesday => 1,
            Self::Wednesday => 2,
            Self::Thursday => 3,
            Self::Friday => 4,
            Self::Saturday => 5,
            Self::Sunday => 6,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScheduleRule {
    DailyKst(KstTimeOfDay),
    WeeklyKst {
        weekday: KstWeekday,
        time: KstTimeOfDay,
    },
    Interval(TimeSpan),
    AfterMarketClose {
        market: KnownMarket,
        delay: TimeSpan,
        fallback: KstTimeOfDay,
    },
    BeforeMarketOpen {
        market: KnownMarket,
        lead: TimeSpan,
        fallback: KstTimeOfDay,
    },
    EventTriggered,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JobConfig {
    pub kind: JobKind,
    pub schedule: ScheduleRule,
    pub missed_policy: MissedPolicy,
    pub enabled: JobEnablement,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JobScheduleOverride {
    pub id: ScheduledJobId,
    pub enabled: JobEnablement,
    pub schedule: ScheduleRule,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobRunDetail {
    pub summary_code: String,
    pub processed_items: u32,
}

impl JobRunDetail {
    #[must_use]
    pub fn new<S: Into<String>>(summary_code: S, processed_items: u32) -> Self {
        Self {
            summary_code: summary_code.into(),
            processed_items,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobError {
    PortUnavailable(String),
    StorageFailure(String),
    CalendarUnavailable,
    ExecutionFailed(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobRunOutcome {
    Executed(JobRunDetail),
    SkippedMissedWindow,
    DeferredByAiQueue { resumes_at: Option<Timestamp> },
    Failed(JobError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobExecutionRecord {
    pub id: ScheduledJobId,
    pub scheduled_for: Timestamp,
    pub evaluated_at: Timestamp,
    pub outcome: JobRunOutcome,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobStatusSnapshot {
    pub id: ScheduledJobId,
    pub config: JobConfig,
    pub next_due: Option<Timestamp>,
    pub last_evaluated_at: Option<Timestamp>,
    pub last_succeeded_at: Option<Timestamp>,
    pub last_outcome: Option<JobRunOutcome>,
    pub recent_history: Vec<JobExecutionRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TickReport {
    pub evaluated_at: Timestamp,
    pub records: Vec<JobExecutionRecord>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoopControl {
    Run,
    Stop,
}
