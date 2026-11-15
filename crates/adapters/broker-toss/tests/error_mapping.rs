#[cfg(test)]
mod tests {
    use application::{
        BugDiagnostic, ClassifiedError, ErrorClass, HttpStatus, MissingData, PortErrorKind,
        ReadEndpoint, ResponseMismatch, RetryDelay, UnavailableCause,
    };
    use broker_toss::{
        ApiErrorCode, DiagnosticId, OAuthErrorCode, TossErrorBody, map_toss_read_error,
        map_toss_token_error,
    };
    use serde_json::{Value, json};
    use std::collections::BTreeSet;

    fn api(code: &str) -> TossErrorBody {
        TossErrorBody::parse(
            &json!({"error":{"code":code,"message":"SERVER_SECRET_SENTINEL"}}).to_string(),
        )
    }

    fn mapped(code: &str, status: HttpStatus) -> PortErrorKind {
        map_toss_read_error(ReadEndpoint::Quotes, status, api(code), RetryDelay::Unknown)
            .port_error()
            .kind()
    }

    use HttpStatus::{BadRequest, NotFound, Unauthorized};

    fn document_rows(path: &str) -> Vec<(String, HttpStatus, String)> {
        let input = std::fs::read_to_string(path).expect("local error table");
        let mut rows = Vec::new();
        for line in input.lines().filter(|line| line.starts_with('|')) {
            let cells: Vec<_> = line.split('|').map(str::trim).collect();
            let Some(status) = cells.get(1).and_then(|cell| document_status(cell)) else {
                continue;
            };
            let Some(code_cell) = cells.get(2) else {
                continue;
            };
            let label = cells
                .iter()
                .rev()
                .find(|cell| !cell.is_empty())
                .expect("classification");
            for code in backtick_codes(code_cell) {
                rows.push((code, status, (*label).to_owned()));
            }
        }
        assert!(!rows.is_empty(), "document error rows");
        rows
    }

    fn document_status(cell: &str) -> Option<HttpStatus> {
        match cell.split_whitespace().next()? {
            "400" => Some(BadRequest),
            "401" => Some(Unauthorized),
            "403" => Some(HttpStatus::Forbidden),
            "404" => Some(NotFound),
            "409" => Some(HttpStatus::Conflict),
            "414" => Some(HttpStatus::UriTooLong),
            "415" => Some(HttpStatus::UnsupportedMediaType),
            "422" => Some(HttpStatus::Unprocessable),
            "429" => Some(HttpStatus::RateLimited),
            "500" => Some(HttpStatus::ServerError),
            _other => None,
        }
    }

    fn backtick_codes(cell: &str) -> Vec<String> {
        cell.split('`')
            .enumerate()
            .filter(|(position, _token)| !position.is_multiple_of(2))
            .map(|(_position, token)| token)
            .filter(|token| {
                !token.is_empty()
                    && token
                        .bytes()
                        .all(|byte| byte.is_ascii_lowercase() || byte == b'-')
            })
            .map(str::to_owned)
            .collect()
    }

    fn documented_class(label: &str) -> ErrorClass {
        if label.contains("버그") {
            ErrorClass::Bug(BugDiagnostic::Response(ResponseMismatch::Schema))
        } else if label.contains("사용자") {
            ErrorClass::UserAction(application::UserAction::CheckCredentials)
        } else if label.contains("기다리면") {
            ErrorClass::Wait(RetryDelay::Unknown)
        } else if label.contains("데이터 없음") {
            ErrorClass::NoData(MissingData::Stock)
        } else if label.contains("상황별") || label.contains("상태 변화") {
            ErrorClass::Bug(BugDiagnostic::Response(ResponseMismatch::Validation))
        } else {
            panic!("unclassified document row: {label}");
        }
    }

