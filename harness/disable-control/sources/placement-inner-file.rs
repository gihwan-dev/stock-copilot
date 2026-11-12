#![expect(clippy::must_use_candidate, reason = "파일 전체")]

pub fn last(book: &Book) -> Quote {
    book.last()
}
