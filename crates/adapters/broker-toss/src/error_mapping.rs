use application::{
    AccessDenial, ClassifiedError, ErrorClass, HttpStatus, MissingData, PortError, PortErrorKind,
    ReadEndpoint, RequestRejection, ResponseMismatch, RetryDelay, UnavailableCause,
};
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
    Malformed {
        diagnostics: DiagnosticIds,
    },
    Api {
        code: ApiErrorCode,
        diagnostics: DiagnosticIds,
        cursor: CursorHint,
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
        let Ok(value) = serde_json::from_str::<Value>(body) else {
            return Self::Malformed {
                diagnostics: DiagnosticIds::default(),
            };
        };
        if let Some(error) = value.get("error").filter(|error| error.is_object()) {
            let diagnostics = DiagnosticIds::parse(error);
            let Some(raw_code) = error.get("code").and_then(Value::as_str) else {
                return Self::Malformed { diagnostics };
            };
            let code = ApiErrorCode::from(raw_code);
            let cursor = if code == InvalidRequest
                && error.pointer("/data/field").and_then(Value::as_str) == Some("cursor")
            {
                CursorHint::Invalid
            } else {
                CursorHint::Unspecified
            };
            return Self::Api {
                code,
                diagnostics,
                cursor,
            };
        }
        let diagnostics = DiagnosticIds::parse(&value);
        match value.get("error").and_then(Value::as_str) {
            Some(code) => Self::OAuth {
                code: OAuthErrorCode::from(code),
                diagnostics,
            },
            None => Self::Malformed { diagnostics },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodeRecognition {
    Known,
    Unknown,
    Absent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorHint {
    Invalid,
    Unspecified,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TossMappedError {
    port: PortError,
    diagnostics: DiagnosticIds,
    recognition: CodeRecognition,
    cursor: CursorHint,
}

impl TossMappedError {
    #[must_use]
    pub const fn code_recognition(&self) -> CodeRecognition {
        self.recognition
    }
    #[must_use]
    pub const fn cursor_hint(&self) -> CursorHint {
        self.cursor
    }
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
    CancelRestricted, ConditionAlreadyMet, ConditionalOrderNotFound, ConfirmHighValueRequired,
    DuplicateConditionalOrder, EdgeBlocked, EdgeRateLimitExceeded, ExchangeRateNotFound,
    ExpiredToken, Forbidden, FractionalQuantityOutsideRegularHours,
    FractionalQuantityScaleExceeded, IdempotencyKeyConflict, InsufficientBuyingPower,
    InsufficientSellableQuantity, InternalError, InvalidRequest, InvalidToken,
    InvestorExchangeNotIntegrated, IpNotAllowed, LoginUserNotFound, Maintenance,
    MarketNotSupportedForStock, MaxOrderAmountExceeded, ModifyRestricted,
    OppositePendingOrderExists, OrderHoursClosed, OrderLimitExceeded, OrderNotFound,
    OrderTypeNotAllowed, PrerequisiteRequired, PriceOutOfRange, RateLimitExceeded,
    RequestInProgress, SectorNotFound, StockNotFound, StockRestricted, TokenRevoked, Unknown,
    UnsupportedContentType, UnsupportedDate, UnsupportedMarket, UnsupportedRankingDuration,
    UnsupportedSymbol, UsModifyQuantityNotSupported,
};

fn edge_blocked() -> (StatusRule, PortErrorKind) {
    (
        StatusRule::Edge,
        PortErrorKind::InvalidRequest(RequestRejection::EdgeBlocked),
    )
}

fn order_not_found_mapping(endpoint: ReadEndpoint) -> (StatusRule, PortErrorKind) {
    if endpoint == ReadEndpoint::Order {
        missing(MissingData::Order)
    } else {
        mismatch(ResponseMismatch::Validation)
    }
}

fn api_mapping(
    endpoint: ReadEndpoint,
    code: ApiErrorCode,
    status: HttpStatus,
    retry: RetryDelay,
) -> (StatusRule, PortErrorKind) {
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
        OrderNotFound => order_not_found_mapping(endpoint),
        EdgeBlocked => edge_blocked(),
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
        | OrderTypeNotAllowed
        | PrerequisiteRequired
        | PriceOutOfRange
        | RequestInProgress
        | StockRestricted
        | UsModifyQuantityNotSupported
        | ConditionAlreadyMet
        | DuplicateConditionalOrder
        | InsufficientSellableQuantity => mismatch(ResponseMismatch::Validation),
        Unknown => (StatusRule::Failure, no_body(status, retry)),
    }
}

fn oauth_mapping(
    code: OAuthErrorCode,
    status: HttpStatus,
    retry: RetryDelay,
) -> (StatusRule, PortErrorKind) {
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
        OAuthErrorCode::ServerError | OAuthErrorCode::TemporarilyUnavailable => (
            StatusRule::Failure,
            unavailable(UnavailableCause::ServerError, retry),
        ),
        OAuthErrorCode::Unknown => (StatusRule::Failure, no_body(status, retry)),
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExpectedEnvelope {
    Read,
    Token,
}

#[must_use]
pub fn map_toss_read_error(
    endpoint: ReadEndpoint,
    status: HttpStatus,
    body: TossErrorBody,
    retry: RetryDelay,
) -> TossMappedError {
    map_body(endpoint, status, body, retry, ExpectedEnvelope::Read)
}

#[must_use]
pub fn map_toss_token_error(
    endpoint: ReadEndpoint,
    status: HttpStatus,
    body: TossErrorBody,
    retry: RetryDelay,
) -> TossMappedError {
    map_body(endpoint, status, body, retry, ExpectedEnvelope::Token)
}

fn api_for_envelope(
    endpoint: ReadEndpoint,
    envelope: ExpectedEnvelope,
    code: ApiErrorCode,
    status: HttpStatus,
    retry: RetryDelay,
) -> (StatusRule, PortErrorKind) {
    match envelope {
        ExpectedEnvelope::Read => api_mapping(endpoint, code, status, retry),
        ExpectedEnvelope::Token => {
            if status == HttpStatus::RateLimited {
                api_mapping(endpoint, code, status, retry)
            } else {
                mismatch(ResponseMismatch::Schema)
            }
        }
    }
}

fn oauth_for_envelope(
    envelope: ExpectedEnvelope,
    code: OAuthErrorCode,
    status: HttpStatus,
    retry: RetryDelay,
) -> (StatusRule, PortErrorKind) {
    match envelope {
        ExpectedEnvelope::Read => mismatch(ResponseMismatch::Schema),
        ExpectedEnvelope::Token => oauth_mapping(code, status, retry),
    }
}

fn map_body(
    endpoint: ReadEndpoint,
    status: HttpStatus,
    body: TossErrorBody,
    retry: RetryDelay,
    envelope: ExpectedEnvelope,
) -> TossMappedError {
    let mut recognition = CodeRecognition::Absent;
    let mut cursor = CursorHint::Unspecified;
    let (rule, kind, diagnostics) = match body {
        TossErrorBody::Missing => {
            return TossMappedError {
                port: PortError::new(endpoint, status, no_body(status, retry)),
                diagnostics: DiagnosticIds::default(),
                recognition,
                cursor,
            };
        }
        TossErrorBody::Malformed { diagnostics } => {
            (StatusRule::Failure, no_body(status, retry), diagnostics)
        }
        TossErrorBody::Api {
            code,
            diagnostics,
            cursor: observed_cursor,
        } => {
            recognition = if code == Unknown {
                CodeRecognition::Unknown
            } else {
                CodeRecognition::Known
            };
            cursor = observed_cursor;
            let (rule, kind) = api_for_envelope(endpoint, envelope, code, status, retry);
            (rule, kind, diagnostics)
        }
        TossErrorBody::OAuth { code, diagnostics } => {
            recognition = if code == OAuthErrorCode::Unknown {
                CodeRecognition::Unknown
            } else {
                CodeRecognition::Known
            };
            let (rule, kind) = oauth_for_envelope(envelope, code, status, retry);
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
        recognition,
        cursor,
    }
}
