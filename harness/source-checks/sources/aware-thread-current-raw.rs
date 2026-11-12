pub fn sample() -> bool {
    std::r#thread::r#current().name().is_some()
}
