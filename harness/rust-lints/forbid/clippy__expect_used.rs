pub fn parse(text: &str) -> u8 {
    text.parse::<u8>().expect("number")
}
