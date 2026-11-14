use crate::{Currency, PortfolioError, PortfolioValuation, TimeSpan};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CashAccessDenial {
    InvalidCredentials,
    IpNotAllowed,
    TokenRevokedRepeated,
    AccountNotFound,
    Forbidden,
    LoginUserNotFound,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CashUnavailableCause {
    RateLimited,
    Maintenance,
    ServerError,
    Network,
    ExpiredToken,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CashRetryDelay {
    Unknown,
    Known(TimeSpan),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CashMissingData {
    Stock,
    ExchangeRate,
    Sector,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CashResponseMismatch {
    Schema,
    Validation,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CashRequestRejection {
    MalformedRequest,
    MissingAccountHeader,
    InvalidToken,
    EdgeBlocked,
    UnsupportedQuery,
    UnsupportedContentType,
    BatchLimitExceeded,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CashReadCapability {
    DailyCandles,
    MinuteCandles,
    AdjustedHistory,
    RawHistory,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CashObservationFailure {
    AccessDenied(CashAccessDenial),
    Unavailable {
        cause: CashUnavailableCause,
        retry: CashRetryDelay,
    },
    NotFound(CashMissingData),
    InvalidResponse(CashResponseMismatch),
    Unsupported(CashReadCapability),
    InvalidRequest(CashRequestRejection),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CashObservationFailures {
    Krw(CashObservationFailure),
    Usd(CashObservationFailure),
    Both {
        krw: CashObservationFailure,
        usd: CashObservationFailure,
    },
}
#[must_use]
pub fn cash_observation_failures(
    valuation: &PortfolioValuation,
) -> Option<CashObservationFailures> {
    let krw_failure = lookup_failure(valuation, Currency::Krw);
    let usd_failure = lookup_failure(valuation, Currency::Usd);
    match (krw_failure, usd_failure) {
        (Some(krw), Some(usd)) => Some(CashObservationFailures::Both { krw, usd }),
        (Some(reason), None) => Some(CashObservationFailures::Krw(reason)),
        (None, Some(reason)) => Some(CashObservationFailures::Usd(reason)),
        (None, None) => None,
    }
}
fn lookup_failure(
    valuation: &PortfolioValuation,
    currency: Currency,
) -> Option<CashObservationFailure> {
    match valuation.buying_power(currency).money() {
        Err(PortfolioError::CashLookupFailed(reason)) => Some(reason),
        Ok(_)
        | Err(
            PortfolioError::TooManyHoldings
            | PortfolioError::DuplicateInstrument
            | PortfolioError::CurrencyMismatch
            | PortfolioError::NegativeBuyingPower,
        ) => None,
    }
}
