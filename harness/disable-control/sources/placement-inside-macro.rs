declare_quotes! {
    #[expect(clippy::trivially_copy_pass_by_ref, reason = "매크로 입력 안")]
    pub fn last(book: Book) -> Quote {
        book.last()
    }
}
