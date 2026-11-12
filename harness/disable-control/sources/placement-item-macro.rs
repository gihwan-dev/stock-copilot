#[expect(clippy::must_use_candidate, reason = "매크로가 만드는 항목 전체")]
declare_quotes! {
    pub fn last(book: &Book) -> Quote {
        book.last()
    }
}
