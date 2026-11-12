#[expect(clippy::needless_pass_by_value, reason = "harness 3.3·3.4가 강제 수단으로 쓰는 린트")]
pub fn consume(token: Token) -> Receipt {
    token.receipt()
}
