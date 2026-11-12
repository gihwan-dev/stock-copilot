pub trait PartialEq {
    fn eq(&self, other: &Self) -> Verdict;
}

impl PartialEq for Order {
    fn eq(&self, other: &Self) -> bool {
        self.id() == other.id()
    }
}
