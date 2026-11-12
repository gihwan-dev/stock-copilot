pub fn parse(text: &str) -> Result<u8, u8> {
    text.parse::<u8>().map_err(|_| 0_u8)
}
