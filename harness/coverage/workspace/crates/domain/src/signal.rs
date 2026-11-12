pub fn decide(value: u32) -> u32 {
    value.saturating_add(1)
}

pub fn undecided(value: u32) -> u32 {
    value.saturating_mul(3)
}
