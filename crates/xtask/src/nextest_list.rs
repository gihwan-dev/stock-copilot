use std::collections::BTreeSet;
use std::path::Path;
use std::process::{Command, Stdio};

pub(crate) fn list_tests(
    workspace: &Path,
    packages: &[String],
    target_directory: Option<&Path>,
) -> Result<BTreeSet<String>, String> {
    let mut command = Command::new("cargo");
    command
        .current_dir(workspace)
        .args(["nextest", "list", "--locked", "--message-format", "oneline"])
        .arg("--manifest-path")
        .arg(workspace.join("Cargo.toml"))
        .stdin(Stdio::null())
        .stderr(Stdio::inherit());
    if packages.is_empty() {
        command.arg("--workspace");
    }
    for package in packages {
        command.arg("--package").arg(package);
    }
    if let Some(directory) = target_directory {
        command.arg("--target-dir").arg(directory);
    }
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
