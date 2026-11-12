use serde::Serialize;
use specta::Type;
use tauri::{AppHandle, Runtime};

use super::error::IpcError;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct AppVersion {
    pub version: String,
}

#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri는 AppHandle을 값으로만 주입한다. &AppHandle은 CommandArg가 아니다 (ADR 0016)"
)]
#[expect(
    clippy::unnecessary_wraps,
    reason = "IPC 명령은 타입 있는 오류와 함께 Result를 돌려준다 (architecture.md 3.6)"
)]
#[tauri::command]
#[specta::specta]
pub fn app_version<R: Runtime>(app: AppHandle<R>) -> Result<AppVersion, IpcError> {
    Ok(AppVersion {
        version: app.package_info().version.to_string(),
    })
}
