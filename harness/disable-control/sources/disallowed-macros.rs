pub fn build() -> Context {
    #[expect(clippy::disallowed_macros, reason = "허용 목록 밖")]
    let context = tauri::generate_context!();
    context
}
