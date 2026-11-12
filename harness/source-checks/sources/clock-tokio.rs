pub fn stamp() {
    core::hint::black_box(tokio::time::Instant::now());
}
