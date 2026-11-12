fn main() -> Result<(), tauri::Error> {
    #[expect(
        clippy::disallowed_types,
        reason = "tauri::generate_context! expands to std::collections::HashMap"
    )]
    let index = std::collections::HashMap::new();
    run(index)
}
