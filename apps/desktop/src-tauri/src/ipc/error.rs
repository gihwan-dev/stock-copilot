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
