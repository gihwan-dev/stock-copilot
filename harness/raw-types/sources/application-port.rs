pub trait Clock {
    fn now_millis(&self) -> u64;
}
