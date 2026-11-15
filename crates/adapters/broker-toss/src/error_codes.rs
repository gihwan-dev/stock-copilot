#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApiErrorCode {
    AccountHeaderRequired,
    AccountNotFound,
    AccountRestricted,
    AlreadyCanceled,
    AlreadyFilled,
    AlreadyModified,
    AlreadyProcessing,
    AlreadyRejected,
    AmountOrderOutsideRegularHours,
    CancelRestricted,
    ConditionalOrderNotFound,
    ConfirmHighValueRequired,
    ConditionAlreadyMet,
    DuplicateConditionalOrder,
    InsufficientSellableQuantity,
    EdgeBlocked,
    EdgeRateLimitExceeded,
    ExchangeRateNotFound,
    ExpiredToken,
    Forbidden,
    FractionalQuantityOutsideRegularHours,
    FractionalQuantityScaleExceeded,
    IdempotencyKeyConflict,
    InsufficientBuyingPower,
    InternalError,
    InvalidRequest,
    InvalidToken,
    InvestorExchangeNotIntegrated,
    IpNotAllowed,
    LoginUserNotFound,
    Maintenance,
    MarketNotSupportedForStock,
    MaxOrderAmountExceeded,
    ModifyRestricted,
    OppositePendingOrderExists,
    OrderHoursClosed,
    OrderLimitExceeded,
    OrderNotFound,
    OrderTypeNotAllowed,
    PrerequisiteRequired,
    PriceOutOfRange,
    RateLimitExceeded,
    RequestInProgress,
    SectorNotFound,
    StockNotFound,
    StockRestricted,
    TokenRevoked,
    UnsupportedContentType,
    UnsupportedDate,
    UnsupportedMarket,
    UnsupportedRankingDuration,
    UnsupportedSymbol,
    UsModifyQuantityNotSupported,
    Unknown,
}
impl ApiErrorCode {
    const KNOWN: &'static [(&'static str, Self)] = &[
        ("account-header-required", Self::AccountHeaderRequired),
        ("account-not-found", Self::AccountNotFound),
        ("account-restricted", Self::AccountRestricted),
        ("already-canceled", Self::AlreadyCanceled),
        ("already-filled", Self::AlreadyFilled),
        ("already-modified", Self::AlreadyModified),
        ("already-processing", Self::AlreadyProcessing),
        ("already-rejected", Self::AlreadyRejected),
        (
            "amount-order-outside-regular-hours",
            Self::AmountOrderOutsideRegularHours,
        ),
        ("cancel-restricted", Self::CancelRestricted),
        (
            "conditional-order-not-found",
            Self::ConditionalOrderNotFound,
        ),
        (
            "confirm-high-value-required",
            Self::ConfirmHighValueRequired,
        ),
        ("condition-already-met", Self::ConditionAlreadyMet),
        (
            "duplicate-conditional-order",
            Self::DuplicateConditionalOrder,
        ),
        (
            "insufficient-sellable-quantity",
            Self::InsufficientSellableQuantity,
        ),
        ("edge-blocked", Self::EdgeBlocked),
        ("edge-rate-limit-exceeded", Self::EdgeRateLimitExceeded),
        ("exchange-rate-not-found", Self::ExchangeRateNotFound),
        ("expired-token", Self::ExpiredToken),
        ("forbidden", Self::Forbidden),
        (
            "fractional-quantity-outside-regular-hours",
            Self::FractionalQuantityOutsideRegularHours,
        ),
        (
            "fractional-quantity-scale-exceeded",
            Self::FractionalQuantityScaleExceeded,
        ),
        ("idempotency-key-conflict", Self::IdempotencyKeyConflict),
        ("insufficient-buying-power", Self::InsufficientBuyingPower),
        ("internal-error", Self::InternalError),
        ("invalid-request", Self::InvalidRequest),
        ("invalid-token", Self::InvalidToken),
        (
            "investor-exchange-not-integrated",
            Self::InvestorExchangeNotIntegrated,
        ),
        ("ip-not-allowed", Self::IpNotAllowed),
        ("login-user-not-found", Self::LoginUserNotFound),
        ("maintenance", Self::Maintenance),
        (
            "market-not-supported-for-stock",
            Self::MarketNotSupportedForStock,
        ),
        ("max-order-amount-exceeded", Self::MaxOrderAmountExceeded),
        ("modify-restricted", Self::ModifyRestricted),
        (
            "opposite-pending-order-exists",
            Self::OppositePendingOrderExists,
        ),
        ("order-hours-closed", Self::OrderHoursClosed),
        ("order-limit-exceeded", Self::OrderLimitExceeded),
        ("order-not-found", Self::OrderNotFound),
        ("order-type-not-allowed", Self::OrderTypeNotAllowed),
        ("prerequisite-required", Self::PrerequisiteRequired),
        ("price-out-of-range", Self::PriceOutOfRange),
        ("rate-limit-exceeded", Self::RateLimitExceeded),
        ("request-in-progress", Self::RequestInProgress),
        ("sector-not-found", Self::SectorNotFound),
        ("stock-not-found", Self::StockNotFound),
        ("stock-restricted", Self::StockRestricted),
        ("token-revoked", Self::TokenRevoked),
        ("unsupported-content-type", Self::UnsupportedContentType),
        ("unsupported-date", Self::UnsupportedDate),
        ("unsupported-market", Self::UnsupportedMarket),
        (
            "unsupported-ranking-duration",
            Self::UnsupportedRankingDuration,
        ),
        ("unsupported-symbol", Self::UnsupportedSymbol),
        (
            "us-modify-quantity-not-supported",
            Self::UsModifyQuantityNotSupported,
        ),
    ];
    #[must_use]
    pub const fn known() -> &'static [(&'static str, Self)] {
        Self::KNOWN
    }
}
impl From<&str> for ApiErrorCode {
    fn from(value: &str) -> Self {
        Self::known()
            .iter()
            .find_map(|(name, code)| (*name == value).then_some(*code))
            .unwrap_or(Self::Unknown)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OAuthErrorCode {
    ServerError,
    TemporarilyUnavailable,
    AccessDenied,
    InvalidClient,
    InvalidGrant,
    InvalidRequest,
    UnauthorizedClient,
    UnsupportedGrantType,
    Unknown,
}
impl OAuthErrorCode {
    const KNOWN: &'static [(&'static str, Self)] = &[
        ("server_error", Self::ServerError),
        ("temporarily_unavailable", Self::TemporarilyUnavailable),
        ("access_denied", Self::AccessDenied),
        ("invalid_client", Self::InvalidClient),
        ("invalid_grant", Self::InvalidGrant),
        ("invalid_request", Self::InvalidRequest),
        ("unauthorized_client", Self::UnauthorizedClient),
        ("unsupported_grant_type", Self::UnsupportedGrantType),
    ];
    #[must_use]
    pub const fn known() -> &'static [(&'static str, Self)] {
        Self::KNOWN
    }
}
impl From<&str> for OAuthErrorCode {
    fn from(value: &str) -> Self {
        Self::known()
            .iter()
            .find_map(|(name, code)| (*name == value).then_some(*code))
            .unwrap_or(Self::Unknown)
    }
}
