use std::future::Future;

use domain::{InstrumentId, Thesis, ThesisVersion, Timestamp};

use crate::{Clock, SystemPortError, ThesisRepository, WriteCompleted};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThesisEntryTime {
    pub sequence: ThesisVersion,
    pub recorded_at: Timestamp,
}

pub trait ThesisHistoryTimes: Send + Sync {
    fn record_times(
        &self,
        instrument: &InstrumentId,
        sequences: &[ThesisVersion],
        recorded_at: Timestamp,
    ) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send;

    fn entry_times(
        &self,
        instrument: &InstrumentId,
    ) -> impl Future<Output = Result<Vec<ThesisEntryTime>, SystemPortError>> + Send;
}

#[derive(Debug)]
pub struct TimedThesisRepository<R, T, C> {
    repository: std::sync::Arc<R>,
    times: std::sync::Arc<T>,
    clock: std::sync::Arc<C>,
}

impl<R, T, C> TimedThesisRepository<R, T, C> {
    #[must_use]
    pub const fn new(
        repository: std::sync::Arc<R>,
        times: std::sync::Arc<T>,
        clock: std::sync::Arc<C>,
    ) -> Self {
        Self {
            repository,
            times,
            clock,
        }
    }
}

impl<R: ThesisRepository, T: ThesisHistoryTimes, C: Clock> ThesisRepository
    for TimedThesisRepository<R, T, C>
{
    async fn store(
        &self,
        instrument: &InstrumentId,
        thesis: &Thesis,
    ) -> Result<WriteCompleted, SystemPortError> {
        let written = self.repository.store(instrument, thesis).await?;
        let sequences = thesis
            .history()
            .iter()
            .map(domain::HistoryEntry::sequence)
            .collect::<Vec<_>>();
        self.times
            .record_times(instrument, &sequences, self.clock.now())
            .await?;
        Ok(written)
    }

    async fn load(&self, instrument: &InstrumentId) -> Result<Option<Thesis>, SystemPortError> {
        self.repository.load(instrument).await
    }
}

#[cfg(test)]
mod tests;
