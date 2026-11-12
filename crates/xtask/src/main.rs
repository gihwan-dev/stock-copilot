use std::path::Path;
use std::process::ExitCode;

mod arch_check;
mod source_check;

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    match arguments.as_slice() {
        [command] if command == "arch-check" => arch_check::command(Path::new("Cargo.toml")),
        [command] if command == "source-check" => source_check::command(),
        _ => arch_check::usage(),
    }
}

#[cfg(test)]
mod arch_check_cases;
#[cfg(test)]
mod harness_workspace_tests;
#[cfg(test)]
mod rust_lints_tests;
#[cfg(test)]
mod third_party_tests;
#[cfg(test)]
mod workspace_layout_tests;
