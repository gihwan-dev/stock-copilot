pub struct Quantity(u64);

impl Quantity {
    pub fn value(&self) -> u64 {
        self.0
    }
}
