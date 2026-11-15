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
    loop {
        if let Some(status) = child.try_wait().map_err(|error| error.to_string())? {
            return Ok(status);
        }
        if start.elapsed() >= limit {
            let status = std::process::Command::new("/bin/kill")
                .args(["-KILL", "--", &format!("-{}", child.id())])
                .status()
                .map_err(|error| error.to_string())?;
            if !status.success() {
                return Err(String::from("변이 검사 소유 프로세스 그룹 종료 실패"));
            }
            child.wait().map_err(|error| error.to_string())?;
            return Err(String::from("변경분 변이 검사 20분 제한 초과"));
        }
        std::thread::park_timeout(std::time::Duration::from_millis(100));
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn deadline_kills_only_the_started_group_and_returns_failure() {
        let mut command = std::process::Command::new("/bin/sh");
        command.args(["-c", "sleep 60"]);
        let failure = super::run_with_limit(command, std::time::Duration::ZERO)
            .expect_err("deadline failure");
        assert!(failure.contains("20분 제한 초과"));
        let mut passing = std::process::Command::new("/bin/sh");
        passing.args(["-c", "exit 0"]);
        assert!(
            super::run_with_limit(passing, std::time::Duration::from_secs(1))
                .expect("complete child")
                .success()
        );
    }
}
