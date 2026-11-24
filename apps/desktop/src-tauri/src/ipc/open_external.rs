use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use application::{
    ClassifiedError, CollectedUrlCatalog, ExternalUrlOpener, OpenExternalError,
    OpenExternalOutcome, OpenExternalUrl,
};
use domain::ExternalUrlInput;
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Manager, Runtime};

use crate::unavailable::Unavailable;

use super::error::IpcError;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Type)]
#[serde(deny_unknown_fields)]
#[specta(skip_attr = "serde")]
pub struct OpenExternalRequest {
    pub url: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
pub enum OpenExternalStatus {
    #[serde(rename = "opened")]
    Opened,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
pub struct OpenExternalResponse {
    pub status: OpenExternalStatus,
}

trait ExternalLinkRunner: Send + Sync {
    fn open<'call>(
        &'call self,
        input: &'call ExternalUrlInput,
    ) -> Pin<Box<dyn Future<Output = Result<OpenExternalOutcome, OpenExternalError>> + Send + 'call>>;
}

struct WiredExternalLinkRunner<C, O> {
    catalog: Arc<C>,
    opener: Arc<O>,
}

impl<C, O> ExternalLinkRunner for WiredExternalLinkRunner<C, O>
where
    C: CollectedUrlCatalog + 'static,
    O: ExternalUrlOpener + 'static,
{
    fn open<'call>(
        &'call self,
        input: &'call ExternalUrlInput,
    ) -> Pin<Box<dyn Future<Output = Result<OpenExternalOutcome, OpenExternalError>> + Send + 'call>>
    {
        Box::pin(async move {
            OpenExternalUrl::new(self.catalog.as_ref(), self.opener.as_ref())
                .execute(input)
                .await
        })
    }
}

pub struct ExternalLinkRuntime {
    runner: Arc<dyn ExternalLinkRunner>,
}

impl fmt::Debug for ExternalLinkRuntime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ExternalLinkRuntime(<redacted>)")
    }
}

impl ExternalLinkRuntime {
    #[must_use]
    pub fn new<C, O>(catalog: Arc<C>, opener: Arc<O>) -> Self
    where
        C: CollectedUrlCatalog + 'static,
        O: ExternalUrlOpener + 'static,
    {
        Self {
            runner: Arc::new(WiredExternalLinkRunner { catalog, opener }),
        }
    }

    #[must_use]
    pub fn unavailable() -> Self {
        Self::new(Arc::new(Unavailable), Arc::new(Unavailable))
    }

    pub async fn open(
        &self,
        input: &ExternalUrlInput,
    ) -> Result<OpenExternalOutcome, OpenExternalError> {
        self.runner.open(input).await
    }
}

#[tauri::command]
#[specta::specta]
pub async fn open_external<R: Runtime>(
    app: AppHandle<R>,
    request: OpenExternalRequest,
) -> Result<OpenExternalResponse, IpcError> {
    let input = ExternalUrlInput::new(&request.url);
    let outcome = match app.try_state::<ExternalLinkRuntime>() {
        Some(runtime) => runtime.open(&input).await,
        None => ExternalLinkRuntime::unavailable().open(&input).await,
    };
    match outcome {
        Ok(_opened) => Ok(OpenExternalResponse {
            status: OpenExternalStatus::Opened,
        }),
        Err(error) => Err(IpcError::from(error.class())),
    }
}

#[cfg(test)]
mod tests;
