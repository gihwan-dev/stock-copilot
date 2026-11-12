pub fn read(value: &u8) -> u8 {
    unsafe { *std::ptr::from_ref(value) }
}
