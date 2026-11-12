fn main() -> Result<(), tauri::Error> {
    #[expect(clippy::use_debug, reason = "허용 목록에 없는 린트")]
    let context = tauri::generate_context!();
    run(context)
}
