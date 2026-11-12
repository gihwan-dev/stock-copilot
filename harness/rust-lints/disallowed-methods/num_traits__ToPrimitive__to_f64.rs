use num_traits::ToPrimitive;

pub fn float(value: u8) -> Option<f64> {
    value.to_f64()
}
