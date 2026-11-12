#[expect(clippy::pedantic, reason = "그룹 이름")]
pub fn noisy(value: Value) -> Value {
    value
}
