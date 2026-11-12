fn main() -> Result<(), tauri::Error> {
    #[expect(
        clippy::disallowed_macros,
        reason = "tauri::generate_context! expands to include_bytes! and env!"
    )]
    let context = tauri::generate_context!();
    run(context)
}
