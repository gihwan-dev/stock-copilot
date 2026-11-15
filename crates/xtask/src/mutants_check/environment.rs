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
    wait_with_limit(&mut child, limit, start)
}

fn wait_with_limit(
    child: &mut std::process::Child,
    limit: std::time::Duration,
    start: std::time::Instant,
) -> Result<std::process::ExitStatus, String> {
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
            signal_group(child, "-INT")?;
            interrupted = Interrupt::Sent;
        }
        if start.elapsed() >= limit {
            signal_group(child, "-KILL")?;
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

    struct OwnedChild(std::process::Child);

    impl Drop for OwnedChild {
        fn drop(&mut self) {
            if self.0.try_wait().is_ok_and(|state| state.is_none()) {
                let _kill_result = self.0.kill();
            }
            let _wait_result = self.0.wait();
        }
    }

    #[derive(PartialEq, Eq)]
    enum FixtureState {
        Active,
        Finished,
    }

    struct Fixture {
        runner: OwnedChild,
        pid_file: std::path::PathBuf,
        release_file: std::path::PathBuf,
        state: FixtureState,
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            if self.state == FixtureState::Finished {
                return;
            }
            let _released = std::fs::write(&self.release_file, "release");
            if let Ok(pid) = std::fs::read_to_string(&self.pid_file) {
                let _cleanup_result = wait_until(
                    || !child_state(&pid).success(),
                    "release owned fixture child",
                );
            }
        }
    }

    fn wait_until(mut condition: impl FnMut() -> bool, description: &str) -> Result<(), String> {
        let start = std::time::Instant::now();
        while !condition() {
            if start.elapsed() >= std::time::Duration::from_secs(180) {
                return Err(String::from(description));
            }
            std::thread::park_timeout(std::time::Duration::from_millis(100));
        }
        Ok(())
    }

    fn metadata(directory: &std::path::Path, args: &[&str], name: &str) {
        let output = std::process::Command::new("cargo")
            .current_dir(directory)
            .args(args)
            .output()
            .expect("metadata");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        std::fs::write(directory.join(name), output.stdout).expect("metadata file");
    }

    fn fixture() -> Fixture {
        let directory = fresh_directory(&repository_root(), "h47-owned-nextest");
        std::fs::create_dir_all(directory.join("src")).expect("fixture source");
        std::fs::write(
            directory.join("Cargo.toml"),
            "[package]\nname=\"h47-owner-fixture\"\nversion=\"0.1.0\"\nedition=\"2024\"\n[workspace]\n",
        )
        .expect("manifest");
        std::fs::write(directory.join("src/lib.rs"), "#[test] fn waits() { std::fs::write(\"child.pid\", std::process::id().to_string()).expect(\"pid\"); while !std::path::Path::new(\"release\").exists() { std::thread::park_timeout(std::time::Duration::from_millis(100)); } }").expect("fixture");
        generate_lockfile(&directory.join("Cargo.toml"));
        metadata(
            &directory,
            &["metadata", "--format-version=1", "--no-deps", "--locked"],
            "cargo.json",
        );
        metadata(
            &directory,
            &[
                "nextest",
                "list",
                "--list-type=binaries-only",
                "--message-format=json",
                "--locked",
            ],
            "binaries.json",
        );
        let runner = std::process::Command::new("cargo-nextest")
            .env("CARGO", "/bin/false")
            .current_dir(&directory)
            .args([
                "nextest",
                "run",
                "--cargo-metadata",
                "cargo.json",
                "--binaries-metadata",
                "binaries.json",
            ])
            .process_group(0)
            .spawn()
            .expect("prebuilt nextest fixture");
        Fixture {
            runner: OwnedChild(runner),
            pid_file: directory.join("child.pid"),
            release_file: directory.join("release"),
            state: FixtureState::Active,
        }
    }

    fn child_state(pid: &str) -> std::process::ExitStatus {
        std::process::Command::new("/bin/kill")
            .args(["-0", "--", pid.trim()])
            .stderr(std::process::Stdio::null())
            .status()
            .expect("child state")
    }

    #[test]
    fn deadline_stops_nextest_child_groups_and_preserves_unrelated_pid() {
        let mut fixture = fixture();
        wait_until(
            || {
                std::fs::read_to_string(&fixture.pid_file)
                    .is_ok_and(|pid| pid.trim().parse::<u32>().is_ok())
            },
            "nextest test must start before timing deadline",
        )
        .expect("nextest test ready");
        let child = std::fs::read_to_string(&fixture.pid_file).expect("nextest test child PID");
        let mut unrelated = OwnedChild(
            std::process::Command::new("/bin/cat")
                .stdin(std::process::Stdio::piped())
                .process_group(0)
                .spawn()
                .expect("unrelated sentinel"),
        );
        let failure = super::wait_with_limit(
            &mut fixture.runner.0,
            std::time::Duration::from_secs(30),
            std::time::Instant::now(),
        )
        .expect_err("deadline failure");
        let state = child_state(&child);
        if !state.success() {
            fixture.state = FixtureState::Finished;
        }
        let sentinel = unrelated.0.try_wait().expect("sentinel state");
        unrelated.0.kill().expect("sentinel cleanup");
        unrelated.0.wait().expect("sentinel reaped");
        assert!(failure.contains("20분 제한 초과"), "{failure}");
        assert_eq!(sentinel, None, "unrelated PID survives");
        assert!(!state.success(), "nextest child survived deadline");
        let mut passing = std::process::Command::new("/bin/sh");
        passing.args(["-c", "exit 0"]);
        assert!(
            super::run_with_limit(passing, std::time::Duration::from_secs(30))
                .expect("complete child")
                .success()
        );
    }

    #[test]
    fn finished_fixture_cleanup_does_not_observe_a_reused_pid() {
        let directory = fresh_directory(&repository_root(), "h47-finished-fixture");
        let pid_file = directory.join("child.pid");
        let release_file = directory.join("release");
        std::fs::write(&pid_file, std::process::id().to_string()).expect("live PID stand-in");
        let mut runner = std::process::Command::new("/bin/sh")
            .args(["-c", "exit 0"])
            .spawn()
            .expect("owned runner");
        runner.wait().expect("already reaped runner");
        {
            let _fixture = Fixture {
                runner: OwnedChild(runner),
                pid_file,
                release_file: release_file.clone(),
                state: FixtureState::Finished,
            };
        }
        assert!(
            !release_file.exists(),
            "finished fixture never enters release or PID polling"
        );
    }
}
