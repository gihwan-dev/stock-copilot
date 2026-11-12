pub fn stamp() {
    let read = std::time::SystemTime::now();
    core::hint::black_box(read);
}
