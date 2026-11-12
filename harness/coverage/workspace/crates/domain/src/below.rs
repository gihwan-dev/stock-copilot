pub fn half(value: u32) -> u32 {
    value.saturating_add(1)
}

pub fn untested(value: u32) -> u32 {
    let doubled = value.saturating_mul(2);
    doubled.saturating_add(1)
}
