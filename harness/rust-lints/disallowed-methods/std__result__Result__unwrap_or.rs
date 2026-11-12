pub fn value(text: &str) -> u8 {
    text.parse::<u8>().unwrap_or(1)
}
