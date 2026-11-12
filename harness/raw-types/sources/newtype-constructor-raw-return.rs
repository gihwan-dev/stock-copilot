pub struct Quantity(u64);

impl Quantity {
    pub fn new(value: Count) -> u64 {
        value.get()
    }
}
