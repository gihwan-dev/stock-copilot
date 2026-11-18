use crate::{CacheWriteOutcome, DeliveryReport, EventBus, MarketDataRepository, SystemPortError};
use domain::{DomainEvent, FxRate};

#[derive(Debug)]
pub struct StoreFxRate<'a, Repository> {
    repository: &'a Repository,
    events: &'a EventBus,
}
impl<'a, Repository: MarketDataRepository> StoreFxRate<'a, Repository> {
    #[must_use]
    pub const fn new(repository: &'a Repository, events: &'a EventBus) -> Self {
        Self { repository, events }
    }
    pub async fn execute(&self, rate: FxRate) -> Result<DeliveryReport, SystemPortError> {
        match self.repository.store_fx_rate(&rate).await? {
            CacheWriteOutcome::Changed => {
                Ok(self.events.publish(DomainEvent::FxRateUpdated(rate.pair())))
            }
            CacheWriteOutcome::Unchanged | CacheWriteOutcome::OlderIgnored => {
                Ok(DeliveryReport::default())
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefreshFxRateReport {
    pub rate: FxRate,
    pub delivery: DeliveryReport,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefreshFxRateError {
    Broker(crate::PortError),
    MismatchedRate,
    Storage(SystemPortError),
}
#[derive(Debug)]
pub struct RefreshFxRate<'a, Market, Repository> {
    market: &'a Market,
    repository: &'a Repository,
    events: &'a EventBus,
}
impl<'a, Market: crate::MarketInfo, Repository: MarketDataRepository>
    RefreshFxRate<'a, Market, Repository>
{
    #[must_use]
    pub const fn new(market: &'a Market, repository: &'a Repository, events: &'a EventBus) -> Self {
        Self {
            market,
            repository,
            events,
        }
    }
    pub async fn execute(
        &self,
        pair: domain::FxPair,
        kind: domain::FxRateKind,
        at: Option<domain::Timestamp>,
    ) -> Result<RefreshFxRateReport, RefreshFxRateError> {
        let rate = self
            .market
            .fx_rate(pair, kind, at)
            .await
            .map_err(RefreshFxRateError::Broker)?;
        if rate.pair() != pair
            || rate.kind() != kind
            || at.is_some_and(|instant| rate.validity_at(instant) != domain::FxValidity::Valid)
        {
            return Err(RefreshFxRateError::MismatchedRate);
        }
        let delivery = StoreFxRate::new(self.repository, self.events)
            .execute(rate)
            .await
            .map_err(RefreshFxRateError::Storage)?;
        Ok(RefreshFxRateReport { rate, delivery })
    }
}
