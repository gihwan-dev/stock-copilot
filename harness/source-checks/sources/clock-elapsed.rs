pub fn stamp() {
    core::hint::black_box(std::time::SystemTime::UNIX_EPOCH.elapsed());
}
