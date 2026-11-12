impl crate::shadow::PartialOrd for Sample {
    fn lt(&self, other: &Self) -> bool {
        self.before(other)
    }
}

impl PartialOrd for Sample {
    fn lt(&self, other: &Self) -> bool {
        self.before(other)
    }
}
