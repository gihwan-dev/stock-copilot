#[cfg(test)]
mod tests {
    use application::{
        ClassifiedError, ErrorClass, HttpStatus, PortErrorKind, ReadEndpoint, ResponseMismatch,
        RetryDelay, UnavailableCause,
    };
    use broker_toss::{
        ApiErrorCode, CodeRecognition, CursorHint, DiagnosticId, TossErrorBody,
        map_toss_read_error, map_toss_token_error,
    };
    use serde_json::json;

    #[test]
    fn user_actions_and_missing_data_keep_the_a05_payload() {
        use application::{MissingData, UserAction};
        for (code, status, expected) in [
            (
                "account-not-found",
                HttpStatus::NotFound,
                ErrorClass::UserAction(UserAction::CheckAccount),
            ),
            (
                "forbidden",
                HttpStatus::Forbidden,
                ErrorClass::UserAction(UserAction::CheckPermissions),
            ),
            (
                "ip-not-allowed",
                HttpStatus::Forbidden,
                ErrorClass::UserAction(UserAction::CheckIpAllowlist),
            ),
            (
                "login-user-not-found",
                HttpStatus::Unauthorized,
                ErrorClass::UserAction(UserAction::CheckCredentials),
            ),
            (
                "token-revoked",
                HttpStatus::Unauthorized,
                ErrorClass::UserAction(UserAction::StopCompetingKeyUse),
            ),
            (
                "stock-not-found",
                HttpStatus::NotFound,
                ErrorClass::NoData(MissingData::Stock),
            ),
            (
                "exchange-rate-not-found",
                HttpStatus::NotFound,
                ErrorClass::NoData(MissingData::ExchangeRate),
            ),
            (
                "sector-not-found",
                HttpStatus::NotFound,
                ErrorClass::NoData(MissingData::Sector),
            ),
        ] {
            let raw = json!({"error":{"code":code}}).to_string();
            let error = map_toss_read_error(
                ReadEndpoint::Quotes,
                status,
                TossErrorBody::parse(&raw),
                RetryDelay::Unknown,
            );
            assert_eq!(error.class(), expected, "A-05 payload {code}");
        }
    }

    #[test]
    fn request_bug_diagnostics_keep_the_documented_distinctions() {
        for (code, status, expected) in [
            (
                "invalid-request",
                HttpStatus::BadRequest,
                "Bug(Request(MalformedRequest))",
            ),
            (
                "account-header-required",
                HttpStatus::BadRequest,
                "Bug(Request(MissingAccountHeader))",
            ),
            (
                "invalid-token",
                HttpStatus::Unauthorized,
                "Bug(Request(InvalidToken))",
            ),
        ] {
            let raw = json!({"error":{"code":code}}).to_string();
            let error = map_toss_read_error(
                ReadEndpoint::Quotes,
                status,
                TossErrorBody::parse(&raw),
                RetryDelay::Unknown,
            );
            assert_eq!(
                format!("{:?}", error.class()),
                expected,
                "request diagnosis {code}"
            );
        }
    }

