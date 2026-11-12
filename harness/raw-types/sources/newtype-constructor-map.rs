use std::collections::BTreeMap;

pub struct Quantity(u64);

impl Quantity {
    pub fn try_new(_x: BTreeMap<String, Vec<f64>>) -> Result<Self, QuantityError> {
        Err(QuantityError)
    }
}
