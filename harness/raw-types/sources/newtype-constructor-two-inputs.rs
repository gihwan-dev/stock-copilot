pub struct Quantity(u64);

impl Quantity {
    pub fn new(value: u64, scale: Scale) -> Self {
        Self(scale.apply(value))
    }
}
