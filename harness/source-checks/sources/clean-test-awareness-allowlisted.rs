fn main() {
    let name = std::thread::current().name().is_some();
}