    #[test]
    fn unknown_read_and_oauth_codes_keep_all_other_client_failures_as_schema() {
        for status in [
            HttpStatus::BadRequest,
            HttpStatus::Unauthorized,
            HttpStatus::Forbidden,
            HttpStatus::NotFound,
            HttpStatus::Conflict,
            HttpStatus::UriTooLong,
            HttpStatus::UnsupportedMediaType,
            HttpStatus::Unprocessable,
            HttpStatus::OtherClientError,
        ] {
            let read = map_toss_read_error(
                ReadEndpoint::Quotes,
                status,
                TossErrorBody::parse(r#"{"error":{"code":"unknown"}}"#),
                RetryDelay::Unknown,
            );
            let token = map_toss_token_error(
                ReadEndpoint::Quotes,
                status,
                TossErrorBody::parse(r#"{"error":"unknown"}"#),
                RetryDelay::Unknown,
            );
            for error in [read, token] {
                assert_eq!(
                    error.port_error().kind(),
                    PortErrorKind::InvalidResponse(ResponseMismatch::Schema),
                    "{status:?}"
                );
                assert_eq!(
                    error.code_recognition(),
                    CodeRecognition::Unknown,
                    "unknown code marker"
                );
            }
        }
    }

    #[test]
    fn unknown_diagnostics_and_retry_survive_without_raw_code() {
        let retry = RetryDelay::Known(
            domain::TimeSpan::try_new(domain::SpanSeconds::new(11), domain::Nanosecond::new(7))
                .expect("span"),
        );
        for status in [HttpStatus::ServerError, HttpStatus::RateLimited] {
            let raw = r#"{"error":{"code":"SECRET_UNKNOWN_CODE","requestId":"req-1","referenceId":"ref.1","message":"SECRET_MESSAGE"}}"#;
            let error = map_toss_read_error(
                ReadEndpoint::Quotes,
                status,
                TossErrorBody::parse(raw),
                retry,
            );
            assert_eq!(error.class(), ErrorClass::Wait(retry), "retry preserved");
            assert_eq!(
                error.code_recognition(),
                CodeRecognition::Unknown,
                "unknown marker"
            );
            assert_eq!(
                error.diagnostics().request_id().map(DiagnosticId::as_str),
                Some("req-1"),
                "request id"
            );
            assert_eq!(
                error.diagnostics().reference_id().map(DiagnosticId::as_str),
                Some("ref.1"),
                "reference id"
            );
            assert!(
                !format!("{error:?} {error}").contains("SECRET"),
                "no raw code or message"
            );
        }
    }

    #[test]
    fn overview_only_order_codes_are_known() {
        for code in [
            "insufficient-sellable-quantity",
            "duplicate-conditional-order",
            "condition-already-met",
        ] {
            assert_ne!(
                ApiErrorCode::from(code),
                ApiErrorCode::Unknown,
                "documented {code}"
            );
        }
    }

    #[test]
    fn unknown_codes_follow_http_failure_status() {
        for (status, expected) in [
            (
                HttpStatus::ServerError,
                ErrorClass::Wait(RetryDelay::Unknown),
            ),
            (
                HttpStatus::RateLimited,
                ErrorClass::Wait(RetryDelay::Unknown),
            ),
        ] {
            let body =
                TossErrorBody::parse(r#"{"error":{"code":"future-error","message":"hidden"}}"#);
            let mapped =
                map_toss_read_error(ReadEndpoint::Quotes, status, body, RetryDelay::Unknown);
            assert_eq!(mapped.class(), expected, "{status:?}");
            assert_eq!(
                mapped.code_recognition(),
                CodeRecognition::Unknown,
                "unknown marker"
            );
        }
    }

    #[test]
    fn valid_ids_survive_a_missing_code_and_message() {
        let body = TossErrorBody::parse(r#"{"error":{"requestId":"req-1","referenceId":"ref.2"}}"#);
        let error = map_toss_read_error(
            ReadEndpoint::Quotes,
            HttpStatus::BadRequest,
            body,
            RetryDelay::Unknown,
        );
        assert_eq!(
            error.diagnostics().request_id().map(DiagnosticId::as_str),
            Some("req-1"),
            "schema mismatch request id"
        );
        assert_eq!(
            error.diagnostics().reference_id().map(DiagnosticId::as_str),
            Some("ref.2"),
            "schema mismatch reference id"
        );
    }

    #[test]
    fn edge_blocked_is_not_accepted_at_400() {
        let body = TossErrorBody::parse(r#"{"error":{"code":"edge-blocked","message":"hidden"}}"#);
        let error = map_toss_read_error(
            ReadEndpoint::Quotes,
            HttpStatus::BadRequest,
            body,
            RetryDelay::Unknown,
        );
        assert_eq!(
            error.port_error().kind(),
            PortErrorKind::InvalidResponse(ResponseMismatch::Validation),
            "edge status mismatch"
        );
    }

    #[test]
    fn redirects_whitespace_and_other_clients_keep_distinct_causes() {
        let redirect = map_toss_read_error(
            ReadEndpoint::Quotes,
            HttpStatus::Redirect,
            TossErrorBody::parse(""),
            RetryDelay::Unknown,
        );
        assert_eq!(
            redirect.port_error().kind(),
            PortErrorKind::InvalidResponse(ResponseMismatch::Validation),
            "empty redirect"
        );
        let whitespace = map_toss_read_error(
            ReadEndpoint::Quotes,
            HttpStatus::NotReceived,
            TossErrorBody::parse(" \t\r\n"),
            RetryDelay::Unknown,
        );
        assert_eq!(
            whitespace.port_error().kind(),
            PortErrorKind::InvalidResponse(ResponseMismatch::Validation),
            "whitespace is present malformed body"
        );
        let other = map_toss_read_error(
            ReadEndpoint::Quotes,
            HttpStatus::OtherClientError,
            TossErrorBody::parse("{}"),
            RetryDelay::Unknown,
        );
        assert_eq!(
            other.port_error().kind(),
            PortErrorKind::InvalidResponse(ResponseMismatch::Schema),
            "other client failure"
        );
    }

    #[test]
    fn identifiers_have_an_exhaustive_ascii_allow_and_reject_table() {
        let allowed = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_.";
        for byte in 0_u8..=127 {
            let candidate = char::from(byte).to_string();
            assert_eq!(
                DiagnosticId::parse(&candidate).is_some(),
                allowed.contains(&byte),
                "ASCII {byte}"
            );
        }
        for rejected in ["+", "é", "한", "😀", "\u{0080}"] {
            assert_eq!(DiagnosticId::parse(rejected), None, "non ASCII or plus");
        }
    }

    #[test]
    fn valid_code_does_not_require_or_interpret_message_or_data() {
        for message in [json!(null), json!(42_i32), json!({"secret":"hidden"})] {
            let body = TossErrorBody::parse(
                &json!({"error":{"code":"expired-token","message":message,"data":["hidden"]}})
                    .to_string(),
            );
            let error = map_toss_read_error(
                ReadEndpoint::Quotes,
                HttpStatus::Unauthorized,
                body,
                RetryDelay::Unknown,
            );
            assert_eq!(
                error.port_error().kind(),
                PortErrorKind::Unavailable {
                    cause: UnavailableCause::ExpiredToken,
                    retry: RetryDelay::Unknown
                },
                "only code matters"
            );
        }
    }
    #[test]
    fn callers_choose_read_or_token_envelopes() {
        let raw = r#"{"error":"invalid_client","requestId":"req.1"}"#;
        let read = map_toss_read_error(
            ReadEndpoint::Quotes,
            HttpStatus::Unauthorized,
            TossErrorBody::parse(raw),
            RetryDelay::Unknown,
        );
        assert_eq!(
            read.port_error().kind(),
            PortErrorKind::InvalidResponse(ResponseMismatch::Schema),
            "OAuth envelope on read"
        );
        assert_eq!(
            read.diagnostics().request_id().map(DiagnosticId::as_str),
            Some("req.1"),
            "wrong envelope id kept"
        );
        let token = map_toss_token_error(
            ReadEndpoint::Quotes,
            HttpStatus::Unauthorized,
            TossErrorBody::parse(raw),
            RetryDelay::Unknown,
        );
        assert_eq!(
            token.class(),
            ErrorClass::UserAction(application::UserAction::CheckCredentials),
            "token credentials"
        );
        let rate = map_toss_token_error(
            ReadEndpoint::Quotes,
            HttpStatus::RateLimited,
            TossErrorBody::parse(r#"{"error":{"code":"rate-limit-exceeded"}}"#),
            RetryDelay::Unknown,
        );
        assert_eq!(
            rate.class(),
            ErrorClass::Wait(RetryDelay::Unknown),
            "token endpoint REST 429 per spec"
        );
    }

    #[test]
    fn standard_oauth_transients_wait_and_known_codes_are_marked() {
        for code in ["server_error", "temporarily_unavailable"] {
            let body = TossErrorBody::parse(&json!({"error":code}).to_string());
            let error = map_toss_token_error(
                ReadEndpoint::Accounts,
                HttpStatus::ServerError,
                body,
                RetryDelay::Unknown,
            );
            assert_eq!(
                error.class(),
                ErrorClass::Wait(RetryDelay::Unknown),
                "OAuth transient {code}"
            );
            assert_eq!(
                error.code_recognition(),
                CodeRecognition::Known,
                "recognized standard code"
            );
        }
    }

    #[test]
    fn cursor_hint_is_closed_and_only_uses_the_documented_field() {
        for (code, data, expected) in [
            (
                "invalid-request",
                json!({"field":"cursor"}),
                CursorHint::Invalid,
            ),
            (
                "invalid-request",
                json!({"field":"from"}),
                CursorHint::Unspecified,
            ),
            (
                "invalid-request",
                json!({"field":"cursor,from"}),
                CursorHint::Unspecified,
            ),
            ("invalid-request", json!(null), CursorHint::Unspecified),
            (
                "invalid-token",
                json!({"field":"cursor"}),
                CursorHint::Unspecified,
            ),
        ] {
            let body = TossErrorBody::parse(
                &json!({"error":{"code":code,"data":data,"message":"SECRET"}}).to_string(),
            );
            let error = map_toss_read_error(
                ReadEndpoint::Transactions,
                HttpStatus::BadRequest,
                body,
                RetryDelay::Unknown,
            );
            assert_eq!(error.cursor_hint(), expected, "closed cursor hint");
            assert!(
                !format!("{error:?} {error}").contains("SECRET"),
                "message discarded"
            );
        }
    }

    #[test]
    fn missing_cursor_mismatch_hint_in_spec_stays_unspecified() {
        let spec: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string("../../../docs/toss-openapi-1.2.28.json").expect("spec file"),
        )
        .expect("spec JSON");
        let examples = spec.pointer("/paths/~1api~1v1~1transactions/get/responses/400/content/application~1json/examples").expect("transaction errors");
        for (name, expected) in [
            ("invalidCursor", CursorHint::Invalid),
            ("cursorQueryMismatch", CursorHint::Unspecified),
        ] {
            let raw = examples
                .get(name)
                .and_then(|example| example.get("value"))
                .expect("spec response")
                .to_string();
            let error = map_toss_read_error(
                ReadEndpoint::Transactions,
                HttpStatus::BadRequest,
                TossErrorBody::parse(&raw),
                RetryDelay::Unknown,
            );
            assert_eq!(error.cursor_hint(), expected, "actual spec example {name}");
        }
    }
    #[test]
    fn token_transients_and_unknowns_do_not_turn_nonfailure_statuses_into_waits() {
        for code in ["server_error", "temporarily_unavailable", "future_code"] {
            for status in [
                HttpStatus::NotReceived,
                HttpStatus::Informational,
                HttpStatus::Success,
                HttpStatus::Redirect,
            ] {
                let body = TossErrorBody::parse(&json!({"error":code}).to_string());
                let error =
                    map_toss_token_error(ReadEndpoint::Accounts, status, body, RetryDelay::Unknown);
                assert_eq!(
                    error.port_error().kind(),
                    PortErrorKind::InvalidResponse(ResponseMismatch::Validation),
                    "nonfailure {status:?} {code}"
                );
            }
        }
    }

    #[test]
    fn token_rest_429_preserves_unknown_codes_and_rejects_unrelated_read_bodies() {
        let unknown = map_toss_token_error(
            ReadEndpoint::Accounts,
            HttpStatus::RateLimited,
            TossErrorBody::parse(r#"{"error":{"code":"future-rate-code"}}"#),
            RetryDelay::Unknown,
        );
        assert_eq!(
            unknown.class(),
            ErrorClass::Wait(RetryDelay::Unknown),
            "REST token 429 fallback"
        );
        assert_eq!(
            unknown.code_recognition(),
            CodeRecognition::Unknown,
            "REST token marker"
        );
        let unrelated = map_toss_token_error(
            ReadEndpoint::Accounts,
            HttpStatus::NotFound,
            TossErrorBody::parse(r#"{"error":{"code":"stock-not-found","referenceId":"ref-1"}}"#),
            RetryDelay::Unknown,
        );
        assert_eq!(
            unrelated.port_error().kind(),
            PortErrorKind::InvalidResponse(ResponseMismatch::Schema),
            "unrelated read envelope"
        );
        assert_eq!(
            unrelated
                .diagnostics()
                .reference_id()
                .map(DiagnosticId::as_str),
            Some("ref-1"),
            "mismatch id retained"
        );
    }

    #[test]
    fn absent_codes_are_distinct_from_unknown_codes() {
        for raw in ["", "not JSON", r#"{"error":{"requestId":"req-1"}}"#] {
            let error = map_toss_read_error(
                ReadEndpoint::Quotes,
                HttpStatus::BadRequest,
                TossErrorBody::parse(raw),
                RetryDelay::Unknown,
            );
            assert_eq!(
                error.code_recognition(),
                CodeRecognition::Absent,
                "absent code {raw}"
            );
            assert_eq!(
                error.cursor_hint(),
                CursorHint::Unspecified,
                "no cursor evidence"
            );
        }
    }
}
