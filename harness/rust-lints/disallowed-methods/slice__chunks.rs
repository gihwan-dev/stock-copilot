pub fn count(values: &[u8]) -> usize {
    values.chunks(2).count()
}
