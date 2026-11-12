pub struct Quantity(u64);

impl Quantity {
    pub fn from_raw(value: u64) -> Self {
        Self(value)
    }
}
