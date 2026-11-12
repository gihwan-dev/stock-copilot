pub fn leak(text: String) {
    std::mem::forget(text);
}
