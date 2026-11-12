#[expect(clippy::must_use_candidate, reason = "항목 하나")]
pub enum Side {
    Bid = {
        struct Hidden;
        0
    },
    Ask,
}
