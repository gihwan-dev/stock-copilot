use crate::{
    AccessDenial, BrokerCapability, MissingData, PortError, PortErrorKind, RequestRejection,
    ResponseMismatch, RetryDelay, UnavailableCause,
};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorClass {
    UserAction(UserAction),
    Wait(RetryDelay),
    Bug(BugDiagnostic),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UserAction {
    CheckCredentials,
    CheckIpAllowlist,
    StopCompetingKeyUse,
    CheckAccount,
    CheckPermissions,
    Reauthenticate,
    CheckData(MissingData),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BugDiagnostic {
    Response(ResponseMismatch),
    Request(RequestRejection),
    UnsupportedCapability(BrokerCapability),
}

pub trait FalliblePort {
    type Error: Into<PortError>;
}

impl fmt::Display for ErrorClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UserAction(_) => f.write_str("UserAction"),
            Self::Wait(_) => f.write_str("Wait"),
            Self::Bug(_) => f.write_str("Bug"),
        }
    }
}

#[must_use]
pub fn classify_port_error(error: PortError) -> ErrorClass {
    match error.kind() {
        PortErrorKind::AccessDenied(denial) => ErrorClass::UserAction(match denial {
            AccessDenial::InvalidCredentials => UserAction::CheckCredentials,
            AccessDenial::IpNotAllowed => UserAction::CheckIpAllowlist,
            AccessDenial::TokenRevokedRepeated => UserAction::StopCompetingKeyUse,
            AccessDenial::AccountNotFound => UserAction::CheckAccount,
            AccessDenial::Forbidden => UserAction::CheckPermissions,
            AccessDenial::LoginUserNotFound => UserAction::Reauthenticate,
        }),
        PortErrorKind::Unavailable { cause, retry } => match cause {
            UnavailableCause::ExpiredToken => ErrorClass::UserAction(UserAction::Reauthenticate),
            UnavailableCause::RateLimited
            | UnavailableCause::Maintenance
            | UnavailableCause::ServerError
            | UnavailableCause::Network => ErrorClass::Wait(retry),
        },
        PortErrorKind::NotFound(data) => match data {
            MissingData::Stock | MissingData::ExchangeRate | MissingData::Sector => {
                ErrorClass::UserAction(UserAction::CheckData(data))
            }
        },
        PortErrorKind::InvalidResponse(mismatch) => match mismatch {
            ResponseMismatch::Schema | ResponseMismatch::Validation => {
                ErrorClass::Bug(BugDiagnostic::Response(mismatch))
            }
        },
        PortErrorKind::Unsupported(capability) => match capability {
            BrokerCapability::DailyCandles
            | BrokerCapability::MinuteCandles
            | BrokerCapability::AdjustedHistory
            | BrokerCapability::RawHistory => {
                ErrorClass::Bug(BugDiagnostic::UnsupportedCapability(capability))
            }
        },
        PortErrorKind::InvalidRequest(rejection) => match rejection {
            RequestRejection::MalformedRequest
            | RequestRejection::MissingAccountHeader
            | RequestRejection::InvalidToken
            | RequestRejection::EdgeBlocked
            | RequestRejection::UnsupportedQuery
            | RequestRejection::UnsupportedContentType
            | RequestRejection::BatchLimitExceeded => {
                ErrorClass::Bug(BugDiagnostic::Request(rejection))
            }
        },
    }
}
