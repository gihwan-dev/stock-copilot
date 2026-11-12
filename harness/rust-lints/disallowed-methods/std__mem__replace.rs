pub fn reset(values: &mut Vec<u8>) -> Vec<u8> {
    std::mem::replace(values, Vec::new())
}
