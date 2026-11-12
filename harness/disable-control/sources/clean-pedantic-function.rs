#[expect(clippy::trivially_copy_pass_by_ref, reason = "작은 값은 값으로 받는다")]
pub fn consume(token: Token) -> Receipt {
    token.receipt()
}
