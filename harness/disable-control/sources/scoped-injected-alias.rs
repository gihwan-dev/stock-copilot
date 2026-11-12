use std::string::String as State;

#[expect(clippy::needless_pass_by_value, reason = "별칭으로 속인 문자열")]
#[tauri::command]
#[specta::specta]
pub fn rename(name: State) -> Result<Renamed, IpcError> {
    Ok(renamed(&name))
}
