use std::sync::atomic::AtomicBool;

pub struct Switch {
    enabled: AtomicBool,
}
