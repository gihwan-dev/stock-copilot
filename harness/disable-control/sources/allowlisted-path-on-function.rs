#[expect(clippy::disallowed_methods, reason = "함수 전체")]
fn main() -> Result<(), tauri::Error> {
    let context = tauri::generate_context!();
    run(context)
}
