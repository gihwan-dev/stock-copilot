pub fn discard(text: &str) {
    drop(text.parse::<u8>());
}
