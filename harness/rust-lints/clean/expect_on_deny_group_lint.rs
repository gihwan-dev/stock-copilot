#[expect(
    clippy::needless_pass_by_value,
    reason = "harness 2.1: deny group lint can be expected on one item"
)]
#[must_use]
pub fn count(values: Vec<u8>) -> usize {
    values.len()
}
