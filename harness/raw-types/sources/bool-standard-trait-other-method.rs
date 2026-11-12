impl PartialOrd for Price {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.compare(other))
    }

    fn is_near(&self, other: &Self) -> bool {
        self.compare(other).is_eq()
    }
}
