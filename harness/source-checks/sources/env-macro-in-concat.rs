pub fn manifest() -> &'static str {
    concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml")
}
