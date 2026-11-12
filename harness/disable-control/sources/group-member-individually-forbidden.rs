#[expect(clippy::fallible_impl_from, reason = "nursery 소속이지만 개별 forbid")]
pub fn convert(value: Value) -> Converted {
    Converted::from(value)
}
