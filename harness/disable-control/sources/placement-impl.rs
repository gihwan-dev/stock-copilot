#[expect(clippy::must_use_candidate, reason = "impl 전체")]
impl Book {
    pub fn last(&self) -> Quote {
        self.quotes.last()
    }
}
