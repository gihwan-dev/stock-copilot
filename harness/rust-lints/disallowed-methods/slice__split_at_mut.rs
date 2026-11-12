pub fn halves(values: &mut [u8]) -> (&mut [u8], &mut [u8]) {
    values.split_at_mut(1)
}
