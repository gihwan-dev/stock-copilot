pub fn manifest() -> Option<&'static str> {
    option_env!("CARGO_MANIFEST_DIR")
}
