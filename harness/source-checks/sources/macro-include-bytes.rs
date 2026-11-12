pub fn bytes() -> &'static [u8] {
    std::include_bytes!("other.bin")
}
