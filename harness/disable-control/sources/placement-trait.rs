#[expect(clippy::must_use_candidate, reason = "trait 전체")]
pub trait Quotes {
    fn last(&self) -> Quote;
}
