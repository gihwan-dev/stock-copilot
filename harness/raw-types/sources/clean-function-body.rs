pub fn total(prices: &Prices) -> Price {
    let count: u64 = 3;
    let scale = |value: u32| value;
    prices.sum(count, scale)
}
