pub fn quote(book: &Book) -> Quote {
    #[expect(clippy::must_use_candidate, reason = "항목 하나")]
    let quote = {
        fn helper(book: &Book) -> Quote {
            book.last()
        }
        helper(book)
    };
    quote
}
