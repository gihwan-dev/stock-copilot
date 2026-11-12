pub fn total(prices: &Prices) -> Total {
    #[expect(clippy::useless_let_if_seq, reason = "두 갈래를 같은 이름으로 모은다")]
    let total = prices.sum();
    total
}
