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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
pub struct IpcError {
    pub kind: IpcErrorKind,
}
