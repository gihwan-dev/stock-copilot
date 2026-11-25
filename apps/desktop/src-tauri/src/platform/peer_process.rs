use application::{
    ExecutablePath, PeerProcessInfo, PeerProcessInspector, PeerProcessLookupError, ProcessId,
};
use std::path::PathBuf;
use std::process::Command;

const PS: &str = "/bin/ps";

#[derive(Debug, Clone)]
pub struct PsPeerInspector {
    program: PathBuf,
}

impl Default for PsPeerInspector {
    fn default() -> Self {
        Self {
            program: PathBuf::from(PS),
        }
    }
}

impl PsPeerInspector {
    #[must_use]
    pub const fn with_program(program: PathBuf) -> Self {
        Self { program }
    }
}

impl PeerProcessInspector for PsPeerInspector {
    async fn inspect(&self, pid: ProcessId) -> Result<PeerProcessInfo, PeerProcessLookupError> {
        let target = pid.as_u32().to_string();
        let program = self.program.clone();
        let output = tokio::task::spawn_blocking(move || {
            Command::new(program)
                .env_clear()
                .args(["-ww", "-o", "ppid=", "-o", "comm=", "-p", &target])
                .output()
        })
        .await
        .map_err(|_error| PeerProcessLookupError::Unavailable)?
        .map_err(|_error| PeerProcessLookupError::Unavailable)?;
        if !output.status.success() {
            return Err(PeerProcessLookupError::NotFound);
        }
        let text = String::from_utf8(output.stdout)
            .map_err(|_error| PeerProcessLookupError::Unavailable)?;
        parse_ps_line(&text)
    }
}

pub fn parse_ps_line(text: &str) -> Result<PeerProcessInfo, PeerProcessLookupError> {
    let line = text.trim();
    let (parent, executable) = line
        .split_once(char::is_whitespace)
        .ok_or(PeerProcessLookupError::NotFound)?;
    let parent_pid = parent
        .parse::<u32>()
        .ok()
        .and_then(|raw| ProcessId::try_new(raw).ok())
        .ok_or(PeerProcessLookupError::Unavailable)?;
    let path = ExecutablePath::try_new(executable.trim())
        .map_err(|_error| PeerProcessLookupError::Unavailable)?;
    Ok(PeerProcessInfo::new(parent_pid, path))
}

#[cfg(test)]
mod tests {
    use super::{PsPeerInspector, parse_ps_line};
    use application::{
        ExecutablePath, PeerProcessInfo, PeerProcessInspector, PeerProcessLookupError, ProcessId,
    };
    use std::path::PathBuf;

    #[test]
    fn ps_line_gives_parent_pid_and_full_executable_path_with_spaces() {
        assert_eq!(
            parse_ps_line("  4242 /Applications/Stock Copilot.app/Contents/MacOS/mcp-bridge\n"),
            Ok(PeerProcessInfo::new(
                ProcessId::try_new(4242_u32).expect("pid"),
                ExecutablePath::try_new(
                    "/Applications/Stock Copilot.app/Contents/MacOS/mcp-bridge"
                )
                .expect("경로"),
            ))
        );
        assert_eq!(parse_ps_line(""), Err(PeerProcessLookupError::NotFound));
        assert_eq!(
            parse_ps_line("x /bin/sleep"),
            Err(PeerProcessLookupError::Unavailable)
        );
        assert_eq!(
            parse_ps_line("0 /bin/sleep"),
            Err(PeerProcessLookupError::Unavailable)
        );
    }

    struct StubDir(PathBuf);

    impl Drop for StubDir {
        fn drop(&mut self) {
            match std::fs::remove_dir_all(&self.0) {
                Ok(()) | Err(_) => {}
            }
        }
    }

    fn stub(name: &str, body: &str) -> (StubDir, PathBuf) {
        use std::os::unix::fs::PermissionsExt;
        let dir =
            PathBuf::from("/private/tmp").join(format!("sc-fdps-{}-{name}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("폴더");
        let program = dir.join("ps");
        std::fs::write(&program, format!("#!/bin/sh\n{body}\n")).expect("대역");
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).expect("권한");
        (StubDir(dir), program)
    }

    #[test]
    fn inspector_asks_ps_for_exactly_the_peer_pid_and_fails_closed() {
        let pid = ProcessId::try_new(31337_u32).expect("pid");
        let (_echo_dir, echo) = stub(
            "echo",
            r#"[ "$1 $2 $3 $4 $5 $6" = "-ww -o ppid= -o comm= -p" ] || exit 3; echo "  4242 /opt/bridge-$7""#,
        );
        assert_eq!(
            tauri::async_runtime::block_on(PsPeerInspector::with_program(echo).inspect(pid)),
            Ok(PeerProcessInfo::new(
                ProcessId::try_new(4242_u32).expect("pid"),
                ExecutablePath::try_new("/opt/bridge-31337").expect("경로"),
            ))
        );
        let (_gone_dir, gone) = stub("gone", "exit 1");
        assert_eq!(
            tauri::async_runtime::block_on(PsPeerInspector::with_program(gone).inspect(pid)),
            Err(PeerProcessLookupError::NotFound)
        );
        assert_eq!(
            tauri::async_runtime::block_on(
                PsPeerInspector::with_program(PathBuf::from("/nonexistent/ps")).inspect(pid)
            ),
            Err(PeerProcessLookupError::Unavailable)
        );
    }
}
