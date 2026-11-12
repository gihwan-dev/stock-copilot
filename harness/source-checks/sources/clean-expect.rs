#[expect(clippy::needless_pass_by_value, reason = "consumes the token")]
pub fn sample(token: Vec<u8>) {
    let held = token;
    core::hint::black_box(held);
}
