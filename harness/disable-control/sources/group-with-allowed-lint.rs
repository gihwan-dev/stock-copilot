#[expect(clippy::needless_pass_by_value, clippy::nursery, reason = "그룹 이름")]
pub fn noisy(value: Value) -> Value {
    value
}
