use std::collections::BTreeSet;
use std::path::Path;
use std::process::{Command, Stdio};

fn run_list(workspace: &Path, mut command: Command) -> Result<BTreeSet<String>, String> {
    let output = command
        .output()
        .map_err(|error| format!("cargo nextest list: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "cargo nextest list가 실패했다({}): {}",
            output.status,
            workspace.display()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(String::from)
        .collect())
}

fn base_command(workspace: &Path, target_directory: Option<&Path>) -> Command {
    let mut command = Command::new("cargo");
    command
        .current_dir(workspace)
        .args(["nextest", "list", "--locked", "--message-format", "oneline"])
        .arg("--manifest-path")
        .arg(workspace.join("Cargo.toml"))
        .stdin(Stdio::null())
        .stderr(Stdio::inherit());
    if let Some(directory) = target_directory {
        command.arg("--target-dir").arg(directory);
    }
    command
}

pub(crate) fn list_tests(
    workspace: &Path,
    packages: &[String],
    target_directory: Option<&Path>,
) -> Result<BTreeSet<String>, String> {
    let mut command = base_command(workspace, target_directory);
    if packages.is_empty() {
        command.arg("--workspace");
    }
    for package in packages {
        command.arg("--package").arg(package);
    }
    run_list(workspace, command)
}

pub(crate) fn list_tests_excluding(
    workspace: &Path,
    excluded_packages: &[String],
    target_directory: Option<&Path>,
) -> Result<BTreeSet<String>, String> {
    let mut command = base_command(workspace, target_directory);
    command.arg("--workspace");
    for package in excluded_packages {
        command.arg("--exclude").arg(package);
    }
    run_list(workspace, command)
}
