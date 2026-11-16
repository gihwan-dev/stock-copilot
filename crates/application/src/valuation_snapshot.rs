use crate::{
    Clock, DeliveryReport, EventBus, FxRateQuery, MarketDataRepository, PortfolioRepository,
    RecordOutcome, SystemPortError, ValuationRepository,
};
use domain::{
    AccountId, DomainEvent, FxPair, FxRateKind, SnapshotCalendar, SnapshotError, SyncOutcome,
    ValuationSnapshot,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValuationSnapshotError {
    Repository(SystemPortError),
    MissingPortfolio,
    UnavailablePortfolio,
    Snapshot(SnapshotError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValuationSnapshotReport {
    pub snapshot: ValuationSnapshot,
    pub recorded: RecordOutcome,
    pub delivery: DeliveryReport,
}

#[derive(Debug)]
pub struct TakeValuationSnapshot<'a, Portfolio, Market, Repository, Time> {
    portfolio: &'a Portfolio,
    market: &'a Market,
    repository: &'a Repository,
    clock: &'a Time,
    events: &'a EventBus,
}
impl<'a, Portfolio, Market, Repository, Time>
    TakeValuationSnapshot<'a, Portfolio, Market, Repository, Time>
where
    Portfolio: PortfolioRepository,
    Market: MarketDataRepository,
    Repository: ValuationRepository,
    Time: Clock,
{
    #[must_use]
    pub const fn new(
        portfolio: &'a Portfolio,
        market: &'a Market,
        repository: &'a Repository,
        clock: &'a Time,
        events: &'a EventBus,
    ) -> Self {
        Self {
            portfolio,
            market,
            repository,
            clock,
            events,
        }
    }
    pub async fn execute(
        &self,
        account: &AccountId,
        calendar: &SnapshotCalendar,
    ) -> Result<ValuationSnapshotReport, ValuationSnapshotError> {
        let record = self
            .portfolio
            .latest(account)
            .await
            .map_err(ValuationSnapshotError::Repository)?
            .ok_or(ValuationSnapshotError::MissingPortfolio)?;
        let portfolio = match record.outcome() {
            SyncOutcome::Complete(value) | SyncOutcome::Partial(value) => value,
            SyncOutcome::Failed | SyncOutcome::AccessBlocked => {
                return Err(ValuationSnapshotError::UnavailablePortfolio);
            }
        };
        let at = self.clock.now();
        let fx = self
            .market
            .fx_rate(FxPair::UsdKrw, FxRateKind::Mid, FxRateQuery::At(at))
            .await
            .map_err(ValuationSnapshotError::Repository)?;
        let snapshot = ValuationSnapshot::observe(calendar, &portfolio.evaluate(fx), at, fx)
            .map_err(ValuationSnapshotError::Snapshot)?;
        let recorded = self
            .repository
            .append(account, &snapshot)
            .await
            .map_err(ValuationSnapshotError::Repository)?;
        let delivery = self
            .events
            .publish(DomainEvent::ValuationSnapshotTaken(account.clone()));
        Ok(ValuationSnapshotReport {
            snapshot,
            recorded,
            delivery,
        })
    }
}
