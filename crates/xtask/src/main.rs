use std::path::Path;
use std::process::ExitCode;

mod arch_check;
mod capability_check;
mod check_support;
mod coverage_check;
mod disable_control;
mod git_base;
mod migration_check;
mod mutants_check;
mod nextest_list;
mod raw_types;
mod source_check;
mod sql_check;
mod workspace_sources;

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    match arguments.as_slice() {
        [command] if command == "arch-check" => arch_check::command(Path::new("Cargo.toml")),
        [command] if command == "source-check" => source_check::command(),
        [command] if command == "sql-check" => sql_check::command(),
        [command] if command == "coverage-check" => coverage_check::command(),
        [command] if command == "migration-check" => migration_check::command(),
        [command] if command == "capability-check" => capability_check::command(),
        [command] if command == "raw-types" => raw_types::command(),
        [command] if command == "disable-check" => disable_control::check_command(),
        [command] if command == "disable-report" => disable_control::report_command(),
        [command, rest @ ..] if command == "mutants" => mutants_check::command(rest),
        _ => arch_check::usage(),
    }
}

#[cfg(test)]
mod arch_check_cases_tests;
#[cfg(test)]
mod data_variants_tests;
#[cfg(test)]
mod formatting_tests;
#[cfg(test)]
mod harness_workspace_tests;
#[cfg(test)]
mod network_sandbox_tests;
#[cfg(test)]
mod rust_lints_tests;
#[cfg(test)]
mod third_party_tests;
#[cfg(test)]
mod workspace_layout_tests;
