#[cfg(test)]
mod tests {
    use application::{
        AccessDenial, BrokerCapability, BugDiagnostic, ErrorClass, FalliblePort, HttpStatus,
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
                ErrorClass::UserAction(UserAction::Reauthenticate),
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
                ErrorClass::UserAction(UserAction::Reauthenticate),
            ),
            (
                PortErrorKind::Unavailable {
                    cause: UnavailableCause::ExpiredToken,
                    retry: RetryDelay::Known(delay()),
                },
                ErrorClass::UserAction(UserAction::Reauthenticate),
            ),
        ]
    }
    fn cases_4() -> Vec<(PortErrorKind, ErrorClass)> {
        vec![
            (
                PortErrorKind::NotFound(MissingData::Stock),
                ErrorClass::UserAction(UserAction::CheckData(MissingData::Stock)),
            ),
            (
                PortErrorKind::NotFound(MissingData::ExchangeRate),
                ErrorClass::UserAction(UserAction::CheckData(MissingData::ExchangeRate)),
            ),
            (
                PortErrorKind::NotFound(MissingData::Sector),
                ErrorClass::UserAction(UserAction::CheckData(MissingData::Sector)),
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

    #[test]
    fn every_leaf_matches_the_decision_table() {
        for (kind, expected) in cases() {
            let error = PortError::new(ReadEndpoint::Quotes, HttpStatus::NotReceived, kind);
            assert_eq!(classify_port_error(error), expected, "{kind:?}");
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
    fn formatting_contains_only_the_closed_fields() {
        for (kind, expected) in cases() {
            let error = PortError::new(ReadEndpoint::Quotes, HttpStatus::NotReceived, kind);
            assert_eq!(
                format!("{error:?}"),
                format!("PortError {{ endpoint: Quotes, status: NotReceived, kind: {kind:?} }}")
            );
            let class = classify_port_error(error);
            let label = match expected {
                ErrorClass::UserAction(_) => "UserAction",
                ErrorClass::Wait(_) => "Wait",
                ErrorClass::Bug(_) => "Bug",
            };
            assert_eq!(format!("{class}"), label);
            for secret in [
                "fixture-client-secret",
                "Bearer fixture-token",
                "12345678901234",
                "{\"response\":",
                "Authorization:",
            ] {
                assert!(!format!("{error:?} {class:?} {class}").contains(secret));
            }
        }
    }

    struct FuturePort;
    impl FalliblePort for FuturePort {
        type Error = PortError;
    }
    fn classify_future_failure<P: FalliblePort>(error: P::Error) -> ErrorClass {
        classify_port_error(error.into())
    }
    #[test]
    fn future_port_boundary_requires_conversion_to_the_closed_error() {
        assert_eq!(
            classify_future_failure::<FuturePort>(PortError::new(
                ReadEndpoint::Accounts,
                HttpStatus::Forbidden,
                PortErrorKind::AccessDenied(AccessDenial::Forbidden)
            )),
            ErrorClass::UserAction(UserAction::CheckPermissions)
        );
    }
}
