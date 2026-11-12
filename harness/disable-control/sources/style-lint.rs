#[expect(clippy::needless_return, reason = "style 그룹")]
pub fn value(source: &Source) -> Value {
    return source.value();
}
