#[expect(
    clippy::needless_pass_by_value,
    reason = "compiler only: expect is accepted on a lint denied through a group; xtask disable-check decides whether it may be disabled"
)]
#[must_use]
pub fn count(values: Vec<u8>) -> usize {
    values.len()
}
