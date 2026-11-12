use chrono::offset::Local::now;

pub fn stamp() {
    core::hint::black_box(now());
}
