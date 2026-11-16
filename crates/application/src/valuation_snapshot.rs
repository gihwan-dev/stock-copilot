use crate::{
    Clock, DeliveryReport, EventBus, FxRateQuery, MarketDataRepository, PortfolioRepository,
    RecordOutcome, SystemPortError, ValuationRepository,
};
use domain::{
    AccountId, DomainEvent, FxPair, FxRateKind, SnapshotCalendar, SnapshotError, SyncOutcome,
    Timestamp, ValuationSnapshot,
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
        let at = validate_observation_at(record.recorded_at(), self.clock.now())?;
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

fn validate_observation_at(
    at: Timestamp,
    now: Timestamp,
) -> Result<Timestamp, ValuationSnapshotError> {
    if at > now {
        return Err(ValuationSnapshotError::Snapshot(SnapshotError::WrongPeriod));
    }
    Ok(at)
}

#[cfg(test)]
mod tests {
    use super::{ValuationSnapshotError, validate_observation_at};
    use domain::{Nanosecond, OffsetSeconds, SnapshotError, Timestamp, UnixSeconds};
    fn at(seconds: i64) -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(seconds),
            Nanosecond::new(37),
            OffsetSeconds::new(0),
        )
        .expect("시각")
    }
    #[test]
    fn observation_is_preserved_before_or_at_now_and_rejected_in_the_future() {
        assert_eq!(validate_observation_at(at(11), at(17)), Ok(at(11)));
        assert_eq!(validate_observation_at(at(17), at(17)), Ok(at(17)));
        assert_eq!(
            validate_observation_at(at(23), at(17)),
            Err(ValuationSnapshotError::Snapshot(SnapshotError::WrongPeriod))
        );
    }
}
