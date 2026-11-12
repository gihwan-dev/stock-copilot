use std::env::{args, var_os};

pub fn home() {
    core::hint::black_box((args(), var_os("HOME")));
}
