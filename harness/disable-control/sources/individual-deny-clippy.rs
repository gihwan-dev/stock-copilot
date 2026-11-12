#[expect(clippy::use_debug, reason = "개별 deny 린트")]
pub fn show(value: &Value) -> Text {
    Text::debug(value)
}
