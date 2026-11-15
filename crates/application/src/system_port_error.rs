use crate::{BugDiagnostic, ClassifiedError, ErrorClass, ResponseMismatch, RetryDelay, UserAction};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SystemPort {
    Repository,
    AuditLog,
    SecretStore,
    Clock,
    Notifier,
    UserPresence,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SystemFailure {
    PermissionDenied,
    Unavailable(RetryDelay),
    InvalidData,
    Conflict,
    IntegrityViolation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SystemPortError {
    port: SystemPort,
    failure: SystemFailure,
}

impl SystemPortError {
    #[must_use]
    pub const fn new(port: SystemPort, failure: SystemFailure) -> Self {
        Self { port, failure }
    }
    #[must_use]
    pub const fn port(self) -> SystemPort {
        self.port
    }
    #[must_use]
    pub const fn failure(self) -> SystemFailure {
        self.failure
    }
}

impl ClassifiedError for SystemPortError {
    fn class(&self) -> ErrorClass {
        match self.failure {
            SystemFailure::PermissionDenied => ErrorClass::UserAction(UserAction::CheckPermissions),
            SystemFailure::Unavailable(retry) => ErrorClass::Wait(retry),
            SystemFailure::InvalidData
            | SystemFailure::Conflict
            | SystemFailure::IntegrityViolation => {
                ErrorClass::Bug(BugDiagnostic::Response(ResponseMismatch::Validation))
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WriteCompleted;
