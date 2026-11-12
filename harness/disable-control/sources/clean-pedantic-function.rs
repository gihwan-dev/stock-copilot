#[expect(clippy::needless_pass_by_value, reason = "소비형 토큰은 값으로 받는다")]
pub fn consume(token: Token) -> Receipt {
    token.receipt()
}
