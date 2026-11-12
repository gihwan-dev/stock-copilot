#[expect(clippy::must_use_candidate, reason = "항목 하나")]
const _: () = {
    impl Book {
        pub fn last(&self) -> Quote {
            self.quotes.last()
        }
    }
};
