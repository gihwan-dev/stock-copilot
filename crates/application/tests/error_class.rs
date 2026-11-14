#[cfg(test)]
mod tests {
    use application::{
        AccessDenial, BrokerCapability, BugDiagnostic, ClassifiedError, ErrorClass, HttpStatus,
        MissingData, PortError, PortErrorKind, ReadEndpoint, RequestRejection, ResponseMismatch,
        RetryDelay, UnavailableCause, UserAction, classify_port_error,
    };
    use domain::{Nanosecond, SpanSeconds, TimeSpan};

    fn delay() -> TimeSpan {
        TimeSpan::try_new(SpanSeconds::new(17), Nanosecond::new(123)).expect("대기 시간")
    }

    fn cases() -> Vec<(PortErrorKind, ErrorClass)> {
        cases_0()
            .into_iter()
            .chain(cases_1())
            .chain(cases_2())
            .chain(cases_3())
            .chain(cases_4())
            .chain(cases_5())
            .chain(cases_6())
            .chain(cases_7())
            .collect()
    }
    fn cases_0() -> Vec<(PortErrorKind, ErrorClass)> {
        vec![
            (
                PortErrorKind::AccessDenied(AccessDenial::InvalidCredentials),
                ErrorClass::UserAction(UserAction::CheckCredentials),
            ),
            (
                PortErrorKind::AccessDenied(AccessDenial::IpNotAllowed),
                ErrorClass::UserAction(UserAction::CheckIpAllowlist),
            ),
            (
                PortErrorKind::AccessDenied(AccessDenial::TokenRevokedRepeated),
                ErrorClass::UserAction(UserAction::StopCompetingKeyUse),
            ),
            (
                PortErrorKind::AccessDenied(AccessDenial::AccountNotFound),
                ErrorClass::UserAction(UserAction::CheckAccount),
            ),
        ]
    }
    fn cases_1() -> Vec<(PortErrorKind, ErrorClass)> {
        vec![
            (
                PortErrorKind::AccessDenied(AccessDenial::Forbidden),
                ErrorClass::UserAction(UserAction::CheckPermissions),
            ),
            (
                PortErrorKind::AccessDenied(AccessDenial::LoginUserNotFound),
                ErrorClass::UserAction(UserAction::CheckCredentials),
            ),
            (
                PortErrorKind::Unavailable {
                    cause: UnavailableCause::RateLimited,
                    retry: RetryDelay::Unknown,
                },
                ErrorClass::Wait(RetryDelay::Unknown),
            ),
            (
                PortErrorKind::Unavailable {
                    cause: UnavailableCause::RateLimited,
                    retry: RetryDelay::Known(delay()),
                },
                ErrorClass::Wait(RetryDelay::Known(delay())),
            ),
        ]
    }
    fn cases_2() -> Vec<(PortErrorKind, ErrorClass)> {
        vec![
            (
                PortErrorKind::Unavailable {
                    cause: UnavailableCause::Maintenance,
                    retry: RetryDelay::Unknown,
                },
                ErrorClass::Wait(RetryDelay::Unknown),
            ),
            (
                PortErrorKind::Unavailable {
                    cause: UnavailableCause::Maintenance,
                    retry: RetryDelay::Known(delay()),
                },
                ErrorClass::Wait(RetryDelay::Known(delay())),
            ),
            (
                PortErrorKind::Unavailable {
                    cause: UnavailableCause::ServerError,
                    retry: RetryDelay::Unknown,
                },
                ErrorClass::Wait(RetryDelay::Unknown),
            ),
            (
                PortErrorKind::Unavailable {
                    cause: UnavailableCause::ServerError,
                    retry: RetryDelay::Known(delay()),
                },
                ErrorClass::Wait(RetryDelay::Known(delay())),
            ),
        ]
    }
    fn cases_3() -> Vec<(PortErrorKind, ErrorClass)> {
        vec![
            (
                PortErrorKind::Unavailable {
                    cause: UnavailableCause::Network,
                    retry: RetryDelay::Unknown,
                },
                ErrorClass::Wait(RetryDelay::Unknown),
            ),
            (
                PortErrorKind::Unavailable {
                    cause: UnavailableCause::Network,
                    retry: RetryDelay::Known(delay()),
                },
                ErrorClass::Wait(RetryDelay::Known(delay())),
            ),
            (
                PortErrorKind::Unavailable {
                    cause: UnavailableCause::ExpiredToken,
                    retry: RetryDelay::Unknown,
                },
                ErrorClass::Wait(RetryDelay::Unknown),
            ),
            (
                PortErrorKind::Unavailable {
                    cause: UnavailableCause::ExpiredToken,
                    retry: RetryDelay::Known(delay()),
                },
                ErrorClass::Wait(RetryDelay::Known(delay())),
            ),
        ]
    }
    fn cases_4() -> Vec<(PortErrorKind, ErrorClass)> {
        vec![
            (
                PortErrorKind::NotFound(MissingData::Stock),
                ErrorClass::NoData(MissingData::Stock),
            ),
            (
                PortErrorKind::NotFound(MissingData::ExchangeRate),
                ErrorClass::NoData(MissingData::ExchangeRate),
            ),
            (
                PortErrorKind::NotFound(MissingData::Sector),
                ErrorClass::NoData(MissingData::Sector),
            ),
            (
                PortErrorKind::InvalidResponse(ResponseMismatch::Schema),
                ErrorClass::Bug(BugDiagnostic::Response(ResponseMismatch::Schema)),
            ),
        ]
    }
    fn cases_5() -> Vec<(PortErrorKind, ErrorClass)> {
        vec![
            (
                PortErrorKind::InvalidResponse(ResponseMismatch::Validation),
                ErrorClass::Bug(BugDiagnostic::Response(ResponseMismatch::Validation)),
            ),
            (
                PortErrorKind::Unsupported(BrokerCapability::DailyCandles),
                ErrorClass::Bug(BugDiagnostic::UnsupportedCapability(
                    BrokerCapability::DailyCandles,
                )),
            ),
            (
                PortErrorKind::Unsupported(BrokerCapability::MinuteCandles),
                ErrorClass::Bug(BugDiagnostic::UnsupportedCapability(
                    BrokerCapability::MinuteCandles,
                )),
            ),
            (
                PortErrorKind::Unsupported(BrokerCapability::AdjustedHistory),
                ErrorClass::Bug(BugDiagnostic::UnsupportedCapability(
                    BrokerCapability::AdjustedHistory,
                )),
            ),
        ]
    }
    fn cases_6() -> Vec<(PortErrorKind, ErrorClass)> {
        vec![
            (
                PortErrorKind::Unsupported(BrokerCapability::RawHistory),
                ErrorClass::Bug(BugDiagnostic::UnsupportedCapability(
                    BrokerCapability::RawHistory,
                )),
            ),
            (
                PortErrorKind::InvalidRequest(RequestRejection::MalformedRequest),
                ErrorClass::Bug(BugDiagnostic::Request(RequestRejection::MalformedRequest)),
            ),
            (
                PortErrorKind::InvalidRequest(RequestRejection::MissingAccountHeader),
                ErrorClass::Bug(BugDiagnostic::Request(
                    RequestRejection::MissingAccountHeader,
                )),
            ),
            (
                PortErrorKind::InvalidRequest(RequestRejection::InvalidToken),
                ErrorClass::Bug(BugDiagnostic::Request(RequestRejection::InvalidToken)),
            ),
        ]
    }
    fn cases_7() -> Vec<(PortErrorKind, ErrorClass)> {
        vec![
            (
                PortErrorKind::InvalidRequest(RequestRejection::EdgeBlocked),
                ErrorClass::Bug(BugDiagnostic::Request(RequestRejection::EdgeBlocked)),
            ),
            (
                PortErrorKind::InvalidRequest(RequestRejection::UnsupportedQuery),
                ErrorClass::Bug(BugDiagnostic::Request(RequestRejection::UnsupportedQuery)),
            ),
            (
                PortErrorKind::InvalidRequest(RequestRejection::UnsupportedContentType),
                ErrorClass::Bug(BugDiagnostic::Request(
                    RequestRejection::UnsupportedContentType,
                )),
            ),
            (
                PortErrorKind::InvalidRequest(RequestRejection::BatchLimitExceeded),
                ErrorClass::Bug(BugDiagnostic::Request(RequestRejection::BatchLimitExceeded)),
            ),
        ]
    }

