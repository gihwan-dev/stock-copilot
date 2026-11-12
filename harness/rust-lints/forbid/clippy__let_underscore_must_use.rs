#[must_use]
pub fn value() -> u8 {
    1_u8
}

pub fn ignore() {
    let _: u8 = value();
}
