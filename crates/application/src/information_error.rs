use crate::{
    BugDiagnostic, ClassifiedError, ErrorClass, ResetTime, ResponseMismatch, RetryDelay, UserAction,
};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CredentialSource {
    Dart,
    Fred,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdentifierError {
    InvalidFormat,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InformationError {
    Unavailable(ResetTime),
    InvalidResponse(ResponseMismatch),
    InvalidCredential(CredentialSource),
}
impl ClassifiedError for InformationError {
    fn class(&self) -> ErrorClass {
        match *self {
            Self::Unavailable(_reset) => ErrorClass::Wait(RetryDelay::Unknown),
            Self::InvalidResponse(reason) => ErrorClass::Bug(BugDiagnostic::Response(reason)),
            Self::InvalidCredential(_source) => {
                ErrorClass::UserAction(UserAction::CheckCredentials)
            }
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DataAvailability<T> {
    Available(T),
    NotConfigured,
    NoData,
}