    fn expected_class(kind: PortErrorKind) -> ErrorClass {
        match kind {
            PortErrorKind::AccessDenied(value) => expected_access(value),
            PortErrorKind::Unavailable { cause, retry } => expected_wait(cause, retry),
            PortErrorKind::NotFound(value) => expected_no_data(value),
            PortErrorKind::InvalidResponse(value) => expected_response(value),
            PortErrorKind::Unsupported(value) => {
                ErrorClass::Bug(BugDiagnostic::UnsupportedCapability(value))
            }
            PortErrorKind::InvalidRequest(value) => expected_request(value),
        }
    }

    fn expected_access(cause: AccessDenial) -> ErrorClass {
        match cause {
            AccessDenial::InvalidCredentials | AccessDenial::LoginUserNotFound => {
                ErrorClass::UserAction(UserAction::CheckCredentials)
            }
            AccessDenial::IpNotAllowed => ErrorClass::UserAction(UserAction::CheckIpAllowlist),
            AccessDenial::TokenRevokedRepeated => {
                ErrorClass::UserAction(UserAction::StopCompetingKeyUse)
            }
            AccessDenial::AccountNotFound => ErrorClass::UserAction(UserAction::CheckAccount),
            AccessDenial::Forbidden => ErrorClass::UserAction(UserAction::CheckPermissions),
        }
    }

