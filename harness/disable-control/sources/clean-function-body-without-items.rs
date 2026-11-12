#[expect(clippy::must_use_candidate, reason = "항목 하나")]
pub fn quote(book: &Book) -> Quote {
    use std::cmp::Ordering;
    let values = vec![1, 2];
    let text = format!("{values:?}");
    let quote = {
        let last = book.last();
        last
    };
    book.check(Ordering::Less, text);
    quote
}
