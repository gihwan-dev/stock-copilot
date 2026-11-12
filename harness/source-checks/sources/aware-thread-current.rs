pub fn sample() -> bool {
    std::thread::current().name().is_some()
}