    fn expected_wait(cause: UnavailableCause, retry: RetryDelay) -> ErrorClass {
        match cause {
            UnavailableCause::RateLimited
            | UnavailableCause::Maintenance
            | UnavailableCause::ServerError
            | UnavailableCause::Network
            | UnavailableCause::ExpiredToken => ErrorClass::Wait(retry),
        }
    }

    fn expected_no_data(cause: MissingData) -> ErrorClass {
        match cause {
            MissingData::Stock => ErrorClass::NoData(MissingData::Stock),
            MissingData::ExchangeRate => ErrorClass::NoData(MissingData::ExchangeRate),
            MissingData::Sector => ErrorClass::NoData(MissingData::Sector),
        }
    }

    fn expected_response(cause: ResponseMismatch) -> ErrorClass {
        match cause {
            ResponseMismatch::Schema => {
                ErrorClass::Bug(BugDiagnostic::Response(ResponseMismatch::Schema))
            }
            ResponseMismatch::Validation => {
                ErrorClass::Bug(BugDiagnostic::Response(ResponseMismatch::Validation))
            }
        }
    }

    fn expected_request(cause: RequestRejection) -> ErrorClass {
        match cause {
            RequestRejection::MalformedRequest => {
                ErrorClass::Bug(BugDiagnostic::Request(RequestRejection::MalformedRequest))
            }
            RequestRejection::MissingAccountHeader => ErrorClass::Bug(BugDiagnostic::Request(
                RequestRejection::MissingAccountHeader,
            )),
            RequestRejection::InvalidToken => {
                ErrorClass::Bug(BugDiagnostic::Request(RequestRejection::InvalidToken))
            }
            RequestRejection::EdgeBlocked => {
                ErrorClass::Bug(BugDiagnostic::Request(RequestRejection::EdgeBlocked))
            }
            RequestRejection::UnsupportedQuery => {
                ErrorClass::Bug(BugDiagnostic::Request(RequestRejection::UnsupportedQuery))
            }
            RequestRejection::UnsupportedContentType => ErrorClass::Bug(BugDiagnostic::Request(
                RequestRejection::UnsupportedContentType,
            )),
            RequestRejection::BatchLimitExceeded => {
                ErrorClass::Bug(BugDiagnostic::Request(RequestRejection::BatchLimitExceeded))
            }
        }
    }

    #[test]
    fn every_leaf_matches_the_decision_table() {
        for (kind, expected) in cases() {
            let error = PortError::new(ReadEndpoint::Quotes, HttpStatus::NotReceived, kind);
            assert_eq!(expected_class(kind), expected, "문서 표: {kind:?}");
            assert_eq!(
                classify_port_error(error),
                expected_class(kind),
                "분류: {kind:?}"
            );
            assert_eq!(class_of(&error), expected_class(kind), "trait: {kind:?}");
        }
    }

    fn assert_cases_for(endpoint: ReadEndpoint, status: HttpStatus) {
        for (kind, expected) in cases() {
            assert_eq!(
                classify_port_error(PortError::new(endpoint, status, kind)),
                expected
            );
        }
    }

    #[test]
    fn endpoint_and_status_do_not_override_the_cause() {
        for endpoint in [
            ReadEndpoint::Accounts,
            ReadEndpoint::Holdings,
            ReadEndpoint::Transactions,
            ReadEndpoint::Quotes,
            ReadEndpoint::Candles,
            ReadEndpoint::BuyingPower,
            ReadEndpoint::FxRate,
            ReadEndpoint::OrderFxRates,
        ] {
            for status in [
                HttpStatus::NotReceived,
                HttpStatus::Informational,
                HttpStatus::Success,
                HttpStatus::Redirect,
                HttpStatus::BadRequest,
                HttpStatus::Unauthorized,
                HttpStatus::Forbidden,
                HttpStatus::NotFound,
                HttpStatus::Conflict,
                HttpStatus::UriTooLong,
                HttpStatus::UnsupportedMediaType,
                HttpStatus::Unprocessable,
                HttpStatus::RateLimited,
                HttpStatus::OtherClientError,
                HttpStatus::ServerError,
            ] {
                assert_cases_for(endpoint, status);
            }
        }
    }

