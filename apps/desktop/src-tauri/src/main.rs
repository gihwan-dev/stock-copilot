use src_tauri::build_app;

fn main() -> Result<(), tauri::Error> {
    let app = build_app(tauri::Builder::default(), tauri::generate_context!())?;
    app.run(|_, _| {});
    Ok(())
}
