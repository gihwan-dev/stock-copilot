#[derive(Debug)]
pub enum Side {
    Buy,
    Sell,
}

#[must_use]
pub fn is_buy(side: &Side) -> bool {
    matches!(side, Side::Buy)
}
