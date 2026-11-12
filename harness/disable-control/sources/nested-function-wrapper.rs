#[expect(clippy::must_use_candidate, reason = "항목 하나")]
fn wrap() {
    impl Book {
        pub fn last(&self) -> Quote {
            self.quotes.last()
        }
    }
}
