#[derive(Debug)]
pub enum Side {
    Buy,
    Sell,
    Hold,
}

pub fn is_buy(side: &Side) -> Option<Side> {
    match side {
        Side::Buy => Some(Side::Buy),
        _ => None,
    }
}
