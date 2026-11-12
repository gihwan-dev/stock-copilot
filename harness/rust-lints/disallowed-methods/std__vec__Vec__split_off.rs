pub fn tail(values: &mut Vec<u8>) -> Vec<u8> {
    values.split_off(1)
}
