use crate::{BugDiagnostic, ClassifiedError, ErrorClass, RetryDelay, UserAction};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SystemPort {
    Repository,
    AuditLog,
    SecretStore,
    Notifier,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SystemFailure {
    PermissionDenied,
    Unavailable(RetryDelay),
    InvalidData,
    RepositoryIntegrity,
    SecretIntegrity,
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
            SystemFailure::InvalidData => {
                ErrorClass::Bug(BugDiagnostic::System(SystemDiagnostic::InvalidStoredData))
            }
            SystemFailure::RepositoryIntegrity => {
                ErrorClass::Bug(BugDiagnostic::System(SystemDiagnostic::RepositoryIntegrity))
            }
            SystemFailure::SecretIntegrity => {
                ErrorClass::Bug(BugDiagnostic::System(SystemDiagnostic::SecretIntegrity))
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WriteCompleted;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SystemDiagnostic {
    InvalidStoredData,
    RepositoryIntegrity,
    SecretIntegrity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordOutcome {
    Recorded,
    AlreadyRecorded,
}
