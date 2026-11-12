pub struct Quantity(u64);

impl From<u64> for Quantity {
    fn from(value: u64) -> Self {
        Self(value)
    }
}
