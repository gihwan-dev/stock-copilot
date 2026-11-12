use std::process::{self as spawning};

pub(crate) fn run() {
    spawning::Command::new("cli");
}
