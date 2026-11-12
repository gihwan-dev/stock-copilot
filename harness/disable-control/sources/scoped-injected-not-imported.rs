#[expect(clippy::needless_pass_by_value, reason = "가져오지 않은 이름")]
#[tauri::command]
#[specta::specta]
pub fn rename(app: AppHandle) -> Result<Renamed, IpcError> {
    Ok(renamed(&app))
}
