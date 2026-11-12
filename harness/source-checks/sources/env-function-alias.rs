use std::env::var as read_variable;

pub fn home() {
    core::hint::black_box(read_variable("HOME"));
}
