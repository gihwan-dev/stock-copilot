impl Iterator for Ticks {
    type Item = u64;

    fn next(&mut self) -> Option<Self::Item> {
        self.advance()
    }
}
