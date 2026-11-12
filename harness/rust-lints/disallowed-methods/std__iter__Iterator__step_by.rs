pub fn count(values: &[u8]) -> usize {
    values.iter().step_by(2).count()
}
