pub fn guarded() -> std::sync::Mutex<u8> {
    std::sync::Mutex::new(1)
}
