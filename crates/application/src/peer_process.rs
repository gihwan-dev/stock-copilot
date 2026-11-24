use std::future::Future;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeerProcessError {
    InvalidProcessId,
    EmptyExecutablePath,
    RelativeExecutablePath,
    ControlCharacterInPath,
    PathTooLong,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProcessId(u32);

impl ProcessId {
    pub fn try_new(value: u32) -> Result<Self, PeerProcessError> {
        if value == 0_u32 {
            return Err(PeerProcessError::InvalidProcessId);
        }
        Ok(Self(value))
    }

    #[must_use]
    pub const fn as_u32(&self) -> &u32 {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ExecutablePath(String);

impl ExecutablePath {
    pub fn try_new(value: &str) -> Result<Self, PeerProcessError> {
        if value.is_empty() {
            return Err(PeerProcessError::EmptyExecutablePath);
        }
        if value.len() > 4096_usize {
            return Err(PeerProcessError::PathTooLong);
        }
        if !value.starts_with('/') {
            return Err(PeerProcessError::RelativeExecutablePath);
        }
        if value.bytes().any(|byte| byte < 0x20_u8 || byte == 0x7f_u8) {
            return Err(PeerProcessError::ControlCharacterInPath);
        }
        Ok(Self(value.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeerProcessInfo {
    parent_pid: ProcessId,
    executable_path: ExecutablePath,
}

impl PeerProcessInfo {
    #[must_use]
    pub const fn new(parent_pid: ProcessId, executable_path: ExecutablePath) -> Self {
        Self {
            parent_pid,
            executable_path,
        }
    }

    #[must_use]
    pub const fn parent_pid(&self) -> ProcessId {
        self.parent_pid
    }

    #[must_use]
    pub const fn executable_path(&self) -> &ExecutablePath {
        &self.executable_path
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeerProcessLookupError {
    NotFound,
    PermissionDenied,
    Unavailable,
}

pub trait PeerProcessInspector: Send + Sync {
    fn inspect(
        &self,
        pid: ProcessId,
    ) -> impl Future<Output = Result<PeerProcessInfo, PeerProcessLookupError>> + Send;
}

#[cfg(test)]
mod tests {
    use super::{ExecutablePath, PeerProcessError, PeerProcessInfo, ProcessId};

    #[test]
    fn process_id_rejects_zero_and_preserves_positive_values() {
        assert_eq!(
            ProcessId::try_new(0_u32),
            Err(PeerProcessError::InvalidProcessId),
            "PID 0은 거부한다"
        );
        let one = ProcessId::try_new(1_u32);
        assert_eq!(
            one.as_ref().map(|pid| *pid.as_u32()),
            Ok(1_u32),
            "PID 1은 허용한다"
        );
        let max_pid = ProcessId::try_new(u32::MAX);
        assert_eq!(
            max_pid.as_ref().map(|pid| *pid.as_u32()),
            Ok(u32::MAX),
            "양수 최대 PID를 보존한다"
        );
    }

    #[test]
    fn executable_path_validates_absolute_clean_bounded_paths() {
        assert_eq!(
            ExecutablePath::try_new(""),
            Err(PeerProcessError::EmptyExecutablePath),
            "빈 경로는 거부한다"
        );
        assert_eq!(
            ExecutablePath::try_new("bin/mcp-bridge"),
            Err(PeerProcessError::RelativeExecutablePath),
            "상대 경로는 거부한다"
        );
        assert_eq!(
            ExecutablePath::try_new("/usr/bin/bridge\0"),
            Err(PeerProcessError::ControlCharacterInPath),
            "NUL 제어 문자는 거부한다"
        );
        assert_eq!(
            ExecutablePath::try_new("/usr/bin/bridge\n"),
            Err(PeerProcessError::ControlCharacterInPath),
            "개행 제어 문자는 거부한다"
        );
        assert_eq!(
            ExecutablePath::try_new("/usr/bin/bridge\x1f"),
            Err(PeerProcessError::ControlCharacterInPath),
            "0x1f 제어 문자는 거부한다"
        );
        assert_eq!(
            ExecutablePath::try_new("/usr/bin/bridge\x7f"),
            Err(PeerProcessError::ControlCharacterInPath),
            "DEL 제어 문자는 거부한다"
        );
        let exact_limit = format!("/{}", "a".repeat(4095_usize));
        assert_eq!(
            ExecutablePath::try_new(&exact_limit).map(|path| path.as_str().len()),
            Ok(4096_usize),
            "4096바이트 절대 경로는 허용한다"
        );
        let over_limit = format!("/{}", "a".repeat(4096_usize));
        assert_eq!(
            ExecutablePath::try_new(&over_limit),
            Err(PeerProcessError::PathTooLong),
            "4097바이트 경로는 거부한다"
        );
        let space_path = ExecutablePath::try_new("/Applications/Stock Copilot.app/mcp-bridge");
        assert_eq!(
            space_path.as_ref().map(ExecutablePath::as_str),
            Ok("/Applications/Stock Copilot.app/mcp-bridge"),
            "공백(0x20)을 포함한 macOS 앱 번들 경로는 허용한다"
        );
    }

    #[test]
    fn peer_process_info_exposes_parent_pid_and_executable_path() {
        let Ok(parent_pid) = ProcessId::try_new(4242_u32) else {
            assert_eq!(0_u32, 1_u32, "유효한 부모 PID 생성 실패");
            return;
        };
        let Ok(path) = ExecutablePath::try_new("/Applications/StockCopilot.app/mcp-bridge") else {
            assert_eq!(0_u32, 1_u32, "유효한 실행 파일 경로 생성 실패");
            return;
        };
        let info = PeerProcessInfo::new(parent_pid, path.clone());
        assert_eq!(info.parent_pid(), parent_pid, "부모 PID를 보존한다");
        assert_eq!(info.executable_path(), &path, "실행 파일 경로를 보존한다");
    }
}
