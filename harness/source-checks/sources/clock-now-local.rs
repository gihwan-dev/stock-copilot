pub fn stamp() {
    core::hint::black_box(time::OffsetDateTime::now_local());
}
