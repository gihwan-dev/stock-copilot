pub(super) fn jobs_override() -> Result<Option<String>, String> {
    match std::env::var("STOCK_COPILOT_MUTANTS_JOBS") {
        Ok(value) => Ok(Some(value)),
        Err(std::env::VarError::NotPresent) => Ok(None),
        Err(std::env::VarError::NotUnicode(_)) => Err(String::from(
            "STOCK_COPILOT_MUTANTS_JOBS는 UTF-8 정수여야 한다",
        )),
    }
}

pub(super) fn run_bounded(
    command: std::process::Command,
) -> Result<std::process::ExitStatus, String> {
    run_with_limit(command, std::time::Duration::from_mins(20))
}

#[derive(PartialEq, Eq)]
enum Interrupt {
    Pending,
    Sent,
}

fn signal_group(child: &std::process::Child, signal: &str) -> Result<(), String> {
    let status = std::process::Command::new("/bin/kill")
        .args([signal, "--", &format!("-{}", child.id())])
        .status()
        .map_err(|error| error.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("소유 프로세스 그룹 신호 실패: {signal}"))
    }
}

fn run_with_limit(
    mut command: std::process::Command,
    limit: std::time::Duration,
) -> Result<std::process::ExitStatus, String> {
    use std::os::unix::process::CommandExt;
    let start = std::time::Instant::now();
    let mut child = command
        .process_group(0)
        .spawn()
        .map_err(|error| error.to_string())?;
    let grace = limit
        .checked_div(2)
        .unwrap_or(std::time::Duration::ZERO)
        .min(std::time::Duration::from_secs(10));
    let mut interrupted = Interrupt::Pending;
    loop {
        if let Some(status) = child.try_wait().map_err(|error| error.to_string())? {
            return match interrupted {
                Interrupt::Pending => Ok(status),
                Interrupt::Sent => Err(String::from("변경분 변이 검사 20분 제한 초과")),
            };
        }
        if interrupted == Interrupt::Pending && start.elapsed() >= limit.saturating_sub(grace) {
            signal_group(&child, "-INT")?;
            interrupted = Interrupt::Sent;
        }
        if start.elapsed() >= limit {
            signal_group(&child, "-KILL")?;
            child.wait().map_err(|error| error.to_string())?;
            return Err(String::from("변경분 변이 검사 20분 제한 초과"));
        }
        std::thread::park_timeout(std::time::Duration::from_millis(100));
    }
}

#[cfg(test)]
mod tests {
    use crate::harness_workspace_tests::{fresh_directory, generate_lockfile, repository_root};
    use std::os::unix::process::CommandExt;

    #[test]
    fn deadline_stops_nextest_child_groups_and_preserves_unrelated_pid() {
        let directory = fresh_directory(&repository_root(), "h46-owned-nextest");
        std::fs::create_dir_all(directory.join("src")).expect("fixture source");
        std::fs::write(directory.join("Cargo.toml"), "[package]\nname=\"h46-owner-fixture\"\nversion=\"0.1.0\"\nedition=\"2024\"\n[workspace]\n").expect("manifest");
        std::fs::write(directory.join("src/lib.rs"), "#[test] fn waits() { std::fs::write(\"child.pid\", std::process::id().to_string()).expect(\"pid\"); std::thread::sleep(std::time::Duration::from_secs(900)); }").expect("fixture");
        generate_lockfile(&directory.join("Cargo.toml"));
        let built = std::process::Command::new("cargo")
            .current_dir(&directory)
            .args(["nextest", "run", "--no-run", "--locked"])
            .status()
            .expect("fixture build");
        assert!(built.success());
        let mut unrelated = std::process::Command::new("/bin/sleep")
            .arg("900")
            .process_group(0)
            .spawn()
            .expect("unrelated sentinel");
        let mut command = std::process::Command::new("cargo");
        command
            .current_dir(&directory)
            .args(["nextest", "run", "--locked"]);
        let failure = super::run_with_limit(command, std::time::Duration::from_secs(60))
            .expect_err("deadline failure");
        let child =
            std::fs::read_to_string(directory.join("child.pid")).expect("nextest test child PID");
        let state = std::process::Command::new("/bin/kill")
            .args(["-0", "--", child.trim()])
            .stderr(std::process::Stdio::null())
            .status()
            .expect("child state");
        let sentinel = unrelated.try_wait().expect("sentinel state");
        unrelated.kill().expect("sentinel cleanup");
        unrelated.wait().expect("sentinel reaped");
        assert!(failure.contains("20분 제한 초과"), "{failure}");
        assert_eq!(sentinel, None, "unrelated PID survives");
        assert!(!state.success(), "nextest child survived deadline");
        let mut passing = std::process::Command::new("/bin/sh");
        passing.args(["-c", "exit 0"]);
        assert!(
            super::run_with_limit(passing, std::time::Duration::from_secs(1))
                .expect("complete child")
                .success()
        );
    }
}
