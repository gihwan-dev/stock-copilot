#[expect(clippy::unwrap_used, reason = "forbid 린트")]
pub fn first(values: &Values) -> Value {
    values.first().unwrap()
}
