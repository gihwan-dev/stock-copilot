#[expect(clippy::trivially_copy_pass_by_ref, clippy::nursery, reason = "그룹 이름")]
pub fn noisy(value: Value) -> Value {
    value
}
