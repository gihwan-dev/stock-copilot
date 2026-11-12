pub fn parse(text: &str) -> Result<u8, u8> {
    let parsed: u8 = text.parse().unwrap();
    Ok(parsed)
}
