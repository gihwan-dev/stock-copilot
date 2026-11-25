use std::future::Future;

use domain::{InstrumentId, Timestamp};

use crate::{Clock, SystemPortError, WriteCompleted};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkipReasonError {
    Empty,
    MultipleLines,
    TooLong,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkipReason(String);

impl SkipReason {
    pub fn try_new(value: &str) -> Result<Self, SkipReasonError> {
        if value.contains(['\n', '\r']) {
            return Err(SkipReasonError::MultipleLines);
        }
        let trimmed = value.trim();
        if trimmed.chars().count() > 200_usize {
            return Err(SkipReasonError::TooLong);
        }
        if trimmed.is_empty() {
            return Err(SkipReasonError::Empty);
        }
        Ok(Self(trimmed.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignalSkip {
    pub instrument: InstrumentId,
    pub reason: Option<SkipReason>,
    pub skipped_at: Timestamp,
}

pub trait SignalSkipRepository: Send + Sync {
    fn record_skip(
        &self,
        skip: &SignalSkip,
    ) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send;
}

pub trait SignalSkipHistory: Send + Sync {
    fn latest_skip(
        &self,
        instrument: &InstrumentId,
    ) -> impl Future<Output = Result<Option<Timestamp>, SystemPortError>> + Send;
}

#[derive(Debug)]
pub struct SkipSignal<'a, Repository, TimeSource> {
    repository: &'a Repository,
    clock: &'a TimeSource,
}

impl<'a, Repository: SignalSkipRepository, TimeSource: Clock>
    SkipSignal<'a, Repository, TimeSource>
{
    #[must_use]
    pub const fn new(repository: &'a Repository, clock: &'a TimeSource) -> Self {
        Self { repository, clock }
    }

    pub async fn execute(
        &self,
        instrument: InstrumentId,
        reason: Option<SkipReason>,
    ) -> Result<SignalSkip, SystemPortError> {
        let skip = SignalSkip {
            instrument,
            reason,
            skipped_at: self.clock.now(),
        };
        self.repository.record_skip(&skip).await?;
        Ok(skip)
    }
}
