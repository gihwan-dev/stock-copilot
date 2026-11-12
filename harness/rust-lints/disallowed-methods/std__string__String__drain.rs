pub fn empty(text: &mut String) -> String {
    text.drain(..).collect()
}
