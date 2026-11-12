pub fn count(values: &[u8]) -> usize {
    values.windows(2).count()
}
