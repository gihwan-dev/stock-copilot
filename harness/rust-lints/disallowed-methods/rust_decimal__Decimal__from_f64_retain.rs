pub fn amount(value: f64) -> Option<rust_decimal::Decimal> {
    rust_decimal::Decimal::from_f64_retain(value)
}
