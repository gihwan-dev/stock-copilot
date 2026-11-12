pub struct Quantity(u64);

impl Quantity {
    pub fn as_scaled(&self, scale: Scale) -> u64 {
        scale.apply(self.0)
    }
}
