use crate::{
    BrokerRead, Clock, DeliveryReport, EventBus, PortError, PortErrorKind, PortfolioRepository,
    SystemPortError,
};
use domain::{
    AccountId, BrokerAccountKeySelection, BuyingPower, CashObservation, CashObservationFailure,
    Currency, DomainEvent, Portfolio, SyncOutcome, SyncRecord, Timestamp,
    portfolio_from_cash_observations, select_brokerage_account_by_id,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortfolioSyncReport {
    pub record: SyncRecord<Portfolio>,
    pub failures: Vec<PortError>,
    pub delivery: DeliveryReport,
}

#[derive(Debug)]
pub struct SyncPortfolio<'a, Broker, Repository, Time> {
    broker: &'a Broker,
    repository: &'a Repository,
    clock: &'a Time,
    events: &'a EventBus,
}
impl<'a, Broker: BrokerRead, Repository: PortfolioRepository, Time: Clock>
    SyncPortfolio<'a, Broker, Repository, Time>
{
    #[must_use]
    pub const fn new(
        broker: &'a Broker,
        repository: &'a Repository,
        clock: &'a Time,
        events: &'a EventBus,
    ) -> Self {
        Self {
            broker,
            repository,
            clock,
            events,
        }
    }
    pub async fn execute(
        &self,
        account: &AccountId,
    ) -> Result<PortfolioSyncReport, SystemPortError> {
        let mut failures = Vec::new();
        let outcome = match self.broker.accounts().await {
            Err(error) => {
                failures.push(error);
                failed_outcome(error)
            }
            Ok(rows) => match select_brokerage_account_by_id(&rows, account) {
                BrokerAccountKeySelection::Selected(_) => {
                    self.read_portfolio(account, &mut failures).await
                }
                BrokerAccountKeySelection::Empty
                | BrokerAccountKeySelection::NotFound
                | BrokerAccountKeySelection::NotBrokerage => SyncOutcome::AccessBlocked,
            },
        };
        let record = SyncRecord::new(self.clock.now(), outcome);
        self.repository.record(account, &record).await?;
        let delivery = self
            .events
            .publish(DomainEvent::PortfolioSyncRecorded(account.clone()));
        Ok(PortfolioSyncReport {
            record,
            failures,
            delivery,
        })
    }
    async fn read_portfolio(
        &self,
        account: &AccountId,
        failures: &mut Vec<PortError>,
    ) -> SyncOutcome<Portfolio> {
        let holdings = self.broker.holdings(account).await;
        let krw = self.broker.buying_power(account, Currency::Krw).await;
        let usd = self.broker.buying_power(account, Currency::Usd).await;
        for error in holdings
            .as_ref()
            .err()
            .into_iter()
            .chain(krw.as_ref().err())
            .chain(usd.as_ref().err())
        {
            failures.push(*error);
        }
        if failures.iter().any(|error| match error.kind() {
            PortErrorKind::AccessDenied(_) => true,
            PortErrorKind::Unavailable { .. }
            | PortErrorKind::NotFound(_)
            | PortErrorKind::InvalidResponse(_)
            | PortErrorKind::Unsupported(_)
            | PortErrorKind::InvalidRequest(_) => false,
        }) {
            return SyncOutcome::AccessBlocked;
        }
        let Ok(observation) = holdings else {
            return SyncOutcome::Failed;
        };
        let observed_at = self.clock.now();
        let (Some(krw_cash), Some(usd_cash)) = (
            cash(krw, Currency::Krw, observed_at),
            cash(usd, Currency::Usd, observed_at),
        ) else {
            return SyncOutcome::Failed;
        };
        match portfolio_from_cash_observations(observation.holdings().to_vec(), krw_cash, usd_cash)
        {
            Err(_) => SyncOutcome::Failed,
            Ok(assembled) => {
                let portfolio = assembled.with_reported(*observation.reported());
                portfolio_outcome(portfolio)
            }
        }
    }
}
fn portfolio_outcome(portfolio: Portfolio) -> SyncOutcome<Portfolio> {
    match (
        portfolio.buying_power(Currency::Krw).money(),
        portfolio.buying_power(Currency::Usd).money(),
    ) {
        (Ok(_), Ok(_)) => SyncOutcome::Complete(portfolio),
        (Err(_), Ok(_) | Err(_)) | (Ok(_), Err(_)) => SyncOutcome::Partial(portfolio),
    }
}

