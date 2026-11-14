use crate::{
    AccessDenial, BrokerCapability, MissingData, PortError, PortErrorKind, RequestRejection,
    ResponseMismatch, RetryDelay, UnavailableCause,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorClass {
    UserAction(UserAction),
    Wait(RetryDelay),
    Bug(BugDiagnostic),
    NoData(MissingData),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UserAction {
    CheckCredentials,
    CheckIpAllowlist,
    StopCompetingKeyUse,
    CheckAccount,
    CheckPermissions,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BugDiagnostic {
    Response(ResponseMismatch),
    Request(RequestRejection),
    UnsupportedCapability(BrokerCapability),
}

pub trait ClassifiedError {
    fn class(&self) -> ErrorClass;
}

impl ClassifiedError for PortError {
    fn class(&self) -> ErrorClass {
        classify_port_error(*self)
    }
}

#[must_use]
pub fn classify_port_error(error: PortError) -> ErrorClass {
    match error.kind() {
        PortErrorKind::AccessDenied(denial) => ErrorClass::UserAction(match denial {
            AccessDenial::InvalidCredentials | AccessDenial::LoginUserNotFound => {
                UserAction::CheckCredentials
            }
            AccessDenial::IpNotAllowed => UserAction::CheckIpAllowlist,
            AccessDenial::TokenRevokedRepeated => UserAction::StopCompetingKeyUse,
            AccessDenial::AccountNotFound => UserAction::CheckAccount,
            AccessDenial::Forbidden => UserAction::CheckPermissions,
        }),
        PortErrorKind::Unavailable { cause, retry } => match cause {
            UnavailableCause::ExpiredToken
            | UnavailableCause::RateLimited
            | UnavailableCause::Maintenance
            | UnavailableCause::ServerError
            | UnavailableCause::Network => ErrorClass::Wait(retry),
        },
        PortErrorKind::NotFound(data) => match data {
            MissingData::Stock | MissingData::ExchangeRate | MissingData::Sector => {
                ErrorClass::NoData(data)
            }
        },
        PortErrorKind::InvalidResponse(mismatch) => match mismatch {
            ResponseMismatch::Schema | ResponseMismatch::Validation => {
                ErrorClass::Bug(BugDiagnostic::Response(mismatch))
            }
        },
        PortErrorKind::Unsupported(capability) => {
            ErrorClass::Bug(BugDiagnostic::UnsupportedCapability(capability))
        }
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
