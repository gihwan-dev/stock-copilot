pub fn empty(values: &mut Vec<u8>) -> Vec<u8> {
    values.drain(..).collect()
}
