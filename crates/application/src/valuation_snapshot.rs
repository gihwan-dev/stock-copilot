use crate::{
    Clock, DeliveryReport, EventBus, FxRateQuery, MarketDataRepository, PortfolioRepository,
    RecordOutcome, SystemPortError, ValuationRepository,
};
use domain::{
    AccountId, CalendarDate, Currency, DomainEvent, FxPair, FxRateKind, SnapshotCalendar,
    SnapshotError, SnapshotPriceBasis, SyncOutcome, Timestamp, ValuationSnapshot,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValuationSnapshotError {
    Repository(SystemPortError),
    MissingPortfolio,
    UnavailablePortfolio,
    PriceBasisDefinitionPending,
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
        validate_price_basis(snapshot.price_basis(Currency::Krw))?;
        validate_price_basis(snapshot.price_basis(Currency::Usd))?;
        for holding in portfolio.holdings() {
            validate_observation_at(holding.observed_at(), at)?;
            let price_date = calendar
                .price_date(holding.currency(), holding.observed_at())
                .map_err(ValuationSnapshotError::Snapshot)?;
            validate_price_date(price_date, snapshot.price_date(holding.currency()))?;
        }
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

fn validate_price_basis(basis: SnapshotPriceBasis) -> Result<(), ValuationSnapshotError> {
    match basis {
        SnapshotPriceBasis::CurrentSession => Ok(()),
        SnapshotPriceBasis::AwaitingSession | SnapshotPriceBasis::HolidayCarry => {
            Err(ValuationSnapshotError::PriceBasisDefinitionPending)
        }
    }
}
fn validate_price_date(
    observed: CalendarDate,
    expected: CalendarDate,
) -> Result<(), ValuationSnapshotError> {
    if observed != expected {
        return Err(ValuationSnapshotError::PriceBasisDefinitionPending);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        ValuationSnapshotError, validate_observation_at, validate_price_basis, validate_price_date,
    };
    use domain::{
        Nanosecond, OffsetSeconds, SnapshotError, SnapshotPriceBasis, Timestamp, UnixSeconds,
    };
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
    #[test]
    fn only_current_session_prices_are_supported_for_observations() {
        assert_eq!(
            validate_price_basis(SnapshotPriceBasis::CurrentSession),
            Ok(())
        );
        for basis in [
            SnapshotPriceBasis::AwaitingSession,
            SnapshotPriceBasis::HolidayCarry,
        ] {
            assert_eq!(
                validate_price_basis(basis),
                Err(ValuationSnapshotError::PriceBasisDefinitionPending)
            );
        }
    }
    #[test]
    fn price_date_must_match_in_both_directions() {
        let old = at(11).offset_calendar_date();
        let new = at(86417).offset_calendar_date();
        assert_eq!(validate_price_date(old, old), Ok(()));
        assert_eq!(
            validate_price_date(old, new),
            Err(ValuationSnapshotError::PriceBasisDefinitionPending)
        );
        assert_eq!(
            validate_price_date(new, old),
            Err(ValuationSnapshotError::PriceBasisDefinitionPending)
        );
    }
}
