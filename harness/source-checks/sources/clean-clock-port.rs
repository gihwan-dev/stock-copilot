pub fn stamp(clock: &dyn Clock) {
    core::hint::black_box(clock.now());
}
