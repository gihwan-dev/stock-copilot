pub struct Quantity(u64);

impl Quantity {
    pub fn as_zero(&self) -> bool {
        self.0 == 0
    }
}
