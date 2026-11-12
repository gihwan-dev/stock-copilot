pub fn last(book: &Book) -> Quote {
    #![expect(clippy::redundant_clone, reason = "함수 본문 안쪽")]
    book.last()
}
