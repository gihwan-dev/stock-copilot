use super::{DiagnosticText, SyncFailureReason, SyncResult, failure_reason, sync_failure_reason};
use crate::{
    AccessDenial, HttpStatus, MasterNotReady, MissingData, PortError, PortErrorKind, ReadEndpoint,
    RequestRejection, ResponseMismatch, RetryDelay, UnavailableCause,
};
use domain::SyncOutcome;

fn error(kind: PortErrorKind) -> PortError {
    PortError::new(ReadEndpoint::Holdings, HttpStatus::Success, kind)
}

fn unavailable(cause: UnavailableCause) -> PortErrorKind {
    PortErrorKind::Unavailable {
        cause,
        retry: RetryDelay::Unknown,
    }
}

#[test]
fn every_port_error_kind_maps_to_one_coarse_reason() {
    let cases = [
        (
            PortErrorKind::AccessDenied(AccessDenial::IpNotAllowed),
            SyncFailureReason::AccessDenied,
        ),
        (
            PortErrorKind::InvalidResponse(ResponseMismatch::Schema),
            SyncFailureReason::ResponseMismatch,
        ),
        (
            PortErrorKind::InvalidResponse(ResponseMismatch::Validation),
            SyncFailureReason::ResponseMismatch,
        ),
        (
            PortErrorKind::InvalidRequest(RequestRejection::MissingAccountHeader),
            SyncFailureReason::Rejected,
        ),
        (
            PortErrorKind::NotFound(MissingData::Stock),
            SyncFailureReason::Rejected,
        ),
        (
            PortErrorKind::Unsupported(crate::BrokerCapability::Calendar),
            SyncFailureReason::Rejected,
        ),
        (
            PortErrorKind::MasterNotReady(MasterNotReady::MissingInstrument),
            SyncFailureReason::InstrumentsNotReady,
        ),
        (
            unavailable(UnavailableCause::Network),
            SyncFailureReason::Network,
        ),
        (
            unavailable(UnavailableCause::RateLimited),
            SyncFailureReason::ServerBusy,
        ),
        (
            unavailable(UnavailableCause::Maintenance),
            SyncFailureReason::ServerBusy,
        ),
        (
            unavailable(UnavailableCause::ServerError),
            SyncFailureReason::ServerBusy,
        ),
        (
            unavailable(UnavailableCause::ExpiredToken),
            SyncFailureReason::ServerBusy,
        ),
    ];
    for (kind, expected) in cases {
        assert_eq!(failure_reason(kind), expected, "{kind:?}");
    }
}

#[test]
fn the_most_actionable_reason_wins_and_no_errors_is_unknown() {
    assert_eq!(sync_failure_reason(&[]), SyncFailureReason::Unknown);
    assert_eq!(
        sync_failure_reason(&[
            error(unavailable(UnavailableCause::Network)),
            error(PortErrorKind::InvalidResponse(ResponseMismatch::Schema)),
            error(unavailable(UnavailableCause::ServerError)),
        ]),
        SyncFailureReason::ResponseMismatch
    );
    assert_eq!(
        sync_failure_reason(&[
            error(unavailable(UnavailableCause::Network)),
            error(unavailable(UnavailableCause::ServerError)),
        ]),
        SyncFailureReason::ServerBusy
    );
    assert_eq!(
        sync_failure_reason(&[
            error(PortErrorKind::MasterNotReady(
                MasterNotReady::MissingInstrument
            )),
            error(PortErrorKind::InvalidRequest(RequestRejection::EdgeBlocked)),
        ]),
        SyncFailureReason::Rejected
    );
}

#[test]
fn diagnostic_text_keeps_only_safe_characters_and_caps_length() {
    assert_eq!(
        DiagnosticText::new("missing field `cashBuyingPower` at line 1 column 42").as_str(),
        "missing field `cashBuyingPower` at line 1 column 42"
    );
    assert_eq!(
        DiagnosticText::new("a\"b\nc\\d$e@f한").as_str(),
        "a?b?c?d?e?f?"
    );
    let long = DiagnosticText::new(&"x".repeat(500));
    assert_eq!(long.as_str().len(), 241);
    assert!(long.as_str().ends_with('~'));
}

#[test]
fn sync_result_follows_the_stored_outcome() {
    assert_eq!(
        SyncResult::of(&SyncOutcome::Complete(())),
        SyncResult::Complete
    );
    assert_eq!(
        SyncResult::of(&SyncOutcome::Partial(())),
        SyncResult::Partial
    );
    assert_eq!(
        SyncResult::of::<()>(&SyncOutcome::Failed),
        SyncResult::Failed
    );
    assert_eq!(
        SyncResult::of::<()>(&SyncOutcome::AccessBlocked),
        SyncResult::AccessBlocked
    );
}
