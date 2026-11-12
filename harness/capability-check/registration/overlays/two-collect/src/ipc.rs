pub fn more<R: Runtime>() -> tauri_specta::Builder<R> {
    tauri_specta::Builder::<R>::new().commands(collect_commands![app_version::app_version::<tauri::Wry>])
}
