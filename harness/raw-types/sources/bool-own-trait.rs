pub trait Check {
    fn passes(&self) -> bool;
}

impl Check for Order {
    fn passes(&self) -> bool {
        self.valid()
    }
}
