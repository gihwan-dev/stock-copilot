pub fn name() -> Option<&'static str> {
    option_env!("CARGO_PKG_NAME")
}
