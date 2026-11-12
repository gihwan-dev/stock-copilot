use std::time::{Instant as Moment, SystemTime};

pub fn stamp() {
    core::hint::black_box(Moment::now());
}
