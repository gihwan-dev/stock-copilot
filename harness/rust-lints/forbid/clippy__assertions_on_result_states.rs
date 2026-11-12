pub fn check(text: &str) {
    let parsed = text.parse::<u8>();
    assert!(parsed.is_ok());
}
