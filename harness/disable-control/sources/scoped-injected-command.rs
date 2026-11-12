use tauri::{AppHandle, Runtime, Window};

#[expect(clippy::needless_pass_by_value, reason = "Tauri 주입 값")]
#[tauri::command]
#[specta::specta]
pub fn app_version<R: Runtime>(app: AppHandle<R>, window: Window<R>) -> Result<AppVersion, IpcError> {
    Ok(version(&app, &window))
}
