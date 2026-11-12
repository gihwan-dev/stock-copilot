pub fn stamp() {
    core::hint::black_box(chrono::Utc::now());
}
