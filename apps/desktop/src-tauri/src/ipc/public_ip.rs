use std::net::IpAddr;
use std::str::FromStr;
use std::sync::Arc;

use application::Clock;
use domain::{Nanosecond, SpanSeconds, TimeSpan, Timestamp};
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Manager, Runtime};
use tokio::sync::Mutex;

use super::error::{IpcError, IpcErrorKind};
use super::state::BoxFuture;

const LOOKUP_INTERVAL_SECONDS: u64 = 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LookupFailed;

pub trait PublicIpLookup: Send + Sync {
    fn lookup(&self) -> BoxFuture<'_, Result<IpAddr, LookupFailed>>;
}

pub trait TextClipboard: Send + Sync {
    fn copy(&self, text: String) -> BoxFuture<'_, Result<(), LookupFailed>>;
}

struct Recent {
    at: Timestamp,
    result: Result<IpAddr, LookupFailed>,
}

pub struct PublicIpRuntime {
    lookup: Arc<dyn PublicIpLookup>,
    clipboard: Arc<dyn TextClipboard>,
    clock: Arc<dyn Clock>,
    recent: Mutex<Option<Recent>>,
}

impl std::fmt::Debug for PublicIpRuntime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PublicIpRuntime")
    }
}

fn interval() -> Option<TimeSpan> {
    TimeSpan::try_new(
        SpanSeconds::new(LOOKUP_INTERVAL_SECONDS),
        Nanosecond::new(0),
    )
    .ok()
}

impl PublicIpRuntime {
    #[must_use]
    pub fn new(
        lookup: Arc<dyn PublicIpLookup>,
        clipboard: Arc<dyn TextClipboard>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            lookup,
            clipboard,
            clock,
            recent: Mutex::new(None),
        }
    }

    pub async fn current(&self) -> Result<IpAddr, LookupFailed> {
        let mut recent = self.recent.lock().await;
        let now = self.clock.now();
        if let Some(previous) = recent.as_ref()
            && interval()
                .and_then(|window| previous.at.checked_add(window).ok())
                .is_some_and(|until| now < until)
        {
            return previous.result;
        }
        let result = self.lookup.lookup().await;
        *recent = Some(Recent { at: now, result });
        result
    }

    pub async fn copy(&self, address: IpAddr) -> Result<(), LookupFailed> {
        self.clipboard.copy(address.to_string()).await
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Type)]
#[serde(deny_unknown_fields)]
#[specta(skip_attr = "serde")]
pub struct GetPublicIpRequest {
    pub query: PublicIpQuery,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Type)]
pub enum PublicIpQuery {
    #[serde(rename = "current")]
    Current,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct UiPublicIp {
    pub address: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Type)]
#[serde(deny_unknown_fields)]
#[specta(skip_attr = "serde")]
pub struct CopyTextRequest {
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
pub enum UiCopyStatus {
    #[serde(rename = "copied")]
    Copied,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
pub struct UiCopyResult {
    pub status: UiCopyStatus,
}

const fn failure(kind: IpcErrorKind) -> IpcError {
    IpcError { kind }
}

pub async fn public_ip_reply(state: Option<&PublicIpRuntime>) -> Result<UiPublicIp, IpcError> {
    let runtime = state.ok_or(failure(IpcErrorKind::NoData))?;
    runtime
        .current()
        .await
        .map(|address| UiPublicIp {
            address: address.to_string(),
        })
        .map_err(|LookupFailed| failure(IpcErrorKind::Retryable))
}

pub async fn copy_reply(
    state: Option<&PublicIpRuntime>,
    request: CopyTextRequest,
) -> Result<UiCopyResult, IpcError> {
    let address =
        IpAddr::from_str(request.text.trim()).map_err(|_error| failure(IpcErrorKind::Bug))?;
    let runtime = state.ok_or(failure(IpcErrorKind::NoData))?;
    runtime
        .copy(address)
        .await
        .map(|()| UiCopyResult {
            status: UiCopyStatus::Copied,
        })
        .map_err(|LookupFailed| failure(IpcErrorKind::Retryable))
}

#[tauri::command]
#[specta::specta]
pub async fn get_public_ip<R: Runtime>(
    app: AppHandle<R>,
    request: GetPublicIpRequest,
) -> Result<UiPublicIp, IpcError> {
    let GetPublicIpRequest {
        query: PublicIpQuery::Current,
    } = request;
    let state = app.try_state::<PublicIpRuntime>();
    public_ip_reply(state.as_deref()).await
}

#[tauri::command]
#[specta::specta]
pub async fn copy_text<R: Runtime>(
    app: AppHandle<R>,
    request: CopyTextRequest,
) -> Result<UiCopyResult, IpcError> {
    let state = app.try_state::<PublicIpRuntime>();
    copy_reply(state.as_deref(), request).await
}

#[cfg(test)]
mod tests;