fn failed_outcome(error: PortError) -> SyncOutcome<Portfolio> {
    match error.kind() {
        PortErrorKind::AccessDenied(_) => SyncOutcome::AccessBlocked,
        PortErrorKind::Unavailable { .. }
        | PortErrorKind::NotFound(_)
        | PortErrorKind::InvalidResponse(_)
        | PortErrorKind::Unsupported(_)
        | PortErrorKind::InvalidRequest(_) => SyncOutcome::Failed,
    }
}
fn cash(
    result: Result<BuyingPower, PortError>,
    currency: Currency,
    observed_at: Timestamp,
) -> Option<CashObservation> {
    match result {
        Ok(power) => Some(CashObservation::Observed(power)),
        Err(error) => cash_failure(error.kind()).map(|reason| CashObservation::Failed {
            currency,
            reason,
            observed_at,
        }),
    }
}
fn cash_failure(kind: PortErrorKind) -> Option<CashObservationFailure> {
    match kind {
        PortErrorKind::NotFound(value) => Some(CashObservationFailure::NotFound(match value {
            crate::MissingData::Stock => domain::CashMissingData::Stock,
            crate::MissingData::ExchangeRate => domain::CashMissingData::ExchangeRate,
            crate::MissingData::Sector => domain::CashMissingData::Sector,
            crate::MissingData::Order => return None,
        })),
        PortErrorKind::InvalidResponse(value) => {
            Some(CashObservationFailure::InvalidResponse(match value {
                crate::ResponseMismatch::Schema => domain::CashResponseMismatch::Schema,
                crate::ResponseMismatch::Validation => domain::CashResponseMismatch::Validation,
            }))
        }
        PortErrorKind::InvalidRequest(value) => {
            Some(CashObservationFailure::InvalidRequest(match value {
                crate::RequestRejection::MalformedRequest => {
                    domain::CashRequestRejection::MalformedRequest
                }
                crate::RequestRejection::MissingAccountHeader => {
                    domain::CashRequestRejection::MissingAccountHeader
                }
                crate::RequestRejection::InvalidToken => domain::CashRequestRejection::InvalidToken,
                crate::RequestRejection::EdgeBlocked => domain::CashRequestRejection::EdgeBlocked,
                crate::RequestRejection::UnsupportedQuery => {
                    domain::CashRequestRejection::UnsupportedQuery
                }
                crate::RequestRejection::UnsupportedContentType => {
                    domain::CashRequestRejection::UnsupportedContentType
                }
                crate::RequestRejection::BatchLimitExceeded => {
                    domain::CashRequestRejection::BatchLimitExceeded
                }
            }))
        }
        PortErrorKind::AccessDenied(_) | PortErrorKind::Unsupported(_) => None,
        PortErrorKind::Unavailable { cause, retry } => Some(CashObservationFailure::Unavailable {
            cause: cash_unavailable(cause),
            retry: match retry {
                crate::RetryDelay::Unknown => domain::CashRetryDelay::Unknown,
                crate::RetryDelay::Known(delay) => domain::CashRetryDelay::Known(delay),
            },
        }),
    }
}

fn cash_unavailable(cause: crate::UnavailableCause) -> domain::CashUnavailableCause {
    match cause {
        crate::UnavailableCause::RateLimited => domain::CashUnavailableCause::RateLimited,
        crate::UnavailableCause::Maintenance => domain::CashUnavailableCause::Maintenance,
        crate::UnavailableCause::ServerError => domain::CashUnavailableCause::ServerError,
        crate::UnavailableCause::Network => domain::CashUnavailableCause::Network,
        crate::UnavailableCause::ExpiredToken => domain::CashUnavailableCause::ExpiredToken,
    }
}

#[cfg(test)]
mod tests {
    use super::{cash, cash_failure, portfolio_outcome};
    use crate::{HttpStatus, PortError, PortErrorKind, ReadEndpoint, RetryDelay, UnavailableCause};
    use domain::{
        BuyingPower, CashObservation, CashObservationFailure, CashRetryDelay, CashUnavailableCause,
        Currency, Money, Nanosecond, NumericInput, OffsetSeconds, Portfolio, SyncOutcome,
        Timestamp, UnixSeconds,
    };

    fn at() -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(137),
            Nanosecond::new(29),
            OffsetSeconds::new(0),
        )
        .expect("시각")
    }
    fn power(value: &str, currency: Currency) -> BuyingPower {
        BuyingPower::new(
            Money::parse(&NumericInput::try_new(value).expect("입력"), currency).expect("금액"),
            at(),
        )
    }
    #[test]
    fn cash_conversion_preserves_success_and_failed_currency_time_and_delay() {
        for currency in [Currency::Krw, Currency::Usd] {
            let value = power("31", currency);
            assert_eq!(
                cash(Ok(value), currency, at()),
                Some(CashObservation::Observed(value))
            );
            let original = PortError::new(
                ReadEndpoint::BuyingPower,
                HttpStatus::RateLimited,
                PortErrorKind::Unavailable {
                    cause: UnavailableCause::RateLimited,
                    retry: RetryDelay::Known(domain::TimeSpan::NANOSECOND),
                },
            );
            let expected = CashObservationFailure::Unavailable {
                cause: CashUnavailableCause::RateLimited,
                retry: CashRetryDelay::Known(domain::TimeSpan::NANOSECOND),
            };
            assert_eq!(cash_failure(original.kind()), Some(expected));
            assert_eq!(
                cash(Err(original), currency, at()),
                Some(CashObservation::Failed {
                    currency,
                    reason: expected,
                    observed_at: at()
                })
            );
        }
    }
    #[test]
    fn only_two_valid_currencies_form_a_complete_outcome() {
        let complete = Portfolio::new(
            vec![],
            power("43", Currency::Krw),
            power("17", Currency::Usd),
        )
        .expect("정상");
        assert_eq!(
            portfolio_outcome(complete.clone()),
            SyncOutcome::Complete(complete)
        );
        for (krw, usd) in [("-5", "17"), ("43", "-7"), ("-5", "-7")] {
            let partial =
                Portfolio::new(vec![], power(krw, Currency::Krw), power(usd, Currency::Usd))
                    .expect("오류 보존");
            assert_eq!(
                portfolio_outcome(partial.clone()),
                SyncOutcome::Partial(partial)
            );
        }
    }
}
