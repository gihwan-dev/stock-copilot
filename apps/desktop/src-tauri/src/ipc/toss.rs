use application::{ClassifiedError, SecretValue};
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Manager, Runtime};

use super::error::{IpcError, IpcErrorKind};
use crate::toss_link::{
    TossConnectionState, TossKeys, TossLinkFailure, TossLinkResult, TossLinkState,
};

pub struct UiSecretInput(SecretValue);

impl std::fmt::Debug for UiSecretInput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("UiSecretInput(<redacted>)")
    }
}

impl UiSecretInput {
    #[must_use]
    pub fn into_secret(self) -> SecretValue {
        self.0
    }
}

struct SecretInputVisitor;

impl serde::de::Visitor<'_> for SecretInputVisitor {
    type Value = UiSecretInput;

    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("secret text")
    }

    fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Self::Value, E> {
        Ok(UiSecretInput(SecretValue::new(v.as_bytes().to_vec())))
    }

    fn visit_string<E: serde::de::Error>(self, v: String) -> Result<Self::Value, E> {
        Ok(UiSecretInput(SecretValue::new(v.into_bytes())))
    }
}

impl<'de> Deserialize<'de> for UiSecretInput {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_string(SecretInputVisitor)
    }
}

#[derive(Debug, Deserialize, Type)]
#[serde(deny_unknown_fields)]
#[specta(skip_attr = "serde")]
pub struct SubmitTossKeysRequest {
    #[specta(type = String)]
    pub client_id: UiSecretInput,
    #[specta(type = String)]
    pub secret: UiSecretInput,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Type)]
#[serde(deny_unknown_fields)]
#[specta(skip_attr = "serde")]
pub struct CheckTossConnectionRequest {
    pub query: TossConnectionQuery,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Type)]
pub enum TossConnectionQuery {
    #[serde(rename = "current")]
    Current,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
pub enum UiTossConnectionState {
    #[serde(rename = "needs-keys")]
    NeedsKeys,
    #[serde(rename = "connected")]
    Connected,
    #[serde(rename = "ip-not-allowed")]
    IpNotAllowed,
    #[serde(rename = "invalid-key")]
    InvalidKey,
    #[serde(rename = "forbidden")]
    Forbidden,
    #[serde(rename = "account-not-found")]
    AccountNotFound,
    #[serde(rename = "key-in-use")]
    KeyInUse,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
pub struct UiTossConnection {
    pub state: UiTossConnectionState,
}

const fn ui_state(state: TossConnectionState) -> UiTossConnectionState {
    match state {
        TossConnectionState::NeedsKeys => UiTossConnectionState::NeedsKeys,
        TossConnectionState::Connected => UiTossConnectionState::Connected,
        TossConnectionState::IpNotAllowed => UiTossConnectionState::IpNotAllowed,
        TossConnectionState::InvalidKey => UiTossConnectionState::InvalidKey,
        TossConnectionState::Forbidden => UiTossConnectionState::Forbidden,
        TossConnectionState::AccountNotFound => UiTossConnectionState::AccountNotFound,
        TossConnectionState::KeyInUse => UiTossConnectionState::KeyInUse,
    }
}

fn reply(result: TossLinkResult) -> Result<UiTossConnection, IpcError> {
    match result {
        Ok(state) => Ok(UiTossConnection {
            state: ui_state(state),
        }),
        Err(TossLinkFailure::Unavailable) => Err(IpcError {
            kind: IpcErrorKind::Retryable,
        }),
        Err(TossLinkFailure::Store(error)) => Err(IpcError::from(error.class())),
    }
}

const fn not_assembled() -> IpcError {
    IpcError {
        kind: IpcErrorKind::NoData,
    }
}

pub async fn check_connection(state: Option<&TossLinkState>) -> Result<UiTossConnection, IpcError> {
    let Some(link) = state else {
        return Err(not_assembled());
    };
    reply(link.backend().check().await)
}

pub async fn submit_keys(
    state: Option<&TossLinkState>,
    request: SubmitTossKeysRequest,
) -> Result<UiTossConnection, IpcError> {
    let SubmitTossKeysRequest { client_id, secret } = request;
    let keys = TossKeys::new(client_id.into_secret(), secret.into_secret());
    let Some(link) = state else {
        return Err(not_assembled());
    };
    reply(link.backend().submit(keys).await)
}

#[tauri::command]
#[specta::specta]
pub async fn check_toss_connection<R: Runtime>(
    app: AppHandle<R>,
    request: CheckTossConnectionRequest,
) -> Result<UiTossConnection, IpcError> {
    let CheckTossConnectionRequest {
        query: TossConnectionQuery::Current,
    } = request;
    let state = app.try_state::<TossLinkState>();
    check_connection(state.as_deref()).await
}

#[tauri::command]
#[specta::specta]
pub async fn submit_toss_keys<R: Runtime>(
    app: AppHandle<R>,
    request: SubmitTossKeysRequest,
) -> Result<UiTossConnection, IpcError> {
    let state = app.try_state::<TossLinkState>();
    submit_keys(state.as_deref(), request).await
}
