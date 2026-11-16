use crate::lifecycle::{ExitDecision, Lifecycle};
use std::sync::Arc;
use tauri::{AppHandle, Manager, Runtime};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExitCode(i32);
impl ExitCode {
    #[must_use]
    pub const fn new(value: i32) -> Self {
        Self(value)
    }
    #[must_use]
    pub fn from_request(value: Option<i32>) -> Self {
        Self(value.unwrap_or(0_i32))
    }
    #[must_use]
    pub const fn as_i32(self) -> i32 {
        self.0
    }
}

pub(crate) trait ExitRequest {
    fn prevent(&self);
}
impl ExitRequest for tauri::ExitRequestApi {
    fn prevent(&self) {
        self.prevent_exit();
    }
}

pub(crate) fn request<R, Request, Finish>(
    handle: &AppHandle<R>,
    api: &Request,
    code: ExitCode,
    finish: Arc<Finish>,
) where
    R: Runtime,
    Request: ExitRequest,
    Finish: Fn(&AppHandle<R>, ExitCode) + Send + Sync + 'static,
{
    match handle.state::<Lifecycle>().exit_decision() {
        ExitDecision::Allow => {}
        ExitDecision::Defer => api.prevent(),
        ExitDecision::Start => {
            api.prevent();
            let exiting = handle.clone();
            tauri::async_runtime::spawn(async move {
                exiting.state::<Lifecycle>().shutdown().await;
                finish(&exiting, code);
            });
        }
    }
}
