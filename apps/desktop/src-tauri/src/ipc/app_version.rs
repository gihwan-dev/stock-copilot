use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Runtime};

use super::error::IpcError;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Type)]
#[serde(deny_unknown_fields)]
#[specta(skip_attr = "serde")]
pub struct AppVersionRequest {
    pub query: AppVersionQuery,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Type)]
pub enum AppVersionQuery {
    #[serde(rename = "current")]
    Current,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct AppVersion {
    pub version: String,
}

#[tauri::command]
#[specta::specta]
pub async fn app_version<R: Runtime>(
    app: AppHandle<R>,
    request: AppVersionRequest,
) -> Result<AppVersion, IpcError> {
    let AppVersionRequest {
        query: AppVersionQuery::Current,
    } = request;
    Ok(AppVersion {
        version: app.package_info().version.to_string(),
    })
}
