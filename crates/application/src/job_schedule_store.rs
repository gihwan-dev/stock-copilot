use crate::{SystemPortError, WriteCompleted};
use domain::Timestamp;
use std::future::Future;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct JobCode(String);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobCodeError {
    InvalidFormat,
}

impl JobCode {
    pub fn try_new(value: &str) -> Result<Self, JobCodeError> {
        if value.is_empty()
            || value.len() > 64
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
        {
            return Err(JobCodeError::InvalidFormat);
        }
        Ok(Self(value.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobSwitch {
    Enabled,
    Disabled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct JobHour(u8);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct JobMinute(u8);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobClockError {
    OutOfRange,
}

impl JobHour {
    pub const fn try_new(value: u8) -> Result<Self, JobClockError> {
        if value > 23 {
            return Err(JobClockError::OutOfRange);
        }
        Ok(Self(value))
    }

    #[must_use]
    pub const fn as_u8(&self) -> &u8 {
        &self.0
    }
}

impl JobMinute {
    pub const fn try_new(value: u8) -> Result<Self, JobClockError> {
        if value > 59 {
            return Err(JobClockError::OutOfRange);
        }
        Ok(Self(value))
    }

    #[must_use]
    pub const fn as_u8(&self) -> &u8 {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JobClockTime {
    pub hour: JobHour,
    pub minute: JobMinute,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavedJobSchedule {
    pub job: JobCode,
    pub switch: JobSwitch,
    pub time: Option<JobClockTime>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobSummary(String);

impl JobSummary {
    #[must_use]
    pub fn new(value: &str) -> Self {
        Self(value.chars().take(128).collect())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct JobItemCount(u32);

impl JobItemCount {
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
pub enum JobFailureKind {
    PortUnavailable,
    StorageFailure,
    CalendarUnavailable,
    ExecutionFailed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobRunResult {
    Executed {
        summary: JobSummary,
        processed: JobItemCount,
    },
    SkippedMissedWindow,
    Deferred {
        resumes_at: Option<Timestamp>,
    },
    Failed(JobFailureKind),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobRunEntry {
    pub job: JobCode,
    pub scheduled_for: Timestamp,
    pub evaluated_at: Timestamp,
    pub result: JobRunResult,
}

pub trait JobScheduleStore: Send + Sync {
    fn save_schedule(
        &self,
        schedule: &SavedJobSchedule,
    ) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send;
    fn schedules(
        &self,
    ) -> impl Future<Output = Result<Vec<SavedJobSchedule>, SystemPortError>> + Send;
    fn append_run(
        &self,
        entry: &JobRunEntry,
    ) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send;
    fn recent_runs(
        &self,
        job: &JobCode,
    ) -> impl Future<Output = Result<Vec<JobRunEntry>, SystemPortError>> + Send;
}
