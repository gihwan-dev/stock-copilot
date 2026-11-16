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
                if failures.is_empty()
                    && portfolio.buying_power(Currency::Krw).money().is_ok()
                    && portfolio.buying_power(Currency::Usd).money().is_ok()
                {
                    SyncOutcome::Complete(portfolio)
                } else {
                    SyncOutcome::Partial(portfolio)
                }
            }
        }
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
        PortErrorKind::AccessDenied(value) => {
            Some(CashObservationFailure::AccessDenied(match value {
                crate::AccessDenial::InvalidCredentials => {
                    domain::CashAccessDenial::InvalidCredentials
                }
                crate::AccessDenial::IpNotAllowed => domain::CashAccessDenial::IpNotAllowed,
                crate::AccessDenial::TokenRevokedRepeated => {
                    domain::CashAccessDenial::TokenRevokedRepeated
                }
                crate::AccessDenial::AccountNotFound => domain::CashAccessDenial::AccountNotFound,
                crate::AccessDenial::Forbidden => domain::CashAccessDenial::Forbidden,
                crate::AccessDenial::LoginUserNotFound => {
                    domain::CashAccessDenial::LoginUserNotFound
                }
            }))
        }
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
        PortErrorKind::Unsupported(_) => None,
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
