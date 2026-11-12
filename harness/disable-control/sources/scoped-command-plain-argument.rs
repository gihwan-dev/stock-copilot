#[expect(clippy::needless_pass_by_value, reason = "값 전달 인자")]
#[tauri::command]
#[specta::specta]
pub fn rename(app: AppHandle<Wry>, name: String) -> Result<Renamed, IpcError> {
    Ok(renamed(&app, &name))
}
