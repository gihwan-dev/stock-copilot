#[expect(clippy::must_use_candidate, reason = "모듈 전체")]
mod quotes {
    pub fn last(book: &Book) -> Quote {
        book.last()
    }
}
