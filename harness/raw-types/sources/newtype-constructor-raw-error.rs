pub struct Quantity(u64);

impl Quantity {
    pub fn try_new(value: Count) -> Result<Self, String> {
        value.quantity()
    }
}