    #[test]
    fn read_codes_follow_the_a01_and_a05_cause_table() {
        let rows = document_rows("../../../docs/research/R-01-toss-open-api.md");
        let mut covered = BTreeSet::new();
        for (code, status, label) in rows {
            let expected = if ORDER_CODES.contains(&code.as_str()) {
                ErrorClass::Bug(BugDiagnostic::Response(ResponseMismatch::Validation))
            } else {
                documented_class(&label)
            };
            let error = map_toss_read_error(
                ReadEndpoint::Quotes,
                status,
                api(&code),
                RetryDelay::Unknown,
            );
            assert_eq!(
                std::mem::discriminant(&error.class()),
                std::mem::discriminant(&expected),
                "{code}: R-01 {label}"
            );
            let _inserted = covered.insert(code);
        }
        for code in ORDER_CODES {
            let error = map_toss_read_error(
                ReadEndpoint::Quotes,
                HttpStatus::Unprocessable,
                api(code),
                RetryDelay::Unknown,
            );
            assert_eq!(
                error.class(),
                ErrorClass::Bug(BugDiagnostic::Response(ResponseMismatch::Validation)),
                "order response {code}"
            );
            let _inserted = covered.insert((*code).to_owned());
        }
        assert_eq!(
            covered,
            ApiErrorCode::known()
                .iter()
                .map(|(code, _variant)| (*code).to_owned())
                .collect(),
            "every known code class tested"
        );
    }

    #[test]
    fn transient_causes_keep_the_injected_retry_delay() {
        let retry = RetryDelay::Known(
            domain::TimeSpan::try_new(domain::SpanSeconds::new(7), domain::Nanosecond::new(25))
                .expect("span"),
        );
        for (code, status, cause) in [
            (
                "expired-token",
                Unauthorized,
                UnavailableCause::ExpiredToken,
            ),
            (
                "rate-limit-exceeded",
                HttpStatus::RateLimited,
                UnavailableCause::RateLimited,
            ),
            (
                "edge-rate-limit-exceeded",
                HttpStatus::RateLimited,
                UnavailableCause::RateLimited,
            ),
            (
                "internal-error",
                HttpStatus::ServerError,
                UnavailableCause::ServerError,
            ),
            (
                "maintenance",
                HttpStatus::ServerError,
                UnavailableCause::Maintenance,
            ),
        ] {
            let error = map_toss_read_error(ReadEndpoint::Holdings, status, api(code), retry);
            assert_eq!(
                error.port_error().kind(),
                PortErrorKind::Unavailable { cause, retry },
                "{code}"
            );
            assert_eq!(error.class(), ErrorClass::Wait(retry), "{code}");
            assert_eq!(
                error.port_error().endpoint(),
                ReadEndpoint::Holdings,
                "endpoint"
            );
            assert_eq!(error.port_error().status(), status, "status");
        }
    }

    const ORDER_CODES: &[&str] = &[
        "account-restricted",
        "insufficient-sellable-quantity",
        "duplicate-conditional-order",
        "condition-already-met",
        "already-canceled",
        "already-filled",
        "already-modified",
        "already-processing",
        "already-rejected",
        "amount-order-outside-regular-hours",
        "cancel-restricted",
        "conditional-order-not-found",
        "confirm-high-value-required",
        "fractional-quantity-outside-regular-hours",
        "fractional-quantity-scale-exceeded",
        "idempotency-key-conflict",
        "insufficient-buying-power",
        "investor-exchange-not-integrated",
        "market-not-supported-for-stock",
        "max-order-amount-exceeded",
        "modify-restricted",
        "opposite-pending-order-exists",
        "order-hours-closed",
        "order-not-found",
        "order-type-not-allowed",
        "prerequisite-required",
        "price-out-of-range",
        "request-in-progress",
        "stock-restricted",
        "us-modify-quantity-not-supported",
        "order-limit-exceeded",
    ];
    #[test]
    fn write_only_codes_are_unexpected_responses_on_the_read_port() {
        for &code in ORDER_CODES {
            assert_ne!(
                ApiErrorCode::from(code),
                ApiErrorCode::Unknown,
                "known {code}"
            );
            let error = map_toss_read_error(
                ReadEndpoint::Accounts,
                HttpStatus::Unprocessable,
                api(code),
                RetryDelay::Unknown,
            );
            assert_eq!(
                error.port_error().kind(),
                PortErrorKind::InvalidResponse(ResponseMismatch::Validation),
                "{code}"
            );
            assert_eq!(
                error.class(),
                ErrorClass::Bug(BugDiagnostic::Response(ResponseMismatch::Validation)),
                "{code}"
            );
        }
    }

