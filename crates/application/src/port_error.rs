use crate::BrokerCapability;
use domain::TimeSpan;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadEndpoint {
    Accounts,
    Holdings,
    Transactions,
    Quotes,
    Candles,
    BuyingPower,
    FxRate,
    OrderFxRates,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpStatus {
    NotReceived,
    Informational,
    Success,
    Redirect,
    BadRequest,
    Unauthorized,
    Forbidden,
    NotFound,
    Conflict,
    UriTooLong,
    UnsupportedMediaType,
    Unprocessable,
    RateLimited,
    OtherClientError,
    ServerError,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessDenial {
    InvalidCredentials,
    IpNotAllowed,
    TokenRevokedRepeated,
    AccountNotFound,
    Forbidden,
    LoginUserNotFound,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnavailableCause {
    RateLimited,
    Maintenance,
    ServerError,
    Network,
    ExpiredToken,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetryDelay {
    Unknown,
    Known(TimeSpan),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MissingData {
    Stock,
    ExchangeRate,
    Sector,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResponseMismatch {
    Schema,
    Validation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestRejection {
    MalformedRequest,
    MissingAccountHeader,
    InvalidToken,
    EdgeBlocked,
    UnsupportedQuery,
    UnsupportedContentType,
    BatchLimitExceeded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortErrorKind {
    AccessDenied(AccessDenial),
    Unavailable {
        cause: UnavailableCause,
        retry: RetryDelay,
    },
    NotFound(MissingData),
    InvalidResponse(ResponseMismatch),
    Unsupported(BrokerCapability),
    InvalidRequest(RequestRejection),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PortError {
    endpoint: ReadEndpoint,
    status: HttpStatus,
    kind: PortErrorKind,
}

impl PortError {
    #[must_use]
    pub const fn new(endpoint: ReadEndpoint, status: HttpStatus, kind: PortErrorKind) -> Self {
        Self {
            endpoint,
            status,
            kind,
        }
    }

    #[must_use]
    pub const fn endpoint(self) -> ReadEndpoint {
        self.endpoint
    }

    #[must_use]
    pub const fn status(self) -> HttpStatus {
        self.status
    }

    #[must_use]
    pub const fn kind(self) -> PortErrorKind {
        self.kind
    }
}
