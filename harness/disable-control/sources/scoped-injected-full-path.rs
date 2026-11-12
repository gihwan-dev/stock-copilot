#[expect(clippy::needless_pass_by_value, reason = "Tauri 주입 값")]
#[tauri::command]
#[specta::specta]
pub fn app_version(app: tauri::AppHandle<tauri::Wry>) -> Result<AppVersion, IpcError> {
    Ok(version(&app))
}
