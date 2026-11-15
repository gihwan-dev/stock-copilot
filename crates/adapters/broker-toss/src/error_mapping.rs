use application::{
    AccessDenial, ClassifiedError, ErrorClass, HttpStatus, MissingData, PortError, PortErrorKind,
    ReadEndpoint, RequestRejection, ResponseMismatch, RetryDelay, UnavailableCause,
};
use broker_toss_schema::types::{ErrorResponse, OAuth2ErrorResponse, OAuth2ErrorResponseError};
use serde_json::Value;
use std::fmt;

use crate::{ApiErrorCode, OAuthErrorCode};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticId(String);

impl DiagnosticId {
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        (!value.is_empty()
            && value.len() <= 128
            && value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"-_.".contains(&byte)))
        .then(|| Self(value.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DiagnosticIds {
    request: Option<DiagnosticId>,
    reference: Option<DiagnosticId>,
}

impl DiagnosticIds {
    fn parse(value: &Value) -> Self {
        Self {
            request: value
                .get("requestId")
                .and_then(Value::as_str)
                .and_then(DiagnosticId::parse),
            reference: value
                .get("referenceId")
                .and_then(Value::as_str)
                .and_then(DiagnosticId::parse),
        }
    }

    #[must_use]
    pub const fn request_id(&self) -> Option<&DiagnosticId> {
        self.request.as_ref()
    }

    #[must_use]
    pub const fn reference_id(&self) -> Option<&DiagnosticId> {
        self.reference.as_ref()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TossErrorBody {
    Missing,
    Malformed,
    Api {
        code: ApiErrorCode,
        diagnostics: DiagnosticIds,
    },
    OAuth {
        code: OAuthErrorCode,
        diagnostics: DiagnosticIds,
    },
}

impl TossErrorBody {
    #[must_use]
    pub fn parse(body: &str) -> Self {
        if body.is_empty() {
            return Self::Missing;
        }
        let Ok(mut value) = serde_json::from_str::<Value>(body) else {
            return Self::Malformed;
        };
        if value.get("error").is_some_and(Value::is_object) {
            let diagnostics = value
                .get("error")
                .map_or_else(DiagnosticIds::default, DiagnosticIds::parse);
            if let Some(object) = value.get_mut("error").and_then(Value::as_object_mut) {
                let _previous = object.insert("requestId".to_owned(), Value::String(String::new()));
            }
            return serde_json::from_value::<ErrorResponse>(value).map_or(
                Self::Malformed,
                |response| Self::Api {
                    code: ApiErrorCode::from(response.error.code.as_str()),
                    diagnostics,
                },
            );
        }
        let diagnostics = DiagnosticIds::parse(&value);
        serde_json::from_value::<OAuth2ErrorResponse>(value).map_or(Self::Malformed, |response| {
            Self::OAuth {
                code: oauth_code(response.error),
                diagnostics,
            }
        })
    }
}

fn oauth_code(code: OAuth2ErrorResponseError) -> OAuthErrorCode {
    match code {
        OAuth2ErrorResponseError::InvalidRequest => OAuthErrorCode::InvalidRequest,
        OAuth2ErrorResponseError::InvalidClient => OAuthErrorCode::InvalidClient,
        OAuth2ErrorResponseError::InvalidGrant => OAuthErrorCode::InvalidGrant,
        OAuth2ErrorResponseError::UnauthorizedClient => OAuthErrorCode::UnauthorizedClient,
        OAuth2ErrorResponseError::UnsupportedGrantType => OAuthErrorCode::UnsupportedGrantType,
        OAuth2ErrorResponseError::AccessDenied => OAuthErrorCode::AccessDenied,
        OAuth2ErrorResponseError::Unknown(_value) => OAuthErrorCode::Unknown,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TossMappedError {
    port: PortError,
    diagnostics: DiagnosticIds,
}

impl TossMappedError {
    #[must_use]
    pub const fn port_error(&self) -> PortError {
        self.port
    }

    #[must_use]
    pub const fn diagnostics(&self) -> &DiagnosticIds {
        &self.diagnostics
    }
}

impl ClassifiedError for TossMappedError {
    fn class(&self) -> ErrorClass {
        self.port.class()
    }
}

impl fmt::Display for TossMappedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("toss read error")
    }
}

impl std::error::Error for TossMappedError {}

#[derive(Debug, Clone, Copy)]
enum StatusRule {
    Exact(HttpStatus),
    Edge,
    Failure,
}

impl StatusRule {
    fn accepts(self, status: HttpStatus) -> bool {
        match self {
            Self::Exact(expected) => expected == status,
            Self::Edge => [
                HttpStatus::Unauthorized,
                HttpStatus::Forbidden,
                HttpStatus::NotFound,
                HttpStatus::UriTooLong,
            ]
            .contains(&status),
            Self::Failure => failure_status(status),
        }
    }
}
fn failure_status(status: HttpStatus) -> bool {
    match status {
        HttpStatus::NotReceived
        | HttpStatus::Informational
        | HttpStatus::Success
        | HttpStatus::Redirect => false,
        HttpStatus::BadRequest
        | HttpStatus::Unauthorized
        | HttpStatus::Forbidden
        | HttpStatus::NotFound
        | HttpStatus::Conflict
        | HttpStatus::UriTooLong
        | HttpStatus::UnsupportedMediaType
        | HttpStatus::Unprocessable
        | HttpStatus::RateLimited
        | HttpStatus::OtherClientError
        | HttpStatus::ServerError => true,
    }
}

fn unavailable(cause: UnavailableCause, retry: RetryDelay) -> PortErrorKind {
    PortErrorKind::Unavailable { cause, retry }
}

fn denied(status: HttpStatus, cause: AccessDenial) -> (StatusRule, PortErrorKind) {
    (
        StatusRule::Exact(status),
        PortErrorKind::AccessDenied(cause),
    )
}
fn rejected(status: HttpStatus, cause: RequestRejection) -> (StatusRule, PortErrorKind) {
    (
        StatusRule::Exact(status),
        PortErrorKind::InvalidRequest(cause),
    )
}
fn missing(data: MissingData) -> (StatusRule, PortErrorKind) {
    (
        StatusRule::Exact(HttpStatus::NotFound),
        PortErrorKind::NotFound(data),
    )
}
fn waiting(
    status: HttpStatus,
    cause: UnavailableCause,
    retry: RetryDelay,
) -> (StatusRule, PortErrorKind) {
    (StatusRule::Exact(status), unavailable(cause, retry))
}
fn mismatch(cause: ResponseMismatch) -> (StatusRule, PortErrorKind) {
    (StatusRule::Failure, PortErrorKind::InvalidResponse(cause))
}
use ApiErrorCode::{
    AccountHeaderRequired, AccountNotFound, AccountRestricted, AlreadyCanceled, AlreadyFilled,
    AlreadyModified, AlreadyProcessing, AlreadyRejected, AmountOrderOutsideRegularHours,
    CancelRestricted, ConditionalOrderNotFound, ConfirmHighValueRequired, EdgeBlocked,
    EdgeRateLimitExceeded, ExchangeRateNotFound, ExpiredToken, Forbidden,
    FractionalQuantityOutsideRegularHours, FractionalQuantityScaleExceeded, IdempotencyKeyConflict,
    InsufficientBuyingPower, InternalError, InvalidRequest, InvalidToken,
    InvestorExchangeNotIntegrated, IpNotAllowed, LoginUserNotFound, Maintenance,
    MarketNotSupportedForStock, MaxOrderAmountExceeded, ModifyRestricted,
    OppositePendingOrderExists, OrderHoursClosed, OrderLimitExceeded, OrderNotFound,
    OrderTypeNotAllowed, PrerequisiteRequired, PriceOutOfRange, RateLimitExceeded,
    RequestInProgress, SectorNotFound, StockNotFound, StockRestricted, TokenRevoked, Unknown,
    UnsupportedContentType, UnsupportedDate, UnsupportedMarket, UnsupportedRankingDuration,
    UnsupportedSymbol, UsModifyQuantityNotSupported,
};

fn api_mapping(code: ApiErrorCode, retry: RetryDelay) -> (StatusRule, PortErrorKind) {
    use HttpStatus::{BadRequest, RateLimited, ServerError, Unauthorized, UnsupportedMediaType};
    match code {
        InvalidRequest => rejected(BadRequest, RequestRejection::MalformedRequest),
        AccountHeaderRequired => rejected(BadRequest, RequestRejection::MissingAccountHeader),
        UnsupportedRankingDuration | UnsupportedSymbol | UnsupportedMarket | UnsupportedDate => {
            rejected(BadRequest, RequestRejection::UnsupportedQuery)
        }
        InvalidToken => rejected(Unauthorized, RequestRejection::InvalidToken),
        ExpiredToken => waiting(Unauthorized, UnavailableCause::ExpiredToken, retry),
        TokenRevoked => denied(Unauthorized, AccessDenial::TokenRevokedRepeated),
        LoginUserNotFound => denied(Unauthorized, AccessDenial::LoginUserNotFound),
        IpNotAllowed => denied(HttpStatus::Forbidden, AccessDenial::IpNotAllowed),
        Forbidden => denied(HttpStatus::Forbidden, AccessDenial::Forbidden),
        AccountNotFound => denied(HttpStatus::NotFound, AccessDenial::AccountNotFound),
        StockNotFound => missing(MissingData::Stock),
        ExchangeRateNotFound => missing(MissingData::ExchangeRate),
        SectorNotFound => missing(MissingData::Sector),
        EdgeBlocked => (
            StatusRule::Edge,
            PortErrorKind::InvalidRequest(RequestRejection::EdgeBlocked),
        ),
        UnsupportedContentType => rejected(
            UnsupportedMediaType,
            RequestRejection::UnsupportedContentType,
        ),
        RateLimitExceeded | EdgeRateLimitExceeded => {
            waiting(RateLimited, UnavailableCause::RateLimited, retry)
        }
        InternalError => waiting(ServerError, UnavailableCause::ServerError, retry),
        Maintenance => waiting(ServerError, UnavailableCause::Maintenance, retry),
        AccountRestricted
        | AlreadyCanceled
        | AlreadyFilled
        | AlreadyModified
        | AlreadyProcessing
        | AlreadyRejected
        | AmountOrderOutsideRegularHours
        | CancelRestricted
        | ConditionalOrderNotFound
        | ConfirmHighValueRequired
        | FractionalQuantityOutsideRegularHours
        | FractionalQuantityScaleExceeded
        | IdempotencyKeyConflict
        | InsufficientBuyingPower
        | InvestorExchangeNotIntegrated
        | MarketNotSupportedForStock
        | MaxOrderAmountExceeded
        | ModifyRestricted
        | OppositePendingOrderExists
        | OrderHoursClosed
        | OrderLimitExceeded
        | OrderNotFound
        | OrderTypeNotAllowed
        | PrerequisiteRequired
        | PriceOutOfRange
        | RequestInProgress
        | StockRestricted
        | UsModifyQuantityNotSupported => mismatch(ResponseMismatch::Validation),
        Unknown => mismatch(ResponseMismatch::Schema),
    }
}

fn oauth_mapping(code: OAuthErrorCode) -> (StatusRule, PortErrorKind) {
    match code {
        OAuthErrorCode::InvalidClient => (
            StatusRule::Exact(HttpStatus::Unauthorized),
            PortErrorKind::AccessDenied(AccessDenial::InvalidCredentials),
        ),
        OAuthErrorCode::InvalidGrant | OAuthErrorCode::UnauthorizedClient => (
            StatusRule::Exact(HttpStatus::BadRequest),
            PortErrorKind::AccessDenied(AccessDenial::InvalidCredentials),
        ),
        OAuthErrorCode::AccessDenied => (
            StatusRule::Exact(HttpStatus::Forbidden),
            PortErrorKind::AccessDenied(AccessDenial::IpNotAllowed),
        ),
        OAuthErrorCode::InvalidRequest | OAuthErrorCode::UnsupportedGrantType => (
            StatusRule::Exact(HttpStatus::BadRequest),
            PortErrorKind::InvalidRequest(RequestRejection::MalformedRequest),
        ),
        OAuthErrorCode::Unknown => (
            StatusRule::Failure,
            PortErrorKind::InvalidResponse(ResponseMismatch::Schema),
        ),
    }
}

fn no_body(status: HttpStatus, retry: RetryDelay) -> PortErrorKind {
    match status {
        HttpStatus::NotReceived => unavailable(UnavailableCause::Network, retry),
        HttpStatus::ServerError => unavailable(UnavailableCause::ServerError, retry),
        HttpStatus::RateLimited => unavailable(UnavailableCause::RateLimited, retry),
        HttpStatus::BadRequest
        | HttpStatus::Unauthorized
        | HttpStatus::Forbidden
        | HttpStatus::NotFound
        | HttpStatus::Conflict
        | HttpStatus::UriTooLong
        | HttpStatus::UnsupportedMediaType
        | HttpStatus::Unprocessable
        | HttpStatus::OtherClientError => PortErrorKind::InvalidResponse(ResponseMismatch::Schema),
        HttpStatus::Informational | HttpStatus::Success | HttpStatus::Redirect => {
            PortErrorKind::InvalidResponse(ResponseMismatch::Validation)
        }
    }
}

#[must_use]
pub fn map_toss_error(
    endpoint: ReadEndpoint,
    status: HttpStatus,
    body: TossErrorBody,
    retry: RetryDelay,
) -> TossMappedError {
    let (rule, kind, diagnostics) = match body {
        TossErrorBody::Missing => {
            return TossMappedError {
                port: PortError::new(endpoint, status, no_body(status, retry)),
                diagnostics: DiagnosticIds::default(),
            };
        }
        TossErrorBody::Malformed => (
            StatusRule::Failure,
            no_body(status, retry),
            DiagnosticIds::default(),
        ),
        TossErrorBody::Api { code, diagnostics } => {
            let (rule, kind) = api_mapping(code, retry);
            (rule, kind, diagnostics)
        }
        TossErrorBody::OAuth { code, diagnostics } => {
            let (rule, kind) = oauth_mapping(code);
            (rule, kind, diagnostics)
        }
    };
    let normalized_kind = if rule.accepts(status) {
        kind
    } else {
        PortErrorKind::InvalidResponse(ResponseMismatch::Validation)
    };
    TossMappedError {
        port: PortError::new(endpoint, status, normalized_kind),
        diagnostics,
    }
}
