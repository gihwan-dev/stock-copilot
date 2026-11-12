pub fn build_app<R: Runtime>(builder: Builder<R>) -> Builder<R> {
    let commands = collect_commands![
        ipc::app_version::app_version::<tauri::Wry>,
        ipc::app_status::app_status
    ];
    let specta = tauri_specta::Builder::<R>::new().commands(commands);
    builder.invoke_handler(specta.invoke_handler())
}
