use std::time::Instant;

pub fn stamp() {
    core::hint::black_box(Instant::now());
}
