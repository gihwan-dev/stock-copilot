pub fn value(found: Option<u8>) -> u8 {
    found.unwrap_or_default()
}
