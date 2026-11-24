use broker_toss::{ApiErrorCode, TossErrorBody};
use domain::{BrokerOrderId, ClientOrderId};
use order_execution::{OrderRejectionReason, OrderUnknownCause};
use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SubmitClassification {
    Accepted(BrokerOrderId),
    Rejected(OrderRejectionReason),
    Unknown(OrderUnknownCause),
    NotSentAuthentication,
    NotSentInvalidRequest,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreatedOrder {
    order_id: String,
    client_order_id: Option<String>,
}

#[derive(Deserialize)]
struct CreatedEnvelope {
    result: CreatedOrder,
}

fn accepted(body: &[u8], client_order_id: &ClientOrderId) -> SubmitClassification {
    let Ok(envelope) = serde_json::from_slice::<CreatedEnvelope>(body) else {
        return SubmitClassification::Unknown(OrderUnknownCause::InvalidResponse);
    };
    if envelope
        .result
        .client_order_id
        .as_deref()
        .is_some_and(|echoed| echoed != client_order_id.as_str())
    {
        return SubmitClassification::Unknown(OrderUnknownCause::InvalidResponse);
    }
    match BrokerOrderId::try_new(&envelope.result.order_id) {
        Ok(order_id) => SubmitClassification::Accepted(order_id),
        Err(_error) => SubmitClassification::Unknown(OrderUnknownCause::InvalidResponse),
    }
}

fn error_code(body: &[u8]) -> Option<ApiErrorCode> {
    let text = std::str::from_utf8(body).ok()?;
    match TossErrorBody::parse(text) {
        TossErrorBody::Api { code, .. } => Some(code),
        TossErrorBody::Missing | TossErrorBody::Malformed { .. } | TossErrorBody::OAuth { .. } => {
            None
        }
    }
}

const REJECTIONS: &[(ApiErrorCode, OrderRejectionReason)] = &[
    (
        ApiErrorCode::ConfirmHighValueRequired,
        OrderRejectionReason::ConfirmHighValueRequired,
    ),
    (
        ApiErrorCode::AccountHeaderRequired,
        OrderRejectionReason::AccountHeaderRequired,
    ),
    (
        ApiErrorCode::OppositePendingOrderExists,
        OrderRejectionReason::OppositePendingOrderExists,
    ),
    (
        ApiErrorCode::InsufficientBuyingPower,
        OrderRejectionReason::InsufficientBuyingPower,
    ),
    (
        ApiErrorCode::InsufficientSellableQuantity,
        OrderRejectionReason::InsufficientSellableQuantity,
    ),
    (
        ApiErrorCode::OrderHoursClosed,
        OrderRejectionReason::OrderHoursClosed,
    ),
    (
        ApiErrorCode::StockRestricted,
        OrderRejectionReason::StockRestricted,
    ),
    (
        ApiErrorCode::PriceOutOfRange,
        OrderRejectionReason::PriceOutOfRange,
    ),
    (
        ApiErrorCode::OrderTypeNotAllowed,
        OrderRejectionReason::OrderTypeNotAllowed,
    ),
    (
        ApiErrorCode::PrerequisiteRequired,
        OrderRejectionReason::PrerequisiteRequired,
    ),
    (
        ApiErrorCode::MarketNotSupportedForStock,
        OrderRejectionReason::MarketNotSupportedForStock,
    ),
    (
        ApiErrorCode::InvestorExchangeNotIntegrated,
        OrderRejectionReason::InvestorExchangeNotIntegrated,
    ),
    (
        ApiErrorCode::AmountOrderOutsideRegularHours,
        OrderRejectionReason::AmountOrderOutsideRegularHours,
    ),
    (
        ApiErrorCode::FractionalQuantityOutsideRegularHours,
        OrderRejectionReason::FractionalQuantityOutsideRegularHours,
    ),
    (
        ApiErrorCode::AccountRestricted,
        OrderRejectionReason::AccountRestricted,
    ),
    (
        ApiErrorCode::MaxOrderAmountExceeded,
        OrderRejectionReason::MaxOrderAmountExceeded,
    ),
    (
        ApiErrorCode::IdempotencyKeyConflict,
        OrderRejectionReason::IdempotencyKeyConflict,
    ),
    (
        ApiErrorCode::InvalidRequest,
        OrderRejectionReason::InvalidRequest,
    ),
    (
        ApiErrorCode::FractionalQuantityScaleExceeded,
        OrderRejectionReason::InvalidRequest,
    ),
    (
        ApiErrorCode::InvalidToken,
        OrderRejectionReason::AuthenticationRequired,
    ),
    (
        ApiErrorCode::ExpiredToken,
        OrderRejectionReason::AuthenticationRequired,
    ),
    (
        ApiErrorCode::TokenRevoked,
        OrderRejectionReason::AuthenticationRequired,
    ),
    (
        ApiErrorCode::Forbidden,
        OrderRejectionReason::AccessForbidden,
    ),
    (
        ApiErrorCode::IpNotAllowed,
        OrderRejectionReason::AccessForbidden,
    ),
    (
        ApiErrorCode::EdgeBlocked,
        OrderRejectionReason::AccessForbidden,
    ),
    (
        ApiErrorCode::RateLimitExceeded,
        OrderRejectionReason::RateLimited,
    ),
    (
        ApiErrorCode::EdgeRateLimitExceeded,
        OrderRejectionReason::RateLimited,
    ),
];

fn rejection(code: ApiErrorCode) -> OrderRejectionReason {
    REJECTIONS
        .iter()
        .find(|(known, _)| *known == code)
        .map_or(OrderRejectionReason::OtherRejected, |(_, reason)| *reason)
}

pub(crate) fn classify(
    status: u16,
    body: &[u8],
    client_order_id: &ClientOrderId,
) -> SubmitClassification {
    let code = error_code(body);
    match status {
        200 | 201 => accepted(body, client_order_id),
        401 => SubmitClassification::Rejected(OrderRejectionReason::AuthenticationRequired),
        403 => SubmitClassification::Rejected(OrderRejectionReason::AccessForbidden),
        429 => SubmitClassification::Rejected(OrderRejectionReason::RateLimited),
        409 if code == Some(ApiErrorCode::RequestInProgress) => {
            SubmitClassification::Unknown(OrderUnknownCause::RequestInProgress)
        }
        500..=599 if code == Some(ApiErrorCode::Maintenance) => {
            SubmitClassification::Unknown(OrderUnknownCause::Maintenance)
        }
        500..=599 => SubmitClassification::Unknown(OrderUnknownCause::ServerError),
        400..=499 => SubmitClassification::Rejected(
            code.map_or(OrderRejectionReason::OtherRejected, rejection),
        ),
        _ => SubmitClassification::Unknown(OrderUnknownCause::InvalidResponse),
    }
}

#[cfg(test)]
mod tests {
    use super::{SubmitClassification, classify};
    use domain::{BrokerOrderId, ClientOrderId};
    use order_execution::{OrderRejectionReason, OrderUnknownCause};

    fn key() -> ClientOrderId {
        ClientOrderId::try_new("order-key-7").expect("키")
    }

    fn error(code: &str) -> Vec<u8> {
        format!(r#"{{"error":{{"requestId":"req-1","code":"{code}","message":"x"}}}}"#).into_bytes()
    }

    #[test]
    fn accepted_response_needs_a_matching_echo_and_order_id() {
        assert_eq!(
            classify(
                200,
                br#"{"result":{"orderId":"toss-ord-1","clientOrderId":"order-key-7"}}"#,
                &key()
            ),
            SubmitClassification::Accepted(BrokerOrderId::try_new("toss-ord-1").expect("번호"))
        );
        for body in [
            br#"{"result":{"orderId":"toss-ord-1","clientOrderId":"other-key"}}"#.as_slice(),
            br#"{"result":{}}"#.as_slice(),
            b"not-json".as_slice(),
            br#"{"result":{"orderId":""}}"#.as_slice(),
        ] {
            assert_eq!(
                classify(200, body, &key()),
                SubmitClassification::Unknown(OrderUnknownCause::InvalidResponse)
            );
        }
    }

    const CASES: &[(u16, &str, SubmitClassification)] = &[
        (
            400,
            "confirm-high-value-required",
            SubmitClassification::Rejected(OrderRejectionReason::ConfirmHighValueRequired),
        ),
        (
            400,
            "invalid-request",
            SubmitClassification::Rejected(OrderRejectionReason::InvalidRequest),
        ),
        (
            400,
            "account-header-required",
            SubmitClassification::Rejected(OrderRejectionReason::AccountHeaderRequired),
        ),
        (
            401,
            "invalid-token",
            SubmitClassification::Rejected(OrderRejectionReason::AuthenticationRequired),
        ),
        (
            403,
            "forbidden",
            SubmitClassification::Rejected(OrderRejectionReason::AccessForbidden),
        ),
        (
            409,
            "opposite-pending-order-exists",
            SubmitClassification::Rejected(OrderRejectionReason::OppositePendingOrderExists),
        ),
        (
            409,
            "request-in-progress",
            SubmitClassification::Unknown(OrderUnknownCause::RequestInProgress),
        ),
        (
            422,
            "insufficient-buying-power",
            SubmitClassification::Rejected(OrderRejectionReason::InsufficientBuyingPower),
        ),
        (
            422,
            "idempotency-key-conflict",
            SubmitClassification::Rejected(OrderRejectionReason::IdempotencyKeyConflict),
        ),
        (
            422,
            "max-order-amount-exceeded",
            SubmitClassification::Rejected(OrderRejectionReason::MaxOrderAmountExceeded),
        ),
        (
            422,
            "something-new",
            SubmitClassification::Rejected(OrderRejectionReason::OtherRejected),
        ),
        (
            429,
            "rate-limit-exceeded",
            SubmitClassification::Rejected(OrderRejectionReason::RateLimited),
        ),
        (
            500,
            "maintenance",
            SubmitClassification::Unknown(OrderUnknownCause::Maintenance),
        ),
        (
            500,
            "internal-error",
            SubmitClassification::Unknown(OrderUnknownCause::ServerError),
        ),
        (
            503,
            "x",
            SubmitClassification::Unknown(OrderUnknownCause::ServerError),
        ),
        (
            302,
            "x",
            SubmitClassification::Unknown(OrderUnknownCause::InvalidResponse),
        ),
    ];

    #[test]
    fn documented_errors_map_to_definite_rejections_or_unknown() {
        for (status, code, expected) in CASES {
            assert_eq!(
                classify(*status, &error(code), &key()),
                *expected,
                "{status} {code}"
            );
        }
        assert_eq!(
            classify(409, b"", &key()),
            SubmitClassification::Rejected(OrderRejectionReason::OtherRejected)
        );
    }
}
