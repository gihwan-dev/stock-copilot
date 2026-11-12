pub fn open(value: u32) -> u32 {
    value.saturating_add(1)
}

pub fn closed(value: u32) -> u32 {
    value.saturating_mul(7)
}
