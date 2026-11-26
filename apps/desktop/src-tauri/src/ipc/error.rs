use serde::Serialize;
use specta::Type;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
pub enum IpcErrorKind {
    #[serde(rename = "user-fixable")]
    UserFixable,
    #[serde(rename = "retryable")]
    Retryable,
    #[serde(rename = "bug")]
    Bug,
    #[serde(rename = "no-data")]
    NoData,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
pub struct IpcError {
    pub kind: IpcErrorKind,
}

impl From<application::ErrorClass> for IpcError {
    fn from(class: application::ErrorClass) -> Self {
        let kind = match class {
            application::ErrorClass::UserAction(_) => IpcErrorKind::UserFixable,
            application::ErrorClass::Wait(_) => IpcErrorKind::Retryable,
            application::ErrorClass::Bug(_) => IpcErrorKind::Bug,
            application::ErrorClass::NoData(_) => IpcErrorKind::NoData,
        };
        Self { kind }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
pub enum IpcFailureReason {
    #[serde(rename = "network")]
    Network,
    #[serde(rename = "server-busy")]
    ServerBusy,
    #[serde(rename = "response-mismatch")]
    ResponseMismatch,
    #[serde(rename = "rejected")]
    Rejected,
    #[serde(rename = "instruments-not-ready")]
    InstrumentsNotReady,
    #[serde(rename = "access-denied")]
    AccessDenied,
    #[serde(rename = "unknown")]
    Unknown,
}

impl From<application::SyncFailureReason> for IpcFailureReason {
    fn from(reason: application::SyncFailureReason) -> Self {
        match reason {
            application::SyncFailureReason::Network => Self::Network,
            application::SyncFailureReason::ServerBusy => Self::ServerBusy,
            application::SyncFailureReason::ResponseMismatch => Self::ResponseMismatch,
            application::SyncFailureReason::Rejected => Self::Rejected,
            application::SyncFailureReason::InstrumentsNotReady => Self::InstrumentsNotReady,
            application::SyncFailureReason::AccessDenied => Self::AccessDenied,
            application::SyncFailureReason::Unknown => Self::Unknown,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
pub struct IpcSyncError {
    pub kind: IpcErrorKind,
    pub reason: Option<IpcFailureReason>,
}

impl IpcSyncError {
    #[must_use]
    pub fn explained(error: IpcError, reason: Option<application::SyncFailureReason>) -> Self {
        Self {
            kind: error.kind,
            reason: match error.kind {
                IpcErrorKind::Retryable => reason.map(IpcFailureReason::from),
                IpcErrorKind::UserFixable | IpcErrorKind::Bug | IpcErrorKind::NoData => None,
            },
        }
    }
}
