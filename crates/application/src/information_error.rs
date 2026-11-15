use crate::{
    BugDiagnostic, ClassifiedError, ErrorClass, RequestRejection, ResponseMismatch, RetryDelay,
};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InformationError {
    Unavailable(RetryDelay),
    InvalidResponse(ResponseMismatch),
    InvalidIdentifier,
}
impl ClassifiedError for InformationError {
    fn class(&self) -> ErrorClass {
        match *self {
            Self::Unavailable(retry) => ErrorClass::Wait(retry),
            Self::InvalidResponse(reason) => ErrorClass::Bug(BugDiagnostic::Response(reason)),
            Self::InvalidIdentifier => {
                ErrorClass::Bug(BugDiagnostic::Request(RequestRejection::MalformedRequest))
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
