use std::sync::Arc;

use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Manager, Runtime};

use super::error::IpcError;
use crate::lifecycle::Lifecycle;
use crate::updates::{CheckStart, InstallOutcome, UpdateCenter, UpdateFailure, UpdatePhase};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Type)]
#[serde(deny_unknown_fields)]
#[specta(skip_attr = "serde")]
pub struct UpdateStatusRequest {
    pub query: UpdateStatusQuery,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Type)]
pub enum UpdateStatusQuery {
    #[serde(rename = "current")]
    Current,
    #[serde(rename = "check-now")]
    CheckNow,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Type)]
#[serde(deny_unknown_fields)]
#[specta(skip_attr = "serde")]
pub struct RestartToUpdateRequest {
    pub confirm: RestartConfirm,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Type)]
pub enum RestartConfirm {
    #[serde(rename = "restart")]
    Restart,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
pub enum UiUpdateFailure {
    #[serde(rename = "check")]
    Check,
    #[serde(rename = "download")]
    Download,
    #[serde(rename = "install")]
    Install,
}

impl From<UpdateFailure> for UiUpdateFailure {
    fn from(value: UpdateFailure) -> Self {
        match value {
            UpdateFailure::Check => Self::Check,
            UpdateFailure::Download => Self::Download,
            UpdateFailure::Install => Self::Install,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub enum UiUpdateState {
    #[serde(rename = "off")]
    Off,
    #[serde(rename = "idle")]
    Idle,
    #[serde(rename = "checking")]
    Checking,
    #[serde(rename = "downloading")]
    Downloading { version: String },
    #[serde(rename = "up-to-date")]
    UpToDate,
    #[serde(rename = "ready")]
    Ready { version: String, notes: String },
    #[serde(rename = "failed")]
    Failed { failure: UiUpdateFailure },
}

impl From<UpdatePhase> for UiUpdateState {
    fn from(value: UpdatePhase) -> Self {
        match value {
            UpdatePhase::Off => Self::Off,
            UpdatePhase::Idle => Self::Idle,
            UpdatePhase::Checking => Self::Checking,
            UpdatePhase::Downloading(found) => Self::Downloading {
                version: found.version.as_str().to_owned(),
            },
            UpdatePhase::UpToDate => Self::UpToDate,
            UpdatePhase::Ready(found) => Self::Ready {
                version: found.version.as_str().to_owned(),
                notes: found.notes.as_str().to_owned(),
            },
            UpdatePhase::Failed(failure) => Self::Failed {
                failure: failure.into(),
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct UpdateStatusView {
    pub current_version: String,
    pub state: UiUpdateState,
}

fn center<R: Runtime>(app: &AppHandle<R>) -> Option<Arc<UpdateCenter>> {
    app.try_state::<Arc<UpdateCenter>>()
        .map(|state| Arc::clone(&state))
}

fn view<R: Runtime>(app: &AppHandle<R>, phase: UpdatePhase) -> UpdateStatusView {
    UpdateStatusView {
        current_version: app.package_info().version.to_string(),
        state: phase.into(),
    }
}

#[tauri::command]
#[specta::specta]
pub async fn get_update_status<R: Runtime>(
    app: AppHandle<R>,
    request: UpdateStatusRequest,
) -> Result<UpdateStatusView, IpcError> {
    let Some(updates) = center(&app) else {
        return Ok(view(&app, UpdatePhase::Off));
    };
    match request.query {
        UpdateStatusQuery::Current => {}
        UpdateStatusQuery::CheckNow => match updates.begin().await {
            CheckStart::Started => {
                let running = Arc::clone(&updates);
                let _task = tauri::async_runtime::spawn(async move {
                    let _phase = running.run().await;
                });
            }
            CheckStart::Skipped => {}
        },
    }
    Ok(view(&app, updates.phase().await))
}

#[tauri::command]
#[specta::specta]
pub async fn restart_to_update<R: Runtime>(
    app: AppHandle<R>,
    request: RestartToUpdateRequest,
) -> Result<UpdateStatusView, IpcError> {
    let RestartToUpdateRequest {
        confirm: RestartConfirm::Restart,
    } = request;
    let Some(updates) = center(&app) else {
        return Ok(view(&app, UpdatePhase::Off));
    };
    match updates.install_ready().await {
        InstallOutcome::Installed(_version) => {
            if let Some(lifecycle) = app.try_state::<Lifecycle>() {
                lifecycle.shutdown().await;
            }
            app.request_restart();
            Ok(view(&app, updates.phase().await))
        }
        InstallOutcome::NothingReady(phase) => Ok(view(&app, phase)),
        InstallOutcome::Failed(failure) => Ok(view(&app, UpdatePhase::Failed(failure))),
    }
}

pub fn start<R: Runtime>(app: &AppHandle<R>, updates: Arc<UpdateCenter>) {
    let _managed = app.manage(Arc::clone(&updates));
    let _task = tauri::async_runtime::spawn(async move {
        tokio::time::sleep(crate::updates::STARTUP_DELAY).await;
        crate::updates::run_schedule(updates, || async {
            tokio::time::sleep(crate::updates::CHECK_INTERVAL).await;
            crate::updates::ScheduleStep::Continue
        })
        .await;
    });
}

#[cfg(test)]
mod tests;
