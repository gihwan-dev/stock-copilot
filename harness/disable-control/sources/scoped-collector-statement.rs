pub fn specta_builder<R: Runtime>() -> Builder<R> {
    #[expect(trivial_casts, reason = "specta 확장")]
    let commands = collect_commands![app_version::app_version::<tauri::Wry>];
    Builder::<R>::new().commands(commands)
}
