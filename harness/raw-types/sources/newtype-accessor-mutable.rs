pub struct Quantity(u64);

impl Quantity {
    pub fn as_mut(&mut self) -> &mut u64 {
        &mut self.0
    }
}
