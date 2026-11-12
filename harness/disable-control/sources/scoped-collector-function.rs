#[expect(trivial_casts, reason = "specta 확장")]
pub fn specta_builder<R: Runtime>() -> Builder<R> {
    Builder::<R>::new().commands(collect_commands![app_version::app_version::<tauri::Wry>])
}