    #[test]
    fn known_zero_and_fractional_delay_are_preserved_without_clock_arithmetic() {
        for duration in [
            TimeSpan::try_new(SpanSeconds::new(0), Nanosecond::new(0)).expect("0초"),
            delay(),
        ] {
            assert_eq!(
                classify_port_error(PortError::new(
                    ReadEndpoint::Quotes,
                    HttpStatus::RateLimited,
                    PortErrorKind::Unavailable {
                        cause: UnavailableCause::RateLimited,
                        retry: RetryDelay::Known(duration)
                    }
                )),
                ErrorClass::Wait(RetryDelay::Known(duration))
            );
        }
    }

    #[test]
    fn debug_has_exactly_the_closed_port_fields() {
        for (kind, _) in cases() {
            let error = PortError::new(ReadEndpoint::Quotes, HttpStatus::NotReceived, kind);
            assert_eq!(
                format!("{error:?}"),
                format!("PortError {{ endpoint: Quotes, status: NotReceived, kind: {kind:?} }}")
            );
            require_type::<ReadEndpoint>(error.endpoint());
            require_type::<HttpStatus>(error.status());
            require_type::<PortErrorKind>(error.kind());
            assert_eq!(
                (error.endpoint(), error.status(), error.kind()),
                (ReadEndpoint::Quotes, HttpStatus::NotReceived, kind)
            );
        }
    }

    fn require_copy<T: Copy>() {}

    #[test]
    fn port_and_classification_types_are_copy() {
        require_copy::<PortError>();
        require_copy::<PortErrorKind>();
        require_copy::<ReadEndpoint>();
        require_copy::<HttpStatus>();
        require_copy::<AccessDenial>();
        require_copy::<UnavailableCause>();
        require_copy::<MissingData>();
        require_copy::<ResponseMismatch>();
        require_copy::<RequestRejection>();
        require_copy::<BrokerCapability>();
        require_copy::<RetryDelay>();
        require_copy::<TimeSpan>();
        require_copy::<ErrorClass>();
        require_copy::<UserAction>();
        require_copy::<BugDiagnostic>();
    }

    fn require_type<T>(_: T) {}

    fn check_retry_fields(retry: RetryDelay) {
        match retry {
            RetryDelay::Unknown => (),
            RetryDelay::Known(duration) => require_type::<TimeSpan>(duration),
        }
    }

    fn check_diagnostic_fields(diagnostic: BugDiagnostic) {
        match diagnostic {
            BugDiagnostic::Response(value) => require_type::<ResponseMismatch>(value),
            BugDiagnostic::Request(value) => require_type::<RequestRejection>(value),
            BugDiagnostic::UnsupportedCapability(value) => require_type::<BrokerCapability>(value),
        }
    }

    fn check_class_fields(class: ErrorClass) {
        match class {
            ErrorClass::UserAction(action) => match action {
                UserAction::CheckCredentials
                | UserAction::CheckIpAllowlist
                | UserAction::StopCompetingKeyUse
                | UserAction::CheckAccount
                | UserAction::CheckPermissions => require_type::<UserAction>(action),
            },
            ErrorClass::Wait(retry) => check_retry_fields(retry),
            ErrorClass::Bug(diagnostic) => check_diagnostic_fields(diagnostic),
            ErrorClass::NoData(data) => require_type::<MissingData>(data),
        }
    }

    #[test]
    fn classification_fields_are_closed_enums_or_time_span() {
        for (kind, _) in cases() {
            check_class_fields(expected_class(kind));
            check_class_fields(classify_port_error(PortError::new(
                ReadEndpoint::Quotes,
                HttpStatus::NotReceived,
                kind,
            )));
        }
    }

    enum OtherPortError {
        TemporarilyUnavailable,
    }

    impl ClassifiedError for OtherPortError {
        fn class(&self) -> ErrorClass {
            match self {
                Self::TemporarilyUnavailable => ErrorClass::Wait(RetryDelay::Unknown),
            }
        }
    }

    fn class_of<E: ClassifiedError>(error: &E) -> ErrorClass {
        error.class()
    }

    #[test]
    fn another_port_error_classifies_without_conversion_to_a_broker_error() {
        assert_eq!(
            class_of(&OtherPortError::TemporarilyUnavailable),
            ErrorClass::Wait(RetryDelay::Unknown)
        );
    }
}
