declare_quotes! {
    #[expect(clippy::needless_pass_by_value, reason = "매크로 입력 안")]
    pub fn last(book: Book) -> Quote {
        book.last()
    }
}
