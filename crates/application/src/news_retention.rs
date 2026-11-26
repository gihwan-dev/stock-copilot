use crate::{Clock, CollectedCount, SystemPortError};
use domain::{Nanosecond, NewsSource, SpanSeconds, TimeSpan, Timestamp};
use std::fmt::{self, Debug, Formatter};
use std::future::Future;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetentionCutoff {
    pub source: NewsSource,
    pub before: Timestamp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetentionDays {
    Week,
    Quarter,
}

impl RetentionDays {
    #[must_use]
    pub const fn of(source: &NewsSource) -> Self {
        match source {
            NewsSource::GoogleNews => Self::Week,
            NewsSource::Hankyung
            | NewsSource::Maekyung
            | NewsSource::Yonhap
            | NewsSource::Edaily
            | NewsSource::SegyeFinance
            | NewsSource::BankOfKorea
            | NewsSource::FederalReserve
            | NewsSource::YahooFinance => Self::Quarter,
        }
    }

    #[must_use]
    pub fn span(self) -> Option<TimeSpan> {
        let seconds = match self {
            Self::Week => SpanSeconds::new(7 * 86_400),
            Self::Quarter => SpanSeconds::new(90 * 86_400),
        };
        TimeSpan::try_new(seconds, Nanosecond::new(0)).ok()
    }
}

#[must_use]
pub fn news_retention_cutoffs(now: Timestamp) -> Vec<RetentionCutoff> {
    [
        NewsSource::Hankyung,
        NewsSource::Maekyung,
        NewsSource::Yonhap,
        NewsSource::Edaily,
        NewsSource::SegyeFinance,
        NewsSource::BankOfKorea,
        NewsSource::FederalReserve,
        NewsSource::GoogleNews,
        NewsSource::YahooFinance,
    ]
    .into_iter()
    .filter_map(|source| {
        let span = RetentionDays::of(&source).span()?;
        let before = now.checked_sub(span).ok()?;
        Some(RetentionCutoff { source, before })
    })
    .collect()
}

pub trait CollectedRetention: Send + Sync {
    fn prune_news(
        &self,
        cutoffs: &[RetentionCutoff],
    ) -> impl Future<Output = Result<CollectedCount, SystemPortError>> + Send;
}

pub struct PruneCollectedNews<'a, Store> {
    pub store: &'a Store,
    pub clock: &'a dyn Clock,
}

impl<Store> Debug for PruneCollectedNews<'_, Store> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("PruneCollectedNews").finish_non_exhaustive()
    }
}

impl<Store: CollectedRetention> PruneCollectedNews<'_, Store> {
    pub async fn run(&self) -> Result<CollectedCount, SystemPortError> {
        let cutoffs = news_retention_cutoffs(self.clock.now());
        self.store.prune_news(&cutoffs).await
    }
}
