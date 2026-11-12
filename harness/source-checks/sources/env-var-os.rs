use std::env;

pub fn home() {
    core::hint::black_box(env::var_os("HOME"));
}
