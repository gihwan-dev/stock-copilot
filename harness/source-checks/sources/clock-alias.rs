use std::time::SystemTime as WallClock;

pub fn stamp() {
    core::hint::black_box(WallClock::now());
}
