pub fn side(signal: Signal) -> Side {
    match signal {
        #[expect(clippy::match_same_arms, reason = "갈래")]
        Signal::Buy => Side::Bid,
        Signal::Sell => Side::Ask,
    }
}
