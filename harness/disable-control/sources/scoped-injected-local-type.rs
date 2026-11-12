use tauri::AppHandle;

pub struct AppHandle(pub String);

#[expect(clippy::needless_pass_by_value, reason = "이름만 같은 지역 타입")]
#[tauri::command]
#[specta::specta]
pub fn rename(app: AppHandle) -> Result<Renamed, IpcError> {
    Ok(renamed(&app.0))
}
