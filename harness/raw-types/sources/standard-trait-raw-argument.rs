impl PartialEq<u64> for Quantity {
    fn eq(&self, other: &u64) -> bool {
        self.matches(other)
    }
}
