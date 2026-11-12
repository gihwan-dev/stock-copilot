pub fn scratch() {
    core::hint::black_box(std::env::temp_dir());
}
