pub fn check(value: u8) -> Result<u8, u8> {
    assert!(value > 1, "too small");
    Ok(value)
}
