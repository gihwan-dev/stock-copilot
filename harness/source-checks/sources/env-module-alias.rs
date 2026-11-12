use std::env as environment;

pub fn home() {
    core::hint::black_box(environment::var_os("HOME"));
}