    fn description_codes(description: &str, codes: &mut BTreeSet<String>) {
        for (_position, token) in description
            .split('`')
            .enumerate()
            .filter(|(position, _token)| !position.is_multiple_of(2))
        {
            if token.bytes().any(|byte| byte.is_ascii_lowercase())
                && token.contains('-')
                && token
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte == b'-')
            {
                let _inserted = codes.insert(token.to_owned());
            }
        }
    }

    fn collect_spec_codes(value: &Value, codes: &mut BTreeSet<String>) {
        match value {
            Value::Object(object) => {
                if let Some(code) = object.get("code") {
                    collect_code_field(code, codes);
                }
                if let Some(description) = object.get("description").and_then(Value::as_str) {
                    description_codes(description, codes);
                }
                for child in object.values() {
                    collect_spec_codes(child, codes);
                }
            }
            Value::Array(array) => {
                for child in array {
                    collect_spec_codes(child, codes);
                }
            }
            Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
        }
    }

    fn collect_code_field(field: &Value, codes: &mut BTreeSet<String>) {
        if let Some(code) = field.as_str() {
            let _inserted = codes.insert(code.to_owned());
        }
        if let Some(example) = field.get("example").and_then(Value::as_str) {
            let _inserted = codes.insert(example.to_owned());
        }
        if let Some(values) = field.get("enum").and_then(Value::as_array) {
            for value in values {
                let _inserted = codes.insert(value.as_str().expect("code enum string").to_owned());
            }
        }
    }

    #[test]
    fn inventory_extractor_covers_enum_example_and_description_sources() {
        let mut found = BTreeSet::new();
        collect_spec_codes(
            &json!({"code":{"enum":["enum-code"],"example":"example-code"},"description":"`description-code` and `-`"}),
            &mut found,
        );
        assert_eq!(
            found,
            BTreeSet::from([
                "enum-code".to_owned(),
                "example-code".to_owned(),
                "description-code".to_owned()
            ]),
            "all code sources"
        );
    }

    #[test]
    fn success_statuses_reject_every_known_api_code() {
        for &(code, _variant) in ApiErrorCode::known() {
            for status in [
                HttpStatus::Success,
                HttpStatus::Informational,
                HttpStatus::Redirect,
                HttpStatus::NotReceived,
            ] {
                assert_eq!(
                    mapped(code, status),
                    PortErrorKind::InvalidResponse(ResponseMismatch::Validation),
                    "{code} {status:?}"
                );
            }
        }
    }

    #[test]
    fn oauth_unknowns_mismatches_and_diagnostics_are_sanitized() {
        for code in ["future_code", ""] {
            let raw = json!({"error":code,"error_description":"SECRET_DESCRIPTION","error_uri":"SECRET_URI","requestId":"req-1","referenceId":"ref.1"});
            let body = TossErrorBody::parse(&raw.to_string());
            assert!(!format!("{body:?}").contains("SECRET"), "body sanitized");
            let error = map_toss_token_error(
                ReadEndpoint::Accounts,
                HttpStatus::ServerError,
                body,
                RetryDelay::Unknown,
            );
            assert_eq!(
                error.port_error().kind(),
                PortErrorKind::Unavailable {
                    cause: UnavailableCause::ServerError,
                    retry: RetryDelay::Unknown
                },
                "unknown follows HTTP"
            );
            assert_eq!(
                error.diagnostics().request_id().map(DiagnosticId::as_str),
                Some("req-1"),
                "OAuth request"
            );
            assert_eq!(
                error.diagnostics().reference_id().map(DiagnosticId::as_str),
                Some("ref.1"),
                "OAuth reference"
            );
        }
        let body = TossErrorBody::parse(r#"{"error":"invalid_client"}"#);
        let error = map_toss_token_error(
            ReadEndpoint::Accounts,
            HttpStatus::Success,
            body,
            RetryDelay::Unknown,
        );
        assert_eq!(
            error.port_error().kind(),
            PortErrorKind::InvalidResponse(ResponseMismatch::Validation),
            "OAuth success mismatch"
        );
    }

    #[test]
    fn inventories_equal_the_spec_examples_descriptions_and_oauth_enum() {
        let spec: Value = serde_json::from_str(
            &std::fs::read_to_string("../../../docs/toss-openapi-1.2.28.json").expect("local spec"),
        )
        .expect("spec");
        let mut expected = BTreeSet::new();
        collect_spec_codes(&spec, &mut expected);
        assert_eq!(expected.len(), 45, "review code inventory changes");
        for path in [
            "../../../docs/research/R-01-toss-open-api.md",
            "../../../docs/toss-openapi-overview.md",
        ] {
            for (code, _status, _label) in document_rows(path) {
                let _inserted = expected.insert(code);
            }
        }
        assert_eq!(expected.len(), 53, "spec 45 plus document supplements 8");
        let actual: BTreeSet<_> = ApiErrorCode::known()
            .iter()
            .map(|(name, code)| {
                assert_eq!(ApiErrorCode::from(*name), *code, "parser {name}");
                (*name).to_owned()
            })
            .collect();
        assert_eq!(actual, expected, "spec and R-01 inventory");
        let oauth = spec
            .pointer("/components/schemas/OAuth2ErrorResponse/properties/error/enum")
            .and_then(Value::as_array)
            .expect("OAuth enum");
        let mut oauth_expected: BTreeSet<_> = oauth
            .iter()
            .map(|value| value.as_str().expect("code"))
            .collect();
        assert_eq!(oauth_expected.len(), 6, "spec OAuth codes");
        let _server = oauth_expected.insert("server_error");
        let _temporary = oauth_expected.insert("temporarily_unavailable");
        let oauth_actual: BTreeSet<_> = OAuthErrorCode::known()
            .iter()
            .map(|(name, _code)| *name)
            .collect();
        assert_eq!(oauth_actual, oauth_expected, "OAuth inventory");
    }

    #[test]
    fn oauth_has_independent_credentials_ip_and_request_expectations() {
        for (code, status, expected) in [
            (
                "invalid_client",
                Unauthorized,
                "UserAction(CheckCredentials)",
            ),
            ("invalid_grant", BadRequest, "UserAction(CheckCredentials)"),
            (
                "unauthorized_client",
                BadRequest,
                "UserAction(CheckCredentials)",
            ),
            (
                "access_denied",
                HttpStatus::Forbidden,
                "UserAction(CheckIpAllowlist)",
            ),
            (
                "invalid_request",
                BadRequest,
                "Bug(Request(MalformedRequest))",
            ),
            (
                "unsupported_grant_type",
                BadRequest,
                "Bug(Request(MalformedRequest))",
            ),
        ] {
            let body = TossErrorBody::parse(
                &json!({"error":code,"error_description":"OAUTH_SECRET_SENTINEL"}).to_string(),
            );
            let error =
                map_toss_token_error(ReadEndpoint::Accounts, status, body, RetryDelay::Unknown);
            assert_eq!(format!("{:?}", error.class()), expected, "{code}");
            assert!(
                !format!("{error:?} {error}").contains("OAUTH_SECRET_SENTINEL"),
                "no description"
            );
        }
    }

    #[test]
    fn diagnostics_preserve_all_four_combinations_and_validate_boundaries() {
        for (request_value, reference_value) in [
            (None, None),
            (Some("request_1"), None),
            (None, Some("0.edge.1")),
            (Some("request_1"), Some("0.edge.1")),
        ] {
            let mut body =
                json!({"error":{"code":"invalid-request","message":"SERVER_SECRET_SENTINEL"}});
            if let Some(id) = request_value {
                body["error"]["requestId"] = json!(id);
            }
            if let Some(id) = reference_value {
                body["error"]["referenceId"] = json!(id);
            }
            let error = map_toss_read_error(
                ReadEndpoint::Quotes,
                BadRequest,
                TossErrorBody::parse(&body.to_string()),
                RetryDelay::Unknown,
            );
            assert_eq!(
                error.diagnostics().request_id().map(DiagnosticId::as_str),
                request_value,
                "request id"
            );
            assert_eq!(
                error.diagnostics().reference_id().map(DiagnosticId::as_str),
                reference_value,
                "reference"
            );
            assert!(
                !format!("{error:?} {error}").contains("SERVER_SECRET_SENTINEL"),
                "no message"
            );
        }
    }

    #[test]
    fn diagnostic_identifier_validation_is_bounded() {
        for invalid in [
            String::new(),
            "a".repeat(129),
            "a b".to_owned(),
            "a\nb".to_owned(),
            "한글".to_owned(),
            "{}".to_owned(),
            "a/b".to_owned(),
        ] {
            assert_eq!(DiagnosticId::parse(&invalid), None, "invalid identifier");
        }
        for valid in ["a".to_owned(), "a".repeat(128), "A-z_0.9".to_owned()] {
            assert_eq!(
                DiagnosticId::parse(&valid)
                    .as_ref()
                    .map(DiagnosticId::as_str),
                Some(valid.as_str()),
                "valid identifier"
            );
        }
        let error = map_toss_read_error(
            ReadEndpoint::Quotes,
            BadRequest,
            TossErrorBody::parse(
                r#"{"error":{"code":"invalid-request","message":"hidden","requestId":"bad id","referenceId":"valid"}}"#,
            ),
            RetryDelay::Unknown,
        );
        assert_eq!(
            error.diagnostics().request_id(),
            None,
            "invalid request dropped"
        );
        assert_eq!(
            error.diagnostics().reference_id().map(DiagnosticId::as_str),
            Some("valid"),
            "other id kept"
        );
    }

    #[test]
    fn status_mismatches_and_unknown_codes_follow_http() {
        for status in [
            HttpStatus::Success,
            HttpStatus::Informational,
            HttpStatus::Redirect,
            HttpStatus::NotReceived,
            BadRequest,
            HttpStatus::Forbidden,
            NotFound,
            HttpStatus::Conflict,
            HttpStatus::UriTooLong,
            HttpStatus::UnsupportedMediaType,
            HttpStatus::Unprocessable,
            HttpStatus::RateLimited,
            HttpStatus::OtherClientError,
            HttpStatus::ServerError,
        ] {
            assert_eq!(
                mapped("expired-token", status),
                PortErrorKind::InvalidResponse(ResponseMismatch::Validation),
                "mismatch {status:?}"
            );
        }
        for code in ["future-code", "", "SERVER_SECRET_SENTINEL"] {
            assert_eq!(ApiErrorCode::from(code), ApiErrorCode::Unknown, "unknown");
            assert_eq!(
                mapped(code, HttpStatus::ServerError),
                PortErrorKind::Unavailable {
                    cause: UnavailableCause::ServerError,
                    retry: RetryDelay::Unknown
                },
                "unknown code follows status"
            );
        }
    }

    #[test]
    fn missing_or_invalid_bodies_follow_status_fallbacks() {
        for raw in [
            "",
            "<html>SECRET</html>",
            "{}",
            r#"{"error":null}"#,
            r#"{"error":{"code":42,"message":"hidden"}}"#,
        ] {
            for (status, expected) in [
                (
                    HttpStatus::ServerError,
                    PortErrorKind::Unavailable {
                        cause: UnavailableCause::ServerError,
                        retry: RetryDelay::Unknown,
                    },
                ),
                (
                    HttpStatus::RateLimited,
                    PortErrorKind::Unavailable {
                        cause: UnavailableCause::RateLimited,
                        retry: RetryDelay::Unknown,
                    },
                ),
                (
                    BadRequest,
                    PortErrorKind::InvalidResponse(ResponseMismatch::Schema),
                ),
            ] {
                let error = map_toss_read_error(
                    ReadEndpoint::Accounts,
                    status,
                    TossErrorBody::parse(raw),
                    RetryDelay::Unknown,
                );
                assert_eq!(error.port_error().kind(), expected, "{status:?} {raw}");
            }
        }
        let error = map_toss_read_error(
            ReadEndpoint::Accounts,
            HttpStatus::NotReceived,
            TossErrorBody::parse(""),
            RetryDelay::Unknown,
        );
        assert_eq!(
            error.port_error().kind(),
            PortErrorKind::Unavailable {
                cause: UnavailableCause::Network,
                retry: RetryDelay::Unknown
            },
            "no response"
        );
    }
}
