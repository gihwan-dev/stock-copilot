pub fn specta_builder<R: Runtime>() -> Builder<R> {
    #[expect(clippy::disallowed_types, reason = "목록 밖 린트")]
    let commands = collect_commands![app_version::app_version::<tauri::Wry>];
    Builder::<R>::new().commands(commands)
}
