pub fn halves(values: &[u8]) -> (&[u8], &[u8]) {
    values.split_at(1)
}
