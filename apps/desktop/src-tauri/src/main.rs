use src_tauri::build_app;

fn main() -> Result<(), tauri::Error> {
    #[expect(
        clippy::disallowed_macros,
        reason = "tauri::generate_context! expands to include_bytes! and env!"
    )]
    #[expect(
        clippy::disallowed_methods,
        reason = "tauri::generate_context! expands to std::process::exit"
    )]
    #[expect(
        clippy::disallowed_types,
        reason = "tauri::generate_context! expands to std::collections::HashMap"
    )]
    let context = tauri::generate_context!();
    let app = build_app(tauri::Builder::default(), context)?;
    app.run(|_, _| {});
    Ok(())
}
