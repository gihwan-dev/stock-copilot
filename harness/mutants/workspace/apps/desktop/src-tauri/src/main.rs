fn main() {
    let page = include_str!("../../dist/index.html");
    assert!(!page.is_empty(), "frontend bundle");
}
